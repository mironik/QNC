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
//! on the screen until it is added or replaced. A take is the piece `qnc-voice-take`'s, writing
//! the script out the piece `qnc-script-files`'; what the panel shows is `view`.

use std::{
    collections::HashMap,
    sync::mpsc,
    time::{Duration, Instant},
};

use qnc_clip_row::ClipRow;
use qnc_db_broker::ProjectDbTarget;
use qnc_source_preview::SourcePreview;
use qnc_source_timecode::SourceTimecode;
use qnc_timeline::TimelineProjection;
use qnc_transcript::TranscriptView;
use qnc_voice_take::{Recorded, Take};

mod read;
mod view;

pub use view::*;

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
/// The script is written out to the project's export folder: clean text, the two-column A/V script
/// and a MOS story (user 2026-10-28: the field crew sends it to the editor and the proofreader).
pub const EXPORT: &str = "script_export";
/// The exported A/V script is shown in the file browser of this OS.
pub const OPEN_EXPORT: &str = "script_open_export";

/// How often the script is read again (another process may change it).
const READ_EVERY: Duration = Duration::from_secs(2);

/// What a take gives back: the take that started, or the finished one as its block keeps it.
enum TakeAnswer {
    Started(Take),
    Finished(Result<Recorded, String>),
}

pub struct Script {
    view: ScriptView,
    blocks: Vec<Block>,
    project: Option<String>,
    reading: Option<mpsc::Receiver<read::Read>>,
    read_at: Option<Instant>,
    /// A write on its way to the database; its answer is the id of an added block.
    writing: Option<mpsc::Receiver<Result<Option<i64>, String>>>,
    source_request: Option<(String, u64, u64)>,
    /// Name, frames and seconds of each clip of the pool.
    names: HashMap<String, (String, u64, f64)>,
    /// The project's reading speed (the reason when it has none).
    words_per_second: Result<f64, String>,
    /// The take that records, of which block (piece `qnc-voice-take`).
    take: Option<(i64, Take)>,
    /// A take on its way to start, or a stopped one on its way to its clip.
    taking: Option<(i64, mpsc::Receiver<TakeAnswer>)>,
    /// An export on its way to the export folder (piece `qnc-script-files`).
    exporting: Option<mpsc::Receiver<Result<String, String>>>,
}

impl Default for Script {
    fn default() -> Self {
        Self { view: ScriptView::default(), blocks: Vec::new(), project: None, reading: None, read_at: None, writing: None, source_request: None, names: HashMap::new(), words_per_second: Ok(0.0), take: None, taking: None, exporting: None }
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
        self.reading.is_some() || self.writing.is_some() || self.take.is_some() || self.taking.is_some() || self.exporting.is_some()
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
            RECORD if target.is_none() && self.view.selected.is_none() && self.take.is_none() => return None,
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
            EXPORT => self.export(preview),
            OPEN_EXPORT => {
                if let Some(file) = &self.view.exported {
                    if let Err(error) = qnc_status_line::open(file) {
                        self.view.message = error;
                    }
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

    /// Starts the take of block `block` (of a recorded kind), or stops the take that records
    /// (whichever block the press names). The take (piece `qnc-voice-take`) opens the
    /// microphone and makes its clip off the form thread; a finished take goes into its block.
    fn record(&mut self, preview: &SourcePreview, block: Option<i64>) -> bool {
        if self.taking.is_some() {
            return true;
        }
        if let Some((id, take)) = self.take.take() {
            let (text, heading) = self.view.blocks.iter().find(|row| row.block.id == id).map(|row| (row.block.text.clone(), row.mark.clone())).unwrap_or_default();
            self.view.message = qnc_ui_text::text("script.making_clip").into();
            return self.spawn_take(id, move || TakeAnswer::Finished(take.finish(&text, &heading)));
        }
        let Some(id) = block.filter(|id| self.view.blocks.iter().any(|row| row.block.id == *id && row.recorded)) else {
            self.view.message = qnc_ui_text::text("script.choose_to_record").into();
            return true;
        };
        let setting = qnc_dev_diagnostics::resolve_qnc_root().map(|root| qnc_dev_diagnostics::config_path(&root).with_file_name(qnc_voice_record::INPUT_SETTING));
        let (Some(locator), Some(setting)) = (preview.still_locator(), setting) else {
            self.view.message = qnc_ui_text::text("script.no_project").into();
            return true;
        };
        self.view.message = qnc_ui_text::text("script.record_starting").into();
        self.spawn_take(id, move || match Take::start(&locator, id, &setting) {
            Ok(take) => TakeAnswer::Started(take),
            Err(error) => TakeAnswer::Finished(Err(error)),
        })
    }

    fn spawn_take(&mut self, id: i64, work: impl FnOnce() -> TakeAnswer + Send + 'static) -> bool {
        let (send, answer) = mpsc::channel();
        if std::thread::Builder::new().name("qnc-script-take".into()).spawn(move || _ = send.send(work())).is_ok() {
            self.taking = Some((id, answer));
        }
        true
    }

    /// Writes the script out off the form thread (piece `qnc-script-files`).
    fn export(&mut self, preview: &SourcePreview) -> bool {
        if self.exporting.is_some() {
            return true;
        }
        let (Some(locator), Some(target)) = (preview.still_locator(), preview.project_target().cloned()) else {
            self.view.message = qnc_ui_text::text("script.no_project").into();
            return true;
        };
        let story = export_story(&self.view);
        let (send, done) = mpsc::channel();
        if std::thread::Builder::new().name("qnc-script-export".into()).spawn(move || _ = send.send(qnc_script_files::write(&locator, &target, story))).is_ok() {
            self.exporting = Some(done);
            self.view.message = qnc_ui_text::text("script.exporting").into();
        }
        true
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

    fn reading_speed(&self) -> f64 {
        self.words_per_second.as_ref().copied().unwrap_or(0.0)
    }

    fn refresh_editor(&mut self) {
        self.view.editor_seconds = reading_seconds(&self.view.editor, self.reading_speed());
    }

    /// Takes the answers of takes, exports, writes and reads, reads the script again now and
    /// then. Returns whether the view changed.
    pub fn poll(&mut self, preview: &SourcePreview, timeline: &TimelineProjection, clips: &[ClipRow]) -> bool {
        let before = self.view.clone();
        if self.names.len() != clips.len() || clips.iter().any(|clip| !self.names.contains_key(&clip.clip_id)) {
            self.names = clips.iter().map(|clip| (clip.clip_id.clone(), (clip.name.clone(), clip.duration_frames, clip.duration_seconds))).collect();
        }
        self.view.marks = preview.view().clip_id.is_some() && timeline.visible_source_marks().is_some_and(|(from, to)| to > from);
        let project = preview.project_target().cloned();
        let key = project.as_ref().map(|target| target.uri().to_string());
        if key != self.project {
            *self = Self { project: key, ..Self::default() };
        }
        // The reading speed is the project's; without it a text has no length and the panel says why.
        self.words_per_second = match preview.still_locator() {
            Some(locator) => locator.settings().reading_words_per_second().map_err(|e| e.message),
            None => Ok(0.0),
        };
        self.take_answers(preview);
        if let Some(project) = project.filter(|_| self.reading.is_none() && self.writing.is_none() && self.read_at.is_none_or(|at| at.elapsed() >= READ_EVERY)) {
            self.read_at = Some(Instant::now());
            let (send, read) = mpsc::channel();
            if std::thread::Builder::new().name("qnc-script-read".into()).spawn(move || _ = send.send(read::read_script(&project))).is_ok() {
                self.reading = Some(read);
            }
        }
        if self.view.selected.is_some_and(|id| !self.blocks.iter().any(|b| b.id == id)) && self.writing.is_none() && self.reading.is_none() && !self.blocks.is_empty() {
            self.view.selected = None;
        }
        if let Err(reason) = &self.words_per_second {
            if self.view.message.is_empty() {
                self.view.message = reason.clone();
            }
        }
        self.view.blocks = rows(&self.blocks, self.view.profile.as_ref(), self.reading_speed());
        let profile = self.view.profile.as_ref();
        let counts = |row: &BlockRow| profile.and_then(|profile| profile.kind(&row.block.kind)).map(|kind| kind.counts);
        let sum = |keep: &dyn Fn(&BlockRow) -> bool| self.view.blocks.iter().filter(|row| keep(row)).map(|row| row.seconds).sum::<f64>();
        self.view.read_seconds = sum(&|row| counts(row) == Some(Counts::Story) && !row.is_clip());
        self.view.clip_seconds = sum(&|row| counts(row) == Some(Counts::Story) && row.is_clip());
        self.view.anchor_seconds = sum(&|row| counts(row) == Some(Counts::Anchor));
        (self.view.in_cue, self.view.out_cue) = story_cues(&self.view.blocks);
        self.refresh_editor();
        self.view != before
    }

    /// The answers of takes, exports, writes and reads that came.
    fn take_answers(&mut self, preview: &SourcePreview) {
        if let Some((id, answer)) = &self.taking {
            if let Ok(answer) = answer.try_recv() {
                let id = *id;
                self.taking = None;
                match answer {
                    TakeAnswer::Started(take) => {
                        self.take = Some((id, take));
                        self.view.message = qnc_ui_text::text("script.recording").into();
                    }
                    // A finished take goes into its block: its clip, length and timecode.
                    TakeAnswer::Finished(Ok(Recorded { uri, seconds, timecode })) => {
                        self.write(preview, move |target| qnc_script_db::set_voice(target, id, uri, seconds, timecode).map(|_| None));
                    }
                    TakeAnswer::Finished(Err(error)) => self.view.message = error,
                }
            }
        }
        self.view.recording = self.take.as_ref().map(|(id, take)| (*id, take.seconds(), take.take_peak()));
        self.view.record_busy = self.taking.is_some();
        if let Some(done) = &self.exporting {
            if let Ok(result) = done.try_recv() {
                match result {
                    Ok(file) => {
                        self.view.message.clear();
                        self.view.exported = Some(file);
                    }
                    Err(error) => self.view.message = error,
                }
                self.exporting = None;
            }
        }
        self.view.exporting = self.exporting.is_some();
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
    }
}

#[cfg(test)]
mod tests;
