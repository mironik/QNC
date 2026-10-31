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
/// SubSegment: the shown IN/OUT is the next part of the chosen clip block, or a new one.
pub const SUBSEGMENT: &str = "script_subsegment";
/// A block is chosen (target: its id); a text block's text goes into the editor.
pub const SELECT: &str = "script_select";
/// A block goes one place up or down, or leaves (target: its id).
pub const MOVE_UP: &str = "script_move_up";
pub const MOVE_DOWN: &str = "script_move_down";
pub const REMOVE: &str = "script_remove";
/// A clip block opens its clip with its IN/OUT (target: its id).
pub const OPEN: &str = "script_open";
/// The open SubSegment's actions and view are the piece `qnc-subsegment-edit`'s.
pub use qnc_subsegment_edit::{
    PieceWrite, SubsegmentEdit, SubsegmentPart, SubsegmentView, DRAG_EDGE, JOIN_CUT, JOIN_GAP, PART_CHANNEL, PART_LEVEL, PART_MOVE, PART_OFF, PART_REMOVE,
    SUBSEGMENT_CLOSE, SUBSEGMENT_CUE, TAKE_EDGE,
};
/// A block of a recorded kind is recorded from the microphone, or its recording stops (target:
/// its id; without one the chosen block). The catalog names it `record_toggle` (v4).
pub const RECORD: &str = "record_toggle";
/// A take of a block's voice record becomes the one used (target: `block:take`; user
/// 2026-10-30: the editor chooses the take as A1 of a camera clip).
pub const VOICE_TAKE: &str = "script_voice_take";
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
/// The draft story of the script is written as a Final Cut Pro 7 XML in the project's export
/// folder (user 2026-10-30): recorded OFFs and statements in reading order.
pub const DRAFT: &str = "script_draft";
/// The skeleton of the script is added at the end of the story (user 2026-10-31): recorded
/// OFFs as Offs, statements as Tons; OFFs not recorded yet are left out.
pub const SEND_STORY: &str = "script_send_story";
/// The exported A/V script is shown in the file browser of this OS.
pub const OPEN_EXPORT: &str = "script_open_export";

/// How often the script is read again (another process may change it).
const READ_EVERY: Duration = Duration::from_secs(2);

/// What a take gives back: the take that started, or the finished one as its block keeps it.
enum TakeAnswer {
    Started(Take),
    Finished(Result<Recorded, String>),
    Cancelled(Result<(), String>),
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
    /// The draft story being written (`qnc-export --draft`).
    draft: qnc_export_launch::ExportRun,
    /// The clip block open as a SubSegment in place of the source timeline, edited by the
    /// piece `qnc-subsegment-edit` (its block id is the open key).
    subsegment: SubsegmentEdit,
    /// The voice records of the blocks as read, and whether their files are there.
    voices: Vec<(Voice, bool)>,
    /// A take to hear: its record's clip and the channel (zero based), once.
    voice_request: Option<(String, u16)>,
    /// The take to hear once the write in progress is done.
    voice_after_write: Option<(String, u16)>,
    /// What the panel says once the write in progress is done.
    message_after_write: Option<String>,
}

impl Default for Script {
    fn default() -> Self {
        Self { view: ScriptView::default(), blocks: Vec::new(), project: None, reading: None, read_at: None, writing: None, source_request: None, names: HashMap::new(), words_per_second: Ok(0.0), take: None, taking: None, exporting: None, draft: Default::default(), subsegment: SubsegmentEdit::new(), voices: Vec::new(), voice_request: None, voice_after_write: None, message_after_write: None }
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
        self.reading.is_some() || self.writing.is_some() || self.take.is_some() || self.taking.is_some() || self.exporting.is_some() || self.draft.running() || self.subsegment.busy()
    }

    /// A clip and IN/OUT a chosen clip block opens on the source timeline, once.
    pub fn take_source_request(&mut self) -> Option<(String, u64, u64)> {
        self.source_request.take()
    }

    /// The take to hear (its record's clip in the catalog and its channel), once.
    pub fn take_voice_request(&mut self) -> Option<(String, u16)> {
        self.voice_request.take()
    }

    /// The parts the player should play and the frame to show, once (the open SubSegment).
    pub fn take_pieces_request(&mut self) -> Option<(Vec<qnc_source_preview::PieceInput>, u64)> {
        self.subsegment.take_pieces_request()
    }

    fn block(&self, id: Option<i64>) -> Option<&Block> {
        id.and_then(|id| self.blocks.iter().find(|block| block.id == id))
    }

    /// The open SubSegment as its block holds it now and the player plays it.
    fn follow_subsegment(&mut self, preview: &SourcePreview) {
        let pieces = self.block(self.subsegment.key()).map(view::block_pieces);
        let settled = self.reading.is_none() && self.writing.is_none();
        self.subsegment.follow(pieces.as_deref(), settled, &self.names, preview, self.project.as_deref());
        self.view.subsegment = self.subsegment.view().cloned();
    }

    /// What the SubSegment's edit did: shown, or a write of its block's parts.
    fn subsegment_answer(&mut self, answer: qnc_subsegment_edit::Answer, preview: &SourcePreview) -> bool {
        self.view.subsegment = self.subsegment.view().cloned();
        let qnc_subsegment_edit::Answer::Write(write) = answer else {
            return matches!(answer, qnc_subsegment_edit::Answer::Done(true));
        };
        let Some(id) = self.subsegment.key() else { return false };
        self.write(preview, move |target| {
            match write {
                PieceWrite::Set { index, piece } => qnc_script_db::set_piece(target, id, index, piece),
                PieceWrite::Move { index, earlier } => qnc_script_db::move_piece(target, id, index, earlier),
                PieceWrite::Remove { index } => qnc_script_db::remove_piece(target, id, index),
                PieceWrite::Join { index, gap, cut } => qnc_script_db::set_join(target, id, index, gap, cut),
            }
            .map(|_| None)
        })
    }

    /// The kind `code` of the project's profile.
    fn kind(&self, code: &str) -> Option<&KindSpec> {
        self.view.profile.as_ref()?.kind(code)
    }

    /// The script's own actions; `None` for any other.
    pub fn act(&mut self, action_id: &str, target: Option<&str>, (preview, timeline, timecode, transcript): (&SourcePreview, &TimelineProjection, Option<&SourceTimecode>, &TranscriptView)) -> Option<bool> {
        // While a SubSegment is open in place of the source timeline its trim keys are its own.
        if let Some(block) = self.block(self.subsegment.key()).cloned() {
            if let Some(answer) = self.subsegment.act(action_id, target, &view::block_pieces(&block), &self.names) {
                return Some(self.subsegment_answer(answer, preview));
            }
        }
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
            SUBSEGMENT => self.add_subsegment(preview, timeline, timecode, transcript),
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
            VOICE_TAKE => {
                let Some((block, take)) = target.and_then(|target| target.split_once(':')).and_then(|(block, take)| Some((block.parse::<i64>().ok()?, take.parse::<u16>().ok()?))) else { return Some(false) };
                // The take is heard at once: its record's clip on the preview, A1 on its
                // channel (user 2026-10-30: "clicking an OFF plays its sound").
                // Its record goes into the catalog first (a record made before the catalog knew
                // voice clips is written there now), then the take used is kept.
                let voice = self.voices.iter().find(|(voice, _)| voice.block_id == block).map(|(voice, _)| voice.clone());
                let (Some(voice), Some(locator)) = (voice, preview.still_locator()) else { return Some(false) };
                self.voice_after_write = Some((qnc_voice_take::clip_id(&voice.name), take.saturating_sub(1)));
                self.write(preview, move |target| {
                    let takes: Vec<f64> = voice.takes.iter().map(|take| take.seconds).collect();
                    let first_tc = voice.takes.first().map(|take| take.tc.clone()).unwrap_or_default();
                    let record = qnc_voice_take::describe_record(locator.settings(), &voice.name, &voice.clip_uri, &takes, &first_tc)?;
                    qnc_voice_catalog::publish_record(locator.reader(), locator.settings(), &voice.name, record)?;
                    qnc_script_db::set_voice_chosen(target, block, take).map(|_| None)
                })
            }
            // Esc while a take records throws it away (user 2026-10-30).
            "clear_focus" if self.take.is_some() => self.cancel_take(),
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
            DRAFT => {
                if self.draft.start(&["--draft"]) {
                    self.view.message = qnc_ui_text::text("script.drafting").into();
                }
                true
            }
            SEND_STORY => self.send_story(preview),
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
            // A block of several parts opens as a SubSegment in place of the source timeline.
            OPEN if self.block(id()).is_some_and(|block| block.pieces.len() > 1) => {
                let block = self.block(id())?.clone();
                self.subsegment.open(block.id, &view::block_pieces(&block));
                self.view.subsegment = self.subsegment.view().cloned();
                true
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
        let Some(piece) = self.marked_piece(preview, timeline, timecode, transcript) else { return true };
        let block = Block::of_pieces(kind, vec![piece]);
        // A block of one range keeps no parts (its range is its own).
        let block = Block { pieces: Vec::new(), ..block };
        let after = self.view.selected;
        self.write(preview, move |target| qnc_script_db::add(target, after, block).map(Some))
    }

    /// SubSegment (user 2026-10-29): the shown clip's IN/OUT with the words said there is the
    /// next part of the chosen clip block; without a chosen clip block it starts a new one of
    /// the profile's first clip kind (chosen then, so the next press adds to it).
    fn add_subsegment(&mut self, preview: &SourcePreview, timeline: &TimelineProjection, timecode: Option<&SourceTimecode>, transcript: &TranscriptView) -> bool {
        let Some(piece) = self.marked_piece(preview, timeline, timecode, transcript) else { return true };
        if let Some(id) = self.view.selected.filter(|id| self.view.blocks.iter().any(|row| row.block.id == *id && row.is_clip())) {
            return self.write(preview, move |target| qnc_script_db::add_piece(target, id, piece).map(|_| None));
        }
        let Some(kind) = self.view.clip_kinds().first().map(|kind| kind.id.clone()) else {
            self.view.message = qnc_ui_text::text("script.no_clip_kind").into();
            return true;
        };
        let block = Block::of_pieces(kind, vec![piece]);
        let after = self.view.selected;
        self.write(preview, move |target| qnc_script_db::add(target, after, block).map(Some))
    }

    /// The shown clip's IN/OUT as a part, with the words said there (none, and the reason in
    /// the panel, without an IN/OUT).
    fn marked_piece(&mut self, preview: &SourcePreview, timeline: &TimelineProjection, timecode: Option<&SourceTimecode>, transcript: &TranscriptView) -> Option<Piece> {
        let marked = preview.view().clip_id.clone().zip(timeline.visible_source_marks()).filter(|(_, (from, to))| to > from);
        let Some((clip_id, (from, to))) = marked else {
            self.view.message = qnc_ui_text::text("script.sot_marks").into();
            return None;
        };
        let at = |frame: u64| timecode.map_or_else(|| frame.to_string(), |tc| tc.label(frame));
        Some(Piece {
            clip_name: self.names.get(&clip_id).map_or_else(|| clip_id.clone(), |(name, _, _)| name.clone()),
            seconds: self.seconds_of(&clip_id, to - from),
            text: if transcript.clip_id.as_deref() == Some(clip_id.as_str()) { transcript.said(from, to) } else { String::new() },
            clip_id,
            from_frame: from,
            to_frame: to,
            start_tc: at(from),
            // The OUT is past the last frame: the end shown is the last frame of the part.
            end_tc: at(to.saturating_sub(1).max(from)),
            length: timecode.map_or_else(|| (to - from).to_string(), |tc| tc.duration_label(to - from)),
            gap_before: 0,
            cut: false,
            a1_channel: None,
            a1_level_db: 0,
            a1_off: false,
        })
    }

    /// Starts the take of block `block` (of a recorded kind), or stops the take that records
    /// (whichever block the press names). The take (piece `qnc-voice-take`) opens the
    /// microphone and makes its clip off the form thread; a finished take goes into its block.
    fn record(&mut self, preview: &SourcePreview, block: Option<i64>) -> bool {
        if self.taking.is_some() {
            return true;
        }
        if let Some((id, take)) = self.take.take() {
            let mark = self.view.blocks.iter().find(|row| row.block.id == id).map(|row| row.mark.clone()).unwrap_or_default();
            self.view.message = qnc_ui_text::text("script.making_clip").into();
            let locator = preview.still_locator();
            return self.spawn_take(id, move || {
                // The picture of the record says which OFF it is (user 2026-10-30); the record
                // goes into the catalog, so the Broadcast Player plays its takes.
                let finished = take.finish(&mark, "");
                if let (Ok(recorded), Some(locator)) = (&finished, locator) {
                    if let Err(error) = qnc_voice_catalog::publish_record(locator.reader(), locator.settings(), &recorded.name, recorded.record.clone()) {
                        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("voice-catalog {} refused: {error}", recorded.name));
                    }
                }
                TakeAnswer::Finished(finished)
            });
        }
        let Some(id) = block.filter(|id| self.view.blocks.iter().any(|row| row.block.id == *id && row.recorded)) else {
            self.view.message = qnc_ui_text::text("script.choose_to_record").into();
            return true;
        };
        let setting = qnc_dev_diagnostics::resolve_qnc_root().map(|root| qnc_dev_diagnostics::config_path(&root).with_file_name(qnc_voice_record::INPUT_SETTING));
        let (Some(locator), Some(setting), Some(target)) = (preview.still_locator(), setting, preview.project_target().cloned()) else {
            self.view.message = qnc_ui_text::text("script.no_project").into();
            return true;
        };
        let mark = self.view.blocks.iter().find(|row| row.block.id == id).map(|row| row.mark.clone()).unwrap_or_default();
        self.view.message = qnc_ui_text::text("script.record_starting").into();
        // The block's record as the database keeps it: the next take is its next channel.
        self.spawn_take(id, move || {
            let earlier = match qnc_script_db::read_voice(&target, id) {
                Ok(voice) => voice.map(|voice| qnc_voice_take::Earlier {
                    first_tc: voice.takes.first().map(|take| take.tc.clone()).unwrap_or_default(),
                    takes: voice.takes.iter().map(|take| (take.wav_uri.clone(), take.seconds)).collect(),
                    name: voice.name,
                }),
                Err(error) => return TakeAnswer::Finished(Err(error)),
            };
            match Take::start(&locator, &mark, earlier, &setting) {
                Ok(take) => TakeAnswer::Started(take),
                Err(error) => TakeAnswer::Finished(Err(error)),
            }
        })
    }

    /// The take that records is thrown away (Esc): nothing is kept.
    fn cancel_take(&mut self) -> bool {
        let Some((id, take)) = self.take.take() else { return false };
        self.spawn_take(id, move || TakeAnswer::Cancelled(take.cancel()))
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

    /// Sends the skeleton to the story (user 2026-10-31): the voice records go into the catalog
    /// first (one made before the catalog knew voice clips is written there now), then the
    /// segments are added at the end of the story in one step, each with the sound of its part
    /// or the one kept for its clip, and where it came from.
    fn send_story(&mut self, preview: &SourcePreview) -> bool {
        let (Some(profile), Some(locator)) = (self.view.profile.clone(), preview.still_locator()) else {
            self.view.message = qnc_ui_text::text("script.no_project").into();
            return true;
        };
        let voices: Vec<Voice> = self.voices.iter().map(|(voice, _)| voice.clone()).collect();
        let fps = match locator.settings().video_picture() {
            Ok((_, _, fps)) => fps,
            Err(error) => {
                self.view.message = error.message;
                return true;
            }
        };
        let rates = qnc_script_draft::DraftRates { words_per_second: self.reading_speed(), fps, clip_id: &qnc_voice_take::clip_id };
        let parts = match qnc_script_draft::skeleton(&self.blocks, &voices, &profile, &rates) {
            Ok(parts) => parts,
            Err(error) => {
                self.view.message = error;
                return true;
            }
        };
        self.message_after_write = Some(qnc_ui_text::fill("script.sent_story", &[("n", &parts.len().to_string())]));
        self.view.message = qnc_ui_text::text("script.sending_story").into();
        self.write(preview, move |target| {
            for voice in voices.iter().filter(|voice| !voice.takes.is_empty()) {
                let takes: Vec<f64> = voice.takes.iter().map(|take| take.seconds).collect();
                let first_tc = voice.takes.first().map(|take| take.tc.clone()).unwrap_or_default();
                let record = qnc_voice_take::describe_record(locator.settings(), &voice.name, &voice.clip_uri, &takes, &first_tc)?;
                qnc_voice_catalog::publish_record(locator.reader(), locator.settings(), &voice.name, record)?;
            }
            // The skeleton goes to S-Story, its own record (user 2026-10-31): each part a source
            // clip; neither the script nor the Story changes.
            let mut elements = Vec::with_capacity(parts.len());
            for part in parts {
                let kept = qnc_clip_sound::read(target, &part.clip_id)?;
                let lane = |lane: Option<&qnc_clip_sound::LaneSound>, channel: u16| lane.map_or((channel, 0, false), |lane| (lane.channel, lane.level_db, lane.off));
                let kept_a1 = lane(kept.as_ref().map(|sound| &sound.a1), 0);
                let a1 = match part.a1 {
                    Some((channel, level, off)) => (channel.unwrap_or(kept_a1.0), level, off),
                    None => kept_a1,
                };
                // An OFF's A2 is its own voice, off; a statement keeps the A2 kept for its clip,
                // else channel 2.
                let off = part.kind == "offovi";
                let a2 = if off { (a1.0, 0, true) } else { lane(kept.as_ref().map(|sound| &sound.a2), 1) };
                let as_lane = |(channel, level_db, off): (u16, i8, bool)| qnc_skeleton_db::Lane { channel, level_db, off };
                elements.push(qnc_skeleton_db::NewElement {
                    kind: if off { qnc_skeleton_db::ElementKind::Off } else { qnc_skeleton_db::ElementKind::Ton },
                    clip_id: part.clip_id,
                    in_frame: part.from_frame,
                    out_frame: part.to_frame,
                    a1: as_lane(a1),
                    a2: as_lane(a2),
                    origin: part.origin,
                });
            }
            qnc_skeleton_db::append(target, elements)?;
            Ok(None)
        })
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
            let locator = preview.still_locator();
            if std::thread::Builder::new().name("qnc-script-read".into()).spawn(move || _ = send.send(read::read_script(&project, locator.as_ref()))).is_ok() {
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
        for row in &mut self.view.blocks {
            row.voice = self.voices.iter().find(|(voice, _)| voice.block_id == row.block.id).map(|(voice, present)| VoiceRow::of(voice, *present, &row.block.text));
        }
        let profile = self.view.profile.as_ref();
        let counts = |row: &BlockRow| profile.and_then(|profile| profile.kind(&row.block.kind)).map(|kind| kind.counts);
        let sum = |keep: &dyn Fn(&BlockRow) -> bool| self.view.blocks.iter().filter(|row| keep(row)).map(|row| row.seconds).sum::<f64>();
        self.view.read_seconds = sum(&|row| counts(row) == Some(Counts::Story) && !row.is_clip());
        self.view.clip_seconds = sum(&|row| counts(row) == Some(Counts::Story) && row.is_clip());
        self.view.anchor_seconds = sum(&|row| counts(row) == Some(Counts::Anchor));
        (self.view.in_cue, self.view.out_cue) = story_cues(&self.view.blocks);
        if self.subsegment.key().is_some() {
            self.follow_subsegment(preview);
        }
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
                    // A finished take goes into its block's record as its next channel, the one used.
                    TakeAnswer::Finished(Ok(Recorded { name, clip_uri, number, wav_uri, seconds, timecode, .. })) => {
                        let text = self.view.blocks.iter().find(|row| row.block.id == id).map(|row| row.block.text.clone()).unwrap_or_default();
                        let take = qnc_script_db::VoiceTake { take: number, wav_uri, seconds, tc: timecode, text };
                        self.write(preview, move |target| qnc_script_db::add_voice_take(target, id, name, clip_uri, take).map(|_| None));
                    }
                    TakeAnswer::Finished(Err(error)) => self.view.message = error,
                    TakeAnswer::Cancelled(Ok(())) => self.view.message = qnc_ui_text::text("script.record_cancelled").into(),
                    TakeAnswer::Cancelled(Err(error)) => self.view.message = error,
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
        if let Some(outcome) = self.draft.take() {
            match outcome {
                Ok(file) => {
                    self.view.message.clear();
                    self.view.exported = Some(file);
                }
                Err(error) => self.view.message = error,
            }
        }
        self.view.exporting = self.exporting.is_some() || self.draft.running();
        if let Some(done) = &self.writing {
            if let Ok(result) = done.try_recv() {
                let heard = self.voice_after_write.take();
                let said = self.message_after_write.take();
                match result {
                    Ok(added) => {
                        self.voice_request = heard;
                        self.view.message = said.unwrap_or_default();
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
                    Ok(read::Script { blocks, profile, target, voices }) => {
                        self.blocks = blocks;
                        self.view.profile = profile;
                        self.view.target_seconds = target;
                        self.voices = voices;
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
