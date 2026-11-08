//! The SubSegment (user 2026-10-29): a statement of the script of several parts of clips,
//! opened in place of the source timeline and edited as covers are (docs/arhitektura.md P1-2:
//! taken out of `qnc-script`, where it made one piece of four jobs). Its parts' edges, the
//! whole cut of a join (a roll), the J and L of a join's sound and picture, a part slid as a
//! slot, a slip, a part's own A1, the pause and join before a part, its place and its leaving;
//! the trim zoom of a taken edge with its pictures, fine wave and words; the one mouse map of a
//! program's row. Every move plays the draft at once (`pieces_request`), Enter writes it.
//!
//! It writes nothing itself: a write is handed to its owner (`take_writes`), the script, which
//! writes it to its database (`qnc-script-db`). What is shown is `view`.

use std::collections::HashMap;

use qnc_db_broker::ProjectDbTarget;
use qnc_script_db::{Block, Piece};
use qnc_source_preview::SourcePreview;

mod assist;
mod level;
mod pointer;
mod view;
pub use level::LEVEL_KEEP_AT;
pub use pointer::{drag_target, nearest_edge, own_action, ROW_ACTIONS};
pub use view::*;

pub const MODULE_ID: &str = "qnc.module.subsegment-edit";

/// The playhead of the open SubSegment on frame `target` (a click on its timeline).
pub const SUBSEGMENT_CUE: &str = "subsegment_cue";
/// A part of the open SubSegment chosen, as a slot is (a click on its length strip): `target`
/// is its index. Ctrl+I / Ctrl+O and Ctrl+1 then take its edges and its A1, Delete takes it out.
pub const SUBSEGMENT_SELECT: &str = "subsegment_select";
/// Several parts chosen by a drag over their length strips (user 2026-11-07): `target` is
/// `first:last` (the strip the drag started on, the one it reached). Ctrl+1 then takes the A1 of
/// all of them: one change for all.
pub const SUBSEGMENT_SELECT_SLOTS: &str = "subsegment_select_slots";
/// A part taken whole, as a slot (a right click on its length strip, as Ctrl+S): `target` is its
/// index; arrows and the wheel then move it along the statement.
pub const SUBSEGMENT_TAKE: &str = "subsegment_take";
/// A word clicked in the trim zoom of a SubSegment (value: its first frame, target: the frame
/// after its last, on the SubSegment's axis): the taken edge goes to it (user 2026-11-04).
pub const ZOOM_WORD: &str = "subsegment_zoom_word";
/// Ctrl+Shift+S: a slip of the part in hand (docs/98 1.5; the same action as a story's slip).
pub const SLIP: &str = "select_slip";
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

/// The A1 of part `index` taken (Ctrl+1) as drafted: camera channel, level, off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PartSound {
    index: usize,
    channel: Option<u16>,
    level_db: i8,
    off: bool,
}

/// An edge of part `index` being trimmed: the OUT or the IN, and the part's range as drafted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trim {
    pub index: usize,
    pub out: bool,
    pub from: u64,
    pub to: u64,
    /// The whole cut of a join taken (user 2026-11-03: edited as a cover's M marker is): the OUT
    /// of this part and the IN of the next move together; the next part's IN as drafted.
    pub roll: Option<u64>,
}

/// The J and L of the join before part `index` being moved: both sides together, or only the
/// J (the sound of the part after starts earlier) or only the L (the part before's goes on).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JoinSound {
    pub index: usize,
    /// Frames the part after's sound starts before its picture (negative: after it).
    pub lead: i64,
    /// Frames the part before's sound goes on after its OUT (negative: it ends before it).
    pub tail: i64,
    pub side: JoinSide,
    /// The join's picture, not its sound (user 2026-11-04: J and L of the picture, as at a
    /// story's cut, the V of the zoom): the parts' pictures cross the join, the sound stays.
    pub picture: bool,
}

/// Which side of a join's sound moves: the one rule of a cut's sound (`qnc-edit-cut`, docs/97
/// step 4), the same as a story's cut.
pub use qnc_edit_cut::SoundSide as JoinSide;

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
const JOIN_SOUND_MAX: i64 = 10_000;

impl JoinSound {
    /// One step later (`step` > 0) or earlier. Each side is where one sound meets the picture
    /// cut, either way of it (user 2026-11-04: the first part's sound may end before the cut and
    /// the second's start after it, a hole between): J is where the sound after starts, L where
    /// the sound before ends; both move together (the overlap or the hole between them moves
    /// whole), or one alone. `lowest` keeps each side inside its own part.
    fn moved(self, step: i64, lowest: (i64, i64)) -> Self {
        // A join's sound allows holes (user 2026-11-04): the rule of a cut's sound with negative
        // sides; its picture does not (the pictures cross the join or meet at it).
        let lowest = if self.picture { (0, 0) } else { lowest };
        let moved = self.sound().moved(self.side, step, !self.picture).bounded(lowest, JOIN_SOUND_MAX);
        JoinSound { lead: moved.lead, tail: moved.tail, ..self }
    }

    fn sound(self) -> qnc_edit_cut::CutSound {
        qnc_edit_cut::CutSound { lead: self.lead, tail: self.tail }
    }
}

/// A write of the SubSegment for its owner's database.
pub type Write = Box<dyn FnOnce(&ProjectDbTarget) -> Result<Option<i64>, String> + Send>;

/// What the SubSegment hands its owner: the message of its last action and the open view.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EditView {
    pub message: String,
    pub subsegment: Option<SubsegmentView>,
}

/// The SubSegment open in place of the source timeline and what of it is taken.
pub struct SubsegmentEdit {
    pub view: EditView,
    /// The statements of the script as last read (the open one among them).
    blocks: Vec<Block>,
    /// Name, frames and seconds of each clip of the pool.
    names: HashMap<String, (String, u64, f64)>,
    /// The project the script reads from.
    project: Option<String>,
    /// Nothing is being read or written: a block gone is gone.
    settled: bool,
    /// Writes for the owner's database, in order.
    writes: Vec<Write>,
    /// The clip block open as a SubSegment in place of the source timeline.
    open_subsegment: Option<i64>,
    /// The parts the player should play and the frame it should show (once).
    pieces_request: Option<(Vec<qnc_source_preview::PieceInput>, u64)>,
    /// The edge of a part being trimmed on the SubSegment timeline, not written yet.
    trim: Option<Trim>,
    /// The part of the open SubSegment chosen by a click on its length strip, as a slot is.
    chosen: Option<usize>,
    /// The other end of slots chosen by a drag over their strips (user 2026-11-07): every part
    /// from `chosen` to it is chosen; `None` for one slot. Only read while `chosen` is.
    chosen_to: Option<usize>,
    /// The other chosen slots whose A1 is taken with `sound` (several slots chosen: one change
    /// for all of them; one slot: that one alone, user 2026-11-07).
    sound_also: Vec<usize>,
    /// The sound of a join taken (Ctrl+1 with its whole cut taken, user 2026-11-03), as drafted.
    join_sound: Option<JoinSound>,
    /// The value of the action being handled (the frame of a click in the trim zoom).
    value: Option<i64>,
    /// A part taken whole (Ctrl+S, a right click on its length strip), as a slot is (user
    /// 2026-11-04): its index and how far it is drafted to move (frames; later is positive).
    slide: Option<(usize, i64)>,
    /// A part slipped (Ctrl+Shift+S): its index and the frames it plays its clip later.
    slip_part: Option<(usize, i64)>,
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
    /// The level curve of a part's A1 taken (Ctrl+Shift+1, a right click on its line), as drafted.
    level: Option<qnc_level_curve::LevelLine>,
    /// AI on (Shift+N while a SubSegment is open): its parts evened out. A choice of this
    /// process, as NORM in the preview; KEEP writes the proposal.
    assist: bool,
    /// The assistant's curves of the open SubSegment now (part, points on its clip's frames).
    assist_curves: Vec<(usize, Vec<(u64, i16)>)>,
    /// The last proposal and what it was made of.
    assisted: Option<(assist::AssistKey, Vec<(usize, Vec<(u64, i16)>)>)>,
}

impl Default for SubsegmentEdit {
    fn default() -> Self {
        Self { view: EditView::default(), blocks: Vec::new(), names: HashMap::new(), project: None, settled: true, writes: Vec::new(), open_subsegment: None, pieces_request: None, trim: None, chosen: None, chosen_to: None, sound_also: Vec::new(), join_sound: None, value: None, slide: None, slip_part: None, waves: qnc_program_waveform::ProgramWaves::new(), filmstrips: qnc_program_filmstrip::ProgramFilmstrips::new(), waves_project: None, trim_stills: qnc_trim_stills::TrimStills::new(), zoom_follow: Default::default(), sound: None, level: None, assist: false, assist_curves: Vec::new(), assisted: None }
    }
}

impl std::fmt::Debug for SubsegmentEdit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubsegmentEdit").field("view", &self.view).finish_non_exhaustive()
    }
}

impl SubsegmentEdit {
    fn block(&self, id: Option<i64>) -> Option<&Block> {
        id.and_then(|id| self.blocks.iter().find(|block| block.id == id))
    }

    /// What is held of parts the statement no longer has (another writer shortened it while an
    /// edge, a slot or a sound was taken) is let go, so no key reaches past its parts (audit
    /// 2026-11-08 #14: a key after that crashed the application).
    fn forget_parts_gone(&mut self, count: usize) {
        if self.trim.is_some_and(|trim| trim.index >= count || (trim.roll.is_some() && trim.index + 1 >= count)) {
            self.trim = None;
        }
        if self.chosen.is_some_and(|index| index >= count) {
            (self.chosen, self.chosen_to) = (None, None);
        }
        if self.chosen_to.is_some_and(|index| index >= count) {
            self.chosen_to = None;
        }
        self.sound_also.retain(|index| *index < count);
        if self.sound.is_some_and(|sound| sound.index >= count) {
            (self.sound, self.sound_also) = (None, Vec::new());
        }
        if self.join_sound.is_some_and(|join| join.index == 0 || join.index >= count) {
            self.join_sound = None;
        }
        if self.slide.is_some_and(|(index, _)| index == 0 || index + 1 >= count) {
            self.slide = None;
        }
        if self.slip_part.is_some_and(|(index, _)| index >= count) {
            self.slip_part = None;
        }
        if self.level.as_ref().is_some_and(|line| line.id.parse::<usize>().map_or(true, |index| index >= count)) {
            self.level = None;
        }
    }

    /// The open SubSegment as its block holds it now and the player plays it: a write that
    /// changed a pause or a join plays again where it stands; a block gone closes it.
    fn follow_subsegment(&mut self, preview: &SourcePreview) {
        let Some(block) = self.block(self.open_subsegment).cloned() else {
            if self.settled {
                (self.open_subsegment, self.view.subsegment) = (None, None);
            }
            return;
        };
        self.forget_parts_gone(view::block_pieces(&block).len());
        let playing = preview.shown_pieces();
        if playing.is_none() && self.pieces_request.is_none() {
            // The player opened something else (a clip of the pool): the source timeline is back.
            (self.open_subsegment, self.view.subsegment) = (None, None);
            return;
        }
        let stored = block.clone();
        // The assistant's curves of the stored parts (AI on), played and drawn with the draft.
        self.assist_curves = self.propose(&block);
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
        let joined = self.trim.map(|trim| if trim.out || trim.roll.is_some() { trim.index + 1 } else { trim.index }).filter(|index| *index > 0 && pieces.get(*index).is_some_and(|piece| piece.gap_before == 0));
        let join_sound = self.join_sound;
        let chosen = self.chosen;
        // A1 of the trimmed part as its lane shows it (AGENTS 0.12, docs/97 step 3).
        let sound_open = self.sound.is_some();
        let a1_lane = self.trim.and_then(|trim| shown.parts.get(trim.index)).and_then(|part| {
            Some(qnc_program_view::ZoomLane { channel: part.a1_channel.unwrap_or(0), count: part.channels?, open: sound_open, off: part.a1_off, level_db: part.a1_level_db })
        });
        let zoom = zoom.map(|mut zoom| {
            if let Some(sound) = zoom.zoom_sound.as_mut() {
                sound.lane_choices[0] = a1_lane;
            }
            // The side in focus (user 2026-11-04: the zoom frames it red, the same in every zoom):
            // the side of the join sound that moves alone, else the side of the chosen slot.
            let chosen_side = chosen.map(|index| index.to_string()).and_then(|chosen| {
                zoom.source_edit_stills.iter().chain(&zoom.edit_stills).find(|still| still.strip.as_ref().is_some_and(|strip| strip.select.as_ref().and_then(|select| select.target.as_deref()) == Some(chosen.as_str()))).map(|still| still.after)
            });
            let sound_side = join_sound.and_then(|join| join.side.focus());
            if let Some(sound) = zoom.zoom_sound.as_mut() {
                sound.focus_side = sound_side.or(chosen_side);
            }
            if let (Some(index), Some(sound)) = (joined.or(join_sound.map(|join| join.index)), zoom.zoom_sound.as_mut()) {
                let marker_id = format!("join:{index}");
                sound.taken_cut = Some(marker_id.clone());
                // The join's picture taken: its V framed red, its sound not shown as taken.
                sound.picture_taken = join_sound.is_some_and(|join| join.picture);
                if let Some(join) = join_sound.filter(|join| !join.picture) {
                    let (lead, tail) = join.sound().sides_at(sound.cut);
                    let moving = join.sound().moving(join.side, sound.cut);
                    sound.cut_sound = Some(qnc_program_view::ZoomCutSound { marker_id, lane: 0, lead: Some(lead), tail: Some(tail), moving });
                }
            }
            zoom
        });
        // A part's own edge taken (no roll, user 2026-11-04 "ne smije utjecati na susjedni
        // slot", as a cover's slot edge): only that edge moves on the timeline, the other parts
        // stay where they are stored until Enter. The player and the zoom play the draft.
        if let Some(trim) = self.trim.filter(|trim| trim.roll.is_none() && self.slide.is_none()) {
            let at_rest = view::subsegment_view(&stored, None);
            if let (Some(rest), Some(drafted)) = (at_rest.parts.get(trim.index).cloned(), shown.parts.get(trim.index).cloned()) {
                let (length, stored_length) = (drafted.end - drafted.start, rest.end - rest.start);
                // Where the draft's frames after the edge are shown (the axis the player plays).
                let shift = stored_length as i64 - length as i64;
                let from = if trim.out { drafted.end } else { drafted.start };
                for (index, part) in shown.parts.iter_mut().enumerate() {
                    if let Some(rest) = at_rest.parts.get(index) {
                        (part.start, part.end) = match index.cmp(&trim.index) {
                            std::cmp::Ordering::Less => (part.start, part.end),
                            std::cmp::Ordering::Equal if trim.out => (rest.start, rest.start + length),
                            std::cmp::Ordering::Equal => (rest.end.saturating_sub(length), rest.end),
                            std::cmp::Ordering::Greater => (rest.start, rest.end),
                        };
                    }
                }
                shown.total_frames = shown.parts.iter().map(|part| part.end).max().unwrap_or(0).max(at_rest.total_frames);
                shown.playhead = shown.playhead.map(|frame| if frame >= from { (frame as i64 + shift).max(0) as u64 } else { frame });
                let placements = view::subsegment_placements(&shown, &view::block_pieces(&block));
                shown.a1 = self.waves.peaks(shown.total_frames, &placements, &[], |clip| names.get(clip).map(|(_, frames, _)| *frames)).a1;
                let at = view::subsegment_pictures(&shown, &view::block_pieces(&block));
                shown.pictures = self.filmstrips.pictures(&at, |clip| names.get(clip).map(|(_, frames, _)| *frames), |_| None);
                shown.trim = shown.parts.get(trim.index).map(|part| (trim.index, trim.out, if trim.out { part.end.saturating_sub(1) } else { part.start }));
            }
        }
        shown.zoom = zoom.map(|mut zoom| {
            self.trim_stills.fill(&mut zoom, preview.still_locator(), None);
            Box::new(zoom)
        });
        shown.sound = self.sound.map(|sound| sound.index);
        // The column of the taken A1 on the timeline, as on the source timeline (AGENTS 0.12):
        // the drafted channel among its clip's channels, the wave already drawn at the stored level.
        shown.sound_choice = self.sound.and_then(|sound| {
            let piece = pieces.get(sound.index)?;
            let count = self.waves.channels(&piece.clip_id).unwrap_or(2).max(1);
            let now = qnc_lane_choice::Lane { channel: sound.channel.unwrap_or(0), off: sound.off, level_db: sound.level_db };
            Some(qnc_lane_choice::drafted_choice(now, count, true, piece.a1_level_db))
        });
        shown.roll = self.trim.is_some_and(|trim| trim.roll.is_some());
        shown.chosen = self.chosen.filter(|index| *index < shown.parts.len());
        shown.level_curves = self.level_curves(&shown, &pieces);
        // The zoom is the same timeline: the parts' level curves on its axis too (user 2026-11-07).
        if let Some(sound) = shown.zoom.as_mut().and_then(|zoom| zoom.zoom_sound.as_mut()) {
            sound.level_curves = shown.level_curves.clone();
        }
        shown.chosen_slots =self.chosen_slots().into_iter().chain(self.sound.is_some().then(|| self.sound_also.clone()).unwrap_or_default()).filter(|index| *index < shown.parts.len()).collect();
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
                if join.picture {
                    (piece.v_lead_frames, piece.v_tail_frames) = (join.lead.max(0) as u64, join.tail.max(0) as u64);
                } else {
                    (piece.a1_lead_frames, piece.a1_tail_frames) = (join.lead, join.tail);
                }
            }
        }
        if let Some((index, by)) = self.slide {
            slide_pieces(&mut block.pieces, index, by);
        }
        // A slipped part: the same length, `by` frames along its clip.
        if let Some((index, by)) = self.slip_part {
            if let Some(piece) = block.pieces.get_mut(index) {
                (piece.from_frame, piece.to_frame) = ((piece.from_frame as i64 + by) as u64, (piece.to_frame as i64 + by) as u64);
            }
        }
        if let Some(sound) = self.sound {
            // The taken slot and the other chosen ones are heard with the drafted sound.
            // (The drafted level curve is applied below.)
            for index in std::iter::once(sound.index).chain(self.sound_also.iter().copied()) {
                if let Some(piece) = block.pieces.get_mut(index) {
                    let count = self.waves.channels(&piece.clip_id).unwrap_or(2).max(1);
                    (piece.a1_channel, piece.a1_level_db, piece.a1_off) = (sound.channel.map(|channel| channel.min(count - 1)), sound.level_db, sound.off);
                }
            }
        }
        // The drafted level curve of the taken part and the assistant's curves of the parts without
        // one of their own (a statement of one range is its own part).
        let drafted_level = self.level_draft();
        if drafted_level.is_some() || !self.assist_curves.is_empty() {
            if block.pieces.is_empty() {
                block.pieces = vec![Piece::of(&block)];
            }
            for (index, keys) in &self.assist_curves {
                if let Some(piece) = block.pieces.get_mut(*index).filter(|piece| piece.level_keys.is_empty()) {
                    piece.level_keys.clone_from(keys);
                }
            }
            if let Some((index, keys)) = drafted_level {
                if let Some(piece) = block.pieces.get_mut(index) {
                    piece.level_keys = keys;
                }
            }
        }
        block
    }

    /// The sound keys of a part, as a cover's lane (user 2026-10-29): Ctrl+1 takes the A1 of the
    /// part under the playhead, up/down its camera channel, left off and right on, Ctrl+arrows
    /// and Ctrl+wheel its level a dB, Enter keeps it, Esc gives it up. Every key plays the
    /// draft at once. `None` for a key that is not the sound's.
    fn sound_key(&mut self, action_id: &str, target: Option<&str>, block: &Block, pieces: &[Piece], preview: &SourcePreview) -> Option<bool> {
        // A channel clicked in the taken lane's column (the timeline's or its zoom's, AGENTS 0.12).
        let key = match action_id {
            qnc_edit_verbs::PICK_A1_CHANNEL => qnc_lane_choice::LaneKey::Pick(u16::try_from(self.value?).ok()?),
            _ => qnc_lane_choice::LaneKey::of(action_id)?,
        };
        let at = self.view.subsegment.as_ref().and_then(|view| view.playhead).unwrap_or(0);
        match key {
            qnc_lane_choice::LaneKey::Take(0) => {
                // A right click on A1 names the part it was on (user 2026-11-04, as a slot's sound).
                let index = target.and_then(|index| index.parse::<usize>().ok()).filter(|index| *index < pieces.len()).unwrap_or_else(|| self.part_in_hand());
                let piece = pieces.get(index)?;
                (self.trim, self.slide) = (None, None);
                self.sound = Some(PartSound { index, channel: piece.a1_channel, level_db: piece.a1_level_db, off: piece.a1_off });
                // Several slots chosen and this one among them: the change is for all of them.
                let chosen = self.chosen_slots();
                self.sound_also = if chosen.contains(&index) { chosen.into_iter().filter(|other| *other != index && *other < pieces.len()).collect() } else { Vec::new() };
                return Some(true);
            }
            qnc_lane_choice::LaneKey::Take(_) => return None,
            _ => {}
        }
        let mut sound = self.sound?;
        let channels = self.waves.channels(&pieces.get(sound.index)?.clip_id).unwrap_or(2).max(1);
        match key {
            qnc_lane_choice::LaneKey::Pick(channel) => sound.channel = Some(channel.min(channels - 1)),
            qnc_lane_choice::LaneKey::Channel(delta) => sound.channel = Some((i64::from(sound.channel.unwrap_or(0)) + delta).clamp(0, i64::from(channels) - 1) as u16),
            qnc_lane_choice::LaneKey::Off(off) => sound.off = off,
            qnc_lane_choice::LaneKey::Level(delta) => sound.level_db = (i16::from(sound.level_db) + i16::from(delta)).clamp(-40, 12) as i8,
            qnc_lane_choice::LaneKey::Keep => {
                self.sound = None;
                // The taken slot and every other chosen one get the same sound (each its channel
                // within the channels of its own clip).
                let also = std::mem::take(&mut self.sound_also);
                let written: Vec<(usize, Piece)> = std::iter::once(sound.index)
                    .chain(also)
                    .filter_map(|index| {
                        let mut piece = pieces.get(index)?.clone();
                        let count = self.waves.channels(&piece.clip_id).unwrap_or(2).max(1);
                        (piece.a1_channel, piece.a1_level_db, piece.a1_off) = (sound.channel.map(|channel| channel.min(count - 1)), sound.level_db, sound.off);
                        Some((index, piece))
                    })
                    .collect();
                let id = block.id;
                // One write and one step of undo for every slot (audit 2026-11-08 #15).
                let changes = written.into_iter().map(|(index, piece)| qnc_script_db::Operation::SetPiece { id, index, piece }).collect();
                return Some(self.write(preview, move |target| qnc_script_db::together(target, changes).map(|_| None)));
            }
            qnc_lane_choice::LaneKey::Drop => {
                self.sound = None;
                self.sound_also.clear();
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
        self.forget_parts_gone(pieces.len());
        let take = |index: usize, out: bool| pieces.get(index).map(|piece| Trim { index, out, from: piece.from_frame, to: piece.to_frame, roll: None });
        if let Some(done) = self.assist_key(action_id, &block, &pieces, preview) {
            return Some(done);
        }
        if let Some(done) = self.level_key(action_id, target, &block, &pieces, preview) {
            return Some(done);
        }
        if let Some(done) = self.slip_key(action_id, &block, &pieces, preview) {
            return Some(done);
        }
        if let Some(done) = self.slide_key(action_id, target, &block, &pieces, preview) {
            return Some(done);
        }
        if let Some(done) = self.join_key(action_id, target, &block, &pieces, preview) {
            return Some(done);
        }
        if let Some(done) = self.sound_key(action_id, target, &block, &pieces, preview) {
            return Some(done);
        }
        // I and O on the SubSegment's timeline as on every timeline (user 2026-11-04: "IN/OUT
        // functions on every timeline must work"): the part under the playhead gets its IN or OUT
        // there; a taken edge of that part moves there as a draft (Enter keeps it).
        if matches!(action_id, "mark_in" | "mark_out") {
            return self.mark_part(action_id == "mark_out", &block, &pieces, preview);
        }
        if action_id == ZOOM_WORD {
            return self.zoom_word(target, &block);
        }
        if action_id == qnc_edit_verbs::SAVE {
            return self.save_and_keep(&block, &pieces, preview);
        }
        if action_id == DRAG_EDGE {
            return self.drag_edge(target, &block, &pieces);
        }
        // A part chosen as a slot is (user 2026-11-03: a click on its length strip): the
        // playhead on its IN; Esc lets it go, Delete takes it out of the statement.
        match action_id {
            SUBSEGMENT_SELECT => {
                let index = target.and_then(|index| index.parse::<usize>().ok()).filter(|index| *index < pieces.len())?;
                (self.chosen, self.chosen_to) = (Some(index), None);
                // In the zoom (a picture clicked): the slot is chosen, the edge taken stays and the
                // playhead does not move.
                if self.trim.is_some() || self.join_sound.is_some() {
                    return Some(true);
                }
                let start = self.view.subsegment.as_ref().and_then(|view| view.parts.get(index)).map_or(0, |part| part.start);
                self.pieces_request = Some((view::piece_inputs(&block), start));
                return Some(true);
            }
            // A drag over the strips (user 2026-11-07): every slot from the one it started on to
            // the one it reached is chosen; the playhead stays.
            SUBSEGMENT_SELECT_SLOTS => {
                let (from, to) = target?.split_once(':')?;
                let (from, to) = (from.parse::<usize>().ok()?, to.parse::<usize>().ok()?);
                if from >= pieces.len() || to >= pieces.len() {
                    return Some(false);
                }
                (self.chosen, self.chosen_to) = (Some(from), (to != from).then_some(to));
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
                // From the slot's strip (user 2026-11-05): the slot is chosen, its own edge taken.
                if parts.next() == Some("slot") && side != Some("cut") && index < pieces.len() {
                    self.chosen = Some(index);
                }
                let (mut index, mut out, mut cut) = (index, side != Some("in"), side == Some("cut"));
                // A chosen slot at the cut takes its own edge, not the roll (user 2026-11-04):
                // the slot gets longer or shorter, the other slots keep theirs.
                match self.chosen {
                    Some(chosen) if cut && chosen == index => cut = false,
                    Some(chosen) if cut && chosen == index + 1 => (index, out, cut) = (chosen, false, false),
                    _ => {}
                }
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
            step if self.trim.is_some() && qnc_edit_step::frames(step).is_some() => qnc_edit_step::frames(step)?,
            "activate_focused_item" if self.trim.is_some() => {
                let trim = self.trim.take()?;
                return Some(self.write_trim(preview, &block, &pieces, trim));
            }
            "clear_focus" if self.trim.is_some() => {
                self.trim = None;
                self.pieces_request = Some((view::piece_inputs(&block), self.edge_frame(&block).unwrap_or(0)));
                return Some(true);
            }
            // Nothing taken: the arrows and the wheel move the playhead a frame (user 2026-11-05:
            // "the basic arrow keys and the mouse wheel do not work on the timeline"; AGENTS
            // 0.12, the same on every timeline).
            step if qnc_edit_step::frames(step).is_some() => {
                let forward = qnc_edit_step::frames(step)? > 0;
                let total = self.view.subsegment.as_ref().map_or(0, |view| view.total_frames);
                let at = self.pieces_request.as_ref().map(|(_, frame)| *frame).or(preview.program_frame()).unwrap_or(0);
                let to = if forward { (at + 1).min(total.saturating_sub(1)) } else { at.saturating_sub(1) };
                self.pieces_request = Some((view::piece_inputs(&block), to));
                return Some(true);
            }
            // Home, Alt+arrows and Alt+I as on the source timeline: the start, the edge of the
            // part before or after the playhead, the IN of the part under it.
            "playlist_input_start" | "navigate_prev_object" | "navigate_next_object" | "go_to_mark_in" if self.trim.is_none() => {
                let view = self.view.subsegment.as_ref()?;
                let at = self.pieces_request.as_ref().map(|(_, frame)| *frame).or(preview.program_frame()).unwrap_or(0);
                let mut edges: Vec<u64> = view.parts.iter().flat_map(|part| [part.start, part.end.saturating_sub(1)]).collect();
                edges.sort_unstable();
                edges.dedup();
                let to = match action_id {
                    "playlist_input_start" => 0,
                    "navigate_prev_object" => edges.iter().rev().find(|&&edge| edge < at).copied().unwrap_or(0),
                    "navigate_next_object" => edges.iter().find(|&&edge| edge > at).copied().unwrap_or(at),
                    _ => view.parts.iter().find(|part| (part.start..part.end).contains(&at)).map_or(0, |part| part.start),
                };
                self.pieces_request = Some((view::piece_inputs(&block), to));
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
                // The one rule of an edge (`qnc-edit-edge`): as far as it can go in its clip.
                let length = pieces.get(trim.index).and_then(|piece| self.names.get(&piece.clip_id)).map_or(u64::MAX, |(_, frames, _)| *frames);
                let range = qnc_edit_edge::Range { from: trim.from, to: trim.to }.moved(qnc_edit_edge::Edge::of_out(trim.out), step, length);
                (trim.from, trim.to) = (range.from, range.to);
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
            // The one rule of an edge (`qnc-edit-edge`): the edge on the clip's frame under the pointer.
            (true, None) => trim.to = qnc_edit_edge::Range { from: trim.from, to: trim.to }.dragged(qnc_edit_edge::Edge::Out, trim.from + frame.saturating_sub(start), length).to,
            (false, _) => trim.from = qnc_edit_edge::Range { from: trim.from, to: trim.to }.dragged(qnc_edit_edge::Edge::In, (piece.from_frame as i64 + frame as i64 - start as i64).max(0) as u64, length).from,
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
        (self.chosen, self.join_sound, self.trim, self.sound, self.slide, self.slip_part) = (None, None, None, None, None, None);
        self.level = None;
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
            // The arrows and the wheel: one step (`qnc-edit-step`, the same on every timeline).
            step if qnc_edit_step::frames(step).is_some() => qnc_edit_step::frames(step)?,
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
                // One write and one step of undo for the slot moved (audit 2026-11-08 #15).
                let changes = writes
                    .into_iter()
                    .flat_map(|(index, piece, join)| {
                        let piece = piece.map(|piece| qnc_script_db::Operation::SetPiece { id, index, piece });
                        let join = join.map(|(gap_before, cut)| qnc_script_db::Operation::SetJoin { id, index, gap_before, cut });
                        piece.into_iter().chain(join)
                    })
                    .collect();
                return Some(self.write(preview, move |target| qnc_script_db::together(target, changes).map(|_| None)));
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
    fn join_key(&mut self, action_id: &str, target: Option<&str>, block: &Block, pieces: &[Piece], preview: &SourcePreview) -> Option<bool> {
        let take_sound = matches!(qnc_lane_choice::LaneKey::of(action_id), Some(qnc_lane_choice::LaneKey::Take(0)));
        // The V of the zoom on a taken join (user 2026-11-04): its picture, as at a story's cut.
        let take_picture = action_id == "select_mark_in" && target.is_some_and(|target| target.starts_with("join:"));
        // A part's J and L of sound or of picture, as stored.
        let stored = |piece: &Piece, picture: bool| if picture { (piece.v_lead_frames as i64, piece.v_tail_frames as i64) } else { (piece.a1_lead_frames, piece.a1_tail_frames) };
        if let Some(join) = self.join_sound.filter(|join| (take_sound && join.picture) || (take_picture && !join.picture)) {
            // The other of picture and sound of the same join: the moved one is written first.
            let piece = pieces.get(join.index)?;
            if stored(piece, join.picture) != (join.lead, join.tail) {
                self.write_join(preview, block, join);
            }
            let (lead, tail) = stored(piece, take_picture);
            self.join_sound = Some(JoinSound { lead, tail, picture: take_picture, ..join });
            return Some(true);
        }
        // With a slot chosen (a picture of the zoom clicked, user 2026-11-04), its sound at the cut
        // is taken alone: the part before's end (its L), the part after's start (its J).
        let side_of = |chosen: Option<usize>, index: usize| match chosen {
            Some(chosen) if chosen + 1 == index || chosen == index => JoinSide::of_piece(chosen + 1 == index),
            _ => JoinSide::Both,
        };
        let mut join = match self.join_sound {
            Some(join) => join,
            None => {
                // An edge taken at a join (the whole cut, the OUT before it or the IN after it).
                let trim = self.trim.filter(|_| take_sound || take_picture)?;
                let index = if trim.out || trim.roll.is_some() { trim.index + 1 } else { trim.index };
                let piece = pieces.get(index).filter(|piece| index > 0 && piece.gap_before == 0)?;
                if pieces.get(trim.index).is_some_and(|stored| (stored.from_frame, stored.to_frame) != (trim.from, trim.to)) || trim.roll.is_some_and(|from| pieces.get(trim.index + 1).is_some_and(|next| next.from_frame != from)) {
                    self.write_trim(preview, block, pieces, trim);
                }
                self.sound = None;
                let (lead, tail) = stored(piece, take_picture);
                self.join_sound = Some(JoinSound { index, lead, tail, side: side_of(self.chosen, index), picture: take_picture });
                return Some(true);
            }
        };
        let step = match action_id {
            _ if take_sound || take_picture => {
                join.side = side_of(self.chosen, join.index);
                0
            }
            // With a slot chosen, Ctrl+I is its own J (the join before it) and Ctrl+O its own L
            // (the join after it): the neighbour's sound never moves (user 2026-11-04).
            "select_mark_in" | "select_mark_out" => {
                let lead = action_id == "select_mark_in";
                let side = if lead { JoinSide::Lead } else { JoinSide::Tail };
                let Some(chosen) = self.chosen else {
                    join.side = side;
                    self.join_sound = Some(join);
                    return Some(true);
                };
                let index = if lead { chosen } else { chosen + 1 };
                let Some(piece) = pieces.get(index).filter(|piece| index > 0 && piece.gap_before == 0) else {
                    self.view.message = qnc_ui_text::text("subsegment.no_join_sound").into();
                    return Some(true);
                };
                if index != join.index {
                    if pieces.get(join.index).is_some_and(|piece| stored(piece, join.picture) != (join.lead, join.tail)) {
                        self.write_join(preview, block, join);
                    }
                    let (lead, tail) = stored(piece, join.picture);
                    join = JoinSound { index, lead, tail, side, picture: join.picture };
                } else {
                    join.side = side;
                }
                self.join_sound = Some(join);
                return Some(true);
            }
            // The arrows and the wheel: one step (`qnc-edit-step`, the same on every timeline).
            step if qnc_edit_step::frames(step).is_some() => qnc_edit_step::frames(step)?,
            // In the zoom (the same system actions as on a story's row): a click near a side takes
            // that side, a drag moves the moving side to the frame.
            "timeline_take_cut_edge" | "timeline_drag_cut_edge" => {
                let frame = u64::try_from(self.value?).ok()?;
                let cut = view::subsegment_view(&self.drafted(block.clone()), None).parts.get(join.index)?.start;
                if action_id == "timeline_take_cut_edge" {
                    // A chosen slot keeps its own side; otherwise the side nearer the click.
                    join.side = match side_of(self.chosen, join.index) {
                        JoinSide::Both => join.sound().nearer(cut, frame, (true, true)).unwrap_or(JoinSide::Both),
                        side => side,
                    };
                    self.join_sound = Some(join);
                    return Some(true);
                }
                // The drag puts the moving side where the pointer is (both as one: the L side).
                let (lead_at, tail_at) = join.sound().sides_at(cut);
                match join.side {
                    JoinSide::Lead => frame as i64 - lead_at as i64,
                    JoinSide::Tail | JoinSide::Both => frame as i64 - tail_at as i64,
                }
            }
            "activate_focused_item" => {
                (self.join_sound, self.trim) = (None, None);
                return Some(self.write_join(preview, block, join));
            }
            "clear_focus" => {
                self.join_sound = None;
                self.pieces_request = Some((view::piece_inputs(block), self.join_hear_from(block, join.index)));
                return Some(true);
            }
            _ => return None,
        };
        // Each sound stays inside its own part: at least a frame of it is heard.
        let frames = |piece: Option<&Piece>| piece.map_or(0, |piece| piece.to_frame.saturating_sub(piece.from_frame) as i64);
        let lowest = (1 - frames(pieces.get(join.index)), 1 - frames(join.index.checked_sub(1).and_then(|before| pieces.get(before))));
        self.join_sound = Some(join.moved(step, lowest));
        if step != 0 {
            let drafted = self.drafted(block.clone());
            self.pieces_request = Some((view::piece_inputs(&drafted), self.join_hear_from(&drafted, join.index)));
        }
        Some(true)
    }

    /// Writes the J and L of a join: of its picture or of its sound.
    fn write_join(&mut self, preview: &SourcePreview, block: &Block, join: JoinSound) -> bool {
        let (id, index) = (block.id, join.index);
        if join.picture {
            let (lead, tail) = (join.lead.max(0) as u64, join.tail.max(0) as u64);
            self.write(preview, move |target| qnc_script_db::set_join_picture(target, id, index, lead, tail).map(|_| None))
        } else {
            self.write(preview, move |target| qnc_script_db::set_join_sound(target, id, index, join.lead, join.tail).map(|_| None))
        }
    }

    /// Where a join sound is heard from: a second before its cut.
    fn join_hear_from(&self, block: &Block, index: usize) -> u64 {
        let at = view::subsegment_view(block, None).parts.get(index).map_or(0, |part| part.start);
        let rate = self.view.subsegment.as_ref().and_then(|view| view.rate).map_or(25, |(num, den)| u64::from(num / den.max(1)));
        at.saturating_sub(rate)
    }

    /// The part Ctrl+I / Ctrl+O and Ctrl+1 work on: the chosen one, else the one under the
    /// playhead (as a slot: the chosen slot, else the slot under the Wrap playhead).
    /// The chosen slots: one, or every part between the two ends of a drag over the strips.
    fn chosen_slots(&self) -> Vec<usize> {
        match (self.chosen, self.chosen_to) {
            (Some(from), Some(to)) => (from.min(to)..=from.max(to)).collect(),
            (Some(one), None) => vec![one],
            _ => Vec::new(),
        }
    }

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
        let (Some(next_from), Some(next), Some(this)) = (trim.roll, pieces.get(trim.index + 1), pieces.get(trim.index)) else { return };
        let length = self.names.get(&this.clip_id).map_or(u64::MAX, |(_, frames, _)| *frames);
        // The one rule of a roll (`qnc-edit-edge`).
        let (before, after) = qnc_edit_edge::rolled(qnc_edit_edge::Range { from: trim.from, to: trim.to }, length, qnc_edit_edge::Range { from: next_from, to: next.to_frame }, to as i64 - trim.to as i64);
        trim.to = before.to;
        trim.roll = Some(after.from);
    }

    /// Writes the parts a trim moved (the part, and the next one of a whole cut); nothing when
    /// nothing moved. Returns whether a write went.
    /// I or O at the playhead of the open SubSegment: the IN or OUT (the OUT after the frame
    /// shown) of the part under it. Its taken edge moves there as a draft; otherwise it is
    /// written at once, a taken edge of another part kept first.
    fn mark_part(&mut self, out: bool, block: &Block, pieces: &[Piece], preview: &SourcePreview) -> Option<bool> {
        let drafted = self.drafted(block.clone());
        let at = preview.program_frame()?;
        let parts = view::subsegment_view(&drafted, None).parts;
        let drafted_pieces = view::block_pieces(&drafted);
        let Some(index) = parts.iter().position(|part| (part.start..part.end).contains(&at)) else {
            self.view.message = qnc_ui_text::text("subsegment.mark_in_pause").into();
            return Some(true);
        };
        let (Some(part), Some(piece)) = (parts.get(index), drafted_pieces.get(index)) else { return Some(true) };
        let frame = piece.from_frame + (at - part.start);
        let (from, to) = if out { (piece.from_frame, frame + 1) } else { (frame, piece.to_frame) };
        if to <= from {
            return Some(true);
        }
        match self.trim {
            Some(trim) if trim.index == index && trim.roll.is_none() => {
                self.trim = Some(Trim { from, to, out, ..trim });
                let drafted = self.drafted(block.clone());
                let edge = view::subsegment_view(&drafted, None).parts.get(index).map_or(at, |part| if out { part.end.saturating_sub(1) } else { part.start });
                self.pieces_request = Some((view::piece_inputs(&drafted), edge));
                Some(true)
            }
            held => {
                if let Some(held) = held {
                    self.trim = None;
                    self.write_trim(preview, block, pieces, held);
                }
                let stored = pieces.get(index)?;
                let piece = self.trimmed_piece(stored, Trim { index, out, from, to, roll: None });
                let id = block.id;
                Some(self.write(preview, move |target| qnc_script_db::set_piece(target, id, index, piece).map(|_| None)))
            }
        }
    }

    /// A slip of a part (docs/98 1.5, Ctrl+Shift+S): the part in hand plays its clip later or
    /// earlier, the same length in the same place (`qnc-edit-edge` `slipped`); the arrows and the
    /// wheel move it a frame and the draft plays at once, Enter (or Save) writes it, Esc gives
    /// it up. `None` for any other key.
    fn slip_key(&mut self, action_id: &str, block: &Block, pieces: &[Piece], preview: &SourcePreview) -> Option<bool> {
        if action_id == SLIP {
            let index = self.part_in_hand();
            pieces.get(index)?;
            (self.trim, self.sound, self.join_sound, self.slide) = (None, None, None, None);
            self.slip_part = Some((index, 0));
            self.view.message = qnc_ui_text::fill("story.msg.slip", &[("frames", "+0")]).into();
            return Some(true);
        }
        let (index, by) = self.slip_part?;
        let piece = pieces.get(index)?;
        let step = match action_id {
            // The arrows and the wheel: one step (`qnc-edit-step`, the same on every timeline).
            step if qnc_edit_step::frames(step).is_some() => qnc_edit_step::frames(step)?,
            "activate_focused_item" => {
                self.slip_part = None;
                return Some(self.write_slip(preview, block, piece, index, by));
            }
            "clear_focus" => {
                self.slip_part = None;
                self.pieces_request = Some((view::piece_inputs(block), self.edge_frame(block).unwrap_or(0)));
                return Some(true);
            }
            _ => return None,
        };
        let length = self.names.get(&piece.clip_id).map_or(u64::MAX, |(_, frames, _)| *frames);
        let range = qnc_edit_edge::Range { from: piece.from_frame, to: piece.to_frame }.slipped(by + step, length);
        let by = range.from as i64 - piece.from_frame as i64;
        self.slip_part = Some((index, by));
        self.view.message = qnc_ui_text::fill("story.msg.slip", &[("frames", &format!("{by:+}"))]).into();
        let drafted = self.drafted(block.clone());
        let at = view::subsegment_view(&drafted, None).parts.get(index).map_or(0, |part| part.start);
        self.pieces_request = Some((view::piece_inputs(&drafted), at));
        Some(true)
    }

    /// Writes a part slipped `by` frames along its clip; nothing when it did not move.
    fn write_slip(&mut self, preview: &SourcePreview, block: &Block, piece: &Piece, index: usize, by: i64) -> bool {
        if by == 0 {
            return true;
        }
        let (from, to) = ((piece.from_frame as i64 + by) as u64, (piece.to_frame as i64 + by) as u64);
        let slipped = self.trimmed_piece(piece, Trim { index, out: true, from, to, roll: None });
        let id = block.id;
        self.write(preview, move |target| qnc_script_db::set_piece(target, id, index, slipped).map(|_| None))
    }

    /// The zoom's Save (user 2026-11-04: "after Save do not close"): what is taken is written
    /// as Enter writes it and stays taken, so the editing goes on; Close (Escape) lets it go.
    fn save_and_keep(&mut self, block: &Block, pieces: &[Piece], preview: &SourcePreview) -> Option<bool> {
        if let Some((index, by)) = self.slip_part {
            let piece = pieces.get(index)?.clone();
            self.slip_part = Some((index, 0));
            return Some(self.write_slip(preview, block, &piece, index, by));
        }
        if let Some(trim) = self.trim {
            return Some(self.write_trim(preview, block, pieces, trim));
        }
        if let Some(join) = self.join_sound {
            return Some(self.write_join(preview, block, join));
        }
        if let Some(sound) = self.sound {
            let done = self.sound_key("activate_focused_item", None, block, pieces, preview);
            self.sound = Some(sound);
            return done;
        }
        if let Some((index, _)) = self.slide {
            let done = self.slide_key("activate_focused_item", None, block, pieces, preview);
            self.slide = Some((index, 0));
            return done;
        }
        None
    }

    /// A word of the trim zoom clicked: the taken edge goes to it (IN to its start, OUT to its
    /// end), within the taken part, drafted as the arrows do; the zoom follows the edge.
    fn zoom_word(&mut self, target: Option<&str>, block: &Block) -> Option<bool> {
        let (from, to) = (u64::try_from(self.value?).ok()?, target?.parse::<u64>().ok()?);
        let Some(trim) = self.trim.filter(|trim| trim.roll.is_none()) else { return Some(true) };
        let drafted = self.drafted(block.clone());
        let part = view::subsegment_view(&drafted, None).parts.get(trim.index).cloned()?;
        let piece = view::block_pieces(&drafted).get(trim.index).cloned()?;
        let source = |axis: u64| piece.from_frame + axis.clamp(part.start, part.end).saturating_sub(part.start);
        // The one meaning of a word (`qnc-edit-words`): the taken edge goes to it.
        let goal = source(qnc_edit_words::Word::new(from, to).goal(Some(qnc_edit_edge::Edge::of_out(trim.out)), 0));
        let (new_from, new_to) = if trim.out { (trim.from, goal) } else { (goal, trim.to) };
        if new_to <= new_from {
            return Some(true);
        }
        self.trim = Some(Trim { from: new_from, to: new_to, ..trim });
        let drafted = self.drafted(block.clone());
        let edge = view::subsegment_view(&drafted, None).parts.get(trim.index).map_or(0, |part| if trim.out { part.end.saturating_sub(1) } else { part.start });
        self.pieces_request = Some((view::piece_inputs(&drafted), edge));
        Some(true)
    }

    fn write_trim(&mut self, preview: &SourcePreview, block: &Block, pieces: &[Piece], trim: Trim) -> bool {
        let id = block.id;
        let mut writes = Vec::new();
        if let Some(piece) = pieces.get(trim.index).filter(|piece| (piece.from_frame, piece.to_frame) != (trim.from, trim.to)) {
            writes.push((trim.index, self.trimmed_piece(piece, trim)));
        }
        if let (Some(from), Some(next)) = (trim.roll, pieces.get(trim.index + 1)) {
            if from != next.from_frame {
                writes.push((trim.index + 1, self.trimmed_piece(next, Trim { index: trim.index + 1, out: false, from, to: next.to_frame, roll: None })));
            }
        }
        if writes.is_empty() {
            return true;
        }
        // A roll is one write and one step of undo (audit 2026-11-08 #15).
        let changes = writes.into_iter().map(|(index, piece)| qnc_script_db::Operation::SetPiece { id, index, piece }).collect();
        self.write(preview, move |target| qnc_script_db::together(target, changes).map(|_| None))
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
        trimmed_piece(&self.names, piece, trim.from, trim.to)
    }

    /// A write for the owner's database (the script writes it; the order is kept).
    fn write(&mut self, _preview: &SourcePreview, write: impl FnOnce(&ProjectDbTarget) -> Result<Option<i64>, String> + Send + 'static) -> bool {
        self.writes.push(Box::new(write));
        true
    }

    /// The writes to make, in order, once.
    pub fn take_writes(&mut self) -> Vec<Write> {
        std::mem::take(&mut self.writes)
    }

    /// The script's blocks as last read, the clips of the pool, the project and whether nothing
    /// is being read or written (a block gone is then gone).
    pub fn sync(&mut self, blocks: &[Block], names: &HashMap<String, (String, u64, f64)>, project: Option<&String>, settled: bool) {
        if self.blocks.as_slice() != blocks {
            self.blocks = blocks.to_vec();
        }
        if &self.names != names {
            self.names = names.clone();
        }
        if self.project.as_ref() != project {
            self.project = project.cloned();
        }
        self.settled = settled;
    }

    /// Whether a SubSegment is open.
    pub fn is_open(&self) -> bool {
        self.open_subsegment.is_some()
    }

    /// Whether its pictures or filmstrip are being read (the form repaints soon).
    pub fn busy(&self) -> bool {
        self.trim_stills.loading() || self.filmstrips.reading()
    }

    /// The parts the player should play and the frame to show, once.
    pub fn take_pieces_request(&mut self) -> Option<(Vec<qnc_source_preview::PieceInput>, u64)> {
        self.pieces_request.take()
    }

    /// Follows the open SubSegment as its block holds it now and the player plays it.
    pub fn poll(&mut self, preview: &SourcePreview) {
        if self.open_subsegment.is_some() {
            self.follow_subsegment(preview);
        }
    }

    /// What is taken is let go (the statement's text was cut, its parts may have moved).
    pub fn let_go(&mut self) {
        (self.trim, self.sound, self.chosen, self.join_sound, self.slide) = (None, None, None, None, None);
        self.level = None;
    }

    /// A pointer on the SubSegment's timeline comes as the mouse map's row action (AGENTS 0.12):
    /// the SubSegment's own action of it, with the frame or part it points at.
    pub fn own(&self, action_id: &str, target: Option<&str>) -> Option<(String, Option<String>)> {
        let view = self.open_subsegment.and(self.view.subsegment.as_ref())?;
        // A click while the level curve is taken writes it and places the playhead: the curve
        // stays taken, the next point is there (as on a Wrap row).
        if self.level_taken() && matches!(action_id, "timeline_cue_frame" | "segments_keep_edit") {
            return Some((LEVEL_KEEP_AT.to_string(), self.value.map(|frame| frame.to_string())));
        }
        let holding = self.trim.is_some() || self.sound.is_some() || self.join_sound.is_some() || self.slide.is_some() || self.level_taken();
        own_action(view, action_id, self.value, target, holding)
    }

    /// The keys of the open SubSegment (its trim, slide, slip, join and sound keys); `None` for
    /// any other key or with nothing open.
    pub fn key(&mut self, action_id: &str, target: Option<&str>, preview: &SourcePreview) -> Option<bool> {
        self.open_subsegment?;
        let done = self.trim_key(action_id, target, preview);
        if qnc_dev_diagnostics::player_diagnostics_enabled() {
            let trim = self.trim.map(|trim| format!("{}:{}{}", trim.index, if trim.out { "out" } else { "in" }, if trim.roll.is_some() { ":cut" } else { "" }));
            qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("subsegment-key action={action_id} target={target:?} done={done:?} trim={trim:?} sound={:?}", self.sound.map(|sound| sound.index)));
        }
        done
    }

    /// A statement of several parts opens as a SubSegment in place of the source timeline;
    /// `false` for a statement of one range.
    pub fn open(&mut self, id: i64) -> bool {
        let Some(block) = self.block(Some(id)).filter(|block| block.pieces.len() > 1).cloned() else { return false };
        self.open_as_subsegment(&block, 0);
        true
    }

    /// A word of a statement (block `id`, part `index`, its frames `frame..word_to`): an edge
    /// of this part taken goes to it (IN to its start, OUT to its end), drafted as the arrows do;
    /// otherwise the statement opens (one part or more) with the playhead on the word.
    pub fn word_cue(&mut self, id: i64, index: usize, frame: u64, word_to: u64) -> bool {
        let Some(block) = self.block(Some(id)).cloned() else { return false };
        if let Some(trim) = self.trim.filter(|trim| self.open_subsegment == Some(id) && trim.index == index && trim.roll.is_none()) {
            // The one meaning of a word (`qnc-edit-words`): the taken edge goes to it.
            let goal = qnc_edit_words::Word::new(frame, word_to).goal(Some(qnc_edit_edge::Edge::of_out(trim.out)), 0);
            let (from, to) = if trim.out { (trim.from, goal) } else { (goal, trim.to) };
            if to > from {
                self.trim = Some(Trim { from, to, ..trim });
                let drafted = self.drafted(block.clone());
                let edge = view::subsegment_view(&drafted, None).parts.get(index).map_or(0, |part| if trim.out { part.end.saturating_sub(1) } else { part.start });
                self.pieces_request = Some((qnc_script_draft::piece_inputs(&drafted), edge));
            }
            return true;
        }
        let pieces = qnc_script_draft::block_pieces(&block);
        let (Some(piece), Some(part)) = (pieces.get(index), view::subsegment_view(&block, None).parts.get(index).cloned()) else { return false };
        let at = part.start + frame.saturating_sub(piece.from_frame).min(part.end.saturating_sub(part.start + 1));
        if self.open_subsegment == Some(id) {
            self.pieces_request = Some((qnc_script_draft::piece_inputs(&block), at));
        } else {
            self.open_as_subsegment(&block, at);
        }
        true
    }

    /// The SubSegment's actions from its panel: the playhead, closing, a part's own A1, its
    /// place and leaving, the pause and join before it. `None` for any other.
    pub fn act(&mut self, action_id: &str, target: Option<&str>, preview: &SourcePreview) -> Option<bool> {
        Some(match action_id {
            SUBSEGMENT_CUE => {
                let (Some(block), Some(frame)) = (self.block(self.open_subsegment), target.and_then(|frame| frame.parse::<u64>().ok())) else { return Some(false) };
                self.pieces_request = Some((qnc_script_draft::piece_inputs(block), frame));
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
                let Some(mut piece) = self.block(Some(id)).map(qnc_script_draft::block_pieces).and_then(|pieces| pieces.get(index).cloned()) else { return Some(false) };
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
                self.let_go();
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
            _ => return None,
        })
    }
}

/// Seconds of `frames` of clip `clip_id` (by its frames and seconds in `names`).
pub fn seconds_of(names: &HashMap<String, (String, u64, f64)>, clip_id: &str, frames: u64) -> f64 {
    names.get(clip_id).filter(|(_, all, seconds)| *all > 0 && *seconds > 0.0).map_or(0.0, |(_, all, seconds)| frames as f64 * seconds / *all as f64)
}

/// A part with the range `[from, to)`: its timecodes and length moved with it.
pub fn trimmed_piece(names: &HashMap<String, (String, u64, f64)>, piece: &Piece, from: u64, to: u64) -> Piece {
    let fps = names.get(&piece.clip_id).filter(|(_, _, seconds)| *seconds > 0.0).map_or(0, |(_, frames, seconds)| (*frames as f64 / seconds).round() as u32);
    let moved = |label: &str, was: u64, now: u64| match qnc_source_timecode::parse_timecode(label, fps).filter(|_| fps > 0) {
        Some(frames) => qnc_source_timecode::format_frames((frames + now).saturating_sub(was), fps),
        None => label.to_string(),
    };
    Piece {
        from_frame: from,
        to_frame: to,
        start_tc: moved(&piece.start_tc, piece.from_frame, from),
        end_tc: moved(&piece.end_tc, piece.to_frame, to),
        length: if fps > 0 { qnc_source_timecode::format_frames(to - from, fps) } else { (to - from).to_string() },
        seconds: seconds_of(names, &piece.clip_id, to - from),
        ..piece.clone()
    }
}

#[cfg(test)]
mod tests;
