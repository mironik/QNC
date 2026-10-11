//! Up/Down on a taken edit (user 2026-10-11: "with Ctrl active, arrow up and down move
//! by a GOP; if the GOP is a complication, to the nearest intra picture; for an intra
//! codec half a second"). Given a clip and the frame of the picture beside the edit, this
//! piece finds the previous or next key picture of the media the project plays (the
//! preview's `StillLocator`) and answers with the frames to move. When every picture is
//! a key picture (an intra codec) the answer is half a second. The key pictures are read
//! from the packets in the background through the decoder catalog: nothing is decoded,
//! probed or written. One search at a time; a search that hangs is cancelled.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};

use qnc_source_preview::{LocatedPicture, StillLocator};

pub const MODULE_ID: &str = "qnc.module.keyframe-nav";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How far a long-GOP search looks for the next key picture.
const SEARCH_SECONDS: f64 = 10.0;
/// A search taking longer is cancelled.
const STUCK: Duration = Duration::from_secs(8);

struct Search {
    receiver: mpsc::Receiver<Result<i64, String>>,
    started: Instant,
    cancel: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct KeyframeNav {
    search: Option<Search>,
    /// Whether a clip's picture is intra (every picture a key picture), once known.
    intra: Arc<Mutex<HashMap<String, bool>>>,
}

impl KeyframeNav {
    pub fn new() -> Self {
        Self::default()
    }

    /// Looks for the key picture before (`forward` false) or after `frame` of `clip_id`.
    /// False while another search runs or without a project.
    pub fn request(&mut self, clip_id: &str, frame: u64, forward: bool, locator: Option<StillLocator>) -> bool {
        let Some(locator) = locator.filter(|_| self.search.is_none()) else {
            return false;
        };
        let (send, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let (clip, stop, intra) = (clip_id.to_string(), cancel.clone(), self.intra.clone());
        let spawned = std::thread::Builder::new().name("qnc-keyframe-nav".into()).spawn(move || {
            let started = Instant::now();
            let answer = locator.locate(&clip).and_then(|place| step(&place, frame, forward, (&stop, &clip, &intra)));
            log(&format!("keyframe-nav {clip}:{frame} forward={forward} -> {answer:?} ms={}", started.elapsed().as_millis()));
            let _ = send.send(answer);
        });
        if spawned.is_ok() {
            self.search = Some(Search { receiver, started: Instant::now(), cancel });
        }
        spawned.is_ok()
    }

    /// The frames to move, once the search has an answer.
    pub fn take(&mut self) -> Option<Result<i64, String>> {
        let search = self.search.as_ref()?;
        let answer = match search.receiver.try_recv() {
            Ok(answer) => answer,
            Err(mpsc::TryRecvError::Empty) if search.started.elapsed() < STUCK => return None,
            Err(mpsc::TryRecvError::Empty) => {
                search.cancel.store(true, Ordering::Release);
                Err("Trazenje kljucne slike je predugo trajalo.".into())
            }
            Err(mpsc::TryRecvError::Disconnected) => Err("Trazenje kljucne slike nije uspjelo.".into()),
        };
        self.search = None;
        Some(answer)
    }

    /// Whether a search runs (the caller repaints soon).
    pub fn busy(&self) -> bool {
        self.search.is_some()
    }
}

/// The frames from `frame` to the key picture before or after it; half a second when
/// the pictures next to it are key pictures too (an intra codec).
fn step(
    place: &LocatedPicture,
    frame: u64,
    forward: bool,
    (cancel, clip, intra): (&AtomicBool, &str, &Mutex<HashMap<String, bool>>),
) -> Result<i64, String> {
    let extractor = qnc_decoder_catalog::installed_filmstrip_extractor()
        .map_err(|error| error.to_string())?
        .ok_or("Ovo racunalo nema izvlakac slika u katalogu dekodera.")?;
    let one = 1.0 / per_second(place);
    let keys = |start: u64, seconds: f64| -> Result<Vec<u64>, String> {
        let times = extractor.key_pictures_with_cancel(&place.file, place.seek_of(start), seconds, cancel)?;
        Ok(times.into_iter().filter_map(|time| place.frame_at(time)).collect())
    };
    let half = (per_second(place) / 2.0).round().max(1.0) as i64;
    if !forward && frame == 0 {
        return Err("Pocetak klipa.".into());
    }
    // Whether every picture is a key picture: two pictures next to it, once per clip.
    let known = intra.lock().ok().and_then(|map| map.get(clip).copied());
    let is_intra = match known {
        Some(is_intra) => is_intra,
        None => {
            let (first, next) = if forward { (frame + 1, frame + 2) } else { (frame.saturating_sub(2), frame.saturating_sub(1)) };
            let near = keys(first, 2.5 * one)?;
            let is_intra = first != next && near.contains(&first) && near.contains(&next);
            if let Ok(mut map) = intra.lock() {
                map.insert(clip.to_string(), is_intra);
            }
            is_intra
        }
    };
    if is_intra {
        return Ok(if forward { half } else { -half });
    }
    if forward {
        let ahead = keys(frame + 1, SEARCH_SECONDS)?;
        let next = ahead.into_iter().filter(|key| *key > frame).min();
        next.map(|key| (key - frame) as i64).ok_or_else(|| "Nema sljedece kljucne slike u 10 s.".into())
    } else {
        let from = frame.saturating_sub((SEARCH_SECONDS * per_second(place)) as u64);
        let behind = keys(from, SEARCH_SECONDS)?;
        let previous = behind.into_iter().filter(|key| *key < frame).max();
        previous.map(|key| key as i64 - frame as i64).ok_or_else(|| "Nema prethodne kljucne slike u 10 s.".into())
    }
}

fn per_second(place: &LocatedPicture) -> f64 {
    f64::from(place.fps.0) / f64::from(place.fps.1.max(1))
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
    fn without_a_project_there_is_no_search() {
        let mut nav = KeyframeNav::new();
        assert!(!nav.request("c", 10, true, None));
        assert!(!nav.busy());
        assert!(nav.take().is_none());
    }

    /// QNC_KEY_FILE=<a local 50p long-GOP media file starting at 0, read only>.
    #[test]
    #[ignore = "reads a real media file"]
    fn the_next_and_previous_key_pictures_of_a_real_file() {
        let place = LocatedPicture {
            file: std::env::var("QNC_KEY_FILE").unwrap().into(),
            fps: (50, 1),
            start: (0, (1, 50)),
        };
        let (cancel, intra) = (AtomicBool::new(false), Mutex::new(HashMap::new()));
        let cancel = (&cancel, "c", &intra);
        println!("from 30 forward {:?}", step(&place, 30, true, cancel));
        println!("from 30 back {:?}", step(&place, 30, false, cancel));
        println!("from 48 forward {:?}", step(&place, 48, true, cancel));
    }
}

impl std::fmt::Debug for KeyframeNav {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyframeNav").field("busy", &self.busy()).finish()
    }
}
