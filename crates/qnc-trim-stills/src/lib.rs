//! The edit pictures of a trim (user 2026-10-10: a moved slot shows the last picture of
//! the slot before, its own first and last and the first of the slot after; a moved
//! marker the picture before and after it). The program view names them (clip and
//! source frame); this piece decodes them in the background through the decoder
//! catalog's extractor, from the media the project plays (the preview's
//! `StillLocator`), and keeps them in memory.
//!
//! An arrow moves an edit one frame, so a wanted picture is decoded with its neighbours:
//! one decoder pass makes `RUN` consecutive pictures around it, and the next steps come
//! from memory. Where a clip's picture is is found once per clip. One job runs at a time
//! and takes the newest draft; a job that hangs is cancelled after `STUCK`. No player,
//! no probe, no database write; the pictures are a passive aid, not the program.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};

use qnc_image_assets::RgbaImage;
use qnc_program_view::{SegmentsView, StillPicture};
use qnc_source_preview::{LocatedPicture, SourcePreview, StillLocator};

pub const MODULE_ID: &str = "qnc.module.trim-stills";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The size an edit picture is decoded at (the trim zoom draws it up to 480x270).
const STILL_SIZE: [u32; 2] = [320, 180];
/// Pictures decoded in one pass: the wanted one and `RUN_BACK` on each side.
const RUN_BACK: u64 = 8;
const RUN: u32 = 17;
/// Pictures kept in memory (about 230 KB each).
const KEPT: usize = 160;
/// A job taking longer is cancelled and the next one starts.
const STUCK: Duration = Duration::from_secs(8);

type Decoded = Vec<(StillPicture, Result<Arc<RgbaImage>, String>)>;
type Located = Arc<Mutex<HashMap<String, LocatedPicture>>>;

struct Job {
    receiver: mpsc::Receiver<Decoded>,
    started: Instant,
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
    /// When each missing picture was asked of the player.
    asked_player: HashMap<StillPicture, Instant>,
}

/// How long a picture asked of the player is waited for before it is decoded here.
const PLAYER_WAIT: Duration = Duration::from_millis(250);

impl TrimStills {
    pub fn new() -> Self {
        Self::default()
    }

    /// Puts the pictures on the edit pictures of `view` (Wrap rows and source timeline):
    /// first the ones the player holds in memory (user 2026-10-11: one picture source,
    /// nothing decoded twice), the others decoded here. Returns whether a picture arrived.
    pub fn fill(&mut self, view: &mut SegmentsView, locator: Option<StillLocator>, player: Option<&SourcePreview>) -> bool {
        let Some(locator) = locator else {
            return false;
        };
        if self.project_id.as_deref() != Some(locator.project_id()) {
            *self = Self { project_id: Some(locator.project_id().to_string()), ..Self::default() };
        }
        let mut arrived = self.take_finished();
        if let Some(player) = player {
            arrived |= self.take_from_player(player);
        }
        let asked = describe(view);
        if asked != self.asked {
            log(&format!("trim-stills asked {asked}"));
            self.asked = asked;
        }
        for still in view.edit_stills.iter_mut().chain(view.source_edit_stills.iter_mut()) {
            still.image = still.picture.as_ref().and_then(|picture| self.cache.get(picture).cloned());
        }
        let mut missing: Vec<StillPicture> = Vec::new();
        let stills = view.edit_stills.iter().chain(&view.source_edit_stills);
        for picture in stills.filter_map(|still| still.picture.as_ref()) {
            if !self.cache.contains_key(picture) && !self.failed.contains(picture) && !missing.contains(picture) {
                missing.push(picture.clone());
            }
        }
        self.asked_player.retain(|picture, _| missing.contains(picture));
        if let Some(player) = player {
            let new: Vec<(String, u64)> = missing
                .iter()
                .filter(|picture| !self.asked_player.contains_key(*picture))
                .map(|picture| (picture.clip_id.clone(), picture.source_frame))
                .collect();
            if !new.is_empty() {
                player.ask_stills(missing.iter().map(|p| (p.clip_id.clone(), p.source_frame)).collect());
                let now = Instant::now();
                for picture in &missing {
                    self.asked_player.entry(picture.clone()).or_insert(now);
                }
            }
        }
        // Decoded here only what the player did not give in time.
        let waited = |picture: &StillPicture| player.is_none() || self.asked_player.get(picture).is_none_or(|at| at.elapsed() >= PLAYER_WAIT);
        let decode: Vec<StillPicture> = missing.iter().filter(|picture| waited(picture)).cloned().collect();
        if self.job.is_none() && !decode.is_empty() {
            self.start(decode, locator);
        }
        arrived
    }

    /// The pictures the player sent, into the cache.
    fn take_from_player(&mut self, player: &SourcePreview) -> bool {
        let stills = player.take_stills();
        let arrived = !stills.is_empty();
        for still in stills {
            let picture = StillPicture { clip_id: still.source_id, source_frame: still.frame };
            let content_key = {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                picture.hash(&mut hasher);
                hasher.finish()
            };
            let image = RgbaImage { size: [still.width as usize, still.height as usize], pixels: still.rgba.to_vec(), content_key };
            self.asked_player.remove(&picture);
            self.failed.remove(&picture);
            if self.cache.insert(picture.clone(), Arc::new(image)).is_none() {
                self.order.push_back(picture);
            }
        }
        if arrived {
            log("trim-stills from player");
        }
        arrived
    }

    /// Whether a job is decoding (the caller repaints soon).
    pub fn loading(&self) -> bool {
        self.job.is_some() || !self.asked_player.is_empty()
    }

    fn take_finished(&mut self) -> bool {
        let Some(job) = &self.job else {
            return false;
        };
        let decoded = match job.receiver.try_recv() {
            Ok(decoded) => decoded,
            Err(mpsc::TryRecvError::Empty) if job.started.elapsed() < STUCK => return false,
            Err(mpsc::TryRecvError::Empty) => {
                log("trim-stills job stuck, cancelled");
                job.cancel.store(true, Ordering::Release);
                self.job = None;
                return false;
            }
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
        let (send, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let (located, stop) = (self.located.clone(), cancel.clone());
        let spawned = std::thread::Builder::new().name("qnc-trim-stills".into()).spawn(move || {
            let _ = send.send(decode_all(&pictures, &locator, &located, &stop));
        });
        if spawned.is_ok() {
            self.job = Some(Job { receiver, started: Instant::now(), cancel });
        }
    }
}

/// Every wanted picture with its neighbours, one decoder pass per picture not already
/// made by an earlier pass of this job.
fn decode_all(pictures: &[StillPicture], locator: &StillLocator, located: &Located, cancel: &AtomicBool) -> Decoded {
    let mut decoded = Decoded::new();
    for picture in pictures {
        if decoded.iter().any(|(made, _)| made == picture) || cancel.load(Ordering::Acquire) {
            continue;
        }
        let started = Instant::now();
        match decode_run(picture, locator, located, cancel) {
            Ok(run) => {
                log(&format!(
                    "trim-stills run {}:{} pictures={} ms={}",
                    picture.clip_id,
                    picture.source_frame,
                    run.len(),
                    started.elapsed().as_millis()
                ));
                decoded.extend(run.into_iter().map(|(frame, image)| (frame, Ok(image))));
            }
            // A cancelled pass is not a failure of the picture: it is asked again later.
            Err(_) if cancel.load(Ordering::Acquire) => {}
            Err(error) => decoded.push((picture.clone(), Err(error))),
        }
    }
    decoded
}

/// `RUN` pictures of the clip around `picture`, as decoded.
fn decode_run(
    picture: &StillPicture,
    locator: &StillLocator,
    located: &Located,
    cancel: &AtomicBool,
) -> Result<Vec<(StillPicture, Arc<RgbaImage>)>, String> {
    let known = located.lock().ok().and_then(|map| map.get(&picture.clip_id).cloned());
    let place = match known {
        Some(place) => place,
        None => {
            let place = locator.locate(&picture.clip_id)?;
            if let Ok(mut map) = located.lock() {
                map.insert(picture.clip_id.clone(), place.clone());
            }
            place
        }
    };
    let extractor = qnc_decoder_catalog::installed_filmstrip_extractor()
        .map_err(|error| error.to_string())?
        .ok_or("Ovo racunalo nema izvlakac slika u katalogu dekodera.")?;
    let (num, den) = place.fps;
    let first = picture.source_frame.saturating_sub(RUN_BACK);
    // A quarter of a frame early: the first picture at or after it is the frame itself.
    let seek = ((first as f64 - 0.25) * f64::from(den) / f64::from(num.max(1))).max(0.0);
    let scratch = scratch_dir();
    std::fs::create_dir_all(&scratch).map_err(|error| error.to_string())?;
    let result = (|| {
        extractor.extract_run_with_cancel(&place.file, seek, RUN, STILL_SIZE, &scratch, cancel)?;
        let mut run = Vec::new();
        for index in 0..u64::from(RUN) {
            let file = scratch.join(format!("{index:03}.jpg"));
            if !file.is_file() {
                break; // the end of the media
            }
            let frame = StillPicture { clip_id: picture.clip_id.clone(), source_frame: first + index };
            run.push((frame, read_picture(&file)?));
        }
        if !run.iter().any(|(frame, _)| frame == picture) {
            return Err("slika nije izvucena".to_string());
        }
        Ok(run)
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

fn read_picture(file: &Path) -> Result<Arc<RgbaImage>, String> {
    let bytes = std::fs::read(file).map_err(|error| format!("slika nije izvucena: {error}"))?;
    qnc_image_assets::decode_thumbnail(&bytes).map(Arc::new)
}

fn scratch_dir() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let unique = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("qnc-trim-still-{}-{unique}", std::process::id()))
}

fn log(line: &str) {
    if qnc_dev_diagnostics::player_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line.to_string());
    }
}

/// What a view asks for, for the log: name, marker frame and picture of every edit picture.
fn describe(view: &SegmentsView) -> String {
    let stills = view.edit_stills.iter().chain(&view.source_edit_stills);
    let parts: Vec<String> = stills
        .map(|still| {
            let picture = still.picture.as_ref().map_or("-".into(), |p| format!("{}:{}", p.clip_id, p.source_frame));
            format!("[{} @{} {}]", still.label, still.marker_frame, picture)
        })
        .collect();
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_project_nothing_is_decoded() {
        let mut stills = TrimStills::new();
        let picture = StillPicture { clip_id: "c".into(), source_frame: 3 };
        let mut view = SegmentsView {
            edit_stills: vec![qnc_program_view::EditStill::new(10, true, Some(picture))],
            ..Default::default()
        };
        assert!(!stills.fill(&mut view, None, None));
        assert!(!stills.loading());
        assert!(view.edit_stills[0].image.is_none());
    }

    /// QNC_TRIM_FILE=<a local 50p media file, read only> QNC_TRIM_OUT=<scratch dir>.
    #[test]
    #[ignore = "reads a real media file"]
    fn a_run_of_pictures_comes_out_in_order() {
        let file = PathBuf::from(std::env::var("QNC_TRIM_FILE").unwrap());
        let out = PathBuf::from(std::env::var("QNC_TRIM_OUT").unwrap());
        let extractor = qnc_decoder_catalog::installed_filmstrip_extractor().unwrap().unwrap();
        let started = Instant::now();
        extractor
            .extract_run_with_cancel(&file, (188.0 - 0.25) / 50.0, RUN, STILL_SIZE, &out, &AtomicBool::new(false))
            .unwrap();
        println!("run of {RUN} in {} ms", started.elapsed().as_millis());
    }
}
