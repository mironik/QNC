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
}

/// The window of one form.
#[derive(Default)]
pub struct ClipWindow {
    shown: Option<(String, u64, u64)>,
    enlarged: Option<Enlarged>,
    fine: FineWave,
    wave_done: bool,
    pictures: Option<Receiver<(usize, FilmstripFrameAsset)>>,
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
            self.enlarged = Some(Enlarged { from, to, assets: Arc::new(stored.window(from, to, total)) });
        }
        if fresh || !self.wave_done {
            if let Some(locator) = locator() {
                if fresh {
                    self.start_pictures(clip, (from, to), &locator);
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

    fn take_pictures(&mut self) {
        let Some(pictures) = &self.pictures else { return };
        let arrived: Vec<_> = pictures.try_iter().collect();
        let Some(enlarged) = self.enlarged.as_mut().filter(|_| !arrived.is_empty()) else { return };
        let assets = Arc::make_mut(&mut enlarged.assets);
        let mut strip = assets.filmstrip_background.as_deref().cloned().unwrap_or_else(|| FilmstripBackground { clip_id: assets.clip_id.clone(), frames: Vec::new() });
        for (place, asset) in arrived {
            match strip.frames.iter_mut().find(|frame| frame.index == place) {
                Some(frame) => *frame = asset,
                None => strip.frames.push(asset),
            }
        }
        assets.filmstrip_background = Some(Arc::new(strip));
    }

    /// One picture a place, the middle of the place, one decoder call each, the window's first
    /// place first; a new window stops them.
    fn start_pictures(&mut self, clip: &str, (from, to): (u64, u64), locator: &StillLocator) {
        let (send, receive) = mpsc::channel();
        let (clip, locator, cancel) = (clip.to_string(), locator.clone(), self.cancel.clone());
        let started = std::thread::Builder::new().name("qnc-clip-window".into()).spawn(move || {
            if let Err(error) = decode_places(&clip, (from, to), &locator, &cancel, &send) {
                log(&format!("clip-window pictures {clip}: {error}"));
            }
        });
        self.pictures = started.is_ok().then_some(receive);
    }
}

fn decode_places(clip: &str, (from, to): (u64, u64), locator: &StillLocator, cancel: &AtomicBool, send: &mpsc::Sender<(usize, FilmstripFrameAsset)>) -> Result<(), String> {
    let place = locator.locate(clip)?;
    let extractor = qnc_decoder_catalog::installed_filmstrip_extractor().map_err(|error| error.to_string())?.ok_or(qnc_ui_text::text("story.msg.no_still_extractor"))?;
    let scratch = std::env::temp_dir().join(format!("qnc-clip-window-{}-{from}-{to}", std::process::id()));
    let width = to.saturating_sub(from) as f64 / FILMSTRIP_FRAME_COUNT as f64;
    for index in 0..FILMSTRIP_FRAME_COUNT {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let frame = (from as f64 + (index as f64 + 0.5) * width) as u64;
        let seek = (frame as f64 - 0.25).max(0.0) * f64::from(place.fps.1) / f64::from(place.fps.0.max(1));
        std::fs::create_dir_all(&scratch).map_err(|error| error.to_string())?;
        let made = extractor.extract_run_with_cancel(&place.file, seek, 1, PICTURE_SIZE, &scratch, cancel).and_then(|()| std::fs::read(scratch.join("000.jpg")).map_err(|error| error.to_string())).and_then(|bytes| qnc_image_assets::decode_thumbnail(&bytes));
        let _ = std::fs::remove_dir_all(&scratch);
        let image = made.map(|image| qnc_image_assets::RgbaImage { content_key: frame, ..image })?;
        let asset = FilmstripFrameAsset { index, seek_sec: seek, uri: format!("qnc-clip-window:{clip}:{frame}"), image };
        if send.send((index, asset)).is_err() {
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
    fn a_window_is_seventy_percent_around_the_marks_and_goes_without_them() {
        let mut window = ClipWindow::default();
        let stored = SourceTimelineAssets::empty_for("c");
        let shown = window.follow(Some("c"), Some((700, 1400)), 5000, &stored, || None).expect("a window");
        assert_eq!((shown.from, shown.to), (550, 1550));
        assert!(window.follow(Some("c"), None, 5000, &stored, || None).is_none());
    }
}
