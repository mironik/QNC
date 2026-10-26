//! What was said in the clip on the preview (user 2026-10-26: Media Assist Audio AI, a clip's
//! IN/OUT or its whole length; a click on a word takes the playhead there). The transcripts
//! kept in the project database are read for the shown clip; the action asks a new one of
//! the source timeline's marks, off the form thread, and its lines show as they are heard;
//! when it is done it is kept in the database (the truth) and read again from there.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};

use qnc_db_broker::ProjectDbTarget;
use qnc_source_preview::SourcePreview;
use qnc_timeline::TimelineProjection;
pub use qnc_transcript_db::{ClipTranscript, Line, Word};

pub const MODULE_ID: &str = "qnc.module.transcript";
/// Transcribe the source timeline's IN/OUT of the shown clip (its start and end without marks).
pub const TRANSCRIBE: &str = "transcribe_marks";
/// A word of the transcript was clicked: the playhead goes to its first frame (value).
pub const CUE_WORD: &str = "transcript_cue_word";
/// The panel the transcript is shown on.
pub const PANEL: &str = "transcript";

/// What the transcript panel shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranscriptView {
    pub clip_id: Option<String>,
    /// The lines of the shown clip in the order of their frames: those kept and those of a
    /// transcription still running.
    pub lines: Vec<Line>,
    /// The clip frames being transcribed now.
    pub running: Option<(u64, u64)>,
    /// The source playhead (the word said there is shown).
    pub playhead: Option<u64>,
    pub message: String,
}

enum Heard {
    Line(Line),
    Done(Result<(), String>),
}

struct Job {
    clip_id: String,
    range: (u64, u64),
    lines: Vec<Line>,
    heard: mpsc::Receiver<Heard>,
    cancel: Arc<AtomicBool>,
}

type Kept = Result<Vec<ClipTranscript>, String>;

#[derive(Default)]
pub struct Transcript {
    view: TranscriptView,
    /// The project and clip whose transcripts are shown, and what is kept for them.
    kept: Option<(String, String, Vec<ClipTranscript>)>,
    reading: Option<(String, String, mpsc::Receiver<Kept>)>,
    job: Option<Job>,
}

impl std::fmt::Debug for Transcript {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transcript").field("view", &self.view).finish_non_exhaustive()
    }
}

impl Transcript {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn view(&self) -> &TranscriptView {
        &self.view
    }

    /// Whether a read or a transcription runs (the form repaints soon).
    pub fn busy(&self) -> bool {
        self.reading.is_some() || self.job.is_some()
    }

    /// The transcript's own actions; `None` for any other.
    pub fn act(&mut self, action_id: &str, value: Option<i64>, preview: &mut SourcePreview, timeline: &TimelineProjection) -> Option<bool> {
        match action_id {
            TRANSCRIBE => Some(self.start(preview, timeline)),
            CUE_WORD => Some(value.and_then(|frame| u64::try_from(frame).ok()).is_some_and(|frame| preview.cue(frame))),
            _ => None,
        }
    }

    fn start(&mut self, preview: &SourcePreview, timeline: &TimelineProjection) -> bool {
        if self.job.is_some() {
            self.view.message = qnc_ui_text::text("transcribe.running").into();
            return true;
        }
        let clip_id = preview.view().clip_id.clone();
        let (Some(clip_id), Some((from, to)), Some(locator), Some(target)) =
            (clip_id, timeline.visible_source_marks(), preview.still_locator(), preview.project_target().cloned())
        else {
            self.view.message = qnc_ui_text::text("transcribe.choose_clip").into();
            return true;
        };
        let ask = qnc_transcribe_run::Ask { clip_id: clip_id.clone(), from, to };
        let (send, heard) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let started = std::thread::Builder::new().name("qnc-transcript".into()).spawn(move || {
            let lines = send.clone();
            let made = qnc_transcribe_run::transcribe(&locator, &ask, &stop, &mut |line| {
                let _ = lines.send(Heard::Line(line.clone()));
            });
            let kept = made.and_then(|transcript| qnc_transcript_db::save(&target, transcript));
            if let Err(error) = &kept {
                qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("transcript {} {}..{}: {error}", ask.clip_id, ask.from, ask.to));
            }
            let _ = send.send(Heard::Done(kept));
        });
        match started {
            Ok(_) => {
                self.job = Some(Job { clip_id, range: (from, to), lines: Vec::new(), heard, cancel });
                self.view.message = qnc_ui_text::text("transcribe.started").into();
            }
            Err(error) => self.view.message = error.to_string(),
        }
        true
    }

    /// Takes what the background heard and reads the shown clip's transcripts when it
    /// changed. Returns whether the view changed.
    pub fn poll(&mut self, preview: &SourcePreview) -> bool {
        let before = self.view.clone();
        let clip_id = preview.view().clip_id.clone();
        let project = preview.project_target().cloned();
        let mut done_for = None;
        if let Some(job) = &mut self.job {
            for heard in job.heard.try_iter() {
                match heard {
                    Heard::Line(line) => job.lines.push(line),
                    Heard::Done(result) => done_for = Some((job.clip_id.clone(), result)),
                }
            }
        }
        if let Some((done_clip, result)) = done_for {
            self.job = None;
            self.view.message = match result {
                Ok(()) => qnc_ui_text::text("transcribe.done").into(),
                Err(error) => error,
            };
            // What was kept is read again from the database.
            if self.kept.as_ref().is_some_and(|(_, clip, _)| *clip == done_clip) {
                self.kept = None;
            }
        }
        self.read_kept(project.as_ref(), clip_id.as_deref());
        self.view.clip_id = clip_id.clone();
        self.view.playhead = preview.view().timeline.playhead_frame;
        let kept = self.kept.as_ref().filter(|(_, clip, _)| Some(clip) == clip_id.as_ref()).map(|(_, _, kept)| kept.as_slice()).unwrap_or_default();
        let running = self.job.as_ref().filter(|job| Some(&job.clip_id) == clip_id.as_ref());
        self.view.running = running.map(|job| job.range);
        self.view.lines = lines(kept, running.map(|job| (job.range, job.lines.as_slice())));
        self.view != before
    }

    /// Stops a running transcription (another project, the form closes).
    pub fn stop(&mut self) {
        if let Some(job) = self.job.take() {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn read_kept(&mut self, project: Option<&ProjectDbTarget>, clip_id: Option<&str>) {
        let (Some(project), Some(clip_id)) = (project, clip_id) else { return };
        if let Some((uri, clip, done)) = &self.reading {
            match done.try_recv() {
                Ok(kept) => {
                    if let Err(error) = &kept {
                        self.view.message = error.clone();
                    }
                    self.kept = Some((uri.clone(), clip.clone(), kept.unwrap_or_default()));
                    self.reading = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.reading = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let is = |uri: &str, clip: &str| uri == project.uri() && clip == clip_id;
        if self.kept.as_ref().is_some_and(|(uri, clip, _)| is(uri, clip)) || self.reading.as_ref().is_some_and(|(uri, clip, _)| is(uri, clip)) {
            return;
        }
        let (send, done) = mpsc::channel();
        let (target, clip) = (project.clone(), clip_id.to_string());
        if std::thread::Builder::new().name("qnc-transcript-read".into()).spawn(move || _ = send.send(qnc_transcript_db::read_clip(&target, &clip))).is_ok() {
            self.reading = Some((project.uri().to_string(), clip_id.to_string(), done));
        }
    }
}

/// The lines of a clip in frame order: the kept transcripts, a running one in place of the
/// kept lines it overlaps.
fn lines(kept: &[ClipTranscript], running: Option<((u64, u64), &[Line])>) -> Vec<Line> {
    let mut lines: Vec<Line> = kept
        .iter()
        .flat_map(|transcript| transcript.lines.iter().cloned())
        .filter(|line| running.is_none_or(|((from, to), _)| line.to <= from || line.from >= to))
        .collect();
    if let Some((_, heard)) = running {
        lines.extend(heard.iter().cloned());
    }
    lines.sort_by_key(|line| (line.from, line.to));
    lines.dedup();
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(from: u64, to: u64, text: &str) -> Line {
        Line { from, to, text: text.into(), words: Vec::new() }
    }

    #[test]
    fn a_running_transcription_shows_in_place_of_the_kept_lines_it_overlaps() {
        let kept = vec![ClipTranscript {
            clip_id: "c".into(),
            from_frame: 0,
            to_frame: 500,
            language: "hr".into(),
            engine: "e".into(),
            lines: vec![line(0, 100, "prva"), line(200, 300, "druga"), line(400, 500, "treca")],
        }];
        let heard = [line(210, 290, "nova druga")];
        let shown = lines(&kept, Some(((150, 350), &heard)));
        assert_eq!(shown.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), ["prva", "nova druga", "treca"]);
        assert_eq!(lines(&kept, None).len(), 3);
    }
}
