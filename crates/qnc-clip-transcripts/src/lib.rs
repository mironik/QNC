//! The one reader of the kept transcripts of a project's clips (user 2026-11-06: "the only
//! truth is the database, no other link between forms"; docs/arhitektura.md P1-1). The panel
//! of the transcript and the script each read the same rows once and kept them, each in its own
//! memory; a transcript written meanwhile by another surface stayed unseen in one of them. Now
//! the pieces say which clips they show, this reads them from the database in one background
//! thread, again every [`READ_EVERY`], and every piece takes them from here. What is held is
//! only the last answer of the database: a change is seen by comparing it with the next one.

use std::{
    collections::{BTreeSet, HashMap},
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

use qnc_db_broker::ProjectDbTarget;
use qnc_transcript_db::ClipTranscript;

pub const MODULE_ID: &str = "qnc.module.clip-transcripts";

/// How often the clips asked for are read from the database again.
pub const READ_EVERY: Duration = Duration::from_secs(1);

type Answer = (String, Instant, Result<HashMap<String, Result<Vec<ClipTranscript>, String>>, String>);

/// The kept transcripts of the clips asked for, as the database last gave them.
#[derive(Debug, Default)]
pub struct ClipTranscripts {
    project: Option<String>,
    wanted: BTreeSet<String>,
    kept: HashMap<String, Arc<Vec<ClipTranscript>>>,
    /// Grows whenever what the database gave differs from what was held.
    rev: u64,
    error: Option<String>,
    reading: Option<mpsc::Receiver<Answer>>,
    read_at: Option<Instant>,
    /// When the last read whose answer was taken started.
    last_read: Option<Instant>,
    /// Clips whose answer is not taken while the form stays as it is (user 2026-11-08: a
    /// statement transcribed again goes to the database only; the form shows it once another
    /// clip is shown or the form opens again). A clip no longer asked for is let go.
    held: BTreeSet<String>,
}

impl ClipTranscripts {
    pub fn new() -> Self {
        Self::default()
    }

    /// Asks for the transcripts of `clips` in `project` and takes the answer of a read done; a
    /// clip not read yet, another project or [`read_now`](Self::read_now) reads at once, else
    /// every [`READ_EVERY`]. True when what is held changed.
    pub fn poll(&mut self, project: Option<&ProjectDbTarget>, clips: impl IntoIterator<Item = String>) -> bool {
        let target = project.cloned();
        let key = project.map(|target| target.uri().to_string());
        self.ask(key, move || target.ok_or_else(String::new), clips)
    }

    /// The same, for a project known by `key` whose database `target` finds (it runs in the
    /// background thread: finding it may read the host's files).
    pub fn poll_from(&mut self, key: &str, target: impl FnOnce() -> Result<ProjectDbTarget, String> + Send + 'static, clips: impl IntoIterator<Item = String>) -> bool {
        self.ask(Some(key.to_string()), target, clips)
    }

    fn ask(&mut self, key: Option<String>, target: impl FnOnce() -> Result<ProjectDbTarget, String> + Send + 'static, clips: impl IntoIterator<Item = String>) -> bool {
        let mut changed = false;
        if key != self.project {
            changed = !self.kept.is_empty();
            *self = Self { project: key.clone(), rev: self.rev + u64::from(changed), ..Self::default() };
        }
        self.wanted = clips.into_iter().filter(|clip| !clip.is_empty()).collect();
        if let Some(reading) = &self.reading {
            match reading.try_recv() {
                Ok((from, started, answer)) => {
                    self.reading = None;
                    if Some(&from) == self.project.as_ref() {
                        self.last_read = Some(started);
                        match answer {
                            Ok(answer) => changed |= self.take(answer),
                            Err(error) => self.error = Some(error),
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => self.reading = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        self.kept.retain(|clip, _| self.wanted.contains(clip));
        self.held.retain(|clip| self.wanted.contains(clip));
        let (Some(key), None) = (key, &self.reading) else { return changed };
        let unread = self.wanted.iter().any(|clip| !self.kept.contains_key(clip));
        let due = self.read_at.is_none_or(|at| at.elapsed() >= READ_EVERY);
        if self.wanted.is_empty() || !(unread || due) {
            return changed;
        }
        let (send, reading) = mpsc::channel();
        let (clips, started) = (self.wanted.clone(), Instant::now());
        let read = move || {
            let answer = target().map(|target| clips.into_iter().map(|clip| {
                let read = qnc_transcript_db::read_clip(&target, &clip);
                (clip, read)
            }).collect());
            _ = send.send((key, started, answer));
        };
        if std::thread::Builder::new().name("qnc-clip-transcripts".into()).spawn(read).is_ok() {
            self.reading = Some(reading);
            self.read_at = Some(started);
        }
        changed
    }

    /// Whether a read started at or after `at` was taken (what is held is the database's
    /// answer after that moment).
    pub fn read_since(&self, at: Instant) -> bool {
        self.last_read.is_some_and(|read| read >= at)
    }

    /// What the database gave, against what was held: true when something differs.
    fn take(&mut self, answer: HashMap<String, Result<Vec<ClipTranscript>, String>>) -> bool {
        let mut changed = false;
        self.error = None;
        for (clip, read) in answer {
            let transcripts = match read {
                Ok(transcripts) => transcripts,
                Err(error) => {
                    // A clip not readable now keeps what was read last; the panel says why.
                    self.error = Some(error);
                    continue;
                }
            };
            // A held clip keeps what it shows (read once, it is not read anew).
            if self.held.contains(&clip) && self.kept.contains_key(&clip) {
                continue;
            }
            if self.kept.get(&clip).is_none_or(|held| **held != transcripts) {
                self.kept.insert(clip, Arc::new(transcripts));
                changed = true;
            }
        }
        if changed {
            self.rev += 1;
        }
        changed
    }

    /// The next poll reads from the database (a transcript was just written).
    pub fn read_now(&mut self) {
        self.read_at = None;
    }

    /// `clip` keeps what is shown of it: new answers of the database are not taken until
    /// [`let_go`](Self::let_go) (or until it is no longer asked for).
    pub fn hold(&mut self, clip: &str) {
        self.held.insert(clip.to_string());
    }

    /// Every held clip takes the database's answer again (another clip shown, the form opened).
    pub fn let_go(&mut self) {
        if !self.held.is_empty() {
            self.held.clear();
            self.read_at = None;
        }
    }

    /// Whether `clip` keeps what is shown of it.
    pub fn is_held(&self, clip: &str) -> bool {
        self.held.contains(clip)
    }

    /// The kept transcripts of `clip`; `None` while it was not read yet.
    pub fn of(&self, clip: &str) -> Option<&Arc<Vec<ClipTranscript>>> {
        self.kept.get(clip)
    }

    /// Grows with every change of what is held (a view built from it is built again).
    pub fn rev(&self) -> u64 {
        self.rev
    }

    /// Why the last read of a clip failed.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Whether a read runs now.
    pub fn busy(&self) -> bool {
        self.reading.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transcript(text: &str) -> ClipTranscript {
        ClipTranscript { clip_id: "a".into(), from_frame: 0, to_frame: 10, language: "hr".into(), engine: "test".into(), lines: vec![qnc_transcript_db::Line { from: 0, to: 10, text: text.into(), words: Vec::new() }] }
    }

    #[test]
    fn only_an_answer_that_differs_changes_what_is_held() {
        let mut held = ClipTranscripts::new();
        assert!(held.take(HashMap::from([("a".to_string(), Ok(vec![transcript("ovo")]))])));
        let rev = held.rev();
        assert!(!held.take(HashMap::from([("a".to_string(), Ok(vec![transcript("ovo")]))])), "the same answer");
        assert_eq!(held.rev(), rev);
        assert!(!held.take(HashMap::from([("a".to_string(), Err("busy".to_string()))])), "a failed read keeps the last answer");
        assert_eq!(held.of("a").map(|kept| kept.len()), Some(1));
        assert_eq!(held.error(), Some("busy"));
        assert!(held.take(HashMap::from([("a".to_string(), Ok(vec![transcript("ovo"), transcript("je")]))])), "a transcript written meanwhile");
        assert!(held.rev() > rev);
    }

    #[test]
    fn a_held_clip_keeps_what_it_shows_until_it_is_let_go() {
        let mut held = ClipTranscripts::new();
        assert!(held.take(HashMap::from([("a".to_string(), Ok(vec![transcript("staro")]))])));
        held.hold("a");
        assert!(!held.take(HashMap::from([("a".to_string(), Ok(vec![transcript("novo")]))])), "the repass is not shown on this form");
        assert_eq!(held.of("a").unwrap()[0].lines[0].text, "staro");
        held.let_go();
        assert!(held.take(HashMap::from([("a".to_string(), Ok(vec![transcript("novo")]))])), "the form opened again reads the database");
    }
}
