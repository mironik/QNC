//! The script of a project's story (user 2026-10-28, Media Assist Audio: writing the offs and
//! putting the story together as text). OFF text is written in the editor at the bottom of
//! the panel and goes into the script with Add OFF; a click on an OFF block takes its text back
//! into the editor to be corrected and put back with Replace. A statement comes from the shown
//! clip's IN/OUT (with the words said there) or from the statements of the transcript. Each
//! block has its length (an OFF block by its words and the project's reading speed), the whole
//! script is held against the target length of the story sheet.
//!
//! The script is the project database's (`qnc-script-db`); the editor's text is only a draft
//! on the screen until Add OFF or Replace.

use std::{
    collections::HashMap,
    sync::mpsc,
    time::{Duration, Instant},
};

use qnc_clip_row::ClipRow;
use qnc_db_broker::ProjectDbTarget;
pub use qnc_script_db::{Block, BlockKind};
use qnc_source_preview::SourcePreview;
use qnc_source_timecode::SourceTimecode;
use qnc_timeline::TimelineProjection;
use qnc_transcript::TranscriptView;

pub const MODULE_ID: &str = "qnc.module.script";
/// The panel the script is shown on.
pub const PANEL: &str = "script";
/// The editor's text changed (target: the whole text).
pub const EDITOR_TEXT: &str = "script_editor_text";
/// The editor's text goes into the script as an OFF block after the chosen one.
pub const ADD_OFF: &str = "script_add_off";
/// The editor's text goes into the script as the anchor's lead-in or tag (user 2026-10-28).
pub const ADD_INTRO: &str = "script_add_intro";
pub const ADD_OUTRO: &str = "script_add_outro";
/// The editor's text replaces the text block it was taken from.
pub const REPLACE_OFF: &str = "script_replace_off";
/// The editor is emptied (and no longer holds a block's text).
pub const CLEAR_EDITOR: &str = "script_editor_clear";
/// The shown clip's IN/OUT goes into the script as a statement.
pub const ADD_SOT: &str = "script_add_sot";
/// A statement of the transcript goes into the script (target: its id).
pub const ADD_QUOTE: &str = "script_add_quote";
/// A block is chosen (target: its id); an OFF block's text goes into the editor.
pub const SELECT: &str = "script_select";
/// A block goes one place up or down, or leaves (target: its id).
pub const MOVE_UP: &str = "script_move_up";
pub const MOVE_DOWN: &str = "script_move_down";
pub const REMOVE: &str = "script_remove";
/// A statement opens its clip with its IN/OUT (target: its id).
pub const OPEN: &str = "script_open";
/// An OFF block is recorded from the microphone, or its recording stops (target: its id;
/// without one the chosen OFF block). The catalog names it `record_toggle` (v4).
pub const RECORD: &str = "record_toggle";

/// How often the script is read again (another process may change it).
const READ_EVERY: Duration = Duration::from_secs(2);

/// One block as the panel shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockRow {
    pub block: Block,
    /// Its length in seconds: a statement's own, an OFF block's from its words.
    pub seconds: f64,
}

/// What the script panel shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScriptView {
    pub blocks: Vec<BlockRow>,
    /// The chosen block: new blocks go after it.
    pub selected: Option<i64>,
    /// The editor's text.
    pub editor: String,
    /// The OFF block the editor's text was taken from (Replace puts it back there).
    pub editing: Option<i64>,
    /// Seconds the editor's text takes to read.
    pub editor_seconds: f64,
    pub off_seconds: f64,
    pub sot_seconds: f64,
    /// The anchor's lead-in and tag (read in the studio, not in the length of the story).
    pub anchor_seconds: f64,
    /// The target length of the story (the story sheet's first length field).
    pub target_seconds: Option<f64>,
    /// Whether the shown clip has an IN/OUT (Add SOT can take it).
    pub marks: bool,
    /// The OFF block being recorded now, the seconds recorded and the level of the microphone.
    pub recording: Option<(i64, f64, f32)>,
    /// A recording is being started or finished.
    pub record_busy: bool,
    pub message: String,
}

impl ScriptView {
    pub fn total_seconds(&self) -> f64 {
        self.off_seconds + self.sot_seconds
    }

    /// The script as text to paste: each block in reading order, a statement with its clip
    /// and timecodes.
    pub fn text(&self) -> String {
        self.blocks
            .iter()
            .map(|row| match row.block.kind {
                BlockKind::Off => format!("OFF ({})\n{}", seconds_label(row.seconds), row.block.text),
                BlockKind::Intro => format!("{} ({})\n{}", qnc_ui_text::text("script.intro_heading"), seconds_label(row.seconds), row.block.text),
                BlockKind::Outro => format!("{} ({})\n{}", qnc_ui_text::text("script.outro_heading"), seconds_label(row.seconds), row.block.text),
                BlockKind::Sot => format!("SOT {} {} - {} ({})\n{}", row.block.clip_name, row.block.start_tc, row.block.end_tc, row.block.length, row.block.text),
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// Seconds as `m:ss`.
pub fn seconds_label(seconds: f64) -> String {
    let whole = seconds.max(0.0).round() as u64;
    format!("{}:{:02}", whole / 60, whole % 60)
}

/// Seconds a block lasts: a statement its own, a recorded OFF its recording, other text the
/// time its words take to read.
pub fn block_seconds(block: &Block, words_per_second: f64) -> f64 {
    match block.kind {
        BlockKind::Sot => block.seconds,
        BlockKind::Off if !block.voice_uri.is_empty() => block.voice_seconds,
        _ => reading_seconds(&block.text, words_per_second),
    }
}

/// Seconds `text` takes to read at `words_per_second`.
pub fn reading_seconds(text: &str, words_per_second: f64) -> f64 {
    if words_per_second <= 0.0 {
        return 0.0;
    }
    text.split_whitespace().count() as f64 / words_per_second
}

type Read = Result<(Vec<Block>, Option<f64>), String>;

/// Where a take goes and what its clip is: the voice-over folder of the project, its name
/// without extension and the project picture.
struct Take {
    folder: std::path::PathBuf,
    base: String,
    stem: String,
    project_dir: std::path::PathBuf,
    width: u32,
    height: u32,
    fps: f64,
}

pub struct Script {
    view: ScriptView,
    blocks: Vec<Block>,
    project: Option<String>,
    reading: Option<mpsc::Receiver<Read>>,
    read_at: Option<Instant>,
    /// A write on its way to the database; its answer is the id of an added block.
    writing: Option<mpsc::Receiver<Result<Option<i64>, String>>>,
    source_request: Option<(String, u64, u64)>,
    /// Name, frames and seconds of each clip of the pool.
    names: HashMap<String, (String, u64, f64)>,
    words_per_second: f64,
    /// The recording that runs, of which block.
    recorder: Option<(i64, qnc_voice_record::Recorder)>,
    /// A recording on its way to start, or a stopped one on its way into the script.
    starting: Option<(i64, mpsc::Receiver<Result<(qnc_voice_record::Recorder, Take), String>>)>,
    take: Option<Take>,
}

impl Default for Script {
    fn default() -> Self {
        Self { view: ScriptView::default(), blocks: Vec::new(), project: None, reading: None, read_at: None, writing: None, source_request: None, names: HashMap::new(), words_per_second: 0.0, recorder: None, starting: None, take: None }
    }
}

impl std::fmt::Debug for Script {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Script").field("view", &self.view).finish_non_exhaustive()
    }
}

impl Script {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn view(&self) -> &ScriptView {
        &self.view
    }

    /// Whether a read or a write of this process runs (the form repaints soon).
    pub fn busy(&self) -> bool {
        self.reading.is_some() || self.writing.is_some() || self.recorder.is_some() || self.starting.is_some()
    }

    /// A clip and IN/OUT a chosen statement opens on the source timeline, once.
    pub fn take_source_request(&mut self) -> Option<(String, u64, u64)> {
        self.source_request.take()
    }

    /// The script's own actions; `None` for any other.
    pub fn act(&mut self, action_id: &str, target: Option<&str>, (preview, timeline, timecode, transcript): (&SourcePreview, &TimelineProjection, Option<&SourceTimecode>, &TranscriptView)) -> Option<bool> {
        let id = || target.and_then(|id| id.parse::<i64>().ok());
        Some(match action_id {
            EDITOR_TEXT => {
                self.view.editor = target.unwrap_or_default().to_string();
                self.refresh_editor();
                true
            }
            CLEAR_EDITOR => {
                self.view.editor.clear();
                self.view.editing = None;
                self.refresh_editor();
                true
            }
            ADD_OFF | ADD_INTRO | ADD_OUTRO => {
                let text = self.view.editor.trim().to_string();
                if text.is_empty() {
                    self.view.message = qnc_ui_text::text("script.write_first").into();
                    return Some(true);
                }
                let kind = match action_id {
                    ADD_INTRO => BlockKind::Intro,
                    ADD_OUTRO => BlockKind::Outro,
                    _ => BlockKind::Off,
                };
                // A block goes after the chosen one; without one the lead-in opens the script and
                // the rest goes at its end.
                let place = match (kind, self.view.selected) {
                    (_, Some(chosen)) => qnc_script_db::Place::After(chosen),
                    (BlockKind::Intro, None) => qnc_script_db::Place::First,
                    _ => qnc_script_db::Place::Last,
                };
                self.write(preview, move |target| qnc_script_db::add_at(target, place, Block::written(kind, text)).map(Some))
            }
            REPLACE_OFF => {
                let (Some(block), text) = (self.view.editing, self.view.editor.trim().to_string()) else {
                    self.view.message = qnc_ui_text::text("script.choose_off").into();
                    return Some(true);
                };
                self.write(preview, move |target| qnc_script_db::set_text(target, block, text).map(|_| None))
            }
            ADD_SOT => self.add_sot(preview, timeline, timecode, transcript),
            ADD_QUOTE => {
                let Some(quote) = id().and_then(|id| transcript.quotes.iter().find(|q| q.id == id)).cloned() else { return Some(false) };
                let seconds = self.seconds_of(&quote.clip_id, quote.to_frame.saturating_sub(quote.from_frame));
                let block = Block {
                    kind: BlockKind::Sot,
                    text: quote.text,
                    clip_id: quote.clip_id,
                    clip_name: quote.clip_name,
                    from_frame: quote.from_frame,
                    to_frame: quote.to_frame,
                    start_tc: quote.start_tc,
                    end_tc: quote.end_tc,
                    length: quote.length,
                    seconds,
                    ..Block::off(String::new())
                };
                let after = self.view.selected;
                self.write(preview, move |target| qnc_script_db::add(target, after, block).map(Some))
            }
            SELECT => {
                let Some(id) = id() else { return Some(false) };
                self.view.selected = Some(id);
                if let Some(block) = self.blocks.iter().find(|b| b.id == id) {
                    if block.is_text() {
                        self.view.editor = block.text.clone();
                        self.view.editing = Some(id);
                        self.refresh_editor();
                    }
                }
                true
            }
            MOVE_UP | MOVE_DOWN => {
                let Some(id) = id() else { return Some(false) };
                let up = action_id == MOVE_UP;
                self.write(preview, move |target| qnc_script_db::move_block(target, id, up).map(|_| None))
            }
            REMOVE => {
                let Some(id) = id() else { return Some(false) };
                if self.view.editing == Some(id) {
                    self.view.editing = None;
                }
                if self.view.selected == Some(id) {
                    self.view.selected = None;
                }
                self.write(preview, move |target| qnc_script_db::remove(target, id).map(|_| None))
            }
            // The key of a form that has no script (nothing chosen, nothing recording) is not the
            // script's.
            RECORD if target.is_none() && self.view.selected.is_none() && self.recorder.is_none() => return None,
            RECORD => {
                let block = id().or(self.view.selected);
                self.record(preview, block)
            }
            OPEN => match id().and_then(|id| self.blocks.iter().find(|b| b.id == id && b.kind == BlockKind::Sot)) {
                Some(block) => {
                    self.source_request = Some((block.clip_id.clone(), block.from_frame, block.to_frame));
                    true
                }
                None => false,
            },
            _ => return None,
        })
    }

    /// The shown clip's IN/OUT as a statement, with the words said there.
    fn add_sot(&mut self, preview: &SourcePreview, timeline: &TimelineProjection, timecode: Option<&SourceTimecode>, transcript: &TranscriptView) -> bool {
        let (Some(clip_id), Some((from, to))) = (preview.view().clip_id.clone(), timeline.visible_source_marks()) else {
            self.view.message = qnc_ui_text::text("script.sot_marks").into();
            return true;
        };
        if to <= from {
            self.view.message = qnc_ui_text::text("script.sot_marks").into();
            return true;
        }
        let at = |frame: u64| timecode.map_or_else(|| frame.to_string(), |tc| tc.label(frame));
        let text = if transcript.clip_id.as_deref() == Some(clip_id.as_str()) { transcript.said(from, to) } else { String::new() };
        let block = Block {
            id: 0,
            kind: BlockKind::Sot,
            text,
            clip_name: self.names.get(&clip_id).map_or_else(|| clip_id.clone(), |(name, _, _)| name.clone()),
            seconds: self.seconds_of(&clip_id, to - from),
            clip_id,
            from_frame: from,
            to_frame: to,
            start_tc: at(from),
            // The OUT is past the last frame: the end shown is the last frame of the statement.
            end_tc: at(to.saturating_sub(1).max(from)),
            length: timecode.map_or_else(|| (to - from).to_string(), |tc| tc.duration_label(to - from)),
            ..Block::off(String::new())
        };
        let after = self.view.selected;
        self.write(preview, move |target| qnc_script_db::add(target, after, block).map(Some))
    }

    /// Starts recording OFF block `block`, or stops the recording that runs (whichever block
    /// the press names). The microphone is opened off the form thread.
    fn record(&mut self, preview: &SourcePreview, block: Option<i64>) -> bool {
        if let Some((id, recorder)) = self.recorder.take() {
            return self.finish(preview, id, recorder);
        }
        if self.starting.is_some() {
            return true;
        }
        let Some(id) = block.filter(|id| self.blocks.iter().any(|b| b.id == *id && b.kind == BlockKind::Off)) else {
            self.view.message = qnc_ui_text::text("script.choose_off_to_record").into();
            return true;
        };
        let Some(locator) = preview.still_locator() else {
            self.view.message = qnc_ui_text::text("script.no_project").into();
            return true;
        };
        let Some(setting) = qnc_dev_diagnostics::resolve_qnc_root().map(|root| qnc_dev_diagnostics::config_path(&root).with_file_name(qnc_voice_record::INPUT_SETTING)) else {
            self.view.message = qnc_ui_text::text("script.no_project").into();
            return true;
        };
        let (send, started) = mpsc::channel();
        let spawned = std::thread::Builder::new().name("qnc-script-record".into()).spawn(move || {
            let start = || -> Result<(qnc_voice_record::Recorder, Take), String> {
                let settings = locator.settings();
                let project_dir = locator.reader().local_workspace_dir(settings).map_err(|e| e.to_string())?.ok_or_else(|| qnc_ui_text::text("script.voice_local_only").to_string())?;
                let (folder, base) = settings.voice_location(&project_dir).map_err(|e| e.to_string())?;
                std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
                let number = |key: &str| settings.video.get(key).and_then(serde_json::Value::as_f64).unwrap_or(0.0);
                let rate = settings.audio.get("sample_rate").and_then(serde_json::Value::as_u64).unwrap_or(0) as u32;
                let take = Take { stem: format!("off_{id}_{}", qnc_voice_record::file_stamp()), folder, base, project_dir, width: number("width") as u32, height: number("height") as u32, fps: number("fps") };
                let recorder = qnc_voice_record::Recorder::start(take.folder.join(format!("{}.wav", take.stem)), &take.project_dir, rate, take.fps, &setting)?;
                Ok((recorder, take))
            };
            let _ = send.send(start());
        });
        if spawned.is_ok() {
            self.starting = Some((id, started));
            self.view.message = qnc_ui_text::text("script.record_starting").into();
        }
        true
    }

    /// Stops the recording of block `id` and puts it into the script.
    /// The recording becomes a clip of the project at once (user 2026-10-28): the title card of
    /// the OFF's text as its picture, the time it was recorded as its timecode.
    fn finish(&mut self, preview: &SourcePreview, id: i64, recorder: qnc_voice_record::Recorder) -> bool {
        let Some(take) = self.take.take() else { return false };
        let text = self.blocks.iter().find(|b| b.id == id).map(|b| b.text.clone()).unwrap_or_default();
        let number = self.blocks.iter().filter(|b| b.kind == BlockKind::Off).position(|b| b.id == id).map_or(0, |at| at + 1);
        let heading = qnc_ui_text::fill("script.card_heading", &[("number", &number.to_string())]);
        self.view.message = qnc_ui_text::text("script.making_clip").into();
        self.write(preview, move |target| {
            let recording = recorder.stop()?;
            let out = take.folder.join(format!("{}.mxf", take.stem));
            let clip = qnc_voice_clip::VoiceClip { sound: &recording.path, text: &text, heading: &heading, width: take.width, height: take.height, fps: take.fps, timecode: &recording.timecode, out: &out, project_dir: &take.project_dir };
            qnc_voice_clip::make(&clip)?;
            let uri = format!("{}/{}.mxf", take.base, take.stem);
            qnc_script_db::set_voice(target, id, uri, recording.seconds, recording.timecode).map(|_| None)
        })
    }

    /// Seconds of `frames` of `clip_id` (0 when its rate is not known).
    fn seconds_of(&self, clip_id: &str, frames: u64) -> f64 {
        self.names.get(clip_id).filter(|(_, all, seconds)| *all > 0 && *seconds > 0.0).map_or(0.0, |(_, all, seconds)| frames as f64 * seconds / *all as f64)
    }

    /// A write off the form thread; the script is read again when it is done.
    fn write(&mut self, preview: &SourcePreview, write: impl FnOnce(&ProjectDbTarget) -> Result<Option<i64>, String> + Send + 'static) -> bool {
        let Some(target) = preview.project_target().cloned() else {
            self.view.message = qnc_ui_text::text("script.no_project").into();
            return true;
        };
        let (send, done) = mpsc::channel();
        if std::thread::Builder::new().name("qnc-script-write".into()).spawn(move || _ = send.send(write(&target))).is_ok() {
            self.writing = Some(done);
        }
        true
    }

    fn refresh_editor(&mut self) {
        self.view.editor_seconds = reading_seconds(&self.view.editor, self.words_per_second);
    }

    /// Takes the answers of writes and reads, reads the script again now and then. Returns
    /// whether the view changed.
    pub fn poll(&mut self, preview: &SourcePreview, timeline: &TimelineProjection, clips: &[ClipRow]) -> bool {
        let before = self.view.clone();
        self.names = clips.iter().map(|clip| (clip.clip_id.clone(), (clip.name.clone(), clip.duration_frames, clip.duration_seconds))).collect();
        self.view.marks = preview.view().clip_id.is_some() && timeline.visible_source_marks().is_some_and(|(from, to)| to > from);
        let project = preview.project_target().cloned();
        let key = project.as_ref().map(|target| target.uri().to_string());
        if key != self.project {
            *self = Self { project: key, ..Self::default() };
        }
        self.words_per_second = preview.still_locator().and_then(|locator| locator.settings().reading_words_per_second().ok()).unwrap_or(0.0);
        if let Some((id, started)) = &self.starting {
            if let Ok(result) = started.try_recv() {
                match result {
                    Ok((recorder, take)) => {
                        self.take = Some(take);
                        self.recorder = Some((*id, recorder));
                        self.view.message = qnc_ui_text::text("script.recording").into();
                    }
                    Err(error) => self.view.message = error,
                }
                self.starting = None;
            }
        }
        self.view.recording = self.recorder.as_ref().map(|(id, recorder)| (*id, recorder.seconds(), recorder.take_peak()));
        self.view.record_busy = self.starting.is_some();
        if let Some(done) = &self.writing {
            if let Ok(result) = done.try_recv() {
                match result {
                    Ok(added) => {
                        self.view.message.clear();
                        if let Some(id) = added {
                            self.view.selected = Some(id);
                            // The editor's text is in the script now.
                            if self.view.editing.is_none() {
                                self.view.editor.clear();
                            }
                        }
                    }
                    Err(error) => self.view.message = error,
                }
                self.writing = None;
                self.read_at = None; // read now
            }
        }
        if let Some(reading) = &self.reading {
            if let Ok(result) = reading.try_recv() {
                match result {
                    Ok((blocks, target)) => {
                        self.blocks = blocks;
                        self.view.target_seconds = target;
                    }
                    Err(error) => self.view.message = error,
                }
                self.reading = None;
            }
        }
        if let Some(project) = project.filter(|_| self.reading.is_none() && self.writing.is_none() && self.read_at.is_none_or(|at| at.elapsed() >= READ_EVERY)) {
            self.read_at = Some(Instant::now());
            let (send, read) = mpsc::channel();
            if std::thread::Builder::new().name("qnc-script-read".into()).spawn(move || _ = send.send(read_script(&project))).is_ok() {
                self.reading = Some(read);
            }
        }
        if self.view.selected.is_some_and(|id| !self.blocks.iter().any(|b| b.id == id)) && self.writing.is_none() && self.reading.is_none() && !self.blocks.is_empty() {
            self.view.selected = None;
        }
        let wps = self.words_per_second;
        self.view.blocks = self
            .blocks
            .iter()
            .map(|block| BlockRow { seconds: block_seconds(block, wps), block: block.clone() })
            .collect();
        let sum = |kinds: &[BlockKind]| self.view.blocks.iter().filter(|row| kinds.contains(&row.block.kind)).map(|row| row.seconds).sum::<f64>();
        let (off, sot, anchor) = (sum(&[BlockKind::Off]), sum(&[BlockKind::Sot]), sum(&[BlockKind::Intro, BlockKind::Outro]));
        (self.view.off_seconds, self.view.sot_seconds, self.view.anchor_seconds) = (off, sot, anchor);
        self.refresh_editor();
        self.view != before
    }
}

/// The script and the target length of the story sheet (its first length field's value).
fn read_script(target: &ProjectDbTarget) -> Read {
    let blocks = qnc_script_db::read(target)?;
    let sheet = qnc_newsroom_sheet_db::read(target).ok();
    let target = sheet.and_then(|sheet| {
        let field = sheet.sheet.get("fields")?.as_array()?.iter().find(|field| field.get("kind").and_then(serde_json::Value::as_str) == Some("duration"))?;
        let code = field.get("id")?.as_str()?;
        sheet.values.get(code)?.as_f64().filter(|seconds| *seconds > 0.0)
    });
    Ok((blocks, target))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_off_block_lasts_as_long_as_its_words_take_to_read() {
        assert_eq!(reading_seconds("jedan dva tri  cetiri pet", 2.5), 2.0);
        assert_eq!(reading_seconds("   ", 2.5), 0.0);
        assert_eq!(seconds_label(83.6), "1:24");
    }

    #[test]
    fn the_script_as_text_has_every_block_in_reading_order() {
        let off = BlockRow { block: Block::off("Uvod u prilog."), seconds: 2.0 };
        let sot = BlockRow {
            block: Block { kind: BlockKind::Sot, clip_name: "Mironik 2002".into(), start_tc: "10:00:02:00".into(), end_tc: "10:00:06:49".into(), length: "5:00".into(), ..Block::off("Rekao je.") },
            seconds: 5.0,
        };
        let view = ScriptView { blocks: vec![off, sot], ..ScriptView::default() };
        assert_eq!(view.text(), "OFF (0:02)\nUvod u prilog.\n\nSOT Mironik 2002 10:00:02:00 - 10:00:06:49 (5:00)\nRekao je.");
    }
}
