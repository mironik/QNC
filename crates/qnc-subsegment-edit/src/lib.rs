//! A run of source parts edited on one flat timeline as covers are edited (user 2026-10-29):
//! Ctrl+I / Ctrl+O (or a right click, or a picture of the trim zoom) take an edge, arrows, the
//! wheel and the mouse move it, Enter keeps it and Esc gives it up; Ctrl+1 takes a part's A1
//! (channel, level, off); a part changes places or leaves; the pause and join before it change.
//! Every change plays at once through the one player as a program of the parts.
//!
//! The parts are their owner's (a SubSegment of the script, the S-Story skeleton): this piece
//! writes nothing. What a change asks to be written comes back as a `PieceWrite`; the owner
//! writes it and gives the parts again.

mod view;

use std::collections::HashMap;

use qnc_program_input::PieceInput;
use qnc_script_db::Piece;
use qnc_source_preview::SourcePreview;

pub use view::*;

pub const MODULE_ID: &str = "qnc.module.subsegment-edit";
/// The playhead on frame `target` (a click on the timeline).
pub const SUBSEGMENT_CUE: &str = "subsegment_cue";
/// The pause before a part one frame longer or shorter: `target` is `key:part:1` or `-1`.
pub const JOIN_GAP: &str = "subsegment_join_gap";
/// The join before a part a cut or a crossfade: `target` is `key:part`.
pub const JOIN_CUT: &str = "subsegment_join_cut";
/// The parts close; the source timeline is back.
pub const SUBSEGMENT_CLOSE: &str = "subsegment_close";
/// An edge of a part is taken for trimming: `target` is `part:in` or `part:out`.
pub const TAKE_EDGE: &str = "subsegment_take_edge";
/// The taken edge dragged by the mouse to a frame of the axis: `part:in:frame` or `part:out:frame`.
pub const DRAG_EDGE: &str = "subsegment_drag_edge";
/// The camera channel a part is heard on (A1): `target` is `key:part:channel` (zero based).
pub const PART_CHANNEL: &str = "subsegment_part_channel";
/// A part's A1 level one dB up or down: `target` is `key:part:1` or `-1`.
pub const PART_LEVEL: &str = "subsegment_part_level";
/// A part's A1 off or on: `target` is `key:part`.
pub const PART_OFF: &str = "subsegment_part_off";
/// A part changes places with the part before (`key:part:-1`) or after (`key:part:1`) it.
pub const PART_MOVE: &str = "subsegment_part_move";
/// A part leaves: `target` is `key:part`.
pub const PART_REMOVE: &str = "subsegment_part_remove";

/// Name, frames and seconds of each clip (by its id).
pub type Clips = HashMap<String, (String, u64, f64)>;

/// A change the owner writes.
#[derive(Debug, Clone, PartialEq)]
pub enum PieceWrite {
    Set { index: usize, piece: Piece },
    Move { index: usize, earlier: bool },
    Remove { index: usize },
    Join { index: usize, gap: u64, cut: bool },
}

/// What an action did: changed what is shown (or not), or asks for a write.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    Done(bool),
    Write(PieceWrite),
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
}

pub struct SubsegmentEdit {
    /// What is open (a script block's id, the skeleton's key).
    key: Option<i64>,
    view: Option<SubsegmentView>,
    /// The parts the player should play and the frame it should show (once).
    pieces_request: Option<(Vec<PieceInput>, u64)>,
    trim: Option<Trim>,
    sound: Option<PartSound>,
    waves: qnc_program_waveform::ProgramWaves,
    filmstrips: qnc_program_filmstrip::ProgramFilmstrips,
    /// The project the waves and filmstrips read from.
    project: Option<String>,
    trim_stills: qnc_trim_stills::TrimStills,
    zoom_follow: qnc_program_view::ZoomFollow,
}

impl Default for SubsegmentEdit {
    fn default() -> Self {
        Self {
            key: None,
            view: None,
            pieces_request: None,
            trim: None,
            sound: None,
            waves: qnc_program_waveform::ProgramWaves::new(),
            filmstrips: qnc_program_filmstrip::ProgramFilmstrips::new(),
            project: None,
            trim_stills: qnc_trim_stills::TrimStills::new(),
            zoom_follow: Default::default(),
        }
    }
}

impl std::fmt::Debug for SubsegmentEdit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubsegmentEdit").field("key", &self.key).field("view", &self.view).finish_non_exhaustive()
    }
}

impl SubsegmentEdit {
    pub fn new() -> Self {
        Self::default()
    }

    /// What is open, if anything.
    pub fn key(&self) -> Option<i64> {
        self.key
    }

    pub fn view(&self) -> Option<&SubsegmentView> {
        self.view.as_ref()
    }

    /// Whether pictures or filmstrips are being read (the form repaints soon).
    pub fn busy(&self) -> bool {
        self.trim_stills.loading() || self.filmstrips.reading()
    }

    /// The parts the player should play and the frame to show, once.
    pub fn take_pieces_request(&mut self) -> Option<(Vec<PieceInput>, u64)> {
        self.pieces_request.take()
    }

    /// `pieces` open as `key`, the player plays them from their start.
    pub fn open(&mut self, key: i64, pieces: &[Piece]) {
        (self.trim, self.sound) = (None, None);
        self.key = Some(key);
        self.pieces_request = Some((qnc_script_draft::inputs_of(pieces), 0));
        self.view = Some(subsegment_view(key, pieces, None));
    }

    pub fn close(&mut self) {
        (self.key, self.view, self.trim, self.sound) = (None, None, None, None);
    }

    /// The open parts as their owner holds them now (`None`: gone) and the player plays them: a
    /// write that changed a pause or a join plays again where it stands; parts gone close it once
    /// nothing is read or written (`settled`); the player opening something else closes it.
    pub fn follow(&mut self, pieces: Option<&[Piece]>, settled: bool, clips: &Clips, preview: &SourcePreview, project: Option<&str>) {
        let Some(key) = self.key else { return };
        let Some(pieces) = pieces.filter(|pieces| !pieces.is_empty()) else {
            if settled {
                self.close();
            }
            return;
        };
        let playing = preview.shown_pieces();
        if playing.is_none() && self.pieces_request.is_none() {
            // The player opened something else (a clip of the pool): the source timeline is back.
            self.close();
            return;
        }
        let pieces = self.drafted(pieces.to_vec());
        let inputs = qnc_script_draft::inputs_of(&pieces);
        if playing.is_some_and(|playing| playing != inputs.as_slice()) && self.pieces_request.is_none() {
            self.pieces_request = Some((inputs, preview.program_frame().unwrap_or(0)));
        }
        let playhead = playing.and_then(|_| preview.program_frame());
        let mut shown = subsegment_view(key, &pieces, playhead);
        if self.project.as_deref() != project {
            self.waves.set_reader(preview.artifact_reader());
            self.filmstrips.set_reader(preview.artifact_reader());
            self.project = project.map(str::to_string);
        }
        let placements = subsegment_placements(&shown, &pieces);
        for (part, piece) in shown.parts.iter_mut().zip(&pieces) {
            part.channels = self.waves.channels(&piece.clip_id);
        }
        let frames = |clip: &str| clips.get(clip).map(|(_, frames, _)| *frames);
        shown.a1 = self.waves.peaks(shown.total_frames, &placements, &[], frames).a1;
        let at = subsegment_pictures(&shown, &pieces);
        shown.pictures = self.filmstrips.pictures(&at, frames, |_| None);
        shown.trim = self.trim.and_then(|trim| shown.parts.get(trim.index).map(|part| (trim.index, trim.out, if trim.out { part.end.saturating_sub(1) } else { part.start })));
        // The trim zoom of the taken edge: its pictures, fine wave and words come from the
        // same pieces as a cover's (`qnc-trim-stills`).
        let rate = pieces.first().and_then(|piece| clips.get(&piece.clip_id)).filter(|(_, _, seconds)| *seconds > 0.0).map_or(25, |(_, frames, seconds)| (*frames as f64 / seconds).round() as u64);
        let position = preview.playing_at().filter(|(program, _, _)| *program).map(|(_, frame, playing)| (frame, playing));
        let zoom = self.trim.and_then(|trim| subsegment_zoom(&shown, &pieces, (trim.index, trim.out), (rate, Some((&self.zoom_follow, position))), frames));
        shown.zoom = zoom.map(|mut zoom| {
            self.trim_stills.fill(&mut zoom, preview.still_locator(), None);
            Box::new(zoom)
        });
        shown.sound = self.sound.map(|sound| sound.index);
        self.view = Some(shown);
    }

    /// The parts with the part being trimmed and the A1 being chosen as drafted (the player plays
    /// the draft, user 2026-10-29: edited as a cover is, heard before Enter).
    fn drafted(&self, mut pieces: Vec<Piece>) -> Vec<Piece> {
        if let Some(trim) = self.trim {
            if let Some(piece) = pieces.get_mut(trim.index) {
                (piece.from_frame, piece.to_frame) = (trim.from, trim.to);
            }
        }
        if let Some(sound) = self.sound {
            if let Some(piece) = pieces.get_mut(sound.index) {
                (piece.a1_channel, piece.a1_level_db, piece.a1_off) = (sound.channel, sound.level_db, sound.off);
            }
        }
        pieces
    }

    fn play(&mut self, pieces: Vec<Piece>, at: u64) {
        self.pieces_request = Some((qnc_script_draft::inputs_of(&pieces), at));
    }

    fn playhead(&self) -> u64 {
        self.view.as_ref().and_then(|view| view.playhead).unwrap_or(0)
    }

    fn part_at_playhead(&self) -> usize {
        let at = self.playhead();
        self.view.as_ref().and_then(|view| view.parts.iter().position(|part| at < part.end)).unwrap_or(0)
    }

    /// The open parts' actions; `None` for any other (or when nothing is open). `pieces` are
    /// the open parts as their owner holds them.
    pub fn act(&mut self, action_id: &str, target: Option<&str>, pieces: &[Piece], clips: &Clips) -> Option<Answer> {
        let key = self.key?;
        if let Some(answer) = self.sound_key(action_id, pieces) {
            return Some(answer);
        }
        if let Some(answer) = self.trim_key(action_id, target, pieces, clips) {
            return Some(answer);
        }
        // `key:part[:value]`: an action of these parts.
        let ours = || {
            let mut parts = target.unwrap_or_default().split(':');
            let owner = parts.next()?.parse::<i64>().ok().filter(|owner| *owner == key)?;
            let index = parts.next()?.parse::<usize>().ok()?;
            let value = parts.next().and_then(|value| value.parse::<i64>().ok());
            Some((owner, index, value))
        };
        Some(match action_id {
            SUBSEGMENT_CUE => {
                let frame = target.and_then(|frame| frame.parse::<u64>().ok())?;
                self.play(pieces.to_vec(), frame);
                Answer::Done(true)
            }
            SUBSEGMENT_CLOSE => {
                self.close();
                Answer::Done(true)
            }
            // A part's own A1 (user 2026-10-29): written at once, the parts play again.
            PART_CHANNEL | PART_LEVEL | PART_OFF => {
                let (_, index, value) = ours()?;
                let mut piece = pieces.get(index)?.clone();
                let value = value.unwrap_or(0);
                match action_id {
                    PART_CHANNEL => piece.a1_channel = u16::try_from(value).ok(),
                    PART_LEVEL => piece.a1_level_db = (i64::from(piece.a1_level_db) + value).clamp(-40, 12) as i8,
                    _ => piece.a1_off = !piece.a1_off,
                }
                Answer::Write(PieceWrite::Set { index, piece })
            }
            // A part's place and its leaving (user 2026-10-29): a taken edge or sound is let go
            // (its part may have moved).
            PART_MOVE | PART_REMOVE => {
                let (_, index, value) = ours()?;
                (self.trim, self.sound) = (None, None);
                match action_id {
                    PART_REMOVE => Answer::Write(PieceWrite::Remove { index }),
                    _ => Answer::Write(PieceWrite::Move { index, earlier: value.unwrap_or(0) < 0 }),
                }
            }
            JOIN_GAP | JOIN_CUT => {
                let (_, index, value) = ours()?;
                let piece = pieces.get(index)?;
                let (gap, cut) = match action_id {
                    JOIN_GAP => (piece.gap_before.saturating_add_signed(value.unwrap_or(0)), piece.cut),
                    _ => (piece.gap_before, !piece.cut),
                };
                Answer::Write(PieceWrite::Join { index, gap, cut })
            }
            _ => return None,
        })
    }

    /// The sound keys of a part, as a cover's lane (user 2026-10-29): Ctrl+1 takes the A1 of the
    /// part under the playhead, up/down its camera channel, left off and right on, Ctrl+arrows
    /// and Ctrl+wheel its level a dB, Enter keeps it, Esc gives it up. Every key plays the
    /// draft at once.
    fn sound_key(&mut self, action_id: &str, pieces: &[Piece]) -> Option<Answer> {
        let key = qnc_timeline::LaneKey::of(action_id)?;
        let at = self.playhead();
        match key {
            qnc_timeline::LaneKey::Take(0) => {
                let index = self.part_at_playhead();
                let piece = pieces.get(index)?;
                self.trim = None;
                self.sound = Some(PartSound { index, channel: piece.a1_channel, level_db: piece.a1_level_db, off: piece.a1_off });
                return Some(Answer::Done(true));
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
                return Some(Answer::Write(PieceWrite::Set { index: sound.index, piece }));
            }
            qnc_timeline::LaneKey::Drop => {
                self.sound = None;
                self.play(pieces.to_vec(), at);
                return Some(Answer::Done(true));
            }
            _ => return None,
        }
        self.sound = Some(sound);
        let drafted = self.drafted(pieces.to_vec());
        self.play(drafted, at);
        Some(Answer::Done(true))
    }

    /// The trim keys (user 2026-10-29: parts are edited as covers are): Ctrl+I / Ctrl+O take the
    /// IN or OUT of the part under the playhead (a right click on a join takes the IN after it),
    /// arrows and the wheel move it a frame, Enter keeps it, Esc gives it up. Every move plays
    /// the draft at the edge.
    fn trim_key(&mut self, action_id: &str, target: Option<&str>, pieces: &[Piece], clips: &Clips) -> Option<Answer> {
        let take = |index: usize, out: bool| pieces.get(index).map(|piece| Trim { index, out, from: piece.from_frame, to: piece.to_frame });
        if action_id == DRAG_EDGE {
            return self.drag_edge(target, pieces, clips);
        }
        let step = match action_id {
            "select_mark_in" | "select_mark_out" => {
                let index = self.part_at_playhead();
                self.sound = None;
                self.trim = take(index, action_id == "select_mark_out");
                0
            }
            TAKE_EDGE => {
                let mut parts = target.unwrap_or_default().split(':');
                let index = parts.next().and_then(|index| index.parse::<usize>().ok())?;
                let out = parts.next() == Some("out");
                self.sound = None;
                // Another edge taken (a picture of the zoom clicked, user 2026-10-29): a moved
                // edge is kept first, the same part keeps its drafted range.
                match self.trim.take() {
                    Some(old) if old.index == index => self.trim = Some(Trim { out, ..old }),
                    Some(old) => {
                        self.trim = take(index, out);
                        if pieces.get(old.index).is_some_and(|piece| (piece.from_frame, piece.to_frame) != (old.from, old.to)) {
                            let piece = trimmed_piece(&pieces[old.index], old, clips);
                            return Some(Answer::Write(PieceWrite::Set { index: old.index, piece }));
                        }
                    }
                    None => self.trim = take(index, out),
                }
                0
            }
            "step_back_frame" | "step_forward_frame" if self.trim.is_some() => if action_id == "step_forward_frame" { 1 } else { -1 },
            "wheel_source_back" | "wheel_source_forward" if self.trim.is_some() => if qnc_wheel_jog::source_step_forward(action_id)? { 1 } else { -1 },
            "activate_focused_item" if self.trim.is_some() => {
                let trim = self.trim.take()?;
                let piece = trimmed_piece(pieces.get(trim.index)?, trim, clips);
                return Some(Answer::Write(PieceWrite::Set { index: trim.index, piece }));
            }
            "clear_focus" if self.trim.is_some() => {
                let at = self.edge_frame(pieces).unwrap_or(0);
                self.trim = None;
                self.play(pieces.to_vec(), at);
                return Some(Answer::Done(true));
            }
            _ => return None,
        };
        let trim = self.trim.as_mut()?;
        let length = clips.get(&pieces.get(trim.index)?.clip_id).map_or(u64::MAX, |(_, frames, _)| *frames);
        match trim.out {
            true => trim.to = trim.to.saturating_add_signed(step).clamp(trim.from + 1, length),
            false => trim.from = trim.from.saturating_add_signed(step).min(trim.to - 1),
        }
        let drafted = self.drafted(pieces.to_vec());
        let at = self.edge_frame(&drafted).unwrap_or(0);
        self.play(drafted, at);
        Some(Answer::Done(true))
    }

    /// The taken edge dragged by the mouse (user 2026-10-29): only the taken one moves, its part
    /// keeps at least a frame and its clip; the draft plays at the edge as with the arrows.
    fn drag_edge(&mut self, target: Option<&str>, pieces: &[Piece], clips: &Clips) -> Option<Answer> {
        let mut parts = target?.split(':');
        let (index, out, frame) = (parts.next()?.parse::<usize>().ok()?, parts.next()? == "out", parts.next()?.parse::<u64>().ok()?);
        // The part's start on the axis does not move while its own edges move.
        let start = subsegment_view(0, pieces, None).parts.get(index)?.start;
        let piece = pieces.get(index)?;
        let length = clips.get(&piece.clip_id).map_or(u64::MAX, |(_, frames, _)| *frames);
        let trim = self.trim.as_mut().filter(|trim| trim.index == index && trim.out == out)?;
        match out {
            true => trim.to = (trim.from + frame.saturating_sub(start)).clamp(trim.from + 1, length),
            false => trim.from = (piece.from_frame as i64 + frame as i64 - start as i64).clamp(0, trim.to as i64 - 1) as u64,
        }
        let drafted = self.drafted(pieces.to_vec());
        let at = self.edge_frame(&drafted).unwrap_or(0);
        self.play(drafted, at);
        Some(Answer::Done(true))
    }

    /// The frame of the trimmed edge on the axis (the last frame of an OUT).
    fn edge_frame(&self, pieces: &[Piece]) -> Option<u64> {
        let trim = self.trim?;
        let shown = subsegment_view(0, pieces, None);
        let part = shown.parts.get(trim.index)?;
        Some(if trim.out { part.end.saturating_sub(1) } else { part.start })
    }
}

/// The part with its trimmed range: its timecodes and length moved with it.
fn trimmed_piece(piece: &Piece, trim: Trim, clips: &Clips) -> Piece {
    let clip = clips.get(&piece.clip_id);
    let fps = clip.filter(|(_, _, seconds)| *seconds > 0.0).map_or(0, |(_, frames, seconds)| (*frames as f64 / seconds).round() as u32);
    let moved = |label: &str, from: u64, to: u64| match qnc_source_timecode::parse_timecode(label, fps).filter(|_| fps > 0) {
        Some(frames) => qnc_source_timecode::format_frames((frames + to).saturating_sub(from), fps),
        None => label.to_string(),
    };
    let frames = trim.to - trim.from;
    Piece {
        from_frame: trim.from,
        to_frame: trim.to,
        start_tc: moved(&piece.start_tc, piece.from_frame, trim.from),
        end_tc: moved(&piece.end_tc, piece.to_frame, trim.to),
        length: if fps > 0 { qnc_source_timecode::format_frames(frames, fps) } else { frames.to_string() },
        seconds: clip.filter(|(_, all, seconds)| *all > 0 && *seconds > 0.0).map_or(0.0, |(_, all, seconds)| frames as f64 * seconds / *all as f64),
        ..piece.clone()
    }
}

#[cfg(test)]
mod tests;
