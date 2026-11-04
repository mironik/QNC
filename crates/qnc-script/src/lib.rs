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
/// The playhead of the open SubSegment on frame `target` (a click on its timeline).
pub const SUBSEGMENT_CUE: &str = "subsegment_cue";
/// A part of the open SubSegment chosen, as a slot is (a click on its length strip): `target`
/// is its index. Ctrl+I / Ctrl+O and Ctrl+1 then take its edges and its A1, Delete takes it out.
pub const SUBSEGMENT_SELECT: &str = "subsegment_select";
/// A part taken whole, as a slot (a right click on its length strip, as Ctrl+S): `target` is its
/// index; arrows and the wheel then move it along the statement.
pub const SUBSEGMENT_TAKE: &str = "subsegment_take";
/// A click on a word of a statement in the script (user 2026-11-04: edited by its text): the
/// statement opens in place of the source timeline and the playhead goes to the word. `target`
/// is `block:part:frame`.
pub const WORD_CUE: &str = "script_word_cue";
/// A double click on a word of a statement: the first is where the part starts or ends, the
/// second on a word of the same part sets its IN at the earlier word's start and its OUT at
/// the later word's end (whichever is first). `target` is `block:part:from:to`.
pub const WORD_CUT: &str = "script_word_cut";
/// The pause before a part one frame longer or shorter: `target` is `block:part:1` or `-1`.
pub const JOIN_GAP: &str = "subsegment_join_gap";
/// The join before a part a cut or a crossfade: `target` is `block:part`.
pub const JOIN_CUT: &str = "subsegment_join_cut";
/// The SubSegment closes; the source timeline is back.
pub const SUBSEGMENT_CLOSE: &str = "subsegment_close";
/// An edge of a part is taken for trimming (a right click on its timeline): `target` is
/// `part:in` or `part:out`.
pub const TAKE_EDGE: &str = "subsegment_take_edge";
/// The taken edge dragged by the mouse to a frame of the SubSegment's axis: `target` is
/// `part:in:frame` or `part:out:frame`.
pub const DRAG_EDGE: &str = "subsegment_drag_edge";
/// The camera channel a part is heard on (A1): `target` is `block:part:channel` (zero based).
pub const PART_CHANNEL: &str = "subsegment_part_channel";
/// A part's A1 level one dB up or down: `target` is `block:part:1` or `-1`.
pub const PART_LEVEL: &str = "subsegment_part_level";
/// A part's A1 off or on: `target` is `block:part`.
pub const PART_OFF: &str = "subsegment_part_off";
/// A part changes places with the part before (`block:part:-1`) or after (`block:part:1`) it.
pub const PART_MOVE: &str = "subsegment_part_move";
/// A part leaves its SubSegment: `target` is `block:part`.
pub const PART_REMOVE: &str = "subsegment_part_remove";
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
    /// The clip block open as a SubSegment in place of the source timeline.
    open_subsegment: Option<i64>,
    /// The parts the player should play and the frame it should show (once).
    pieces_request: Option<(Vec<qnc_source_preview::PieceInput>, u64)>,
    /// The edge of a part being trimmed on the SubSegment timeline, not written yet.
    trim: Option<Trim>,
    /// The part of the open SubSegment chosen by a click on its length strip, as a slot is.
    chosen: Option<usize>,
    /// The sound of a join taken (Ctrl+1 with its whole cut taken, user 2026-11-03), as drafted.
    join_sound: Option<JoinSound>,
    /// The value of the action being handled (the frame of a click in the trim zoom).
    value: Option<i64>,
    /// The transcripts of the clips of the clip blocks, as last read.
    transcripts: HashMap<String, Vec<qnc_transcript_db::ClipTranscript>>,
    /// The first word of a cut by text (a double click on a word): its block, part and frames.
    word_cut: Option<(i64, usize, u64, u64)>,
    /// A part taken whole (Ctrl+S, a right click on its length strip), as a slot is (user
    /// 2026-11-04): its index and how far it is drafted to move (frames; later is positive).
    slide: Option<(usize, i64)>,
    /// The waves of the SubSegment's clips (read in the background).
    waves: qnc_program_waveform::ProgramWaves,
    /// The filmstrips of the SubSegment's clips (read in the background).
    filmstrips: qnc_program_filmstrip::ProgramFilmstrips,
    /// The project the waves read from.
    waves_project: Option<String>,
    /// The pictures, fine wave and words of the trim zoom.
    trim_stills: qnc_trim_stills::TrimStills,
    /// Where the trim zoom's playhead is: the play, and a pause until the cut moves.
    zoom_follow: qnc_program_view::ZoomFollow,
    /// The A1 of a part taken with Ctrl+1, as drafted.
    sound: Option<PartSound>,
    /// The voice records of the blocks as read, and whether their files are there.
    voices: Vec<(Voice, bool)>,
    /// A take to hear: its record's clip and the channel (zero based), once.
    voice_request: Option<(String, u16)>,
    /// The take to hear once the write in progress is done.
    voice_after_write: Option<(String, u16)>,
    /// What the panel says once the write in progress is done.
    message_after_write: Option<String>,
    /// The editor is emptied once the write in progress is done (a saved text).
    clear_after_write: bool,
}

/// The A1 of part `index` taken (Ctrl+1) as drafted: camera channel, level, off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PartSound {
    index: usize,
    channel: Option<u16>,
    level_db: i8,
    off: bool,
}

/// An edge of part `index` being trimmed: the OUT or the IN, and the part's range as drafted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Trim {
    index: usize,
    out: bool,
    from: u64,
    to: u64,
    /// The whole cut of a join taken (user 2026-11-03: edited as a cover's M marker is): the OUT
    /// of this part and the IN of the next move together; the next part's IN as drafted.
    roll: Option<u64>,
}

/// The J and L of the join before part `index` being moved: both sides together, or only the
/// J (the sound of the part after starts earlier) or only the L (the part before's goes on).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JoinSound {
    pub index: usize,
    pub lead: u64,
    pub tail: u64,
    pub side: JoinSide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinSide {
    Both,
    Lead,
    Tail,
}

/// Part `index` moved `by` frames along the statement as a slot moves with its two markers
/// (user 2026-11-04: all the slot's functions): it keeps its own picture and sound; the part
/// before plays on longer (its OUT later) or its pause grows, and the part after starts later in
/// its clip (its IN later) or its pause shrinks. The statement keeps its length.
fn slide_pieces(pieces: &mut [Piece], index: usize, by: i64) {
    if index == 0 || index + 1 >= pieces.len() {
        return;
    }
    let add = |value: u64, by: i64| (value as i64 + by).max(0) as u64;
    if pieces[index].gap_before == 0 {
        pieces[index - 1].to_frame = add(pieces[index - 1].to_frame, by);
    } else {
        pieces[index].gap_before = add(pieces[index].gap_before, by);
    }
    let next = &mut pieces[index + 1];
    if next.gap_before == 0 {
        next.from_frame = add(next.from_frame, by);
    } else {
        next.gap_before = add(next.gap_before, -by);
    }
}

/// How far part `index` can slide: the part before keeps a frame and stays in its clip
/// (`length` of a clip by its id), a pause never goes below none, the part after keeps a frame.
/// `None` for the first and the last part (a slot at the start or end marker does not move).
fn slide_range(pieces: &[Piece], index: usize, length: impl Fn(&str) -> u64) -> Option<(i64, i64)> {
    if index == 0 || index + 1 >= pieces.len() {
        return None;
    }
    let (before, this, next) = (&pieces[index - 1], &pieces[index], &pieces[index + 1]);
    let (mut low, mut high) = (i64::MIN, i64::MAX);
    if this.gap_before == 0 {
        low = low.max(before.from_frame as i64 + 1 - before.to_frame as i64);
        high = high.min(length(&before.clip_id) as i64 - before.to_frame as i64);
    } else {
        low = low.max(-(this.gap_before as i64));
    }
    if next.gap_before == 0 {
        low = low.max(-(next.from_frame as i64));
        high = high.min(next.to_frame as i64 - 1 - next.from_frame as i64);
    } else {
        high = high.min(next.gap_before as i64);
    }
    Some((low.min(0), high.max(0)))
}

/// The longest J or L of a join (frames); the sources limit it further where it plays.
const JOIN_SOUND_MAX: u64 = 10_000;

impl JoinSound {
    /// One step later (`step` > 0) or earlier: the sound cut moves (a J that shrinks becomes an
    /// L that grows); with both a J and an L the overlap moves whole; one side moves alone.
    fn moved(mut self, step: i64) -> Self {
        let shift = |value: u64, by: i64| (value as i64 + by).clamp(0, JOIN_SOUND_MAX as i64) as u64;
        match self.side {
            JoinSide::Lead => self.lead = shift(self.lead, -step),
            JoinSide::Tail => self.tail = shift(self.tail, step),
            JoinSide::Both if self.lead > 0 && self.tail > 0 => (self.lead, self.tail) = (shift(self.lead, -step), shift(self.tail, step)),
            JoinSide::Both => {
                let offset = self.tail as i64 - self.lead as i64 + step;
                (self.lead, self.tail) = ((-offset).clamp(0, JOIN_SOUND_MAX as i64) as u64, offset.clamp(0, JOIN_SOUND_MAX as i64) as u64);
            }
        }
        self
    }
}

impl Default for Script {
    fn default() -> Self {
        Self { view: ScriptView::default(), blocks: Vec::new(), project: None, reading: None, read_at: None, writing: None, source_request: None, names: HashMap::new(), words_per_second: Ok(0.0), take: None, taking: None, exporting: None, draft: Default::default(), open_subsegment: None, pieces_request: None, trim: None, chosen: None, join_sound: None, value: None, transcripts: HashMap::new(), word_cut: None, slide: None, waves: qnc_program_waveform::ProgramWaves::new(), filmstrips: qnc_program_filmstrip::ProgramFilmstrips::new(), waves_project: None, trim_stills: qnc_trim_stills::TrimStills::new(), zoom_follow: Default::default(), sound: None, voices: Vec::new(), voice_request: None, voice_after_write: None, message_after_write: None, clear_after_write: false }
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
        self.reading.is_some() || self.writing.is_some() || self.take.is_some() || self.taking.is_some() || self.exporting.is_some() || self.draft.running() || self.trim_stills.loading() || self.filmstrips.reading()
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
        self.pieces_request.take()
    }

    fn block(&self, id: Option<i64>) -> Option<&Block> {
        id.and_then(|id| self.blocks.iter().find(|block| block.id == id))
    }

    /// The open SubSegment as its block holds it now and the player plays it: a write that
    /// changed a pause or a join plays again where it stands; a block gone closes it.
    fn follow_subsegment(&mut self, preview: &SourcePreview) {
        let Some(block) = self.block(self.open_subsegment).cloned() else {
            if self.reading.is_none() && self.writing.is_none() {
                (self.open_subsegment, self.view.subsegment) = (None, None);
            }
            return;
        };
        let playing = preview.shown_pieces();
        if playing.is_none() && self.pieces_request.is_none() {
            // The player opened something else (a clip of the pool): the source timeline is back.
            (self.open_subsegment, self.view.subsegment) = (None, None);
            return;
        }
        let block = self.drafted(block);
        let inputs = view::piece_inputs(&block);
        if playing.is_some_and(|playing| playing != inputs.as_slice()) && self.pieces_request.is_none() {
            self.pieces_request = Some((inputs, preview.program_frame().unwrap_or(0)));
        }
        let playhead = playing.and_then(|_| preview.program_frame());
        let mut shown = view::subsegment_view(&block, playhead);
        if self.waves_project != self.project {
            self.waves.set_reader(preview.artifact_reader());
            self.filmstrips.set_reader(preview.artifact_reader());
            self.waves_project = self.project.clone();
        }
        let placements = view::subsegment_placements(&shown, &view::block_pieces(&block));
        let names = &self.names;
        for (part, piece) in shown.parts.iter_mut().zip(view::block_pieces(&block)) {
            part.channels = self.waves.channels(&piece.clip_id);
        }
        shown.a1 = self.waves.peaks(shown.total_frames, &placements, &[], |clip| names.get(clip).map(|(_, frames, _)| *frames)).a1;
        let at = view::subsegment_pictures(&shown, &view::block_pieces(&block));
        shown.pictures = self.filmstrips.pictures(&at, |clip| names.get(clip).map(|(_, frames, _)| *frames), |_| None);
        shown.trim = self.trim.and_then(|trim| shown.parts.get(trim.index).map(|part| (trim.index, trim.out, if trim.out { part.end.saturating_sub(1) } else { part.start })));
        // The trim zoom of the taken edge: its pictures, fine wave and words come from the
        // same pieces as a cover's (`qnc-trim-stills`).
        let pieces = view::block_pieces(&block);
        let rate = pieces.first().and_then(|piece| names.get(&piece.clip_id)).filter(|(_, _, seconds)| *seconds > 0.0).map_or(25, |(_, frames, seconds)| (*frames as f64 / seconds).round() as u64);
        let zoom = self.trim.and_then(|trim| view::subsegment_zoom(&shown, &pieces, (trim.index, trim.out), (rate, Some((&self.zoom_follow, preview.playing_at().filter(|(program, _, _)| *program).map(|(_, frame, playing)| (frame, playing))))), |clip| names.get(clip).map(|(_, frames, _)| *frames)));
        // A whole cut taken: the zoom's A1 takes its sound (its label, a right click), and the
        // taken join sound shows its J and L there, red, moved by click and drag (user
        // 2026-11-03: the same zoom as a story's cut).
        let joined = self.trim.filter(|trim| trim.roll.is_some()).map(|trim| trim.index + 1);
        let join_sound = self.join_sound;
        let zoom = zoom.map(|mut zoom| {
            if let (Some(index), Some(sound)) = (joined.or(join_sound.map(|join| join.index)), zoom.zoom_sound.as_mut()) {
                let marker_id = format!("join:{index}");
                sound.taken_cut = Some(marker_id.clone());
                if let Some(join) = join_sound {
                    let (lead, tail) = (sound.cut.saturating_sub(join.lead), sound.cut + join.tail);
                    let moving = match join.side {
                        JoinSide::Both => vec![lead, tail],
                        JoinSide::Lead => vec![lead],
                        JoinSide::Tail => vec![tail],
                    };
                    sound.cut_sound = Some(qnc_program_view::ZoomCutSound { marker_id, lane: 0, lead: Some(lead), tail: Some(tail), moving });
                }
            }
            zoom
        });
        shown.zoom = zoom.map(|mut zoom| {
            self.trim_stills.fill(&mut zoom, preview.still_locator(), None);
            Box::new(zoom)
        });
        shown.sound = self.sound.map(|sound| sound.index);
        shown.roll = self.trim.is_some_and(|trim| trim.roll.is_some());
        shown.chosen = self.chosen.filter(|index| *index < shown.parts.len());
        shown.join_sound = self.join_sound.map(|join| (join.index, join.side));
        shown.slide = self.slide.map(|(index, _)| index);
        shown.rate = pieces.first().and_then(|piece| names.get(&piece.clip_id)).filter(|(_, frames, seconds)| *seconds > 0.0 && *frames > 0).map(|(_, frames, seconds)| ((*frames as f64 / seconds).round() as u32, 1));
        self.view.subsegment = Some(shown);
    }

    /// The block with the part being trimmed as drafted (the player plays the draft, user
    /// 2026-10-29: edited as a cover is, heard before Enter).
    fn drafted(&self, mut block: Block) -> Block {
        if let Some(trim) = self.trim {
            if block.pieces.len() > trim.index {
                (block.pieces[trim.index].from_frame, block.pieces[trim.index].to_frame) = (trim.from, trim.to);
            }
            if let (Some(from), Some(next)) = (trim.roll, block.pieces.get_mut(trim.index + 1)) {
                next.from_frame = from;
            }
        }
        if let Some(join) = self.join_sound {
            if let Some(piece) = block.pieces.get_mut(join.index) {
                (piece.a1_lead_frames, piece.a1_tail_frames) = (join.lead, join.tail);
            }
        }
        if let Some((index, by)) = self.slide {
            slide_pieces(&mut block.pieces, index, by);
        }
        if let Some(sound) = self.sound {
            if let Some(piece) = block.pieces.get_mut(sound.index) {
                (piece.a1_channel, piece.a1_level_db, piece.a1_off) = (sound.channel, sound.level_db, sound.off);
            }
        }
        block
    }

    /// The sound keys of a part, as a cover's lane (user 2026-10-29): Ctrl+1 takes the A1 of the
    /// part under the playhead, up/down its camera channel, left off and right on, Ctrl+arrows
    /// and Ctrl+wheel its level a dB, Enter keeps it, Esc gives it up. Every key plays the
    /// draft at once. `None` for a key that is not the sound's.
    fn sound_key(&mut self, action_id: &str, target: Option<&str>, block: &Block, pieces: &[Piece], preview: &SourcePreview) -> Option<bool> {
        let key = qnc_timeline::LaneKey::of(action_id)?;
        let at = self.view.subsegment.as_ref().and_then(|view| view.playhead).unwrap_or(0);
        match key {
            qnc_timeline::LaneKey::Take(0) => {
                // A right click on A1 names the part it was on (user 2026-11-04, as a slot's sound).
                let index = target.and_then(|index| index.parse::<usize>().ok()).filter(|index| *index < pieces.len()).unwrap_or_else(|| self.part_in_hand());
                let piece = pieces.get(index)?;
                (self.trim, self.slide) = (None, None);
                self.sound = Some(PartSound { index, channel: piece.a1_channel, level_db: piece.a1_level_db, off: piece.a1_off });
                return Some(true);
            }
            qnc_timeline::LaneKey::Take(_) | qnc_timeline::LaneKey::Pick(_) => return None,
            _ => {}
        }
        let mut sound = self.sound?;
        let channels = self.waves.channels(&pieces.get(sound.index)?.clip_id).unwrap_or(2).max(1);
        match key {
            qnc_timeline::LaneKey::Channel(delta) => sound.channel = Some((i64::from(sound.channel.unwrap_or(0)) + delta).clamp(0, i64::from(channels) - 1) as u16),
            qnc_timeline::LaneKey::Off(off) => sound.off = off,
            qnc_timeline::LaneKey::Level(delta) => sound.level_db = (i16::from(sound.level_db) + i16::from(delta)).clamp(-40, 12) as i8,
            qnc_timeline::LaneKey::Keep => {
                self.sound = None;
                let mut piece = pieces.get(sound.index)?.clone();
                (piece.a1_channel, piece.a1_level_db, piece.a1_off) = (sound.channel, sound.level_db, sound.off);
                let id = block.id;
                return Some(self.write(preview, move |target| qnc_script_db::set_piece(target, id, sound.index, piece).map(|_| None)));
            }
            qnc_timeline::LaneKey::Drop => {
                self.sound = None;
                self.pieces_request = Some((view::piece_inputs(block), at));
                return Some(true);
            }
            _ => return None,
        }
        self.sound = Some(sound);
        self.pieces_request = Some((view::piece_inputs(&self.drafted(block.clone())), at));
        Some(true)
    }

    /// The trim keys of the open SubSegment (user 2026-10-29: its parts are edited as covers
    /// are): Ctrl+I / Ctrl+O take the IN or OUT of the part under the playhead (a right click
    /// on a join takes the IN after it), arrows and the wheel move it a frame, Enter keeps it,
    /// Esc gives it up. Every move plays the draft at the edge. `None` for any other key.
    fn trim_key(&mut self, action_id: &str, target: Option<&str>, preview: &SourcePreview) -> Option<bool> {
        let block = self.block(self.open_subsegment)?.clone();
        let pieces = view::block_pieces(&block);
        let take = |index: usize, out: bool| pieces.get(index).map(|piece| Trim { index, out, from: piece.from_frame, to: piece.to_frame, roll: None });
        if let Some(done) = self.slide_key(action_id, target, &block, &pieces, preview) {
            return Some(done);
        }
        if let Some(done) = self.join_key(action_id, &block, &pieces, preview) {
            return Some(done);
        }
        if let Some(done) = self.sound_key(action_id, target, &block, &pieces, preview) {
            return Some(done);
        }
        if action_id == DRAG_EDGE {
            return self.drag_edge(target, &block, &pieces);
        }
        // A part chosen as a slot is (user 2026-11-03: a click on its length strip): the
        // playhead on its IN; Esc lets it go, Delete takes it out of the statement.
        match action_id {
            SUBSEGMENT_SELECT => {
                let index = target.and_then(|index| index.parse::<usize>().ok()).filter(|index| *index < pieces.len())?;
                self.chosen = Some(index);
                let start = self.view.subsegment.as_ref().and_then(|view| view.parts.get(index)).map_or(0, |part| part.start);
                self.pieces_request = Some((view::piece_inputs(&block), start));
                return Some(true);
            }
            "clear_focus" if self.chosen.is_some() && self.trim.is_none() && self.sound.is_none() => {
                self.chosen = None;
                return Some(true);
            }
            "delete_marker" if self.trim.is_none() && self.sound.is_none() => {
                let index = self.chosen?;
                if pieces.len() < 2 {
                    self.view.message = qnc_ui_text::text("db.script_last_piece").into();
                    return Some(true);
                }
                self.chosen = None;
                let id = block.id;
                return Some(self.write(preview, move |target| qnc_script_db::remove_piece(target, id, index).map(|_| None)));
            }
            _ => {}
        }
        let step = match action_id {
            "select_mark_in" | "select_mark_out" => {
                let index = self.part_in_hand();
                self.slide = None;
                self.sound = None;
                self.trim = take(index, action_id == "select_mark_out");
                0
            }
            TAKE_EDGE => {
                let mut parts = target.unwrap_or_default().split(':');
                let index = parts.next().and_then(|index| index.parse::<usize>().ok())?;
                let side = parts.next();
                let (out, cut) = (side != Some("in"), side == Some("cut"));
                // The whole cut of a join: this part's OUT with the next part's IN.
                let take = |index: usize, out: bool| {
                    let trim = take(index, out)?;
                    Some(if cut { Trim { roll: Some(pieces.get(index + 1)?.from_frame), ..trim } } else { trim })
                };
                // Another edge taken (a picture of the zoom clicked, user 2026-10-29): a moved
                // edge is kept first, the same part keeps its drafted range.
                let new = match self.trim.take() {
                    Some(old) if old.index == index && !cut && old.roll.is_none() => Some(Trim { out, ..old }),
                    Some(old) => {
                        self.write_trim(preview, &block, &pieces, old);
                        take(index, out)
                    }
                    None => take(index, out),
                };
                (self.sound, self.join_sound, self.slide) = (None, None, None);
                self.trim = new;
                0
            }
            "step_back_frame" | "step_forward_frame" if self.trim.is_some() => if action_id == "step_forward_frame" { 1 } else { -1 },
            "wheel_source_back" | "wheel_source_forward" if self.trim.is_some() => if qnc_wheel_jog::source_step_forward(action_id)? { 1 } else { -1 },
            "activate_focused_item" if self.trim.is_some() => {
                let trim = self.trim.take()?;
                return Some(self.write_trim(preview, &block, &pieces, trim));
            }
            "clear_focus" if self.trim.is_some() => {
                self.trim = None;
                self.pieces_request = Some((view::piece_inputs(&block), self.edge_frame(&block).unwrap_or(0)));
                return Some(true);
            }
            _ => return None,
        };
        let mut trim = self.trim?;
        match trim.roll {
            Some(_) => {
                let to = trim.to.saturating_add_signed(step);
                self.roll_to(&mut trim, &pieces, to);
            }
            None => {
                let length = self.names.get(&pieces[trim.index].clip_id).map_or(u64::MAX, |(_, frames, _)| *frames);
                match trim.out {
                    true => trim.to = trim.to.saturating_add_signed(step).clamp(trim.from + 1, length),
                    false => trim.from = trim.from.saturating_add_signed(step).min(trim.to - 1),
                }
            }
        }
        self.trim = Some(trim);
        let drafted = self.drafted(block);
        self.pieces_request = Some((view::piece_inputs(&drafted), self.edge_frame(&drafted).unwrap_or(0)));
        Some(true)
    }

    /// The taken edge dragged by the mouse (user 2026-10-29): only the taken one moves (the
    /// mouse rule: a drag drags what is taken), its part keeps at least a frame and its clip;
    /// the draft plays at the edge as with the arrows. Letting go keeps it (as Enter).
    fn drag_edge(&mut self, target: Option<&str>, block: &Block, pieces: &[Piece]) -> Option<bool> {
        let mut parts = target?.split(':');
        let (index, out, frame) = (parts.next()?.parse::<usize>().ok()?, parts.next()? == "out", parts.next()?.parse::<u64>().ok()?);
        let mut trim = self.trim.filter(|trim| trim.index == index && trim.out == out)?;
        // The part's start on the axis does not move while its own edges move.
        let start = view::subsegment_view(block, None).parts.get(index)?.start;
        let piece = pieces.get(index)?;
        let length = self.names.get(&piece.clip_id).map_or(u64::MAX, |(_, frames, _)| *frames);
        match (out, trim.roll) {
            (true, Some(_)) => {
                let to = trim.from + frame.saturating_sub(start);
                self.roll_to(&mut trim, pieces, to);
            }
            (true, None) => trim.to = (trim.from + frame.saturating_sub(start)).clamp(trim.from + 1, length),
            (false, _) => trim.from = (piece.from_frame as i64 + frame as i64 - start as i64).clamp(0, trim.to as i64 - 1) as u64,
        }
        self.trim = Some(trim);
        let drafted = self.drafted(block.clone());
        self.pieces_request = Some((view::piece_inputs(&drafted), self.edge_frame(&drafted).unwrap_or(0)));
        Some(true)
    }

    /// Opens `block` as a SubSegment in place of the source timeline (its parts one after
    /// another; a statement of one range is one part), the playhead at `at` on its axis.
    fn open_as_subsegment(&mut self, block: &Block, at: u64) {
        self.open_subsegment = Some(block.id);
        (self.chosen, self.join_sound, self.trim, self.sound, self.slide) = (None, None, None, None, None);
        self.pieces_request = Some((view::piece_inputs(block), at));
        self.view.subsegment = Some(view::subsegment_view(block, None));
    }

    /// A part taken whole as a slot is (user 2026-11-04): Ctrl+S takes the chosen part (else
    /// the one under the playhead), a right click on its length strip takes that one; arrows and
    /// the wheel move it a frame along the statement with its neighbours' edges (heard at once),
    /// Enter or a left click keeps it, Esc gives it up. `None` for a key that is not the slide's.
    fn slide_key(&mut self, action_id: &str, target: Option<&str>, block: &Block, pieces: &[Piece], preview: &SourcePreview) -> Option<bool> {
        let length = |clip: &str| self.names.get(clip).map_or(u64::MAX / 2, |(_, frames, _)| *frames);
        if action_id == "select_current_marker_slot" || action_id == SUBSEGMENT_TAKE {
            let index = target.and_then(|index| index.parse::<usize>().ok()).unwrap_or_else(|| self.part_in_hand());
            if slide_range(pieces, index, length).is_none() {
                self.view.message = qnc_ui_text::text("subsegment.slide_locked").into();
                return Some(true);
            }
            (self.trim, self.sound, self.join_sound, self.chosen) = (None, None, None, Some(index));
            self.slide = Some((index, 0));
            return Some(true);
        }
        let (index, by) = self.slide?;
        let step = match action_id {
            "step_back_frame" => -1,
            "step_forward_frame" => 1,
            "wheel_source_back" | "wheel_source_forward" => if qnc_wheel_jog::source_step_forward(action_id)? { 1 } else { -1 },
            "activate_focused_item" => {
                self.slide = None;
                let mut moved = pieces.to_vec();
                slide_pieces(&mut moved, index, by);
                let id = block.id;
                let mut writes = Vec::new();
                for at in [index - 1, index + 1] {
                    if (moved[at].from_frame, moved[at].to_frame) != (pieces[at].from_frame, pieces[at].to_frame) {
                        let trim = Trim { index: at, out: at < index, from: moved[at].from_frame, to: moved[at].to_frame, roll: None };
                        writes.push((at, Some(self.trimmed_piece(&pieces[at], trim)), None));
                    }
                }
                for at in [index, index + 1] {
                    if moved[at].gap_before != pieces[at].gap_before {
                        writes.push((at, None, Some((moved[at].gap_before, moved[at].cut))));
                    }
                }
                if writes.is_empty() {
                    return Some(true);
                }
                return Some(self.write(preview, move |target| {
                    for (at, piece, join) in writes {
                        if let Some(piece) = piece {
                            qnc_script_db::set_piece(target, id, at, piece)?;
                        }
                        if let Some((gap, cut)) = join {
                            qnc_script_db::set_join(target, id, at, gap, cut)?;
                        }
                    }
                    Ok(None)
                }));
            }
            "clear_focus" => {
                self.slide = None;
                self.pieces_request = Some((view::piece_inputs(block), self.view.subsegment.as_ref().and_then(|view| view.playhead).unwrap_or(0)));
                return Some(true);
            }
            _ => return None,
        };
        let (low, high) = slide_range(pieces, index, length)?;
        self.slide = Some((index, (by + step).clamp(low, high)));
        let drafted = self.drafted(block.clone());
        let start = view::subsegment_view(&drafted, None).parts.get(index).map_or(0, |part| part.start);
        self.pieces_request = Some((view::piece_inputs(&drafted), start));
        Some(true)
    }

    /// The value of the next action (a frame of the trim zoom's click or drag).
    pub fn set_value(&mut self, value: Option<i64>) {
        self.value = value;
    }

    /// The J and L of a join (user 2026-11-03), as at a story's cut: with the whole cut of a join
    /// taken, Ctrl+1 takes its sound (both sides); then Ctrl+I only the J (the part after's sound
    /// starts earlier), Ctrl+O only the L (the part before's goes on), Ctrl+1 both again; arrows
    /// and the wheel move it a frame and it is heard at once; Enter (or a left click) keeps it,
    /// Esc gives it up. `None` for a key that is not the join sound's.
    fn join_key(&mut self, action_id: &str, block: &Block, pieces: &[Piece], preview: &SourcePreview) -> Option<bool> {
        let take_sound = matches!(qnc_timeline::LaneKey::of(action_id), Some(qnc_timeline::LaneKey::Take(0)));
        let mut join = match self.join_sound {
            Some(join) => join,
            None => {
                let trim = self.trim.filter(|trim| trim.roll.is_some() && take_sound)?;
                let index = trim.index + 1;
                let piece = pieces.get(index)?;
                if pieces.get(trim.index).is_some_and(|before| (before.from_frame, before.to_frame) != (trim.from, trim.to)) {
                    self.write_trim(preview, block, pieces, trim);
                }
                self.sound = None;
                self.join_sound = Some(JoinSound { index, lead: piece.a1_lead_frames, tail: piece.a1_tail_frames, side: JoinSide::Both });
                return Some(true);
            }
        };
        let step = match action_id {
            _ if take_sound => {
                join.side = JoinSide::Both;
                0
            }
            "select_mark_in" => {
                join.side = JoinSide::Lead;
                0
            }
            "select_mark_out" => {
                join.side = JoinSide::Tail;
                0
            }
            "step_back_frame" => -1,
            "step_forward_frame" => 1,
            "wheel_source_back" | "wheel_source_forward" => if qnc_wheel_jog::source_step_forward(action_id)? { 1 } else { -1 },
            // In the zoom (the same system actions as on a story's row): a click near a side takes
            // that side, a drag moves the moving side to the frame.
            "timeline_take_cut_edge" | "timeline_drag_cut_edge" => {
                let frame = u64::try_from(self.value?).ok()?;
                let cut = view::subsegment_view(&self.drafted(block.clone()), None).parts.get(join.index)?.start;
                let (lead_at, tail_at) = (cut.saturating_sub(join.lead), cut + join.tail);
                if action_id == "timeline_take_cut_edge" {
                    join.side = if frame.abs_diff(lead_at) <= frame.abs_diff(tail_at) { JoinSide::Lead } else { JoinSide::Tail };
                    self.join_sound = Some(join);
                    return Some(true);
                }
                let to = |from: u64| frame as i64 - from as i64;
                match join.side {
                    JoinSide::Lead => to(lead_at),
                    JoinSide::Tail | JoinSide::Both => to(tail_at),
                }
            }
            "activate_focused_item" => {
                (self.join_sound, self.trim) = (None, None);
                let (id, index) = (block.id, join.index);
                return Some(self.write(preview, move |target| qnc_script_db::set_join_sound(target, id, index, join.lead, join.tail).map(|_| None)));
            }
            "clear_focus" => {
                self.join_sound = None;
                self.pieces_request = Some((view::piece_inputs(block), self.join_hear_from(block, join.index)));
                return Some(true);
            }
            _ => return None,
        };
        self.join_sound = Some(join.moved(step));
        if step != 0 {
            let drafted = self.drafted(block.clone());
            self.pieces_request = Some((view::piece_inputs(&drafted), self.join_hear_from(&drafted, join.index)));
        }
        Some(true)
    }

    /// Where a join sound is heard from: a second before its cut.
    fn join_hear_from(&self, block: &Block, index: usize) -> u64 {
        let at = view::subsegment_view(block, None).parts.get(index).map_or(0, |part| part.start);
        let rate = self.view.subsegment.as_ref().and_then(|view| view.rate).map_or(25, |(num, den)| u64::from(num / den.max(1)));
        at.saturating_sub(rate)
    }

    /// The part Ctrl+I / Ctrl+O and Ctrl+1 work on: the chosen one, else the one under the
    /// playhead (as a slot: the chosen slot, else the slot under the Wrap playhead).
    fn part_in_hand(&self) -> usize {
        let view = self.view.subsegment.as_ref();
        let count = view.map_or(0, |view| view.parts.len());
        match self.chosen.filter(|index| *index < count) {
            Some(index) => index,
            None => {
                let at = view.and_then(|view| view.playhead).unwrap_or(0);
                view.and_then(|view| view.parts.iter().position(|part| at < part.end)).unwrap_or(0)
            }
        }
    }

    /// The whole cut of a join moved so this part's OUT is `to`: the next part's IN moves as far,
    /// each within its own clip and keeping at least a frame; a move one of them cannot take is
    /// taken only as far as both can (a roll, the length of the statement stays).
    fn roll_to(&self, trim: &mut Trim, pieces: &[Piece], to: u64) {
        let (Some(next_from), Some(next)) = (trim.roll, pieces.get(trim.index + 1)) else { return };
        let length = self.names.get(&pieces[trim.index].clip_id).map_or(u64::MAX, |(_, frames, _)| *frames);
        let wanted = to as i64 - trim.to as i64;
        let earliest = (trim.from as i64 + 1 - trim.to as i64).max(-(next_from as i64));
        let latest = (length as i64 - trim.to as i64).min(next.to_frame as i64 - 1 - next_from as i64);
        let moved = wanted.clamp(earliest.min(0), latest.max(0));
        trim.to = (trim.to as i64 + moved) as u64;
        trim.roll = Some((next_from as i64 + moved) as u64);
    }

    /// Writes the parts a trim moved (the part, and the next one of a whole cut); nothing when
    /// nothing moved. Returns whether a write went.
    fn write_trim(&mut self, preview: &SourcePreview, block: &Block, pieces: &[Piece], trim: Trim) -> bool {
        let id = block.id;
        let mut writes = Vec::new();
        if pieces.get(trim.index).is_some_and(|piece| (piece.from_frame, piece.to_frame) != (trim.from, trim.to)) {
            writes.push((trim.index, self.trimmed_piece(&pieces[trim.index], trim)));
        }
        if let (Some(from), Some(next)) = (trim.roll, pieces.get(trim.index + 1)) {
            if from != next.from_frame {
                writes.push((trim.index + 1, self.trimmed_piece(next, Trim { index: trim.index + 1, out: false, from, to: next.to_frame, roll: None })));
            }
        }
        if writes.is_empty() {
            return true;
        }
        self.write(preview, move |target| {
            for (index, piece) in writes {
                qnc_script_db::set_piece(target, id, index, piece)?;
            }
            Ok(None)
        })
    }

    /// The frame of the trimmed edge on the SubSegment's axis (the last frame of an OUT).
    fn edge_frame(&self, block: &Block) -> Option<u64> {
        let trim = self.trim?;
        let shown = view::subsegment_view(block, None);
        let part = shown.parts.get(trim.index)?;
        Some(if trim.out { part.end.saturating_sub(1) } else { part.start })
    }

    /// The part with its trimmed range: its timecodes and length moved with it.
    fn trimmed_piece(&self, piece: &Piece, trim: Trim) -> Piece {
        let fps = self.names.get(&piece.clip_id).filter(|(_, _, seconds)| *seconds > 0.0).map_or(0, |(_, frames, seconds)| (*frames as f64 / seconds).round() as u32);
        let moved = |label: &str, from: u64, to: u64| match qnc_source_timecode::parse_timecode(label, fps).filter(|_| fps > 0) {
            Some(frames) => qnc_source_timecode::format_frames((frames + to).saturating_sub(from), fps),
            None => label.to_string(),
        };
        Piece {
            from_frame: trim.from,
            to_frame: trim.to,
            start_tc: moved(&piece.start_tc, piece.from_frame, trim.from),
            end_tc: moved(&piece.end_tc, piece.to_frame, trim.to),
            length: if fps > 0 { qnc_source_timecode::format_frames(trim.to - trim.from, fps) } else { (trim.to - trim.from).to_string() },
            seconds: self.seconds_of(&piece.clip_id, trim.to - trim.from),
            ..piece.clone()
        }
    }

    /// The kind `code` of the project's profile.
    fn kind(&self, code: &str) -> Option<&KindSpec> {
        self.view.profile.as_ref()?.kind(code)
    }

    /// The script's own actions; `None` for any other.
    pub fn act(&mut self, action_id: &str, target: Option<&str>, (preview, timeline, timecode, transcript): (&SourcePreview, &TimelineProjection, Option<&SourceTimecode>, &TranscriptView)) -> Option<bool> {
        // While a SubSegment is open in place of the source timeline its trim keys are its own.
        if self.open_subsegment.is_some() {
            let done = self.trim_key(action_id, target, preview);
            if qnc_dev_diagnostics::player_diagnostics_enabled() {
                let trim = self.trim.map(|trim| format!("{}:{}{}", trim.index, if trim.out { "out" } else { "in" }, if trim.roll.is_some() { ":cut" } else { "" }));
                qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("subsegment-key action={action_id} target={target:?} done={done:?} trim={trim:?} sound={:?}", self.sound.map(|sound| sound.index)));
            }
            if let Some(done) = done {
                return Some(done);
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
            SAVE_TEXT => match self.view.editing {
                Some(block) => {
                    let text = self.view.editor.trim().to_string();
                    if text.is_empty() {
                        self.view.message = qnc_ui_text::text("script.write_first").into();
                        return Some(true);
                    }
                    self.clear_after_write = true;
                    self.write(preview, move |target| qnc_script_db::set_text(target, block, text).map(|_| None))
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
                self.open_as_subsegment(&block, 0);
                true
            }
            // A word of a statement: the statement opens (as a SubSegment, one part or more) and
            // the playhead goes to the word.
            WORD_CUE => {
                let mut parts = target.unwrap_or_default().split(':');
                let (Some(id), Some(index), Some(frame)) = (parts.next().and_then(|v| v.parse::<i64>().ok()), parts.next().and_then(|v| v.parse::<usize>().ok()), parts.next().and_then(|v| v.parse::<u64>().ok())) else { return Some(false) };
                let Some(block) = self.block(Some(id)).cloned() else { return Some(false) };
                let pieces = view::block_pieces(&block);
                let (Some(piece), Some(part)) = (pieces.get(index), view::subsegment_view(&block, None).parts.get(index).cloned()) else { return Some(false) };
                let at = part.start + frame.saturating_sub(piece.from_frame).min(part.end.saturating_sub(part.start + 1));
                if self.open_subsegment == Some(id) {
                    self.pieces_request = Some((view::piece_inputs(&block), at));
                } else {
                    self.open_as_subsegment(&block, at);
                }
                true
            }
            // A cut by text: two words of one part set its IN and OUT (user 2026-11-04).
            WORD_CUT => {
                let mut parts = target.unwrap_or_default().split(':').map(|v| v.parse::<u64>().ok());
                let (Some(Some(id)), Some(Some(index)), Some(Some(from)), Some(Some(to))) = (parts.next(), parts.next(), parts.next(), parts.next()) else { return Some(false) };
                let (id, index) = (id as i64, index as usize);
                match self.word_cut.take() {
                    Some((first_id, first_index, first_from, first_to)) if (first_id, first_index) == (id, index) => {
                        let Some(block) = self.block(Some(id)).cloned() else { return Some(false) };
                        let pieces = view::block_pieces(&block);
                        let Some(piece) = pieces.get(index) else { return Some(false) };
                        let trim = Trim { index, out: true, from: first_from.min(from), to: first_to.max(to), roll: None };
                        if trim.to <= trim.from {
                            return Some(false);
                        }
                        let piece = self.trimmed_piece(piece, trim);
                        self.view.message = qnc_ui_text::text("script.word_cut_done").into();
                        self.write(preview, move |target| qnc_script_db::set_piece(target, id, index, piece).map(|_| None))
                    }
                    _ => {
                        self.word_cut = Some((id, index, from, to));
                        self.view.message = qnc_ui_text::text("script.word_cut_first").into();
                        true
                    }
                }
            }
            SUBSEGMENT_CUE => {
                let (Some(block), Some(frame)) = (self.block(self.open_subsegment), target.and_then(|frame| frame.parse::<u64>().ok())) else { return Some(false) };
                self.pieces_request = Some((view::piece_inputs(block), frame));
                // A click off the length strip only places the playhead: a chosen part is let go.
                self.chosen = None;
                true
            }
            SUBSEGMENT_CLOSE => {
                (self.open_subsegment, self.view.subsegment) = (None, None);
                true
            }
            // A part's own A1 (user 2026-10-29): written at once, the SubSegment plays it again.
            PART_CHANNEL | PART_LEVEL | PART_OFF => {
                let mut parts = target.unwrap_or_default().split(':');
                let (Some(id), Some(index)) = (parts.next().and_then(|id| id.parse::<i64>().ok()), parts.next().and_then(|index| index.parse::<usize>().ok())) else { return Some(false) };
                let Some(mut piece) = self.block(Some(id)).map(view::block_pieces).and_then(|pieces| pieces.get(index).cloned()) else { return Some(false) };
                let value = parts.next().and_then(|value| value.parse::<i64>().ok()).unwrap_or(0);
                match action_id {
                    PART_CHANNEL => piece.a1_channel = u16::try_from(value).ok(),
                    PART_LEVEL => piece.a1_level_db = (i64::from(piece.a1_level_db) + value).clamp(-40, 12) as i8,
                    _ => piece.a1_off = !piece.a1_off,
                }
                self.write(preview, move |target| qnc_script_db::set_piece(target, id, index, piece).map(|_| None))
            }
            // A part's place and its leaving (user 2026-10-29): written at once, the SubSegment
            // plays it again; a taken edge or sound is let go (its part may have moved).
            PART_MOVE | PART_REMOVE => {
                let mut parts = target.unwrap_or_default().split(':');
                let (Some(id), Some(index)) = (parts.next().and_then(|id| id.parse::<i64>().ok()), parts.next().and_then(|index| index.parse::<usize>().ok())) else { return Some(false) };
                (self.trim, self.sound, self.chosen, self.join_sound, self.slide) = (None, None, None, None, None);
                if action_id == PART_REMOVE {
                    return Some(self.write(preview, move |target| qnc_script_db::remove_piece(target, id, index).map(|_| None)));
                }
                let earlier = parts.next().and_then(|step| step.parse::<i64>().ok()).unwrap_or(0) < 0;
                self.write(preview, move |target| qnc_script_db::move_piece(target, id, index, earlier).map(|_| None))
            }
            JOIN_GAP | JOIN_CUT => {
                let mut parts = target.unwrap_or_default().split(':');
                let (Some(id), Some(index)) = (parts.next().and_then(|id| id.parse::<i64>().ok()), parts.next().and_then(|index| index.parse::<usize>().ok())) else { return Some(false) };
                let Some(piece) = self.block(Some(id)).and_then(|block| block.pieces.get(index)).cloned() else { return Some(false) };
                let (gap, cut) = match action_id {
                    JOIN_GAP => (piece.gap_before.saturating_add_signed(parts.next().and_then(|step| step.parse::<i64>().ok()).unwrap_or(0)), piece.cut),
                    _ => (piece.gap_before, !piece.cut),
                };
                self.write(preview, move |target| qnc_script_db::set_join(target, id, index, gap, cut).map(|_| None))
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
            a1_lead_frames: 0,
            a1_tail_frames: 0,
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
        let project_id = locator.settings().project_id.clone();
        self.message_after_write = Some(qnc_ui_text::fill("script.sent_story", &[("n", &parts.len().to_string())]));
        self.view.message = qnc_ui_text::text("script.sending_story").into();
        self.write(preview, move |target| {
            for voice in voices.iter().filter(|voice| !voice.takes.is_empty()) {
                let takes: Vec<f64> = voice.takes.iter().map(|take| take.seconds).collect();
                let first_tc = voice.takes.first().map(|take| take.tc.clone()).unwrap_or_default();
                let record = qnc_voice_take::describe_record(locator.settings(), &voice.name, &voice.clip_uri, &takes, &first_tc)?;
                qnc_voice_catalog::publish_record(locator.reader(), locator.settings(), &voice.name, record)?;
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
                segments.push(qnc_program_db::AppendSegment {
                    kind: part.kind.into(),
                    clip_id: part.clip_id,
                    in_frame: part.from_frame,
                    out_frame: part.to_frame,
                    a1,
                    a2,
                    origin: part.origin,
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
            // The words of a statement with their times (user 2026-11-04: edited by its text).
            if row.is_clip() {
                let names = &self.names;
                row.words = view::block_pieces(&row.block)
                    .iter()
                    .map(|piece| {
                        let fps = names.get(&piece.clip_id).filter(|(_, frames, seconds)| *frames > 0 && *seconds > 0.0).map_or(0, |(_, frames, seconds)| (*frames as f64 / seconds).round() as u32);
                        let start = qnc_source_timecode::parse_timecode(&piece.start_tc, fps).filter(|_| fps > 0);
                        let tc = |frame: u64| start.map_or_else(String::new, |start| qnc_source_timecode::format_frames(start + frame.saturating_sub(piece.from_frame), fps));
                        self.transcripts.get(&piece.clip_id).map_or_else(Vec::new, |transcripts| view::timed_words(transcripts, piece, tc))
                    })
                    .collect();
            }
        }
        let profile = self.view.profile.as_ref();
        let counts = |row: &BlockRow| profile.and_then(|profile| profile.kind(&row.block.kind)).map(|kind| kind.counts);
        let sum = |keep: &dyn Fn(&BlockRow) -> bool| self.view.blocks.iter().filter(|row| keep(row)).map(|row| row.seconds).sum::<f64>();
        self.view.read_seconds = sum(&|row| counts(row) == Some(Counts::Story) && !row.is_clip());
        self.view.clip_seconds = sum(&|row| counts(row) == Some(Counts::Story) && row.is_clip());
        self.view.anchor_seconds = sum(&|row| counts(row) == Some(Counts::Anchor));
        (self.view.in_cue, self.view.out_cue) = story_cues(&self.view.blocks);
        if self.open_subsegment.is_some() {
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
                        // A saved text leaves the editor empty (MA Script).
                        if std::mem::take(&mut self.clear_after_write) {
                            self.view.editor.clear();
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
        if let Some(reading) = &self.reading {
            if let Ok(result) = reading.try_recv() {
                match result {
                    Ok(read::Script { blocks, profile, target, title, voices, transcripts }) => {
                        self.blocks = blocks;
                        self.transcripts = transcripts;
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
