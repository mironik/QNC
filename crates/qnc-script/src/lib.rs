//! The script of a project's story (user 2026-10-28, Media Assist Audio: writing the offs and
//! putting the story together as text). Text is written in the editor at the bottom of the panel
//! and goes into the script as a block of one of the text kinds of the project's script profile
//! (OFF, lead-in, tag, a note...); a click on a text block takes its text back into the editor
//! to be corrected and put back with Replace. A clip block (a statement, a stand-up...) comes
//! from the shown clip's IN/OUT (with the words said there) or from the statements of the
//! transcript. Each block has its length (text by its words and the project's reading speed, a
//! recording by its own), the script is held against the target length of the story sheet.
//!
//! What the kinds are, their marks and numbered cues, which are recorded and which length they
//! count in comes from the script profile the project keeps (user 2026-10-28: the form of the
//! script is programmable); a project without one gets the profile chosen on this computer
//! when its script is first read.
//!
//! The script is the project database's (`qnc-script-db`); the editor's text is only a draft
//! on the screen until it is added or replaced.

use std::{
    collections::{BTreeMap, HashMap},
    sync::mpsc,
    time::{Duration, Instant},
};

use qnc_clip_row::ClipRow;
use qnc_db_broker::ProjectDbTarget;
pub use qnc_script_db::{Block, Counts, KindSpec, Role, ScriptProfile};
use qnc_source_preview::SourcePreview;
use qnc_source_timecode::SourceTimecode;
use qnc_timeline::TimelineProjection;
use qnc_transcript::TranscriptView;

pub const MODULE_ID: &str = "qnc.module.script";
/// The panel the script is shown on.
pub const PANEL: &str = "script";
/// The editor's text changed (target: the whole text).
pub const EDITOR_TEXT: &str = "script_editor_text";
/// The editor's text goes into the script as a block of a text kind after the chosen block
/// (target: the kind's code).
pub const ADD_TEXT: &str = "script_add_text";
/// The editor's text replaces the text block it was taken from.
pub const REPLACE_TEXT: &str = "script_replace_text";
/// The editor is emptied (and no longer holds a block's text).
pub const CLEAR_EDITOR: &str = "script_editor_clear";
/// The shown clip's IN/OUT goes into the script as a block of a clip kind (target: its code).
pub const ADD_CLIP: &str = "script_add_clip";
/// A statement of the transcript goes into the script (target: its id).
pub const ADD_QUOTE: &str = "script_add_quote";
/// A block is chosen (target: its id); a text block's text goes into the editor.
pub const SELECT: &str = "script_select";
/// A block goes one place up or down, or leaves (target: its id).
pub const MOVE_UP: &str = "script_move_up";
pub const MOVE_DOWN: &str = "script_move_down";
pub const REMOVE: &str = "script_remove";
/// A clip block opens its clip with its IN/OUT (target: its id).
pub const OPEN: &str = "script_open";
/// A block of a recorded kind is recorded from the microphone, or its recording stops (target:
/// its id; without one the chosen block). The catalog names it `record_toggle` (v4).
pub const RECORD: &str = "record_toggle";
/// A field of a block is being typed (target: `{"block":id,"field":code,"value":text}`); the text is a
/// draft on the screen until it is committed.
pub const FIELD_TEXT: &str = "script_field_text";
/// The drafts of a block's fields go into the script (target: the block id).
pub const FIELD_COMMIT: &str = "script_field_commit";
/// A field is set and goes into the script at once (a choice; target as `FIELD_TEXT`).
pub const FIELD_SET: &str = "script_field_set";

/// How often the script is read again (another process may change it).
const READ_EVERY: Duration = Duration::from_secs(2);

/// One block as the panel shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockRow {
    pub block: Block,
    /// Its length in seconds: a clip's own, a recording's own, text's from its words.
    pub seconds: f64,
    /// Its mark: the numbered cue of its kind (`OFF 2`, `Take SOT 1`), else the kind's short mark.
    pub mark: String,
    /// What its kind is (none when the project's profile does not have it).
    pub role: Option<Role>,
    /// Whether its kind is recorded from the microphone.
    pub recorded: bool,
}

impl BlockRow {
    pub fn is_clip(&self) -> bool {
        self.role == Some(Role::Clip)
    }
}

/// What the script panel shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScriptView {
    pub blocks: Vec<BlockRow>,
    /// The script profile of the project (none until it is read).
    pub profile: Option<ScriptProfile>,
    /// The chosen block: new blocks go after it.
    pub selected: Option<i64>,
    /// The editor's text.
    pub editor: String,
    /// The text block the editor's text was taken from (Replace puts it back there).
    pub editing: Option<i64>,
    /// Seconds the editor's text takes to read.
    pub editor_seconds: f64,
    /// Text and clips in the length of the story.
    pub read_seconds: f64,
    pub clip_seconds: f64,
    /// Text the anchor reads in the studio (not in the length of the story).
    pub anchor_seconds: f64,
    /// The target length of the story (the story sheet's first length field).
    pub target_seconds: Option<f64>,
    /// Whether the shown clip has an IN/OUT (a clip block can take it).
    pub marks: bool,
    /// The block being recorded now, the seconds recorded and the level of the microphone.
    pub recording: Option<(i64, f64, f32)>,
    /// A recording is being started or finished.
    pub record_busy: bool,
    /// Field values typed and not yet in the script, by block and field code.
    pub field_drafts: BTreeMap<(i64, String), String>,
    pub message: String,
}

impl ScriptView {
    pub fn total_seconds(&self) -> f64 {
        self.read_seconds + self.clip_seconds
    }

    /// The kinds of the profile that are text written in the editor, in its order.
    pub fn text_kinds(&self) -> Vec<&KindSpec> {
        self.profile.iter().flat_map(|profile| &profile.kinds).filter(|kind| kind.role != Role::Clip).collect()
    }

    /// The kinds of the profile that are ranges of a clip, in its order.
    pub fn clip_kinds(&self) -> Vec<&KindSpec> {
        self.profile.iter().flat_map(|profile| &profile.kinds).filter(|kind| kind.role == Role::Clip).collect()
    }

    /// The kind of block `id`, if the profile has it.
    pub fn kind_of(&self, id: i64) -> Option<&KindSpec> {
        let block = self.blocks.iter().find(|row| row.block.id == id)?;
        self.profile.as_ref()?.kind(&block.block.kind)
    }

    /// The value of field `code` of block `id` as shown: the draft while it is typed, else the
    /// value in the script.
    pub fn field_value(&self, id: i64, code: &str) -> String {
        if let Some(draft) = self.field_drafts.get(&(id, code.to_string())) {
            return draft.clone();
        }
        self.blocks.iter().find(|row| row.block.id == id).and_then(|row| row.block.fields.get(code).cloned()).unwrap_or_default()
    }

    /// The fields of a block as lines of the pasted script (`Caption: name: Ivan Horvat`).
    fn fields_text(&self, row: &BlockRow) -> String {
        let Some(kind) = self.profile.as_ref().and_then(|profile| profile.kind(&row.block.kind)) else { return String::new() };
        kind.fields
            .iter()
            .filter_map(|field| row.block.fields.get(&field.path).map(|value| format!("
{}: {}", field.label, value)))
            .collect()
    }

    /// The script as text to paste: each block in reading order with its mark, a clip block
    /// with its clip and timecodes.
    pub fn text(&self) -> String {
        self.blocks
            .iter()
            .map(|row| {
                if row.is_clip() {
                    format!("{} {} {} - {} ({})\n{}{}", row.mark, row.block.clip_name, row.block.start_tc, row.block.end_tc, row.block.length, row.block.text, self.fields_text(row))
                } else {
                    format!("{} ({})\n{}{}", row.mark, seconds_label(row.seconds), row.block.text, self.fields_text(row))
                }
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

/// Seconds a block of kind `kind` lasts: a clip its own, a recording its own, a note none,
/// other text the time its words take to read.
pub fn block_seconds(block: &Block, kind: Option<&KindSpec>, words_per_second: f64) -> f64 {
    match kind.map(|kind| kind.role) {
        Some(Role::Clip) => block.seconds,
        Some(Role::Note) => 0.0,
        _ if !block.voice_uri.is_empty() => block.voice_seconds,
        Some(_) => reading_seconds(&block.text, words_per_second),
        // A kind the profile does not have: a clip by its range, text by its words.
        None if !block.clip_id.is_empty() => block.seconds,
        None => reading_seconds(&block.text, words_per_second),
    }
}

/// Seconds `text` takes to read at `words_per_second`.
pub fn reading_seconds(text: &str, words_per_second: f64) -> f64 {
    if words_per_second <= 0.0 {
        return 0.0;
    }
    text.split_whitespace().count() as f64 / words_per_second
}

/// The rows of `blocks`: their lengths, marks (numbered among the blocks of their kind) and
/// what their kinds are.
pub fn rows(blocks: &[Block], profile: Option<&ScriptProfile>, words_per_second: f64) -> Vec<BlockRow> {
    let mut numbers: HashMap<&str, usize> = HashMap::new();
    blocks
        .iter()
        .map(|block| {
            let kind = profile.and_then(|profile| profile.kind(&block.kind));
            let number = numbers.entry(block.kind.as_str()).or_insert(0);
            *number += 1;
            let cue = profile.map(|profile| profile.cue_label_with(&block.kind, *number, &block.fields)).unwrap_or_default();
            let mark = if !cue.is_empty() {
                cue
            } else {
                kind.map_or_else(|| block.kind.to_uppercase(), |kind| kind.short.clone())
            };
            BlockRow { seconds: block_seconds(block, kind, words_per_second), mark, role: kind.map(|kind| kind.role), recorded: kind.is_some_and(|kind| kind.recorded), block: block.clone() }
        })
        .collect()
}

/// The target of `FIELD_TEXT` and `FIELD_SET`.
#[derive(serde::Deserialize, serde::Serialize)]
pub struct FieldEdit {
    pub block: i64,
    pub field: String,
    pub value: String,
}

impl FieldEdit {
    pub fn target(block: i64, field: &str, value: &str) -> String {
        serde_json::to_string(&Self { block, field: field.into(), value: value.into() }).unwrap_or_default()
    }
}

type Read =Result<(Vec<Block>, Option<ScriptProfile>, Option<f64>), String>;

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

    /// A clip and IN/OUT a chosen clip block opens on the source timeline, once.
    pub fn take_source_request(&mut self) -> Option<(String, u64, u64)> {
        self.source_request.take()
    }

    /// The kind `code` of the project's profile.
    fn kind(&self, code: &str) -> Option<&KindSpec> {
        self.view.profile.as_ref()?.kind(code)
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
            ADD_TEXT => {
                let text = self.view.editor.trim().to_string();
                if text.is_empty() {
                    self.view.message = qnc_ui_text::text("script.write_first").into();
                    return Some(true);
                }
                let Some(kind) = target.and_then(|code| self.kind(code)).filter(|kind| kind.role != Role::Clip).cloned() else { return Some(false) };
                let place = match self.view.selected {
                    Some(chosen) => qnc_script_db::Place::After(chosen),
                    // Without a chosen block the anchor's text before the story (a kind the
                    // profile lists before any other) opens the script; the rest goes at its end.
                    None if self.opens_the_script(&kind) => qnc_script_db::Place::First,
                    None => qnc_script_db::Place::Last,
                };
                self.write(preview, move |target| qnc_script_db::add_at(target, place, Block::written(kind.id, text)).map(Some))
            }
            REPLACE_TEXT => {
                let (Some(block), text) = (self.view.editing, self.view.editor.trim().to_string()) else {
                    self.view.message = qnc_ui_text::text("script.choose_text").into();
                    return Some(true);
                };
                self.write(preview, move |target| qnc_script_db::set_text(target, block, text).map(|_| None))
            }
            ADD_CLIP => {
                let Some(kind) = target.and_then(|code| self.kind(code)).filter(|kind| kind.role == Role::Clip).map(|kind| kind.id.clone()) else { return Some(false) };
                self.add_clip(preview, timeline, timecode, transcript, kind)
            }
            ADD_QUOTE => {
                let Some(quote) = id().and_then(|id| transcript.quotes.iter().find(|q| q.id == id)).cloned() else { return Some(false) };
                // A statement of the transcript is a block of the profile's first clip kind.
                let Some(kind) = self.view.clip_kinds().first().map(|kind| kind.id.clone()) else {
                    self.view.message = qnc_ui_text::text("script.no_clip_kind").into();
                    return Some(true);
                };
                let seconds = self.seconds_of(&quote.clip_id, quote.to_frame.saturating_sub(quote.from_frame));
                let block = Block {
                    clip_id: quote.clip_id,
                    clip_name: quote.clip_name,
                    from_frame: quote.from_frame,
                    to_frame: quote.to_frame,
                    start_tc: quote.start_tc,
                    end_tc: quote.end_tc,
                    length: quote.length,
                    seconds,
                    ..Block::written(kind, quote.text)
                };
                let after = self.view.selected;
                self.write(preview, move |target| qnc_script_db::add(target, after, block).map(Some))
            }
            SELECT => {
                let Some(id) = id() else { return Some(false) };
                self.view.selected = Some(id);
                if let Some(row) = self.view.blocks.iter().find(|row| row.block.id == id).filter(|row| !row.is_clip()) {
                    self.view.editor = row.block.text.clone();
                    self.view.editing = Some(id);
                    self.refresh_editor();
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
            FIELD_TEXT | FIELD_SET => {
                let Some(edit) = target.and_then(|target| serde_json::from_str::<FieldEdit>(target).ok()) else { return Some(false) };
                let block = edit.block;
                self.view.field_drafts.insert((edit.block, edit.field), edit.value);
                if action_id == FIELD_SET {
                    return Some(self.commit_fields(preview, block));
                }
                true
            }
            FIELD_COMMIT => {
                let Some(block) = id() else { return Some(false) };
                self.commit_fields(preview, block)
            }
            OPEN => match id().and_then(|id| self.view.blocks.iter().find(|row| row.block.id == id && row.is_clip())) {
                Some(row) => {
                    self.source_request = Some((row.block.clip_id.clone(), row.block.from_frame, row.block.to_frame));
                    true
                }
                None => false,
            },
            _ => return None,
        })
    }

    /// Whether `kind` is the anchor's text before the story: an anchor kind the profile lists
    /// before every kind that is not the anchor's.
    fn opens_the_script(&self, kind: &KindSpec) -> bool {
        let Some(profile) = &self.view.profile else { return false };
        let at = |found: &dyn Fn(&KindSpec) -> bool| profile.kinds.iter().position(|k| found(k));
        kind.role == Role::Anchor && at(&|k| k.id == kind.id) < at(&|k| k.role != Role::Anchor)
    }

    /// The shown clip's IN/OUT as a block of clip kind `kind`, with the words said there.
    fn add_clip(&mut self, preview: &SourcePreview, timeline: &TimelineProjection, timecode: Option<&SourceTimecode>, transcript: &TranscriptView, kind: String) -> bool {
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
            clip_name: self.names.get(&clip_id).map_or_else(|| clip_id.clone(), |(name, _, _)| name.clone()),
            seconds: self.seconds_of(&clip_id, to - from),
            clip_id,
            from_frame: from,
            to_frame: to,
            start_tc: at(from),
            // The OUT is past the last frame: the end shown is the last frame of the block.
            end_tc: at(to.saturating_sub(1).max(from)),
            length: timecode.map_or_else(|| (to - from).to_string(), |tc| tc.duration_label(to - from)),
            ..Block::written(kind, text)
        };
        let after = self.view.selected;
        self.write(preview, move |target| qnc_script_db::add(target, after, block).map(Some))
    }

    /// Starts recording block `block` (of a recorded kind), or stops the recording that runs
    /// (whichever block the press names). The microphone is opened off the form thread.
    fn record(&mut self, preview: &SourcePreview, block: Option<i64>) -> bool {
        if let Some((id, recorder)) = self.recorder.take() {
            return self.finish(preview, id, recorder);
        }
        if self.starting.is_some() {
            return true;
        }
        let Some(id) = block.filter(|id| self.view.blocks.iter().any(|row| row.block.id == *id && row.recorded)) else {
            self.view.message = qnc_ui_text::text("script.choose_to_record").into();
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
    /// the block's text as its picture (its mark above it), the time it was recorded as its
    /// timecode.
    fn finish(&mut self, preview: &SourcePreview, id: i64, recorder: qnc_voice_record::Recorder) -> bool {
        let Some(take) = self.take.take() else { return false };
        let (text, heading) = self.view.blocks.iter().find(|row| row.block.id == id).map(|row| (row.block.text.clone(), row.mark.clone())).unwrap_or_default();
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

    /// The drafts of block `block`'s fields go into the script (nothing is written when they
    /// change nothing). The block shows them at once; the script read after the write is the truth.
    fn commit_fields(&mut self, preview: &SourcePreview, block: i64) -> bool {
        let drafts: Vec<(String, String)> = self.view.field_drafts.iter().filter(|((id, _), _)| *id == block).map(|((_, code), value)| (code.clone(), value.clone())).collect();
        self.view.field_drafts.retain(|(id, _), _| *id != block);
        let Some(stored) = self.blocks.iter_mut().find(|b| b.id == block) else { return false };
        let mut fields = stored.fields.clone();
        for (code, value) in drafts {
            if value.trim().is_empty() {
                fields.remove(&code);
            } else {
                fields.insert(code, value.trim().to_string());
            }
        }
        if fields == stored.fields {
            return true;
        }
        stored.fields = fields.clone();
        self.write(preview, move |target| qnc_script_db::set_fields(target, block, fields).map(|_| None))
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
                    Ok((blocks, profile, target)) => {
                        self.blocks = blocks;
                        self.view.profile = profile;
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
        self.view.blocks = rows(&self.blocks, self.view.profile.as_ref(), self.words_per_second);
        let profile = self.view.profile.as_ref();
        let counts = |row: &BlockRow| profile.and_then(|profile| profile.kind(&row.block.kind)).map(|kind| kind.counts);
        let sum = |keep: &dyn Fn(&BlockRow) -> bool| self.view.blocks.iter().filter(|row| keep(row)).map(|row| row.seconds).sum::<f64>();
        self.view.read_seconds = sum(&|row| counts(row) == Some(Counts::Story) && !row.is_clip());
        self.view.clip_seconds = sum(&|row| counts(row) == Some(Counts::Story) && row.is_clip());
        self.view.anchor_seconds = sum(&|row| counts(row) == Some(Counts::Anchor));
        self.refresh_editor();
        self.view != before
    }
}

/// The script, the profile it is written with and the target length of the story sheet (its
/// first length field's value). A project without a profile gets the one chosen on this
/// computer.
fn read_script(target: &ProjectDbTarget) -> Read {
    let blocks = qnc_script_db::read(target)?;
    let profile = match qnc_script_db::read_profile(target)? {
        Some(profile) => Some(profile),
        None => {
            let root = qnc_dev_diagnostics::resolve_qnc_root().ok_or_else(|| qnc_ui_text::text("script.no_profile").to_string())?;
            let (profiles, errors) = qnc_script_profile::Profiles::load(&root);
            for error in errors {
                qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("script-profile refused {error}"));
            }
            let chosen = profiles.current().cloned().ok_or_else(|| qnc_ui_text::text("script.no_profile").to_string())?;
            qnc_script_db::set_profile(target, chosen)?;
            qnc_script_db::read_profile(target)?
        }
    };
    let sheet = qnc_newsroom_sheet_db::read(target).ok();
    let length = sheet.and_then(|sheet| {
        let field = sheet.sheet.get("fields")?.as_array()?.iter().find(|field| field.get("kind").and_then(serde_json::Value::as_str) == Some("duration"))?;
        let code = field.get("id")?.as_str()?;
        sheet.values.get(code)?.as_f64().filter(|seconds| *seconds > 0.0)
    });
    Ok((blocks, profile, length))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn general_tv() -> ScriptProfile {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/script-profiles/general_tv.json");
        serde_json::from_value(qnc_ui_text::resolve_contract(&std::fs::read_to_string(file).unwrap()).unwrap()).unwrap()
    }

    fn sot(text: &str) -> Block {
        Block { clip_id: "c".into(), clip_name: "Mironik 2002".into(), start_tc: "10:00:02:00".into(), end_tc: "10:00:06:49".into(), length: "5:00".into(), seconds: 5.0, ..Block::written("sot", text) }
    }

    #[test]
    fn a_text_block_lasts_as_long_as_its_words_take_to_read() {
        assert_eq!(reading_seconds("jedan dva tri  cetiri pet", 2.5), 2.0);
        assert_eq!(reading_seconds("   ", 2.5), 0.0);
        assert_eq!(seconds_label(83.6), "1:24");
    }

    #[test]
    fn marks_are_the_numbered_cues_of_the_profile_and_lengths_its_rules() {
        let profile = general_tv();
        let mut recorded = Block::written("off", "Drugi off koji je snimljen.");
        (recorded.voice_uri, recorded.voice_seconds) = ("qnc://v.mxf".into(), 7.0);
        let blocks = [Block::written("intro", "Najava."), Block::written("off", "Uvod u prilog."), sot("Rekao je."), recorded, Block::written("nat", "")];
        let rows = rows(&blocks, Some(&profile), 2.5);
        let marks: Vec<&str> = rows.iter().map(|row| row.mark.as_str()).collect();
        assert_eq!(marks, [profile.kind("intro").unwrap().short.as_str(), "OFF 1", "SOT 1", "OFF 2", "NAT"]);
        assert_eq!(rows.iter().map(|row| row.seconds).collect::<Vec<_>>(), [0.4, 1.2, 5.0, 7.0, 0.0]);
        assert!(rows[1].recorded && !rows[0].recorded && rows[2].is_clip());
    }

    #[test]
    fn the_script_as_text_has_every_block_in_reading_order_with_its_mark() {
        let profile = general_tv();
        let blocks = [Block::written("off", "Uvod u prilog."), sot("Rekao je.")];
        let view = ScriptView { blocks: rows(&blocks, Some(&profile), 2.5), profile: Some(profile), ..ScriptView::default() };
        assert_eq!(view.text(), "OFF 1 (0:01)\nUvod u prilog.\n\nSOT 1 Mironik 2002 10:00:02:00 - 10:00:06:49 (5:00)\nRekao je.");
        assert_eq!(view.text_kinds().iter().map(|k| k.id.as_str()).collect::<Vec<_>>(), ["intro", "off", "outro"]);
        assert_eq!(view.clip_kinds().iter().map(|k| k.id.as_str()).collect::<Vec<_>>(), ["sot", "standup", "nat"]);
    }

    #[test]
    fn the_fields_of_a_block_are_in_the_pasted_script_and_a_draft_is_shown_until_committed() {
        let profile = general_tv();
        let mut statement = sot("Rekao je.");
        statement.id = 4;
        statement.fields.insert("caption_name".into(), "Ivan Horvat".into());
        let name = profile.kind("sot").unwrap().fields[0].label.clone();
        let mut view = ScriptView { blocks: rows(&[statement], Some(&profile), 2.5), profile: Some(profile), ..ScriptView::default() };
        assert!(view.text().ends_with(&format!("Rekao je.\n{name}: Ivan Horvat")));
        view.field_drafts.insert((4, "caption_name".into()), "Ivan Horvat Mlađi".into());
        assert_eq!(view.field_value(4, "caption_name"), "Ivan Horvat Mlađi");
        assert_eq!(view.field_value(4, "caption_role"), "");
    }
}
