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
use qnc_timeline_model::TimelineProjection;
use qnc_transcript::TranscriptView;
use qnc_voice_take::{Recorded, Take};

mod read;
mod view;
mod text_edit;
mod text_marks;
pub use text_marks::MARK_AT;
mod session;
pub use session::{OFF_RECORDING, SESSION, SESSION_MARK};
mod word_pick;
pub use word_pick::{WordPick, TEXT_CUT, TEXT_DRAFT, TEXT_REVERT, WORD_PICK, WORD_TAKE};
pub use text_edit::text_of;
pub use qnc_subsegment_edit::{
    drag_target, nearest_edge, own_action, subsegment_pictures, subsegment_placements, subsegment_view, subsegment_zoom, JoinSide, JoinSound, SubsegmentPart, SubsegmentView, DRAG_EDGE, JOIN_CUT, JOIN_GAP, JOIN_SMOOTH, PART_CHANNEL, PART_LEVEL, PART_MOVE, PART_OFF, PART_REMOVE, SLIP, SUBSEGMENT_CLOSE, SUBSEGMENT_CUE, SUBSEGMENT_SELECT, SUBSEGMENT_TAKE, TAKE_EDGE, ZOOM_WORD,
};

pub use view::*;

pub const MODULE_ID: &str = "qnc.module.script";
/// The panel the script is shown on.
pub const PANEL: &str = "script";
/// The editor's text changed (target: the whole text).
pub const EDITOR_TEXT: &str = "script_editor_text";
/// The shots the reporter wants under the OFF in the editor changed (target: the whole text;
/// user 2026-11-11).
pub const EDITOR_NOTE: &str = "script_editor_note";
/// The editor's text goes into the script as a block of a text kind after the chosen block
/// (target: the kind's code).
pub const ADD_TEXT: &str = "script_add_text";
/// The editor's text replaces the text block it was taken from.
pub const REPLACE_TEXT: &str = "script_replace_text";
/// The editor is emptied (and no longer holds a block's text).
pub const CLEAR_EDITOR: &str = "script_editor_clear";
/// The editor's text is saved (user 2026-11-01, MA Script): back into the text block it was
/// taken from, otherwise as a block of the text kind named (target: the kind's code); once it
/// is in the script the editor is empty and holds no block.
pub const SAVE_TEXT: &str = "script_save_text";
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
/// A click on a word of a statement in the script (user 2026-11-04: edited by its text): the
/// statement opens in place of the source timeline and the playhead goes to the word. `target`
/// is `block:part:frame`.
pub const WORD_CUE: &str = "script_word_cue";
/// A double click on a word of a statement: the first is where the part starts or ends, the
/// second on a word of the same part sets its IN at the earlier word's start and its OUT at
/// the later word's end (whichever is first). `target` is `block:part:from:to`.
pub const WORD_CUT: &str = "script_word_cut";
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
/// The field code of the B-roll note of an OFF in `FIELD_TEXT` and `FIELD_COMMIT` (user
/// 2026-11-11: terms or a phrase the B-roll suggestion covers the OFF with first).
pub const BROLL_NOTE: &str = "@broll";
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
/// B-roll is suggested for every OFF of the story (user 2026-11-11: "a button starts the
/// suggestion"): with the shots the reporter wants first, then by the words.
pub const SUGGEST_BROLL: &str = "script_suggest_broll";
/// The B-roll suggested goes into the script, all OFFs in one step of undo (user 2026-11-11:
/// "write the series with one confirmation").
pub const ACCEPT_BROLL: &str = "script_accept_broll";
/// The B-roll suggested is dropped.
pub const DROP_BROLL: &str = "script_drop_broll";
/// The exported A/V script is shown in the file browser of this OS.
pub const OPEN_EXPORT: &str = "script_open_export";

/// How often the script is read again (another process may change it).
const READ_EVERY: Duration = Duration::from_secs(2);

/// A reading's own clip into the catalog, so the Broadcast Player plays it.
fn publish_reading(locator: &qnc_source_preview::StillLocator, record_name: &str, reading: &qnc_script_db::VoiceTake) -> Result<(), String> {
    match qnc_voice_take::reading_record(locator.settings(), record_name, reading) {
        Some(made) => {
            let (name, record) = made?;
            qnc_voice_catalog::publish_record(locator.reader(), locator.settings(), &name, record)
        }
        None => Ok(()),
    }
}

/// A write waiting for the one on its way.
type QueuedWrite = Box<dyn FnOnce(&ProjectDbTarget) -> Result<Option<i64>, String> + Send>;

/// What a take gives back: the take that started, or the finished one as its block keeps it.
enum TakeAnswer {
    Started(Take),
    Finished(Result<Vec<Recorded>, String>),
    Cancelled(Result<(), String>),
}

pub struct Script {
    /// The reporter asked for B-roll; taken by the side pieces that hold the suggestion.
    broll_asked: bool,
    view: ScriptView,
    blocks: Vec<Block>,
    project: Option<String>,
    reading: Option<mpsc::Receiver<read::Read>>,
    read_at: Option<Instant>,
    /// A write on its way to the database; its answer is the id of an added block.
    writing: Option<mpsc::Receiver<Result<Option<i64>, String>>>,
    /// Writes made while one is on its way: they go in order, each its own answer (audit
    /// 2026-11-08 #16: a second write overwrote the first's answer, which was lost).
    queued: std::collections::VecDeque<(ProjectDbTarget, QueuedWrite)>,
    source_request: Option<(String, u64, u64)>,
    /// Name, frames and seconds of each clip of the pool.
    names: HashMap<String, (String, u64, f64)>,
    /// The project's reading speed (the reason when it has none).
    words_per_second: Result<f64, String>,
    /// The take that records, of which block (piece `qnc-voice-take`).
    take: Option<(i64, Take)>,
    /// A take on its way to start, or a stopped one on its way to its clip.
    taking: Option<(i64, mpsc::Receiver<TakeAnswer>)>,
    /// The reading of the whole story that records, and one on its way to start or stop.
    session: Option<qnc_voice_take::Session>,
    session_busy: Option<mpsc::Receiver<session::SessionAnswer>>,
    /// An export on its way to the export folder (piece `qnc-script-files`).
    exporting: Option<mpsc::Receiver<Result<String, String>>>,
    /// The draft story being written (`qnc-export --draft`).
    draft: qnc_export_launch::ExportRun,
    /// The statement open as a SubSegment in place of the source timeline (`qnc-subsegment-edit`).
    edit: qnc_subsegment_edit::SubsegmentEdit,
    /// The fine times of the letters of those words, as last read, and the background work that
    /// lays the letters of the words shown without them (user 2026-11-05).
    letters: HashMap<String, Vec<qnc_letter_db::WordLetters>>,
    letter_times: qnc_letter_times::LetterTimes,
    /// The in-between pictures of the smooth cuts, made in the background (user 2026-11-08).
    smooth_joins: qnc_smooth_joins::SmoothJoins,
    /// The words heard in the takes, in the background (user 2026-11-08: the check of what was
    /// read).
    take_heard: qnc_take_heard::TakeHeard,
    /// The check of a take against the text it was made with (it is made once per text).
    read_checks: HashMap<(i64, u16), (String, std::sync::Arc<ReadCheck>)>,
    /// The first word of a cut by text (a double click on a word): its block, part and frames.
    word_cut: qnc_edit_words::WordPair<(i64, usize)>,
    /// Words picked in the text of a statement, to be cut out of its video with Delete.
    word_pick: Option<WordPick>,
    /// The voice records of the blocks as read, and whether their files are there.
    voices: Vec<(Voice, bool)>,
    /// A take to hear: its record's clip and the channel (zero based), once.
    voice_request: Option<(String, u16, bool, u64)>,
    /// The take to hear once the write in progress is done.
    voice_after_write: Option<(String, u16, bool, u64)>,
    marks_after_write: Option<(String, u64, u64)>,
    /// How many takes each record had when it was last heard here: one with more was made
    /// again and is prepared anew.
    heard_takes: HashMap<String, usize>,
    /// What the panel says once the write in progress is done.
    message_after_write: Option<String>,
    /// The editor is emptied once the write in progress is done (a saved text).
    clear_after_write: bool,
}


impl Default for Script {
    fn default() -> Self {
        Self { broll_asked: false, view: ScriptView::default(), blocks: Vec::new(), project: None, reading: None, read_at: None, writing: None, queued: std::collections::VecDeque::new(), source_request: None, names: HashMap::new(), words_per_second: Ok(0.0), take: None, taking: None, session: None, session_busy: None, exporting: None, draft: Default::default(), edit: Default::default(), letters: HashMap::new(), letter_times: qnc_letter_times::LetterTimes::new(), smooth_joins: qnc_smooth_joins::SmoothJoins::new(), take_heard: qnc_take_heard::TakeHeard::new(), read_checks: HashMap::new(), word_cut: qnc_edit_words::WordPair::new(), word_pick: None, voices: Vec::new(), voice_request: None, voice_after_write: None, marks_after_write: None, heard_takes: HashMap::new(), message_after_write: None, clear_after_write: false }
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

    /// The reporter's ask for B-roll for the OFFs, once (the side pieces work it out with the
    /// shots of the project).
    pub fn take_broll_ask(&mut self) -> bool {
        std::mem::take(&mut self.broll_asked)
    }

    /// The B-roll suggested for the OFFs (by block), shown until accepted or dropped; `None`:
    /// none could be made (`why`: the dictionary key of what says why).
    pub fn propose(&mut self, proposal: Option<Vec<(i64, Vec<qnc_script_db::Cover>)>>, why: &'static str) {
        self.view.suggesting = false;
        let shots: usize = proposal.iter().flatten().map(|(_, covers)| covers.len()).sum();
        self.view.message = if shots > 0 { qnc_ui_text::fill("script.broll_suggested", &[("count", &shots.to_string())]) } else { qnc_ui_text::text(why).to_string() };
        self.view.proposal = proposal.filter(|_| shots > 0);
    }

    /// Whether a read or a write of this process runs (the form repaints soon).
    pub fn busy(&self) -> bool {
        self.reading.is_some() || self.writing.is_some() || self.take.is_some() || self.taking.is_some() || self.exporting.is_some() || self.draft.running() || self.edit.busy()
    }

    /// A clip and IN/OUT a chosen clip block opens on the source timeline, once.
    pub fn take_source_request(&mut self) -> Option<(String, u64, u64)> {
        self.source_request.take()
    }

    /// The take to hear (its record's clip in the catalog and its channel), once.
    pub fn take_voice_request(&mut self) -> Option<(String, u16, bool, u64)> {
        self.voice_request.take()
    }

    /// The parts the player should play and the frame to show, once (the open SubSegment).
    pub fn take_pieces_request(&mut self) -> Option<(Vec<qnc_source_preview::PieceInput>, u64)> {
        self.edit.take_pieces_request()
    }

    fn block(&self, id: Option<i64>) -> Option<&Block> {
        id.and_then(|id| self.blocks.iter().find(|block| block.id == id))
    }

    /// The value of the next action (a frame of the trim zoom's click or drag).
    pub fn set_value(&mut self, value: Option<i64>) {
        self.edit.set_value(value);
    }

    /// AI of the open SubSegment; `None` while none is open.
    pub fn subsegment_assist(&self) -> Option<bool> {
        self.edit.sound_assist_on()
    }

    /// What the SubSegment did: its writes go to the script's database in one write, in order;
    /// its message and its view are the panel's.
    fn take_edit(&mut self, preview: &SourcePreview) {
        let writes = self.edit.take_writes();
        if !writes.is_empty() {
            self.write(preview, move |target| {
                for write in writes {
                    write(target)?;
                }
                Ok(None)
            });
        }
        let message = std::mem::take(&mut self.edit.view.message);
        if !message.is_empty() {
            self.view.message = message;
        }
        if self.view.subsegment != self.edit.view.subsegment {
            self.view.subsegment.clone_from(&self.edit.view.subsegment);
        }
    }

    /// The kind `code` of the project's profile.
    fn kind(&self, code: &str) -> Option<&KindSpec> {
        self.view.profile.as_ref()?.kind(code)
    }

    /// The script's own actions; `None` for any other.
    pub fn act(&mut self, action_id: &str, target: Option<&str>, context: (&SourcePreview, &TimelineProjection, Option<&SourceTimecode>, &TranscriptView)) -> Option<bool> {
        self.edit.sync(&self.blocks, &self.names, self.project.as_ref(), self.reading.is_none() && self.writing.is_none());
        let done = self.act_own(action_id, target, context);
        self.take_edit(context.0);
        done
    }

    fn act_own(&mut self, action_id: &str, target: Option<&str>, (preview, timeline, timecode, transcript): (&SourcePreview, &TimelineProjection, Option<&SourceTimecode>, &TranscriptView)) -> Option<bool> {
        // While a SubSegment is open in place of the source timeline its keys are its own
        // (`qnc-subsegment-edit`). A pointer on its timeline comes as the mouse map's row action
        // (AGENTS 0.12): the SubSegment's own action of it, with the frame or part it points at.
        if let Some(done) = self.session_key(action_id, preview) {
            return Some(done);
        }
        let own = self.edit.own(action_id, target);
        let (action_id, target) = match &own {
            Some((action, own_target)) => (action.as_str(), own_target.as_deref()),
            None => (action_id, target),
        };
        if let Some(done) = self.mark_key(action_id, target, preview) {
            return Some(done);
        }
        if let Some(done) = self.pick_key(action_id, target, preview) {
            return Some(done);
        }
        if let Some(done) = self.edit.key(action_id, target, preview) {
            return Some(done);
        }
        if let Some(done) = self.edit.act(action_id, target, preview) {
            return Some(done);
        }
        let id = || target.and_then(|id| id.parse::<i64>().ok());
        Some(match action_id {
            EDITOR_TEXT => {
                self.view.editor = target.unwrap_or_default().to_string();
                self.refresh_editor();
                true
            }
            EDITOR_NOTE => {
                self.view.editor_note = target.unwrap_or_default().to_string();
                true
            }
            CLEAR_EDITOR => {
                self.view.editor.clear();
                self.view.editor_note.clear();
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
                // An OFF goes in with the shots the reporter wants under it.
                let broll_note = if kind.role == Role::Spoken { self.view.editor_note.trim().to_string() } else { String::new() };
                self.view.editor_note.clear();
                self.write(preview, move |target| qnc_script_db::add_at(target, place, Block { broll_note, ..Block::written(kind.id, text) }).map(Some))
            }
            SAVE_TEXT => match self.view.editing {
                Some(block) => {
                    let text = self.view.editor.trim().to_string();
                    if text.is_empty() {
                        self.view.message = qnc_ui_text::text("script.write_first").into();
                        return Some(true);
                    }
                    self.clear_after_write = true;
                    // The shots wanted under an OFF are saved with its text, when they changed.
                    let stored = self.blocks.iter().find(|b| b.id == block);
                    let spoken = stored.and_then(|b| self.kind(&b.kind)).is_some_and(|kind| kind.role == Role::Spoken);
                    let note = self.view.editor_note.trim().to_string();
                    let note = (spoken && stored.is_some_and(|b| b.broll_note != note)).then_some(note);
                    self.write(preview, move |target| {
                        qnc_script_db::set_text(target, block, text)?;
                        if let Some(note) = note {
                            qnc_script_db::set_broll_note(target, block, note)?;
                        }
                        Ok(None)
                    })
                }
                None => return self.act(ADD_TEXT, target, (preview, timeline, timecode, transcript)),
            },
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
            session::OFF_RECORDING => self.sort_off_recording(preview, timeline),
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
                    self.view.editor_note = row.block.broll_note.clone();
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
                let Some(reading) = voice.takes.iter().find(|reading| reading.take == take).cloned() else { return Some(false) };
                let name = voice.name.clone();
                // A good reading opens with its IN and OUT where its sound is (user 2026-11-09);
                // one found in a recording made elsewhere opens on its range of that clip.
                let text = self.view.blocks.iter().find(|row| row.block.id == block).map(|row| row.block.text.clone()).unwrap_or_default();
                let fps = locator.settings().video_picture().map_or(0.0, |(_, _, fps)| fps);
                let place = qnc_script_draft::reading_place(&text, &voice, &reading, fps, &qnc_voice_take::clip_id);
                let clip = place.clip_id.clone();
                let fresh = self.heard_takes.insert(clip.clone(), 1).is_none();
                self.marks_after_write = Some((clip.clone(), place.from, place.to));
                self.voice_after_write = Some((clip, place.channel, fresh, place.from));
                self.write(preview, move |target| {
                    publish_reading(&locator, &name, &reading)?;
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
            SUGGEST_BROLL => {
                self.broll_asked = true;
                self.view.suggesting = true;
                self.view.message = qnc_ui_text::text("script.broll_suggesting").into();
                true
            }
            ACCEPT_BROLL => {
                let Some(proposal) = self.view.proposal.take() else { return Some(false) };
                self.view.message = qnc_ui_text::text("script.broll_accepted").into();
                self.write(preview, move |target| qnc_script_db::set_covers(target, proposal).map(|_| None))
            }
            DROP_BROLL => {
                self.view.proposal = None;
                true
            }
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
            OPEN if id().is_some_and(|id| self.edit.open(id)) => true,
            // A word of a statement: the statement opens (as a SubSegment, one part or more) and
            // the playhead goes to the word; an edge of its part taken goes to the word.
            WORD_CUE => {
                let mut parts = target.unwrap_or_default().split(':');
                let (Some(id), Some(index), Some(frame)) = (parts.next().and_then(|v| v.parse::<i64>().ok()), parts.next().and_then(|v| v.parse::<usize>().ok()), parts.next().and_then(|v| v.parse::<u64>().ok())) else { return Some(false) };
                let word_to = parts.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(frame + 1);
                self.edit.word_cue(id, index, frame, word_to)
            }
            // A cut by text: two words of one part set its IN and OUT (user 2026-11-04).
            WORD_CUT => {
                let mut parts = target.unwrap_or_default().split(':').map(|v| v.parse::<u64>().ok());
                let (Some(Some(id)), Some(Some(index)), Some(Some(from)), Some(Some(to))) = (parts.next(), parts.next(), parts.next(), parts.next()) else { return Some(false) };
                let (id, index) = (id as i64, index as usize);
                // The one meaning of two words (`qnc-edit-words`, AGENTS 0.12).
                match self.word_cut.give((id, index), qnc_edit_words::Word::new(from, to)) {
                    qnc_edit_words::Paired::Range(range) => {
                        let Some(block) = self.block(Some(id)).cloned() else { return Some(false) };
                        let pieces = view::block_pieces(&block);
                        let Some(piece) = pieces.get(index) else { return Some(false) };
                        let piece = qnc_subsegment_edit::trimmed_piece(&self.names, piece, range.from, range.to);
                        self.view.message = qnc_ui_text::text("script.word_cut_done").into();
                        self.write(preview, move |target| qnc_script_db::set_piece(target, id, index, piece).map(|_| None))
                    }
                    qnc_edit_words::Paired::First(_) => {
                        self.view.message = qnc_ui_text::text("script.word_cut_first").into();
                        true
                    }
                }
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
            smooth_frames: 0,
            smooth_clip: String::new(),
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
            a1_lead_frames: 0,
            a1_tail_frames: 0,
            v_lead_frames: 0,
            v_tail_frames: 0,
            level_keys: Vec::new(),
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
                if let (Ok(readings), Some(locator)) = (&finished, locator) {
                    for recorded in readings {
                        let name = qnc_script_db::take_name(&recorded.name, recorded.number);
                        if let Err(error) = qnc_voice_catalog::publish_record(locator.reader(), locator.settings(), &name, recorded.record.clone()) {
                            qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("voice-catalog {name} refused: {error}"));
                        }
                    }
                    session::wake_worker();
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
                    takes: voice.takes.last().map_or(0, |take| take.take),
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
        let mut drafts: Vec<(String, String)> = self.view.field_drafts.iter().filter(|((id, _), _)| *id == block).map(|((_, code), value)| (code.clone(), value.clone())).collect();
        self.view.field_drafts.retain(|(id, _), _| *id != block);
        // The B-roll note of an OFF is kept as typed, apart from its kind's fields.
        let note = drafts.iter().position(|(code, _)| code == BROLL_NOTE).map(|at| drafts.remove(at).1);
        let Some(stored) = self.blocks.iter_mut().find(|b| b.id == block) else { return false };
        let note = note.map(|note| note.trim().to_string()).filter(|note| *note != stored.broll_note);
        let mut fields = stored.fields.clone();
        for (code, value) in drafts {
            if value.trim().is_empty() {
                fields.remove(&code);
            } else {
                fields.insert(code, value.trim().to_string());
            }
        }
        let fields = (fields != stored.fields).then_some(fields);
        if fields.is_none() && note.is_none() {
            return true;
        }
        if let Some(fields) = &fields {
            stored.fields = fields.clone();
        }
        if let Some(note) = &note {
            stored.broll_note = note.clone();
        }
        self.write(preview, move |target| {
            if let Some(note) = note {
                qnc_script_db::set_broll_note(target, block, note)?;
            }
            if let Some(fields) = fields {
                qnc_script_db::set_fields(target, block, fields)?;
            }
            Ok(None)
        })
    }

    /// Seconds of `frames` of `clip_id` (0 when its rate is not known).
    fn seconds_of(&self, clip_id: &str, frames: u64) -> f64 {
        qnc_subsegment_edit::seconds_of(&self.names, clip_id, frames)
    }

    /// A write off the form thread; the script is read again when it is done.
    fn write(&mut self, preview: &SourcePreview, write: impl FnOnce(&ProjectDbTarget) -> Result<Option<i64>, String> + Send + 'static) -> bool {
        let Some(target) = preview.project_target().cloned() else {
            self.view.message = qnc_ui_text::text("script.no_project").into();
            return true;
        };
        self.queued.push_back((target, Box::new(write)));
        if self.writing.is_none() {
            self.write_next();
        }
        true
    }

    /// The next queued write goes, off the form thread.
    fn write_next(&mut self) {
        let Some((target, write)) = self.queued.pop_front() else { return };
        let (send, done) = mpsc::channel();
        if std::thread::Builder::new().name("qnc-script-write".into()).spawn(move || _ = send.send(write(&target))).is_ok() {
            self.writing = Some(done);
        }
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
        let project_id = locator.settings().project_id.clone();
        self.message_after_write = Some(qnc_ui_text::fill("script.sent_story", &[("n", &parts.len().to_string())]));
        self.view.message = qnc_ui_text::text("script.sending_story").into();
        self.write(preview, move |target| {
            for voice in &voices {
                for reading in &voice.takes {
                    publish_reading(&locator, &voice.name, reading)?;
                }
            }
            // The skeleton goes to S-Story as a new version (user 2026-10-31: "always a new
            // version"; every version is a copy of the Story of its own); neither the script nor
            // the Story changes.
            let mut segments = Vec::with_capacity(parts.len());
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
                let a2 = if part.kind == "offovi" { (a1.0, 0, true) } else { lane(kept.as_ref().map(|sound| &sound.a2), 1) };
                // A compound statement goes as one segment with its parts, each with its own A1
                // (user 2026-11-09); the story fills in the rate of each clip.
                let mut compound = Vec::with_capacity(part.parts.len());
                for piece in &part.parts {
                    let kept = qnc_clip_sound::read(target, &piece.clip_id)?.map_or((0, 0, false), |sound| (sound.a1.channel, sound.a1.level_db, sound.a1.off));
                    let sound = piece.a1.map_or(kept, |(channel, level, off)| (channel.unwrap_or(kept.0), level, off));
                    compound.push(qnc_program_input::compound_part(piece, (0, 0), sound));
                }
                let whole = !compound.is_empty();
                segments.push(qnc_program_db::AppendSegment {
                    kind: part.kind.into(),
                    clip_id: part.clip_id,
                    in_frame: if whole { 0 } else { part.from_frame },
                    out_frame: if whole { 0 } else { part.to_frame },
                    a1: if whole { (a1.0, 0, false) } else { a1 },
                    a2,
                    origin: part.origin,
                    marks: part.marks,
                    parts: compound,
                });
            }
            let version = qnc_skeleton_db::new_version(target)?;
            qnc_program_db::StoryWriter::start(target.draft_story(version))?.call(&qnc_program_db::Operation::AppendSegments { project_id, segments })?;
            Ok(None)
        })
    }

    fn reading_speed(&self) -> f64 {
        self.words_per_second.as_ref().copied().unwrap_or(0.0)
    }

    fn refresh_editor(&mut self) {
        self.view.editor_seconds = reading_seconds(&self.view.editor, self.reading_speed());
    }

    /// The clips whose transcripts the script shows (the clips of its statements): the one
    /// reader of transcripts (`qnc-clip-transcripts`) reads them for it.
    pub fn transcript_clips(&self) -> Vec<String> {
        let mut clips: Vec<String> = self.blocks.iter().flat_map(|block| view::block_pieces(block).into_iter().map(|piece| piece.clip_id)).filter(|clip| !clip.is_empty()).collect();
        clips.sort();
        clips.dedup();
        clips
    }

    /// Takes the answers of takes, exports, writes and reads, reads the script again now and
    /// then; the words of its statements are the transcripts `transcripts` read from the
    /// database. Returns whether the view changed.
    pub fn poll(&mut self, preview: &SourcePreview, timeline: &TimelineProjection, clips: &[ClipRow], transcripts: &qnc_clip_transcripts::ClipTranscripts) -> bool {
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
        self.session_answers(preview);
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
            row.voice = self.voices.iter().find(|(voice, _)| voice.block_id == row.block.id).map(|(voice, present)| {
                let mut shown = VoiceRow::of(voice, *present, &row.block.text);
                // What was read against the text as it stands (user 2026-11-08).
                shown.checks = voice
                    .takes
                    .iter()
                    .filter_map(|take| {
                        let heard = take.heard.as_ref()?;
                        let key = (voice.block_id, take.take);
                        if self.read_checks.get(&key).is_none_or(|(text, _)| *text != row.block.text) {
                            let words: Vec<&str> = heard.iter().map(|word| word.text.as_str()).collect();
                            self.read_checks.insert(key, (row.block.text.clone(), std::sync::Arc::new(qnc_read_check::check(&row.block.text, &words))));
                        }
                        Some((take.take, self.read_checks[&key].1.clone()))
                    })
                    .collect();
                shown
            });
            // The shots the M markers of a text make: their lengths, by what was heard or the
            // reading time.
            if !row.block.marks.is_empty() {
                let take = self.voices.iter().find(|(voice, _)| voice.block_id == row.block.id).and_then(|(voice, _)| voice.chosen_take());
                let check = row.voice.as_ref().and_then(|voice| voice.check_of(voice.chosen));
                let starts: Vec<f64> = take.and_then(|take| take.heard.as_ref()).map_or_else(Vec::new, |heard| heard.iter().map(|word| word.start).collect());
                let at = qnc_script_draft::mark_seconds(&row.block, check.map(|check| (check, starts.as_slice())), row.seconds);
                let ends = at.iter().copied().chain([row.seconds]);
                row.mark_lengths = std::iter::once(0.0).chain(at.iter().copied()).zip(ends).map(|(from, to)| (to - from).max(0.0)).collect();
            }
            // The words of a statement with their times (user 2026-11-04: edited by its text).
            if row.is_clip() {
                let names = &self.names;
                row.words = view::block_pieces(&row.block)
                    .iter()
                    .map(|piece| {
                        let fps = names.get(&piece.clip_id).filter(|(_, frames, seconds)| *frames > 0 && *seconds > 0.0).map_or(0, |(_, frames, seconds)| (*frames as f64 / seconds).round() as u32);
                        let start = qnc_source_timecode::parse_timecode(&piece.start_tc, fps).filter(|_| fps > 0);
                        let tc = |frame: u64| start.map_or_else(String::new, |start| qnc_source_timecode::format_frames(start + frame.saturating_sub(piece.from_frame), fps));
                        transcripts.of(&piece.clip_id).map_or_else(Vec::new, |transcripts| view::timed_words(transcripts, piece, self.letters.get(&piece.clip_id).map_or(&[][..], Vec::as_slice), tc))
                    })
                    .collect();
            }
        }
        // The letters of the words the script shows are laid in the background while the
        // player is idle (user 2026-11-05: "only on the text shown in the script").
        if let Some(locator) = preview.still_locator() {
            let needs = view::letter_needs(&self.view.blocks, &self.names);
            if self.letter_times.poll(&locator, &needs, !preview.player_view().playing()) {
                self.read_at = None; // the letters kept are read now
            }
            // A smooth cut made is on the join in the database: read it, and the open
            // SubSegment plays it.
            let joins = qnc_smooth_joins::wanted(&self.blocks);
            if self.smooth_joins.poll(&locator, &joins, !preview.player_view().playing()) {
                self.read_at = None;
            }
            // The words of a finished take are heard while nothing plays or records.
            let takes = qnc_take_heard::wanted(self.voices.iter().map(|(voice, _)| voice));
            let idle = !preview.player_view().playing() && self.take.is_none() && self.taking.is_none();
            if self.take_heard.poll(&locator, &takes, idle) {
                self.read_at = None;
            }
            self.view.read_note = self.take_heard.note().map(str::to_string);
        }
        let profile = self.view.profile.as_ref();
        let counts = |row: &BlockRow| profile.and_then(|profile| profile.kind(&row.block.kind)).map(|kind| kind.counts);
        let sum = |keep: &dyn Fn(&BlockRow) -> bool| self.view.blocks.iter().filter(|row| keep(row)).map(|row| row.seconds).sum::<f64>();
        self.view.read_seconds = sum(&|row| counts(row) == Some(Counts::Story) && !row.is_clip());
        self.view.clip_seconds = sum(&|row| counts(row) == Some(Counts::Story) && row.is_clip());
        self.view.anchor_seconds = sum(&|row| counts(row) == Some(Counts::Anchor));
        (self.view.in_cue, self.view.out_cue) = story_cues(&self.view.blocks);
        self.edit.sync(&self.blocks, &self.names, self.project.as_ref(), self.reading.is_none() && self.writing.is_none());
        self.edit.poll(preview);
        self.take_edit(preview);
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
                    TakeAnswer::Finished(Ok(readings)) => for Recorded { name, clip_uri, number, wav_uri, seconds, timecode, silent, .. } in readings {
                        // A take that heard nothing is kept, and said at once (user 2026-11-08).
                        if silent {
                            self.message_after_write = Some(qnc_notice::raised(qnc_ui_text::fill("script.take_silent", &[("take", &number.to_string())])));
                        }
                        let text = self.view.blocks.iter().find(|row| row.block.id == id).map(|row| row.block.text.clone()).unwrap_or_default();
                        let take = qnc_script_db::VoiceTake { take: number, wav_uri, seconds, tc: timecode, text, heard: None, source: None };
                        self.write(preview, move |target| qnc_script_db::add_voice_take(target, id, name, clip_uri, take).map(|_| None));
                    }
                    TakeAnswer::Finished(Err(error)) => {
                        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("script-record block={id} failed: {error}"));
                        self.view.message = qnc_notice::raised(error);
                    }
                    TakeAnswer::Cancelled(Ok(())) => self.view.message = qnc_ui_text::text("script.record_cancelled").into(),
                    TakeAnswer::Cancelled(Err(error)) => self.view.message = error,
                }
            }
        }
        self.view.recording = self.take.as_ref().map(|(id, take)| (*id, take.seconds(), take.take_peak()));
        self.view.recording_marks = self.take.as_ref().map_or(0, |(_, take)| take.marks());
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
                // More writes queued: the next goes; what follows the writes waits for the last.
                if !self.queued.is_empty() {
                    match result {
                        Ok(Some(id)) => self.view.selected = Some(id),
                        Ok(None) => {}
                        Err(error) => self.view.message = error,
                    }
                    self.writing = None;
                    self.write_next();
                } else {
                    let heard = self.voice_after_write.take();
                    let said = self.message_after_write.take();
                    match result {
                        Ok(added) => {
                            self.voice_request = heard;
                            if let Some(marks) = self.marks_after_write.take() {
                                self.source_request = Some(marks);
                            }
                            self.view.message = said.unwrap_or_default();
                            // A saved text leaves the editor empty (MA Script).
                            if std::mem::take(&mut self.clear_after_write) {
                                self.view.editor.clear();
                                self.view.editor_note.clear();
                                self.view.editing = None;
                            }
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
        }
        if let Some(reading) = &self.reading {
            if let Ok(result) = reading.try_recv() {
                match result {
                    Ok(read::Script { blocks, profile, target, title, voices, letters }) => {
                        self.letters = letters;
                        self.blocks = blocks;
                        self.view.profile = profile;
                        self.view.target_seconds = target;
                        self.view.title = title;
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
