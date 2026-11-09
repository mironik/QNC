//! A window of a clip enlarged on the source timeline (user 2026-11-09: a statement at 70 % of
//! the width with its own fine wave and real pictures). At once it shows what the stored wave
//! and filmstrip have of the window; in the background the sound of the window is read from the
//! original (`qnc-fine-wave`, the zoom's own) and a real picture is decoded for each filmstrip
//! place through the decoder catalog. Kept for the window while it is shown; no probe, no
//! database, nothing written next to the media.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
    Arc,
};

use qnc_fine_wave::FineWave;
use qnc_image_assets::RgbaImage;
use qnc_program_waveform::Placement;
use qnc_still_locator::StillLocator;
use qnc_timeline_assets::{FilmstripBackground, FilmstripFrameAsset, SourceTimelineAssets, FILMSTRIP_FRAME_COUNT};
use qnc_wave::WaveformPeaks;

pub const MODULE_ID: &str = "qnc.module.clip-window";

/// Columns of the fine wave: as many as a stored wave has over a whole clip.
const COLUMNS: usize = 1200;
/// The camera channels a source timeline can show.
const CHANNELS: u16 = 4;
const PICTURE_SIZE: [u32; 2] = [320, 180];

/// What the source timeline shows of a window of its clip's frames `[from, to)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Enlarged {
    pub from: u64,
    pub to: u64,
    pub assets: Arc<SourceTimelineAssets>,
    /// The statement's pictures made so far, by their clip frame (the trim zoom takes them).
    pub pictures: Vec<(u64, Arc<RgbaImage>)>,
}

/// The window of one form.
#[derive(Default)]
pub struct ClipWindow {
    shown: Option<(String, u64, u64)>,
    enlarged: Option<Enlarged>,
    fine: FineWave,
    wave_done: bool,
    pictures: Option<Receiver<(u64, Arc<RgbaImage>)>>,
    marks: Option<(u64, u64)>,
    cancel: Arc<AtomicBool>,
}

impl std::fmt::Debug for ClipWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClipWindow").field("shown", &self.shown).finish_non_exhaustive()
    }
}

impl ClipWindow {
    /// The window around the marks of `clip` (of `total` frames), or nothing without them.
    /// `stored` is what the clip has for its whole length; `locator` is asked only while
    /// something is still to be read.
    pub fn follow(&mut self, clip: Option<&str>, marks: Option<(u64, u64)>, total: u64, stored: &SourceTimelineAssets, locator: impl FnOnce() -> Option<StillLocator>) -> Option<Enlarged> {
        let (Some(clip), Some((start, end))) = (clip, marks) else {
            self.forget();
            return None;
        };
        let (from, to) = qnc_timeline_model::enlarged(start, end, total);
        let key = (clip.to_string(), from, to);
        let fresh = self.shown.as_ref() != Some(&key);
        if fresh {
            self.forget();
            self.shown = Some(key);
            self.marks = Some((start, end));
            self.enlarged = Some(Enlarged { from, to, assets: Arc::new(stored.window(from, to, total)), pictures: Vec::new() });
        }
        if fresh || !self.wave_done {
            if let Some(locator) = locator() {
                if fresh {
                    self.start_pictures(clip, (start, end), total, &locator);
                }
                self.fill_wave(clip, (from, to), total, &locator);
            }
        }
        self.take_pictures();
        self.enlarged.clone()
    }

    fn forget(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::default();
        (self.shown, self.enlarged, self.pictures, self.wave_done) = (None, None, None, false);
    }

    /// The fine wave once every channel is read; a window nothing could be read of keeps the
    /// stored wave.
    fn fill_wave(&mut self, clip: &str, (from, to): (u64, u64), total: u64, locator: &StillLocator) {
        let lanes: Vec<(Vec<f32>, bool)> = (0..CHANNELS)
            .map(|channel| {
                let whole = Placement { clip_id: clip.to_string(), channel, program_start: 0, program_end: total, source_in: 0, source_out: total, level_db: 0, off: false, curve: Vec::new() };
                self.fine.lane_peaks(locator, std::slice::from_ref(&whole), from, to, COLUMNS)
            })
            .collect();
        if !lanes.iter().all(|(_, complete)| *complete) {
            return;
        }
        self.wave_done = true;
        if lanes.iter().all(|(peaks, _)| peaks.iter().all(|peak| *peak == 0.0)) {
            return;
        }
        let mut lanes = lanes.into_iter().map(|(peaks, _)| peaks);
        let mut next = || lanes.next().unwrap_or_default();
        let wave = WaveformPeaks { clip_id: clip.to_string(), a1_peaks: next(), a2_peaks: next(), a3_peaks: next(), a4_peaks: next(), coarse: false };
        if let Some(enlarged) = &mut self.enlarged {
            Arc::make_mut(&mut enlarged.assets).wave = Some(Arc::new(wave));
        }
    }

    /// The pictures that came: the zoom takes them as they are, and every filmstrip place of
    /// their part of the window shows one.
    fn take_pictures(&mut self) {
        let Some(pictures) = &self.pictures else { return };
        let arrived: Vec<_> = pictures.try_iter().collect();
        let (Some(enlarged), Some(marks)) = (self.enlarged.as_mut().filter(|_| !arrived.is_empty()), self.marks) else { return };
        enlarged.pictures.extend(arrived);
        let (from, to) = (enlarged.from, enlarged.to);
        let width = to.saturating_sub(from) as f64 / FILMSTRIP_FRAME_COUNT as f64;
        let assets = Arc::make_mut(&mut enlarged.assets);
        let mut strip = assets.filmstrip_background.as_deref().cloned().unwrap_or_else(|| FilmstripBackground { clip_id: assets.clip_id.clone(), frames: Vec::new() });
        for index in 0..FILMSTRIP_FRAME_COUNT {
            let wanted = picture_of(from as f64 + (index as f64 + 0.5) * width, marks);
            let Some((frame, image)) = enlarged.pictures.iter().find(|(frame, _)| *frame == wanted) else { continue };
            let asset = FilmstripFrameAsset { index, seek_sec: 0.0, uri: format!("qnc-clip-window:{}:{frame}", assets.clip_id), image: (**image).clone() };
            match strip.frames.iter_mut().find(|frame| frame.index == index) {
                Some(place) => *place = asset,
                None => strip.frames.push(asset),
            }
        }
        assets.filmstrip_background = Some(Arc::new(strip));
    }

    /// The statement's four pictures (user 2026-11-09): the last before it, its first, its last
    /// and the first after it, one decoder call each; a new window stops them.
    fn start_pictures(&mut self, clip: &str, (start, end): (u64, u64), total: u64, locator: &StillLocator) {
        let (send, receive) = mpsc::channel();
        let (clip, locator, cancel) = (clip.to_string(), locator.clone(), self.cancel.clone());
        let mut frames = vec![start.saturating_sub(1), start, end.saturating_sub(1)];
        if end < total {
            frames.push(end);
        }
        frames.dedup();
        let started = std::thread::Builder::new().name("qnc-clip-window".into()).spawn(move || {
            if let Err(error) = decode_pictures(&clip, &frames, &locator, &cancel, &send) {
                log(&format!("clip-window pictures {clip}: {error}"));
            }
        });
        self.pictures = started.is_ok().then_some(receive);
    }
}

/// Which of the statement's four pictures a place at frame `at` of the window shows: around it
/// the one next to it, inside it its first in the first half and its last in the second.
fn picture_of(at: f64, (start, end): (u64, u64)) -> u64 {
    if at < start as f64 {
        start.saturating_sub(1)
    } else if at >= end as f64 {
        end
    } else if at < (start + end) as f64 / 2.0 {
        start
    } else {
        end.saturating_sub(1)
    }
}

fn decode_pictures(clip: &str, frames: &[u64], locator: &StillLocator, cancel: &AtomicBool, send: &mpsc::Sender<(u64, Arc<RgbaImage>)>) -> Result<(), String> {
    let place = locator.locate(clip)?;
    let extractor = qnc_decoder_catalog::installed_filmstrip_extractor().map_err(|error| error.to_string())?.ok_or(qnc_ui_text::text("story.msg.no_still_extractor"))?;
    let scratch = std::env::temp_dir().join(format!("qnc-clip-window-{}-{}", std::process::id(), frames.first().copied().unwrap_or(0)));
    for &frame in frames {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let seek = (frame as f64 - 0.25).max(0.0) * f64::from(place.fps.1) / f64::from(place.fps.0.max(1));
        std::fs::create_dir_all(&scratch).map_err(|error| error.to_string())?;
        let made = extractor.extract_run_with_cancel(&place.file, seek, 1, PICTURE_SIZE, &scratch, cancel).and_then(|()| std::fs::read(scratch.join("000.jpg")).map_err(|error| error.to_string())).and_then(|bytes| qnc_image_assets::decode_thumbnail(&bytes));
        let _ = std::fs::remove_dir_all(&scratch);
        let image = made.map(|image| RgbaImage { content_key: frame, ..image })?;
        if send.send((frame, Arc::new(image))).is_err() {
            break;
        }
    }
    Ok(())
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
    fn every_place_shows_one_of_the_four_pictures_of_the_statement() {
        let marks = (100, 200);
        assert_eq!(picture_of(80.0, marks), 99, "before it");
        assert_eq!(picture_of(120.0, marks), 100, "its first half");
        assert_eq!(picture_of(180.0, marks), 199, "its second half");
        assert_eq!(picture_of(210.0, marks), 200, "after it");
    }

    #[test]
    fn a_window_is_seventy_percent_around_the_marks_and_goes_without_them() {
        let mut window = ClipWindow::default();
        let stored = SourceTimelineAssets::empty_for("c");
        let shown = window.follow(Some("c"), Some((700, 1400)), 5000, &stored, || None).expect("a window");
        assert_eq!((shown.from, shown.to), (550, 1550));
        assert!(window.follow(Some("c"), None, 5000, &stored, || None).is_none());
    }
}
