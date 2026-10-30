//! The edit pictures of a trim (user 2026-10-10: a moved slot shows the last picture of
//! the slot before, its own first and last and the first of the slot after; a moved
//! marker the picture before and after it). The program view names them (clip and
//! source frame); this piece decodes them in the background through the decoder
//! catalog's extractor, from the media the project plays (the preview's
//! `StillLocator`), and keeps them in memory.
//!
//! An arrow moves an edit one frame, so a wanted picture is decoded with its neighbours:
//! one decoder pass makes `RUN` consecutive pictures around it, and the next steps come
//! from memory. Where a clip's picture is is found once per clip. One background worker
//! decodes the pictures still missing in the newest draft, one pass at a time, and each
//! pass is shown as soon as it is made; a pass that hangs is cancelled after `STUCK`.
//! A picture not shown yet comes first on its own (user 2026-10-13: "the picture of the
//! neighbouring clip shows slowly"): every missing picture is made alone, one decoder
//! per picture at the same time, and only then the passes with neighbours for the arrows.
//! While a new picture is being made, its place keeps the last picture it showed
//! (user 2026-10-11: "ima pa nema"), never a black hole. No probe, no database write;
//! the pictures are a passive aid, not the program.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};

mod open_run;
mod worker;
use worker::*;

use qnc_image_assets::RgbaImage;
use qnc_program_view::{SegmentsView, StillPicture};
use qnc_still_locator::{LocatedPicture, StillLocator};

pub const MODULE_ID: &str = "qnc.module.trim-stills";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The size an edit picture is decoded at (the trim zoom draws it up to 480x270).
const STILL_SIZE: [u32; 2] = [320, 180];
/// Pictures decoded in one pass: the wanted one and `RUN_BACK` on each side.
const RUN_BACK: u64 = 12;
const RUN: u32 = 25;
/// Pictures kept in memory (about 230 KB each).
const KEPT: usize = 160;
/// Columns of the zoom's fine wave (about one per pixel of the window).
const ZOOM_COLUMNS: usize = 900;
/// A pass taking longer is cancelled and the worker starts again.
const STUCK: Duration = Duration::from_secs(8);

type Decoded = Vec<(StillPicture, Result<Arc<RgbaImage>, String>)>;
type Located = Arc<Mutex<HashMap<String, LocatedPicture>>>;
/// A picture to make, the first frame of the pass that makes it and how many pictures
/// the pass makes (1: the picture alone, made first).
type Want = (StillPicture, u64, u32);

/// The background worker: it takes the first of `wanted` (the newest draft, kept up to
/// date by `fill`), sends each pass when made and ends when nothing is wanted.
struct Job {
    receiver: mpsc::Receiver<Decoded>,
    wanted: Arc<Mutex<Vec<Want>>>,
    /// When the pass being made started.
    pass: Arc<Mutex<Option<Instant>>>,
    cancel: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct TrimStills {
    project_id: Option<String>,
    cache: HashMap<StillPicture, Arc<RgbaImage>>,
    order: VecDeque<StillPicture>,
    failed: HashSet<StillPicture>,
    located: Located,
    job: Option<Job>,
    /// The pictures last asked for, as logged (diagnostics only).
    asked: String,
    /// The monitor picture last taken (output generation, sequence).
    offered: Option<(u64, u64)>,
    /// The last picture each place of the zoom (by its name) showed.
    shown: HashMap<String, Arc<RgbaImage>>,
    /// Each place's last picture and the way its frame last moved (+1, -1), for making
    /// the next pictures ahead of the arrow.
    moving: HashMap<String, (StillPicture, i8)>,
    /// The fine wave of the zoom's cut (user 2026-10-25), read around it from the original.
    fine: qnc_fine_wave::FineWave,
    /// The words A1 says there (user 2026-10-26), from the project's transcripts.
    words: qnc_zoom_words::ZoomWords,
}

impl TrimStills {
    pub fn new() -> Self {
        Self::default()
    }

    /// The picture on the monitor as the clip picture the story shows there (user
    /// 2026-10-13): a picture of the zoom that is it needs no decoding.
    /// Only a picture the zoom of `view` wants is taken (nothing is copied while playing).
    fn offer(&mut self, view: &SegmentsView, shown: Option<(StillPicture, qnc_source_preview::ShownPicture)>) {
        let Some((picture, shown)) = shown else { return };
        if self.offered == Some(shown.sequence) || self.cache.contains_key(&picture) {
            return;
        }
        let wanted = view.edit_stills.iter().chain(&view.source_edit_stills).any(|still| still.picture.as_ref() == Some(&picture));
        if !wanted {
            return;
        }
        self.offered = Some(shown.sequence);
        let content_key = shown.sequence.0.wrapping_mul(1_000_003) ^ shown.sequence.1;
        let image = RgbaImage { size: shown.size, pixels: shown.rgba.to_vec(), content_key };
        self.order.push_back(picture.clone());
        self.cache.insert(picture, Arc::new(image));
    }

    /// Puts the pictures on the edit pictures of `view` (Wrap rows and source timeline),
    /// decoded here. Returns whether a picture arrived.
    pub fn fill(
        &mut self,
        view: &mut SegmentsView,
        locator: Option<StillLocator>,
        shown: Option<(StillPicture, qnc_source_preview::ShownPicture)>,
    ) -> bool {
        self.offer(view, shown);
        let Some(locator) = locator else {
            return false;
        };
        if self.project_id.as_deref() != Some(locator.project_id()) {
            *self = Self { project_id: Some(locator.project_id().to_string()), ..Self::default() };
        }
        let arrived = self.take_finished();
        let asked = describe(view);
        if asked != self.asked {
            log(&format!("trim-stills asked {asked}"));
            self.asked = asked;
        }
        if view.edit_stills.is_empty() && view.source_edit_stills.is_empty() {
            self.shown.clear();
        }
        // The sound under the pictures: a peak per column of each lane, per mille.
        if let Some(sound) = view.zoom_sound.as_mut() {
            let mut a1_read = false;
            for lane in 0..2 {
                // While the window follows the play its sound is read ahead (user 2026-10-30).
                let reach = qnc_program_view::zoom_reach(sound.from, sound.to, sound.playhead);
                if reach > sound.to {
                    self.fine.lane_peaks(&locator, &sound.lanes[lane], sound.from, reach, 1);
                }
                let (peaks, complete) = self.fine.lane_peaks(&locator, &sound.lanes[lane], sound.from, sound.to, ZOOM_COLUMNS);
                sound.peaks[lane] = peaks.iter().map(|peak| (peak * 1000.0).round() as u16).collect();
                a1_read |= lane == 0 && complete;
            }
            self.words.fill(&locator, sound, a1_read);
        }
        for still in view.edit_stills.iter_mut().chain(view.source_edit_stills.iter_mut()) {
            still.image = still.picture.as_ref().and_then(|picture| self.cache.get(picture).cloned());
            match (&still.image, still.picture.is_some()) {
                (Some(image), _) => {
                    self.shown.insert(still.label.clone(), image.clone());
                }
                // Not made yet: the place keeps what it showed until the new one comes.
                (None, true) => still.image = self.shown.get(&still.label).cloned(),
                (None, false) => {
                    self.shown.remove(&still.label);
                }
            }
        }
        let mut missing: Vec<StillPicture> = Vec::new();
        let stills = view.edit_stills.iter().chain(&view.source_edit_stills);
        for picture in stills.filter_map(|still| still.picture.as_ref()) {
            if !self.cache.contains_key(picture) && !self.failed.contains(picture) && !missing.contains(picture) {
                missing.push(picture.clone());
            }
        }
        // The missing pictures first, each alone; then the neighbours of every shown
        // picture and the ones the arrow reaches next, so stepping frame by frame finds
        // them made.
        let mut wants: Vec<Want> = missing.iter().map(|picture| (picture.clone(), picture.source_frame, 1)).collect();
        let shown: Vec<StillPicture> = view
            .edit_stills
            .iter()
            .chain(&view.source_edit_stills)
            .filter_map(|still| still.picture.clone())
            .collect();
        for picture in shown {
            let near = |step: i64| StillPicture {
                clip_id: picture.clip_id.clone(),
                source_frame: picture.source_frame.saturating_add_signed(step),
            };
            let lacks = |frame: &StillPicture| !self.cache.contains_key(frame) && !self.failed.contains(frame);
            let end = view
                .edit_stills
                .iter()
                .chain(&view.source_edit_stills)
                .find(|still| still.picture.as_ref() == Some(&picture))
                .and_then(|still| still.clip_frames)
                .unwrap_or(u64::MAX);
            if ((picture.source_frame + 1 < end && lacks(&near(1))) || (picture.source_frame > 0 && lacks(&near(-1))))
                && !wants.iter().any(|(want, _, count)| *want == picture && *count == RUN)
            {
                wants.push((picture.clone(), picture.source_frame.saturating_sub(RUN_BACK), RUN));
            }
        }
        for (picture, first) in self.ahead(view) {
            if !wants.iter().any(|(want, _, _)| *want == picture) {
                wants.push((picture, first, RUN));
            }
        }
        match &self.job {
            Some(job) => {
                if let Ok(mut wanted) = job.wanted.lock() {
                    *wanted = wants;
                }
            }
            None if !wants.is_empty() => self.start(wants, locator),
            None => {}
        }
        arrived
    }

    /// For each place whose picture moves frame by frame, the first picture not yet made
    /// in that direction within two passes, with a pass that starts there.
    fn ahead(&mut self, view: &SegmentsView) -> Vec<(StillPicture, u64)> {
        let mut ahead = Vec::new();
        let mut seen = HashSet::new();
        for still in view.edit_stills.iter().chain(&view.source_edit_stills) {
            let Some(picture) = &still.picture else { continue };
            seen.insert(still.label.clone());
            if still.clip_edge.is_some() {
                continue; // at the end of its clip: nothing further to make that way
            }
            let way = match self.moving.get(&still.label) {
                Some((last, way)) if last.clip_id == picture.clip_id && last.source_frame != picture.source_frame => {
                    if picture.source_frame > last.source_frame { 1 } else { -1 }
                }
                Some((last, way)) if last == picture => *way,
                _ => 0,
            };
            self.moving.insert(still.label.clone(), (picture.clone(), way));
            if way == 0 {
                continue;
            }
            let end = still.clip_frames.unwrap_or(u64::MAX);
            let next = (1..=2 * u64::from(RUN))
                .filter_map(|step| match way {
                    1 => picture.source_frame.checked_add(step),
                    _ => picture.source_frame.checked_sub(step),
                })
                .filter(|frame| *frame < end) // never past the end of the clip
                .map(|frame| StillPicture { clip_id: picture.clip_id.clone(), source_frame: frame })
                .find(|frame| !self.cache.contains_key(frame) && !self.failed.contains(frame));
            if let Some(next) = next {
                let first = if way > 0 { next.source_frame } else { next.source_frame.saturating_sub(u64::from(RUN) - 1) };
                if !ahead.iter().any(|(want, _)| *want == next) {
                    ahead.push((next, first));
                }
            }
        }
        self.moving.retain(|label, _| seen.contains(label));
        ahead
    }

    /// Whether a picture is being made (the caller repaints soon).
    pub fn loading(&self) -> bool {
        self.job.is_some() || self.fine.loading() || self.words.loading()
    }

    fn take_finished(&mut self) -> bool {
        let Some(job) = &self.job else {
            return false;
        };
        let mut decoded = Decoded::new();
        loop {
            match job.receiver.try_recv() {
                Ok(pass) => decoded.extend(pass),
                Err(mpsc::TryRecvError::Empty) => {
                    let stuck = job.pass.lock().ok().and_then(|pass| *pass).is_some_and(|at| at.elapsed() >= STUCK);
                    if stuck {
                        log("trim-stills pass stuck, cancelled");
                        job.cancel.store(true, Ordering::Release);
                        self.job = None;
                    }
                    break;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.job = None;
                    break;
                }
            }
        }
        let arrived = !decoded.is_empty();
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
        arrived
    }

    fn start(&mut self, pictures: Vec<Want>, locator: StillLocator) {
        let (send, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let wanted = Arc::new(Mutex::new(pictures));
        let pass = Arc::new(Mutex::new(None));
        let worker = (self.located.clone(), cancel.clone(), wanted.clone(), pass.clone());
        let spawned = std::thread::Builder::new().name("qnc-trim-stills".into()).spawn(move || {
            let (located, cancel, wanted, pass) = worker;
            work(&send, &wanted, &pass, &locator, &located, &cancel);
        });
        if spawned.is_ok() {
            self.job = Some(Job { receiver, wanted, pass, cancel });
        }
    }
}


#[cfg(test)]
mod tests;
