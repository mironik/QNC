//! What was said in the clips of a project (user 2026-10-26: Media Assist Audio AI). Clips
//! ticked in the pool, or without a tick the shown clip's IN/OUT (its start and end without
//! marks), are queued in the project database with Fast or Quality, and run in the background
//! by the project's one queue runner (`qnc-transcribe-queue`), yielding to the player; the
//! transcripts kept for the shown clip are read from the database and a running one's lines
//! show as they are heard; a click on a word takes the playhead there. The ticks are only a
//! choice on the screen until Transcribe; the queue and the transcripts are the truth.

use std::{
    collections::{BTreeSet, HashMap},
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

use qnc_clip_row::ClipRow;
use qnc_db_broker::ProjectDbTarget;
use qnc_source_preview::SourcePreview;
use qnc_timeline::TimelineProjection;
use qnc_transcribe_queue::{Heard, Progress, QueueRunner, Queued};
pub use qnc_transcribe_queue::{QueueQuality, QueueState};
pub use qnc_transcript_db::{ClipTranscript, Line, Quote, Word};
use qnc_source_timecode::SourceTimecode;

pub const MODULE_ID: &str = "qnc.module.transcript";
/// Queue the ticked clips (without ticks the shown clip's IN/OUT).
pub const TRANSCRIBE: &str = "transcribe_marks";
/// Fast (on the processor) or Quality (the best engine) for what is queued next.
pub const FAST: &str = "transcribe_fast";
pub const BEST: &str = "transcribe_best";
/// A clip's tick in the pool (target: the clip).
pub const MARK: &str = "transcribe_mark";
/// A word of the transcript was clicked: the playhead goes to its first frame (value).
pub const CUE_WORD: &str = "transcript_cue_word";
/// A word of the transcript was double clicked: a cut there (value: its first frame, target:
/// the frame after its last), IN at a start, OUT at an end.
pub const CUT_WORD: &str = "transcript_cut_word";
/// A transcribed clip of the list opens on the preview (target: the clip; user 2026-10-26: its
/// text without looking for it in the pool).
pub const OPEN_CLIP: &str = "transcript_open_clip";
/// The text between the shown clip's IN and OUT goes into the statements.
pub const ADD_QUOTE: &str = "transcript_add_quote";
/// A statement opens its clip with its IN/OUT (target: its id).
pub const OPEN_QUOTE: &str = "transcript_open_quote";
/// A statement leaves the list (target: its id).
pub const DELETE_QUOTE: &str = "transcript_delete_quote";
/// The panel the transcript is shown on.
pub const PANEL: &str = "transcript";
/// How often the queue is read again.
const QUEUE_EVERY: Duration = Duration::from_secs(1);

/// A stretch of transcribed sound shorter than this with no words is not marked.
const SILENCE_SECONDS: f64 = 2.0;

/// One queued clip as the panel shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct QueueRow {
    pub clip_id: String,
    pub name: String,
    pub state: QueueState,
    pub quality: QueueQuality,
    pub error: String,
    /// Seconds of sound from its IN to its OUT.
    pub length_seconds: f64,
    /// While it runs in this process: how far it is (`None` while its sound is read).
    pub progress: Option<f64>,
    /// While it runs in this process: seconds it has run.
    pub elapsed_seconds: Option<f64>,
    /// When done: seconds it took.
    pub took_seconds: f64,
}

impl QueueRow {
    /// How many times faster than real time it was done.
    pub fn speed(&self) -> Option<f64> {
        (self.state == QueueState::Done && self.took_seconds > 0.0 && self.length_seconds > 0.0).then(|| self.length_seconds / self.took_seconds)
    }
}

/// What the transcript panel shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TranscriptView {
    pub clip_id: Option<String>,
    /// The lines of the shown clip in the order of their frames: those kept and those of a
    /// transcription still running.
    pub lines: Arc<Vec<Line>>,
    /// Clip frames `[from, to)` of transcribed sound in which no word was heard, of two
    /// seconds or more (user 2026-10-26: "audio without text from TC to TC" in red).
    pub silences: Arc<Vec<(u64, u64)>>,
    /// The clip frames being transcribed now, of the shown clip.
    pub running: Option<(u64, u64)>,
    /// The source playhead (the word said there is shown).
    pub playhead: Option<u64>,
    pub message: String,
    /// The queue, in the order it was asked.
    pub queue: Vec<QueueRow>,
    /// The engine what is queued next is transcribed with.
    pub fast: bool,
    /// Clips ticked in the pool.
    pub ticked: usize,
    /// The shown clip's IN/OUT (`[from, to)`): its words are lit as what a statement takes.
    pub marks: Option<(u64, u64)>,
    /// The statements chosen (user 2026-10-26), in the order they were added.
    pub quotes: Vec<Quote>,
    /// The clips that have a kept transcript (the pool's Transcript tab, user 2026-10-28).
    pub transcribed: Arc<BTreeSet<String>>,
    /// True on the poll the whole queue asked was transcribed: the Transcript tab opens.
    pub all_done: bool,
}

impl TranscriptView {
    /// The clips the pool shows: on the Transcript tab those with a kept transcript, else all.
    pub fn pool_clips<'a>(&self, transcript_tab: bool, clips: &'a [ClipRow]) -> Vec<&'a ClipRow> {
        clips.iter().filter(|clip| !transcript_tab || self.transcribed.contains(&clip.clip_id)).collect()
    }

    /// The words of the shown clip said between clip frames `from` and `to` (empty when none
    /// were heard there); the script takes them for a statement.
    pub fn said(&self, from: u64, to: u64) -> String {
        said_between(&self.lines, from, to)
    }

    /// Whether the switch `action_id` (Fast or Quality) is the one chosen.
    pub fn selected(&self, action_id: &str) -> bool {
        (action_id == FAST && self.fast) || (action_id == BEST && !self.fast)
    }

    /// The statements as text to paste into a script: a row each, start and end timecode,
    /// length, clip and text, by tabs (user 2026-10-26).
    pub fn quotes_text(&self) -> String {
        self.quotes
            .iter()
            // For the script (user 2026-10-27): start timecode, the said text, end timecode.
            .map(|q| format!("{}\n{}\n{}", q.start_tc, q.text, q.end_tc))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// The words of `lines` said between `from` and `to` (a word belongs where its middle is).
fn said_between(lines: &[Line], from: u64, to: u64) -> String {
    let mut words = Vec::new();
    for line in lines {
        if line.words.is_empty() {
            if (from..to).contains(&((line.from + line.to) / 2)) {
                words.push(line.text.trim().to_string());
            }
            continue;
        }
        words.extend(line.words.iter().filter(|w| (from..to).contains(&((w.from + w.to.max(w.from + 1)) / 2))).map(|w| w.text.trim().to_string()));
    }
    words.retain(|w| !w.is_empty());
    words.join(" ")
}

type Kept = Result<Vec<ClipTranscript>, String>;
/// The queue, the statements and the transcribed clips, read together.
type QueueRead = (Vec<Queued>, Vec<Quote>, Vec<String>);

/// Whether something of the queue still waits or runs.
fn pending(queue: &[Queued]) -> bool {
    queue.iter().any(|q| matches!(q.state, QueueState::Waiting | QueueState::Running))
}

/// The clip being transcribed now and its lines heard so far.
struct Running {
    clip_id: String,
    range: (u64, u64),
    lines: Vec<Line>,
    progress: Progress,
    started: Instant,
}

pub struct Transcript {
    view: TranscriptView,
    /// The project and clip whose transcripts are shown, and what is kept for them.
    kept: Option<(String, String, Vec<ClipTranscript>)>,
    reading: Option<(String, String, mpsc::Receiver<Kept>)>,
    ticks: BTreeSet<String>,
    quality: QueueQuality,
    runner: QueueRunner,
    running: Option<Running>,
    queue: Vec<Queued>,
    queue_read: Option<mpsc::Receiver<Result<QueueRead, String>>>,
    /// A clip and IN/OUT to open on the source timeline (a statement chosen).
    source_request: Option<(String, u64, u64)>,
    queue_at: Option<Instant>,
    /// An enqueue on its way to the database.
    asking: Option<mpsc::Receiver<Result<(), String>>>,
    /// Name, frames and seconds of each clip of the pool.
    names: HashMap<String, (String, u64, f64)>,
    /// Grows when what the lines are made of changes; the lines are made again only then
    /// (live 2026-10-26: making them at every paint took a fifth of a core).
    rev: u64,
    built: Option<(u64, Option<String>)>,
}

impl Default for Transcript {
    fn default() -> Self {
        Self {
            view: TranscriptView { fast: true, ..TranscriptView::default() },
            kept: None,
            reading: None,
            ticks: BTreeSet::new(),
            quality: QueueQuality::Fast,
            runner: QueueRunner::new(),
            running: None,
            queue: Vec::new(),
            queue_read: None,
            source_request: None,
            queue_at: None,
            asking: None,
            names: HashMap::new(),
            rev: 0,
            built: None,
        }
    }
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

    /// Whether a read or an enqueue of this process runs (the form repaints soon).
    pub fn busy(&self) -> bool {
        self.reading.is_some() || self.asking.is_some()
    }

    /// Whether this process transcribes in the background: the form repaints now and then for
    /// its progress, not at every frame (live 2026-10-26: 61 paints a second took half a core
    /// from the transcription).
    pub fn working(&self) -> bool {
        self.runner.busy()
    }

    /// The transcript's own actions; `None` for any other.
    pub fn act(&mut self, action_id: &str, value: Option<i64>, target: Option<&str>, preview: &mut SourcePreview, timeline: &TimelineProjection, timecode: Option<&SourceTimecode>) -> Option<bool> {
        match action_id {
            OPEN_CLIP => Some(target.is_some_and(|clip| preview.open(clip))),
            ADD_QUOTE => Some(self.add_quote(preview, timeline, timecode)),
            OPEN_QUOTE => Some(target.and_then(|id| id.parse::<i64>().ok()).and_then(|id| self.view.quotes.iter().find(|q| q.id == id)).is_some_and(|q| {
                self.source_request = Some((q.clip_id.clone(), q.from_frame, q.to_frame));
                true
            })),
            DELETE_QUOTE => Some(target.and_then(|id| id.parse::<i64>().ok()).is_some_and(|id| self.write(preview, move |target| qnc_transcript_db::delete_quote(target, id)))),
            TRANSCRIBE => Some(self.start(preview, timeline)),
            FAST | BEST => {
                self.quality = if action_id == FAST { QueueQuality::Fast } else { QueueQuality::Best };
                Some(true)
            }
            MARK => Some(target.is_some_and(|clip| {
                if !self.ticks.remove(clip) {
                    self.ticks.insert(clip.to_string());
                }
                true
            })),
            CUE_WORD => Some(value.and_then(|frame| u64::try_from(frame).ok()).is_some_and(|frame| preview.cue(frame))),
            _ => None,
        }
    }

    /// A clip and IN/OUT a chosen statement opens on the source timeline, once.
    pub fn take_source_request(&mut self) -> Option<(String, u64, u64)> {
        self.source_request.take()
    }

    /// The words said between the shown clip's IN and OUT go into the statements with their
    /// start and end timecode and length (user 2026-10-26).
    fn add_quote(&mut self, preview: &SourcePreview, timeline: &TimelineProjection, timecode: Option<&SourceTimecode>) -> bool {
        let (Some(clip_id), Some((from, to))) = (preview.view().clip_id.clone(), timeline.visible_source_marks()) else {
            self.view.message = qnc_ui_text::text("transcript.quote_marks").into();
            return true;
        };
        let text = if self.view.clip_id.as_deref() == Some(clip_id.as_str()) { said_between(&self.view.lines, from, to) } else { String::new() };
        if text.is_empty() || to <= from {
            self.view.message = qnc_ui_text::text("transcript.quote_empty").into();
            return true;
        }
        let at = |frame: u64| timecode.map_or_else(|| frame.to_string(), |tc| tc.label(frame));
        let quote = Quote {
            id: 0,
            clip_name: self.names.get(&clip_id).map_or_else(|| clip_id.clone(), |(name, _, _)| name.clone()),
            clip_id,
            from_frame: from,
            to_frame: to,
            start_tc: at(from),
            // The OUT is past the last frame: the end shown is the last frame said.
            end_tc: at(to.saturating_sub(1).max(from)),
            length: timecode.map_or_else(|| (to - from).to_string(), |tc| tc.duration_label(to - from)),
            text,
        };
        self.write(preview, move |target| qnc_transcript_db::add_quote(target, quote))
    }

    /// A write of the statements off the form thread; the list is read again when it is done.
    fn write(&mut self, preview: &SourcePreview, write: impl FnOnce(&ProjectDbTarget) -> Result<(), String> + Send + 'static) -> bool {
        let Some(target) = preview.project_target().cloned() else { return false };
        let (send, done) = mpsc::channel();
        if std::thread::Builder::new().name("qnc-transcript-quote".into()).spawn(move || _ = send.send(write(&target))).is_ok() {
            self.asking = Some(done);
        }
        true
    }

    /// Queues the ticked clips whole, or without ticks the shown clip's IN/OUT.
    fn start(&mut self, preview: &SourcePreview, timeline: &TimelineProjection) -> bool {
        let Some(target) = preview.project_target().cloned() else {
            self.view.message = qnc_ui_text::text("transcribe.choose_clip").into();
            return true;
        };
        // Off in the project: said at once, nothing queued to fail (user 2026-10-26).
        if !preview.still_locator().is_some_and(|locator| locator.settings().transcription_enabled().unwrap_or(false)) {
            self.view.message = qnc_ui_text::text("transcribe.off_in_project").into();
            return true;
        }
        let waiting = |clip_id: String, from_frame: u64, to_frame: u64| Queued { clip_id, from_frame, to_frame, quality: self.quality, state: QueueState::Waiting, error: String::new(), took_ms: 0 };
        let ranges: Vec<Queued> = if self.ticks.is_empty() {
            match (preview.view().clip_id.clone(), timeline.visible_source_marks()) {
                (Some(clip), Some((from, to))) => vec![waiting(clip, from, to)],
                _ => Vec::new(),
            }
        } else {
            self.ticks.iter().filter_map(|clip| self.names.get(clip).filter(|(_, frames, _)| *frames > 0).map(|(_, frames, _)| waiting(clip.clone(), 0, *frames))).collect()
        };
        if ranges.is_empty() {
            self.view.message = qnc_ui_text::text("transcribe.choose_clip").into();
            return true;
        }
        let (send, done) = mpsc::channel();
        let count = ranges.len();
        if std::thread::Builder::new().name("qnc-transcript-ask".into()).spawn(move || _ = send.send(qnc_transcript_db::enqueue(&target, ranges))).is_ok() {
            self.asking = Some(done);
            self.ticks.clear();
            self.view.message = qnc_ui_text::fill("transcribe.queued", &[("count", &count.to_string())]);
        }
        true
    }

    /// Ticks the clips on `clips`, takes what the runner heard, reads the queue again now and
    /// then (starting the runner when something waits) and the shown clip's transcripts when
    /// they changed. Returns whether the view changed.
    pub fn poll(&mut self, preview: &SourcePreview, timeline: &TimelineProjection, clips: &mut [ClipRow]) -> bool {
        let before = self.view.clone();
        self.view.all_done = false;
        self.names = clips.iter().map(|clip| (clip.clip_id.clone(), (clip.name.clone(), clip.duration_frames, clip.duration_seconds))).collect();
        self.ticks.retain(|clip| self.names.contains_key(clip));
        for clip in clips.iter_mut() {
            clip.selected = self.ticks.contains(&clip.clip_id);
        }
        let project = preview.project_target().cloned();
        let clip_id = preview.view().clip_id.clone();
        self.take_heard();
        if let Some(done) = &self.asking {
            if let Ok(result) = done.try_recv() {
                if let Err(error) = result {
                    self.view.message = error;
                }
                self.asking = None;
                self.queue_at = None; // read the queue now
            }
        }
        if let (Some(project), Some(locator)) = (project.as_ref(), preview.still_locator()) {
            self.read_queue(project, preview);
            if pending(&self.queue) {
                self.runner.ensure(&locator, project);
            }
        } else {
            self.runner.stop();
        }
        self.read_kept(project.as_ref(), clip_id.as_deref());
        self.view.clip_id = clip_id.clone();
        self.view.playhead = preview.view().timeline.playhead_frame;
        self.view.fast = self.quality == QueueQuality::Fast;
        self.view.ticked = self.ticks.len();
        self.view.marks = timeline.visible_source_marks();
        self.view.queue = self.queue.iter().map(|q| self.queue_row(q)).collect();
        let kept = self.kept.as_ref().filter(|(_, clip, _)| Some(clip) == clip_id.as_ref()).map(|(_, _, kept)| kept.as_slice()).unwrap_or_default();
        let running = self.running.as_ref().filter(|run| Some(&run.clip_id) == clip_id.as_ref());
        self.view.running = running.map(|run| run.range);
        let key = (self.rev, clip_id.clone());
        if self.built.as_ref() == Some(&key) {
            return self.view != before;
        }
        self.built = Some(key);
        self.view.lines = Arc::new(lines(kept, running.map(|run| (run.range, run.lines.as_slice()))));
        let fps = clip_id.as_ref().and_then(|clip| self.names.get(clip)).and_then(|(_, frames, seconds)| (*seconds > 0.0).then(|| *frames as f64 / seconds));
        self.view.silences = Arc::new(fps.map_or_else(Vec::new, |fps| {
            let mut heard: Vec<(u64, u64)> = kept.iter().map(|t| (t.from_frame, t.to_frame)).collect();
            if let Some(run) = running {
                let done = match run.progress {
                    Progress::Heard { seconds, .. } => run.range.0 + (seconds * fps) as u64,
                    Progress::Reading => run.range.0,
                };
                heard.push((run.range.0, done.min(run.range.1)));
            }
            silences(&heard, &self.view.lines, (SILENCE_SECONDS * fps).round() as u64)
        }));
        self.view != before
    }

    fn queue_row(&self, q: &Queued) -> QueueRow {
        let clip = self.names.get(&q.clip_id);
        let fps = clip.and_then(|(_, frames, seconds)| (*seconds > 0.0 && *frames > 0).then(|| *frames as f64 / seconds));
        let running = self.running.as_ref().filter(|run| run.clip_id == q.clip_id && q.state == QueueState::Running);
        QueueRow {
            clip_id: q.clip_id.clone(),
            name: clip.map_or_else(|| q.clip_id.clone(), |(name, _, _)| name.clone()),
            state: q.state,
            quality: q.quality,
            error: q.error.clone(),
            length_seconds: fps.map_or(0.0, |fps| q.to_frame.saturating_sub(q.from_frame) as f64 / fps),
            progress: running.and_then(|run| match run.progress {
                Progress::Heard { seconds, of } if of > 0.0 => Some((seconds / of).clamp(0.0, 1.0)),
                _ => None,
            }),
            // Whole seconds: the view changes once a second, not at every frame.
            elapsed_seconds: running.map(|run| run.started.elapsed().as_secs() as f64),
            took_seconds: q.took_ms as f64 / 1000.0,
        }
    }

    /// Stops this process's runner (another project, the form closes); what it did waits again.
    pub fn stop(&mut self) {
        self.runner.stop();
        self.running = None;
        self.rev += 1;
    }

    fn take_heard(&mut self) {
        for heard in self.runner.take() {
            self.rev += 1;
            match heard {
                Heard::Started { clip_id, from, to } => {
                    self.running = Some(Running { clip_id, range: (from, to), lines: Vec::new(), progress: Progress::Reading, started: Instant::now() });
                    self.queue_at = None;
                }
                Heard::Progress { clip_id, progress } => {
                    if let Some(run) = self.running.as_mut().filter(|run| run.clip_id == clip_id) {
                        run.progress = progress;
                    }
                }
                Heard::Line { clip_id, line } => {
                    if let Some(run) = self.running.as_mut().filter(|run| run.clip_id == clip_id) {
                        run.lines.push(line);
                    }
                }
                Heard::Finished { clip_id, result } => {
                    if let Err(error) = result {
                        self.view.message = error;
                    }
                    self.running = None;
                    self.queue_at = None;
                    // What was kept is read again from the database.
                    if self.kept.as_ref().is_some_and(|(_, clip, _)| *clip == clip_id) {
                        self.kept = None;
                    }
                }
            }
        }
    }

    fn read_queue(&mut self, project: &ProjectDbTarget, preview: &SourcePreview) {
        if let Some(done) = self.queue_read.take() {
            match done.try_recv() {
                Ok(read) => {
                    match read {
                        Ok((queue, quotes, transcribed)) => {
                            // All that was asked is transcribed (user 2026-10-28): its list goes
                            // and the Transcript tab opens; failed entries stay, with why.
                            let finished = pending(&self.queue) && !pending(&queue) && queue.iter().any(|q| q.state == QueueState::Done);
                            self.queue = queue;
                            self.view.quotes = quotes;
                            if self.view.transcribed.len() != transcribed.len() || !transcribed.iter().all(|c| self.view.transcribed.contains(c)) {
                                self.view.transcribed = Arc::new(transcribed.into_iter().collect());
                            }
                            if finished {
                                self.view.all_done = true;
                                self.write(preview, qnc_transcript_db::clear_done);
                            }
                        }
                        Err(error) => self.view.message = error,
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {}
                Err(mpsc::TryRecvError::Empty) => return self.queue_read = Some(done),
            }
        }
        if self.queue_at.is_some_and(|at| at.elapsed() < QUEUE_EVERY) {
            return;
        }
        let (send, done) = mpsc::channel();
        let target = project.clone();
        if std::thread::Builder::new().name("qnc-transcript-queue-read".into()).spawn(move || _ = send.send(qnc_transcript_db::read_queue(&target).and_then(|queue| Ok((queue, qnc_transcript_db::read_quotes(&target)?, qnc_transcript_db::read_transcribed(&target)?))))).is_ok() {
            self.queue_read = Some(done);
            self.queue_at = Some(Instant::now());
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
                    self.rev += 1;
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

/// The stretches of `heard` (transcribed clip frames) no line covers, of `least` frames or more.
fn silences(heard: &[(u64, u64)], lines: &[Line], least: u64) -> Vec<(u64, u64)> {
    let merged = |mut spans: Vec<(u64, u64)>| {
        spans.retain(|(from, to)| to > from);
        spans.sort_unstable();
        let mut out: Vec<(u64, u64)> = Vec::new();
        for (from, to) in spans {
            match out.last_mut() {
                Some(last) if from <= last.1 => last.1 = last.1.max(to),
                _ => out.push((from, to)),
            }
        }
        out
    };
    let said = merged(lines.iter().map(|line| (line.from, line.to)).collect());
    let mut gaps = Vec::new();
    for (from, to) in merged(heard.to_vec()) {
        let mut at = from;
        for &(said_from, said_to) in said.iter().filter(|(f, t)| *t > from && *f < to) {
            if said_from > at {
                gaps.push((at, said_from));
            }
            at = at.max(said_to);
        }
        if to > at {
            gaps.push((at, to));
        }
    }
    gaps.retain(|(from, to)| to - from >= least.max(1));
    gaps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcribed_sound_without_words_is_marked_from_two_seconds_on() {
        let lines = [line(100, 200, "a"), line(210, 300, "b"), line(500, 600, "c")];
        // 50 frames a second: a gap of 100 frames or more is marked.
        assert_eq!(silences(&[(0, 800)], &lines, 100), [(0, 100), (300, 500), (600, 800)]);
        assert_eq!(silences(&[(0, 250)], &lines, 100), [(0, 100)], "only the transcribed part");
        assert!(silences(&[], &lines, 100).is_empty());
        let row = QueueRow { clip_id: "c".into(), name: "c".into(), state: QueueState::Done, quality: QueueQuality::Fast, error: String::new(), length_seconds: 60.0, progress: None, elapsed_seconds: None, took_seconds: 8.0 };
        assert_eq!(row.speed(), Some(7.5));
    }

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

    #[test]
    fn the_transcript_tab_shows_only_the_transcribed_clips() {
        let clips: Vec<ClipRow> = ["a", "b", "c"].iter().map(|id| ClipRow { clip_id: (*id).into(), ..ClipRow::default() }).collect();
        let view = TranscriptView { transcribed: Arc::new(["b".to_string(), "c".to_string()].into_iter().collect()), ..TranscriptView::default() };
        let ids = |shown: Vec<&ClipRow>| shown.iter().map(|c| c.clip_id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(view.pool_clips(true, &clips)), ["b", "c"]);
        assert_eq!(ids(view.pool_clips(false, &clips)), ["a", "b", "c"], "the other tabs show all");
    }

    #[test]
    fn the_whole_queue_is_done_only_when_nothing_waits_or_runs() {
        let q = |state| Queued { clip_id: "a".into(), from_frame: 0, to_frame: 10, quality: QueueQuality::Fast, state, error: String::new(), took_ms: 0 };
        assert!(pending(&[q(QueueState::Done), q(QueueState::Running)]));
        assert!(pending(&[q(QueueState::Waiting)]));
        assert!(!pending(&[q(QueueState::Done), q(QueueState::Failed)]));
    }

    #[test]
    fn fast_or_quality_is_one_switch() {
        let view = TranscriptView { fast: true, ..TranscriptView::default() };
        assert!(view.selected(FAST) && !view.selected(BEST));
        let view = TranscriptView { fast: false, ..TranscriptView::default() };
        assert!(view.selected(BEST) && !view.selected(FAST));
    }
}

#[cfg(test)]
mod quote_tests {
    use super::*;

    #[test]
    fn a_statement_takes_the_words_said_between_in_and_out_and_copies_start_text_end() {
        let words = |list: &[(u64, u64, &str)]| list.iter().map(|&(from, to, text)| Word { from, to, text: text.into() }).collect();
        let lines = [Line { from: 864, to: 900, text: "Pa dosad njih igrača".into(), words: words(&[(864, 871), (873, 876), (881, 891), (893, 900)].iter().zip(["Pa", "dosad", "njih", "igrača"]).map(|(&(a, b), t)| (a, b, t)).collect::<Vec<_>>()) }];
        assert_eq!(said_between(&lines, 873, 892), "dosad njih");
        assert_eq!(said_between(&lines, 0, 10), "");
        let view = TranscriptView {
            quotes: vec![Quote { id: 1, clip_id: "c".into(), clip_name: "Mironik 2680.MXF".into(), from_frame: 873, to_frame: 892, start_tc: "00:21:56:17".into(), end_tc: "00:21:56:35".into(), length: "00:00:00:19".into(), text: "dosad njih".into() }],
            ..TranscriptView::default()
        };
        assert_eq!(view.quotes_text(), "00:21:56:17\ndosad njih\n00:21:56:35");
    }
}
