//! Passive program filmstrip: the pictures of a Wrap row (user 2026-10-15: the video part
//! of the Wrap segment shows the filmstrip). It never decodes media, never writes and
//! never drives playback: it lays the filmstrip already stored for each clip onto the
//! program axis by the source IN/OUT of each segment with a picture and of each cover
//! (covers on top), the way `qnc-program-waveform` lays the waves.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub use qnc_timeline_assets::{FilmstripBackground, TimelineArtifactRead};

pub const MODULE_ID: &str = "qnc.module.program-filmstrip";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const FILMSTRIP_RETRY_DELAY: Duration = Duration::from_secs(2);

/// Where a clip's picture is in the program: program frames `[program_start,
/// program_end)` show source frames `[source_in, source_out)` of `clip_id`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PicturePlacement {
    pub clip_id: String,
    pub program_start: u64,
    pub program_end: u64,
    pub source_in: u64,
    pub source_out: u64,
}

/// One filmstrip picture on the program axis: `strip.frames[frame]` over program frames
/// `[program_start, program_end)`.
#[derive(Debug, Clone, PartialEq)]
pub struct ProgramPicture {
    pub program_start: u64,
    pub program_end: u64,
    pub strip: Arc<FilmstripBackground>,
    pub frame: usize,
}

// The pictures and their seek times are stored values (never NaN), so equality is total.
impl Eq for ProgramPicture {}

/// A clip's stored filmstrip and its length in the frames its IN/OUT use.
#[derive(Debug, Clone)]
pub struct ClipStrip {
    pub strip: Arc<FilmstripBackground>,
    pub duration_frames: u64,
}

/// The pictures of the program, in paint order (segments, then covers over them).
pub fn compose(placements: &[PicturePlacement], strips: &HashMap<String, ClipStrip>) -> Vec<ProgramPicture> {
    let mut out = Vec::new();
    for at in placements {
        let Some(clip) = strips.get(&at.clip_id) else { continue };
        place(&mut out, at, clip);
    }
    out
}

/// The filmstrip spreads its pictures evenly over the whole clip (as the source
/// timeline draws it); each picture covers its share of the clip's frames.
fn place(out: &mut Vec<ProgramPicture>, at: &PicturePlacement, clip: &ClipStrip) {
    let frames = &clip.strip.frames;
    if frames.is_empty() || at.program_end <= at.program_start || clip.duration_frames == 0 {
        return;
    }
    let slots = frames.iter().map(|frame| frame.index + 1).max().unwrap_or(1).max(qnc_filmstrip_count());
    let duration = clip.duration_frames as f64;
    let source_in = at.source_in.min(clip.duration_frames) as f64;
    let source_out = (at.source_out.max(at.source_in + 1) as f64).min(duration).max(source_in + 1.0);
    let (program_start, program_len) = (at.program_start as f64, (at.program_end - at.program_start) as f64);
    let to_program = |source: f64| program_start + (source - source_in) / (source_out - source_in) * program_len;
    for (position, frame) in frames.iter().enumerate() {
        let from = (frame.index as f64 * duration / slots as f64).max(source_in);
        let to = ((frame.index + 1) as f64 * duration / slots as f64).min(source_out);
        if to <= from {
            continue;
        }
        let (start, end) = (to_program(from).floor() as u64, to_program(to).ceil() as u64);
        let (start, end) = (start.max(at.program_start), end.min(at.program_end));
        if end > start {
            out.push(ProgramPicture { program_start: start, program_end: end, strip: clip.strip.clone(), frame: position });
        }
    }
}

fn qnc_filmstrip_count() -> usize {
    qnc_timeline_assets::FILMSTRIP_FRAME_COUNT
}

/// The pictures of `program` between program frames `start` and `end`, cut to that
/// window and given in its local frames.
pub fn row_pictures(program: &[ProgramPicture], start: u64, end: u64) -> Vec<ProgramPicture> {
    program
        .iter()
        .filter(|picture| picture.program_end > start && picture.program_start < end)
        .map(|picture| ProgramPicture {
            program_start: picture.program_start.max(start) - start,
            program_end: picture.program_end.min(end) - start,
            ..picture.clone()
        })
        .collect()
}

/// The filmstrips of the clips a program shows, read from the project database through
/// the public timeline artifact reader on a thread of their own: a clip whose filmstrip
/// is not there yet is asked again after two seconds.
#[derive(Default)]
pub struct ProgramFilmstrips {
    reader: Option<Arc<dyn TimelineArtifactRead>>,
    strips: HashMap<String, Arc<FilmstripBackground>>,
    /// A clip's poster in every slot while its filmstrip is not made yet (user 2026-10-20:
    /// a Wrap row showed nothing until then; as the source timeline does since 2026-10-19).
    poster_strips: HashMap<String, Arc<FilmstripBackground>>,
    retry_after: HashMap<String, Instant>,
    read: Arc<Mutex<Vec<(String, Option<FilmstripBackground>)>>>,
    asked: HashSet<String>,
    composed: Option<(Vec<PicturePlacement>, usize, Vec<ProgramPicture>)>,
}

impl ProgramFilmstrips {
    pub fn new() -> Self {
        Self::default()
    }

    /// The reader of the active project; a new project forgets the old filmstrips.
    pub fn set_reader(&mut self, reader: Option<Arc<dyn TimelineArtifactRead>>) {
        *self = Self { reader, ..Self::default() };
    }

    /// The program's pictures for these placements; `duration_frames` gives each clip's
    /// length in the frames its IN/OUT use, `poster` its poster (shown until its filmstrip
    /// is made).
    pub fn pictures(
        &mut self,
        placements: &[PicturePlacement],
        duration_frames: impl Fn(&str) -> Option<u64>,
        poster: impl Fn(&str) -> Option<Arc<qnc_image_assets::RgbaImage>>,
    ) -> Vec<ProgramPicture> {
        let now = Instant::now();
        let read = self.read.lock().map(|mut read| std::mem::take(&mut *read)).unwrap_or_default();
        for (clip_id, strip) in read {
            self.asked.remove(&clip_id);
            match strip.filter(|strip| !strip.is_empty()) {
                Some(strip) => _ = self.strips.insert(clip_id, Arc::new(strip)),
                None => _ = self.retry_after.insert(clip_id, now + FILMSTRIP_RETRY_DELAY),
            }
        }
        let mut wanted = Vec::new();
        for placement in placements {
            let clip_id = placement.clip_id.as_str();
            if self.strips.contains_key(clip_id) || self.asked.contains(clip_id) || self.retry_after.get(clip_id).is_some_and(|at| *at > now) {
                continue;
            }
            self.asked.insert(clip_id.to_string());
            wanted.push(clip_id.to_string());
        }
        self.ask(wanted);
        for placement in placements {
            let clip_id = placement.clip_id.as_str();
            if self.strips.contains_key(clip_id) || self.poster_strips.contains_key(clip_id) {
                continue;
            }
            if let Some(image) = poster(clip_id) {
                let assets = qnc_timeline_assets::SourceTimelineAssets::empty_for(clip_id);
                if let Some(strip) = qnc_timeline_assets::with_poster_filmstrip(assets, false, Some(&image)).filmstrip_background {
                    self.poster_strips.insert(clip_id.to_string(), strip);
                }
            }
        }
        let loaded = self.strips.len() + self.poster_strips.len();
        if let Some((at, count, pictures)) = &self.composed {
            if at == placements && *count == loaded {
                return pictures.clone();
            }
        }
        let poster_only = self.poster_strips.iter().filter(|(id, _)| !self.strips.contains_key(*id));
        let strips = self
            .strips
            .iter()
            .chain(poster_only)
            .filter_map(|(id, strip)| Some((id.clone(), ClipStrip { strip: strip.clone(), duration_frames: duration_frames(id)? })))
            .collect();
        let pictures = compose(placements, &strips);
        self.composed = Some((placements.to_vec(), loaded, pictures.clone()));
        pictures
    }

    /// Whether filmstrips are still being read.
    pub fn reading(&self) -> bool {
        !self.asked.is_empty()
    }

    fn ask(&mut self, clips: Vec<String>) {
        if clips.is_empty() {
            return;
        }
        let Some(reader) = self.reader.clone() else {
            self.asked.clear();
            return;
        };
        let mailbox = self.read.clone();
        let started = std::thread::Builder::new().name("qnc-program-filmstrips".into()).spawn(move || {
            for clip_id in clips {
                let strip = qnc_timeline_assets::read_filmstrip_background(reader.as_ref(), &clip_id).ok().flatten();
                if let Ok(mut read) = mailbox.lock() {
                    read.push((clip_id, strip));
                }
            }
        });
        if started.is_err() {
            self.asked.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_timeline_assets::FilmstripFrameAsset;

    fn strip(count: usize) -> ClipStrip {
        let frames = (0..count)
            .map(|index| FilmstripFrameAsset {
                index,
                seek_sec: index as f64,
                uri: format!("f{index}"),
                image: qnc_image_assets::RgbaImage { size: [1, 1], pixels: vec![0; 4], content_key: index as u64 },
            })
            .collect();
        ClipStrip { strip: Arc::new(FilmstripBackground { clip_id: "c".into(), frames }), duration_frames: 130 }
    }

    #[test]
    fn a_segment_shows_the_pictures_of_its_part_of_the_clip() {
        let strips = HashMap::from([("c".to_string(), strip(13))]);
        // Source 20..50 of a 130 frame clip (13 pictures of 10 frames) at program 100..130.
        let at = PicturePlacement { clip_id: "c".into(), program_start: 100, program_end: 130, source_in: 20, source_out: 50 };
        let pictures = compose(&[at], &strips);
        let shown: Vec<_> = pictures.iter().map(|p| (p.frame, p.program_start, p.program_end)).collect();
        assert_eq!(shown, [(2, 100, 110), (3, 110, 120), (4, 120, 130)]);
        let row = row_pictures(&pictures, 105, 125);
        assert_eq!(row.iter().map(|p| (p.program_start, p.program_end)).collect::<Vec<_>>(), [(0, 5), (5, 15), (15, 20)]);
    }

    #[test]
    fn a_clip_without_a_filmstrip_shows_its_poster_until_the_filmstrip_is_made() {
        let poster = Arc::new(qnc_image_assets::RgbaImage { size: [1, 1], pixels: vec![9; 4], content_key: 7 });
        let mut strips = ProgramFilmstrips::new();
        let at = PicturePlacement { clip_id: "c".into(), program_start: 0, program_end: 130, source_in: 0, source_out: 130 };
        let pictures = strips.pictures(&[at.clone()], |_| Some(130), |_| Some(poster.clone()));
        assert_eq!(pictures.len(), 13, "the poster in every slot");
        assert!(pictures.iter().all(|picture| picture.strip.frames[picture.frame].image.content_key == 7));
        assert!(strips.pictures(&[at], |_| Some(130), |_| None).len() == 13, "kept until the filmstrip comes");
    }

    #[test]
    fn a_clip_without_a_filmstrip_shows_nothing() {
        let at = PicturePlacement { clip_id: "x".into(), program_start: 0, program_end: 10, source_in: 0, source_out: 10 };
        assert!(compose(&[at], &HashMap::new()).is_empty());
    }
}
