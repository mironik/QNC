//! The words around the cut of the trim zoom (user 2026-10-26: "a second or two before and
//! after the IN/OUT cut, so the start or end of a statement is easy to find"). For every piece
//! of sound A1 plays in the zoom's window, the words its clip said there are taken from the
//! transcripts kept in the project database; a part no transcript covers is transcribed once,
//! five seconds on each side of it, off the form thread, kept in the database and read again
//! from there. The database is the truth; what is held here is only what was read.

use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
};

use qnc_program_view::{ZoomSound, ZoomWord};
use qnc_still_locator::StillLocator;
use qnc_transcript_db::ClipTranscript;

pub const MODULE_ID: &str = "qnc.module.zoom-words";
/// Seconds transcribed on each side of the part the zoom shows.
const AROUND_SECONDS: u64 = 5;

type Read = Result<Vec<ClipTranscript>, String>;

/// A piece of the zoom's A1: clip frames `[first, last)` it plays in the window.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Need {
    clip_id: String,
    first: u64,
    last: u64,
    /// The piece on the zoom's axis and in its clip: `(program_start, program_end, source_in, source_out)`.
    piece: (u64, u64, u64, u64),
}

struct Job {
    need: Need,
    done: mpsc::Receiver<Result<(), String>>,
    cancel: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct ZoomWords {
    project_id: Option<String>,
    kept: HashMap<String, Vec<ClipTranscript>>,
    reading: Option<(String, mpsc::Receiver<Read>)>,
    job: Option<Job>,
    /// Clips whose transcription failed: not asked again in this run (each arrow step moves
    /// the window, so a failed clip would be asked at every step).
    failed: HashSet<String>,
}

impl ZoomWords {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a read or a transcription runs (the form repaints soon).
    pub fn loading(&self) -> bool {
        self.reading.is_some() || self.job.is_some()
    }

    /// Puts the words A1 says in the window of `sound` on it.
    pub fn fill(&mut self, locator: &StillLocator, sound: &mut ZoomSound) {
        if self.project_id.as_deref() != Some(locator.project_id()) {
            self.stop();
            *self = Self { project_id: Some(locator.project_id().to_string()), ..Self::default() };
        }
        self.take_finished();
        let needs = needs(sound);
        for need in &needs {
            match self.kept.get(&need.clip_id) {
                None => self.read(locator, &need.clip_id),
                Some(kept) if !covered(kept, need) => self.transcribe(locator, need),
                Some(_) => {}
            }
        }
        sound.words = needs.iter().flat_map(|need| self.words_of(sound, need)).collect();
        sound.words.sort_by_key(|word| word.from);
        sound.words.dedup_by(|a, b| a.from == b.from && a.text == b.text);
    }

    /// Stops a running transcription (the zoom closed or another project).
    pub fn stop(&mut self) {
        if let Some(job) = self.job.take() {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn take_finished(&mut self) {
        if let Some((clip, done)) = &self.reading {
            match done.try_recv() {
                Ok(read) => {
                    if let Err(error) = &read {
                        log(&format!("zoom-words read {clip}: {error}"));
                    }
                    self.kept.insert(clip.clone(), read.unwrap_or_default());
                    self.reading = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.reading = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(job) = &self.job {
            match job.done.try_recv() {
                Ok(result) => {
                    if let Err(error) = &result {
                        log(&format!("zoom-words transcribe {} {}..{}: {error}", job.need.clip_id, job.need.first, job.need.last));
                        self.failed.insert(job.need.clip_id.clone());
                    }
                    // What was kept is read again from the database.
                    self.kept.remove(&job.need.clip_id);
                    self.job = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.job = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
    }

    fn read(&mut self, locator: &StillLocator, clip_id: &str) {
        if self.reading.is_some() {
            return;
        }
        let (send, done) = mpsc::channel();
        let (locator, clip) = (locator.clone(), clip_id.to_string());
        let spawned = std::thread::Builder::new().name("qnc-zoom-words-read".into()).spawn(move || {
            let read = qnc_db_broker::ProjectDbTarget::for_project(locator.reader(), locator.settings())
                .map_err(|error| error.to_string())
                .and_then(|target| qnc_transcript_db::read_clip(&target, &clip));
            let _ = send.send(read);
        });
        if spawned.is_ok() {
            self.reading = Some((clip_id.to_string(), done));
        }
    }

    fn transcribe(&mut self, locator: &StillLocator, need: &Need) {
        if self.job.is_some() || self.failed.contains(&need.clip_id) || !locator.settings().transcription_enabled().unwrap_or(false) {
            return;
        }
        let (send, done) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let (locator, wanted, stop) = (locator.clone(), need.clone(), cancel.clone());
        let spawned = std::thread::Builder::new().name("qnc-zoom-words".into()).spawn(move || {
            let made = locator.locate_sound(&wanted.clip_id).and_then(|place| {
                let around = AROUND_SECONDS * u64::from(place.fps.0) / u64::from(place.fps.1.max(1));
                let ask = qnc_transcribe_run::Ask { clip_id: wanted.clip_id.clone(), from: wanted.first.saturating_sub(around), to: wanted.last + around };
                let transcript = qnc_transcribe_run::transcribe(&locator, &ask, &stop, &mut |_| {})?;
                let target = qnc_db_broker::ProjectDbTarget::for_project(locator.reader(), locator.settings()).map_err(|error| error.to_string())?;
                qnc_transcript_db::save(&target, transcript)
            });
            let _ = send.send(made);
        });
        if spawned.is_ok() {
            self.job = Some(Job { need: need.clone(), done, cancel });
        }
    }

    /// The words of `need`'s clip in its part, on the zoom's axis.
    fn words_of(&self, sound: &ZoomSound, need: &Need) -> Vec<ZoomWord> {
        let Some(kept) = self.kept.get(&need.clip_id) else { return Vec::new() };
        // The one transcript covering the part, else every one that touches it.
        let covering: Vec<&ClipTranscript> = match kept.iter().find(|t| t.from_frame <= need.first && need.last <= t.to_frame) {
            Some(one) => vec![one],
            None => kept.iter().filter(|t| t.from_frame < need.last && need.first < t.to_frame).collect(),
        };
        let (program_start, program_end, source_in, source_out) = need.piece;
        let axis = |frame: u64| to_axis(program_start, program_end, source_in, source_out, frame).clamp(sound.from, sound.to);
        covering
            .into_iter()
            .flat_map(|t| t.lines.iter().flat_map(|line| line.words.iter()))
            .filter(|word| word.from < need.last && need.first < word.to.max(word.from + 1))
            .map(|word| ZoomWord { from: axis(word.from), to: axis(word.to.max(word.from + 1)), text: word.text.clone() })
            .collect()
    }
}

/// The clip frames each piece of A1 plays in the zoom's window.
fn needs(sound: &ZoomSound) -> Vec<Need> {
    sound.lanes[0]
        .iter()
        .filter(|at| !at.off && at.program_start < sound.to && sound.from < at.program_end && at.source_out > at.source_in)
        .map(|at| {
            let source = |frame: u64| to_source(at.program_start, at.program_end, at.source_in, at.source_out, frame);
            Need {
                clip_id: at.clip_id.clone(),
                first: source(sound.from.max(at.program_start)),
                last: source(sound.to.min(at.program_end)),
                piece: (at.program_start, at.program_end, at.source_in, at.source_out),
            }
        })
        .filter(|need| need.last > need.first)
        .collect()
}

fn covered(kept: &[ClipTranscript], need: &Need) -> bool {
    kept.iter().any(|t| t.from_frame <= need.first && need.last <= t.to_frame)
}

/// A program frame of a piece as its clip frame (the piece may play the clip at another rate).
fn to_source(program_start: u64, program_end: u64, source_in: u64, source_out: u64, frame: u64) -> u64 {
    let span = (program_end - program_start).max(1);
    source_in + (frame.saturating_sub(program_start)) * (source_out - source_in) / span
}

fn to_axis(program_start: u64, program_end: u64, source_in: u64, source_out: u64, frame: u64) -> u64 {
    let span = (source_out - source_in).max(1);
    program_start + (frame.saturating_sub(source_in)) * (program_end - program_start) / span
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
    fn a_piece_maps_between_program_and_clip_frames_at_its_rate() {
        // A 25p clip in a 50p story: two program frames per clip frame.
        assert_eq!(to_source(1000, 1200, 300, 400, 1100), 350);
        assert_eq!(to_axis(1000, 1200, 300, 400, 350), 1100);
        let kept = vec![ClipTranscript { clip_id: "c".into(), from_frame: 200, to_frame: 500, language: "hr".into(), engine: "e".into(), lines: Vec::new() }];
        let need = |first, last| Need { clip_id: "c".into(), first, last, piece: (0, 0, 0, 0) };
        assert!(covered(&kept, &need(300, 400)));
        assert!(!covered(&kept, &need(450, 550)));
    }
}
