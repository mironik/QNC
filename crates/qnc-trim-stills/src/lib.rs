//! The edit pictures of a trim (user 2026-10-10: a moved slot shows the last picture of
//! the slot before, its own first and last and the first of the slot after; a moved
//! marker the picture before and after it). The program view names them (clip and
//! source frame); this piece decodes them in the background, the exact frame through
//! the decoder catalog's extractor, from the media the project plays (the preview's
//! `StillLocator`), and keeps them in memory. One job runs at a time and takes the
//! newest draft, so held arrow keys never queue work. No player, no probe, no database
//! write; the pictures are a passive aid, not the program.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, mpsc, Arc},
};

use qnc_decoder_catalog::{FilmstripExtractFrame, FilmstripExtractMode};
use qnc_image_assets::RgbaImage;
use qnc_program_view::{EditStill, StillPicture};
use qnc_source_preview::StillLocator;

pub const MODULE_ID: &str = "qnc.module.trim-stills";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The size an edit picture is decoded at (it is drawn in the video row of a timeline).
const STILL_SIZE: [u32; 2] = [256, 144];
/// Pictures kept in memory.
const KEPT: usize = 64;

type Decoded = Vec<(StillPicture, Result<Arc<RgbaImage>, String>)>;

#[derive(Default)]
pub struct TrimStills {
    project_id: Option<String>,
    cache: HashMap<StillPicture, Arc<RgbaImage>>,
    order: VecDeque<StillPicture>,
    failed: HashSet<StillPicture>,
    job: Option<mpsc::Receiver<Decoded>>,
}

impl TrimStills {
    pub fn new() -> Self {
        Self::default()
    }

    /// Puts the decoded pictures on `stills` and starts decoding the missing ones.
    /// Returns whether a picture arrived (the view changed).
    pub fn fill(&mut self, stills: &mut [EditStill], locator: Option<StillLocator>) -> bool {
        let Some(locator) = locator else {
            return false;
        };
        if self.project_id.as_deref() != Some(locator.project_id()) {
            *self = Self { project_id: Some(locator.project_id().to_string()), ..Self::default() };
        }
        let arrived = self.take_finished();
        for still in stills.iter_mut() {
            still.image = still.picture.as_ref().and_then(|picture| self.cache.get(picture).cloned());
        }
        if self.job.is_none() {
            let mut missing: Vec<StillPicture> = Vec::new();
            for picture in stills.iter().filter_map(|still| still.picture.as_ref()) {
                if !self.cache.contains_key(picture) && !self.failed.contains(picture) && !missing.contains(picture) {
                    missing.push(picture.clone());
                }
            }
            if !missing.is_empty() {
                self.start(missing, locator);
            }
        }
        arrived
    }

    /// Whether a job is decoding (the caller repaints soon).
    pub fn loading(&self) -> bool {
        self.job.is_some()
    }

    fn take_finished(&mut self) -> bool {
        let Some(job) = &self.job else {
            return false;
        };
        let decoded = match job.try_recv() {
            Ok(decoded) => decoded,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => Vec::new(),
        };
        self.job = None;
        for (picture, image) in decoded {
            match image {
                Ok(image) => {
                    self.order.push_back(picture.clone());
                    self.cache.insert(picture, image);
                }
                Err(error) => {
                    log(&format!("trim-still {}:{} {error}", picture.clip_id, picture.source_frame));
                    self.failed.insert(picture);
                }
            }
        }
        while self.order.len() > KEPT {
            if let Some(old) = self.order.pop_front() {
                self.cache.remove(&old);
            }
        }
        true
    }

    fn start(&mut self, pictures: Vec<StillPicture>, locator: StillLocator) {
        let (send, receive) = mpsc::channel();
        let spawned = std::thread::Builder::new().name("qnc-trim-stills".into()).spawn(move || {
            let _ = send.send(decode_all(&pictures, &locator));
        });
        if spawned.is_ok() {
            self.job = Some(receive);
        }
    }
}

/// One extractor run per clip, every wanted frame of that clip in it.
fn decode_all(pictures: &[StillPicture], locator: &StillLocator) -> Decoded {
    let mut decoded = Decoded::new();
    let mut clips: Vec<&str> = pictures.iter().map(|picture| picture.clip_id.as_str()).collect();
    clips.sort_unstable();
    clips.dedup();
    for clip_id in clips {
        let frames: Vec<&StillPicture> = pictures.iter().filter(|picture| picture.clip_id == clip_id).collect();
        match decode_clip(clip_id, &frames, locator) {
            Ok(images) => decoded.extend(frames.iter().map(|p| (*p).clone()).zip(images.into_iter().map(Ok))),
            Err(error) => decoded.extend(frames.iter().map(|p| ((*p).clone(), Err(error.clone())))),
        }
    }
    decoded
}

fn decode_clip(clip_id: &str, frames: &[&StillPicture], locator: &StillLocator) -> Result<Vec<Arc<RgbaImage>>, String> {
    let located = locator.locate(clip_id)?;
    let extractor = qnc_decoder_catalog::installed_filmstrip_extractor()
        .map_err(|error| error.to_string())?
        .ok_or("Ovo racunalo nema izvlakac slika u katalogu dekodera.")?;
    let (num, den) = located.fps;
    let timebase = qnc_frame_timebase::FrameTimebase::new(i64::from(num), i64::from(den))?;
    // A quarter of a frame early: the first picture at or after it is the frame itself.
    let seek = |frame: u64| ((frame as f64 - 0.25) * f64::from(den) / f64::from(num.max(1))).max(0.0);
    let mut wanted: Vec<FilmstripExtractFrame> =
        frames.iter().map(|picture| FilmstripExtractFrame { seek_sec: seek(picture.source_frame) }).collect();
    if wanted.len() == 1 {
        wanted.push(wanted[0].clone()); // the extractor makes at least two pictures
    }
    let scratch = scratch_dir();
    std::fs::create_dir_all(&scratch).map_err(|error| error.to_string())?;
    let result = (|| {
        extractor.extract_frames_with_cancel(
            &located.file,
            &wanted,
            timebase,
            FilmstripExtractMode::ExactSeek,
            STILL_SIZE,
            &scratch,
            &AtomicBool::new(false),
        )?;
        (0..frames.len()).map(|index| read_picture(&scratch.join(format!("{index:03}.jpg")))).collect()
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

fn read_picture(file: &Path) -> Result<Arc<RgbaImage>, String> {
    let bytes = std::fs::read(file).map_err(|error| format!("slika nije izvucena: {error}"))?;
    qnc_image_assets::decode_thumbnail(&bytes).map(Arc::new)
}

fn scratch_dir() -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("qnc-trim-still-{}-{unique}", std::process::id()))
}

fn log(line: &str) {
    if qnc_dev_diagnostics::player_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_project_nothing_is_decoded() {
        let mut stills = TrimStills::new();
        let mut wanted = vec![EditStill {
            marker_frame: 10,
            after: true,
            picture: Some(StillPicture { clip_id: "c".into(), source_frame: 3 }),
            image: None,
        }];
        assert!(!stills.fill(&mut wanted, None));
        assert!(!stills.loading());
        assert!(wanted[0].image.is_none());
    }
}
