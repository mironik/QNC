//! M markers and M-M slots of the program as stored by the content store (v5
//! `story_markers`, `story_marker_slots`; rule `qnc-story-segment-timeline.mdc`).
//!
//! Pure functions, program frames only. The locked start (frame 0) and end
//! (program length) are stored markers; segment borders are no markers. Slots
//! come from the database, named by their marker pair. Navigation follows v5
//! `editorial/segment_program.rs`.

use qnc_program_db::{ProgramMarker, ProgramSlot};

use crate::{SegmentRow, SegmentsView};

/// What of a taken M marker's cut moves (user 2026-10-22: the whole marker is red, only its
/// picture's part or only its sound's part when one of them is taken).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TakenPart {
    #[default]
    Whole,
    Picture,
    /// Its A2 sound, both sides at once (Ctrl+2).
    Sound,
    /// Only the J of the cover after (Ctrl+I, a click on that line; user 2026-10-29).
    SoundLead,
    /// Only the L of the cover before (Ctrl+O, a click on that line).
    SoundTail,
}

/// An M marker on the program axis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerPin {
    pub marker_id: String,
    pub frame: u64,
    /// The locked program start or end marker.
    pub locked: bool,
    pub selected: bool,
    /// Taken for a move (Ctrl+M, a right click): the mouse drags only it.
    pub taken: bool,
    /// Its part that moves, while taken.
    pub taken_part: TakenPart,
    /// While taken: where it is stored (drawn white, user 2026-10-22).
    pub origin: Option<u64>,
    /// Its A2 sound (user 2026-10-29: J and L together): where the sound of the cover before
    /// ends (`sound_tail`, its L) and where the sound of the cover after starts (`sound_lead`,
    /// its J); `None` for a side with no cover.
    pub sound_tail: Option<u64>,
    pub sound_lead: Option<u64>,
    /// The cut between two clips on A1 (user 2026-10-29: "every cut is a marker, the join of
    /// two clips is a marker, the OUT of the one before and the IN of the one after"), not an M
    /// marker: its id is `join:` and the clip after; it has no slot.
    pub join: bool,
    /// The A1 sound at this cut when it lies between two clips: where the sound of the clip
    /// before ends (its L) and where the sound of the clip after starts (its J).
    pub a1_tail: Option<u64>,
    pub a1_lead: Option<u64>,
    /// The lane whose sound moves while its sound is taken: 0 A1, 1 A2.
    pub sound_lane: u8,
}

/// The id of the cut before the clip `segment_id` on A1.
pub const JOIN_PREFIX: &str = "join:";

/// The clip after the cut of a join pin's id; `None` for an M marker.
pub fn join_segment(marker_id: &str) -> Option<&str> {
    marker_id.strip_prefix(JOIN_PREFIX)
}

/// The cut between two clips of the user on A1 at `frame`: the clip before and the one after.
pub fn join_at(view: &SegmentsView, frame: u64) -> Option<(&SegmentRow, &SegmentRow)> {
    view.rows.windows(2).find_map(|pair| {
        let (before, after) = (&pair[0], &pair[1]);
        (!before.lead && !after.lead && before.end_frame == frame && after.start_frame == frame).then_some((before, after))
    })
}

/// The cuts between two clips on A1 where no M marker stands (an M marker on such a cut is
/// that cut, user 2026-10-29).
pub fn join_pins(view: &SegmentsView, markers: &[MarkerPin]) -> Vec<MarkerPin> {
    view.rows
        .windows(2)
        .filter(|pair| !pair[0].lead && !pair[1].lead)
        .map(|pair| (pair[1].start_frame, &pair[1].segment_id))
        .filter(|(frame, _)| !markers.iter().any(|pin| pin.frame == *frame))
        .map(|(frame, segment_id)| MarkerPin {
            marker_id: format!("{JOIN_PREFIX}{segment_id}"),
            frame,
            join: true,
            ..MarkerPin::at(frame)
        })
        .collect()
}

impl MarkerPin {
    fn at(frame: u64) -> Self {
        Self {
            marker_id: String::new(),
            frame,
            locked: false,
            selected: false,
            taken: false,
            taken_part: TakenPart::Whole,
            origin: None,
            sound_tail: None,
            sound_lead: None,
            join: false,
            a1_tail: None,
            a1_lead: None,
            sound_lane: 1,
        }
    }
}

/// The program between two neighbouring M markers, `[start, end)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub slot_id: String,
    pub start_frame: u64,
    pub end_frame: u64,
    pub has_cover: bool,
    pub selected: bool,
}

/// The stored markers inside the program, by frame.
pub fn resolve(
    view: &SegmentsView,
    stored: &[ProgramMarker],
    selected: Option<&str>,
) -> Vec<MarkerPin> {
    let mut pins = stored
        .iter()
        .filter(|marker| marker.program_frame <= view.total_frames && !view.is_empty())
        .map(|marker| MarkerPin {
            marker_id: marker.marker_id.clone(),
            locked: !marker.system_role.is_empty(),
            selected: selected == Some(marker.marker_id.as_str()),
            ..MarkerPin::at(marker.program_frame)
        })
        .collect::<Vec<_>>();
    pins.sort_by(|a, b| a.frame.cmp(&b.frame).then(a.marker_id.cmp(&b.marker_id)));
    pins
}

/// The stored slots with the current selection.
pub fn slots(stored: &[ProgramSlot], selected: Option<&str>) -> Vec<Slot> {
    stored
        .iter()
        .map(|slot| Slot {
            slot_id: slot.slot_id.clone(),
            start_frame: slot.start_frame,
            end_frame: slot.end_frame,
            has_cover: slot.has_cover,
            selected: selected == Some(slot.slot_id.as_str()),
        })
        .collect()
}

/// Story frames and source frames of a segment: equal, or 2:1 either way when its
/// source has twice or half the story rate (user 2026-10-09).
fn lengths(segment: &SegmentRow) -> (u64, u64) {
    let story = segment.end_frame.saturating_sub(segment.start_frame).max(1);
    let source = segment.source_out_frame.saturating_sub(segment.source_in_frame).max(1);
    (story, source)
}

/// Program frame of a picture of a segment, if the picture is inside it.
pub fn program_frame(segment: &SegmentRow, source_frame: u64) -> Option<u64> {
    let (story, source) = lengths(segment);
    (segment.source_in_frame..segment.source_out_frame)
        .contains(&source_frame)
        .then(|| segment.start_frame + (source_frame - segment.source_in_frame) * story / source)
}

/// The segment and picture at a program frame; the last frame belongs to the last segment.
pub fn source_at(view: &SegmentsView, frame: u64) -> Option<(&SegmentRow, u64)> {
    let segment = view.segment_at(frame)?;
    let local = frame.min(segment.end_frame.saturating_sub(1)) - segment.start_frame;
    let (story, source) = lengths(segment);
    Some((segment, segment.source_in_frame + local * source / story))
}

/// v5 `marker_slot_at_program_frame`: `[start, end)`, and the last slot also at its end.
pub fn slot_at(slots: &[Slot], frame: u64) -> Option<&Slot> {
    slots
        .iter()
        .find(|slot| (slot.start_frame..slot.end_frame).contains(&frame))
        .or_else(|| slots.last().filter(|slot| slot.end_frame == frame))
}

/// v5 `first_empty_marker_slot`.
pub fn first_empty_slot(slots: &[Slot]) -> Option<&Slot> {
    slots.iter().find(|slot| !slot.has_cover)
}

/// docs/94 7a (user decision): a selected marker moves only between its
/// neighbours; the locked start and end do not move.
pub fn check_move(pins: &[MarkerPin], marker_id: &str, frame: u64) -> Result<(), String> {
    let index = pins
        .iter()
        .position(|pin| pin.marker_id == marker_id)
        .ok_or(qnc_ui_text::text("story.msg.marker_not_found"))?;
    if pins[index].locked {
        return Err(qnc_ui_text::text("story.msg.edge_markers_locked").into());
    }
    let low = index.checked_sub(1).map_or(0, |before| pins[before].frame);
    let high = pins.get(index + 1).map_or(u64::MAX, |after| after.frame);
    if frame <= low || frame >= high {
        return Err(qnc_ui_text::text("story.msg.marker_between_neighbours").into());
    }
    Ok(())
}

/// v5 `previous_marker_for_playhead` / `next_marker_for_playhead`.
pub fn neighbour_marker(pins: &[MarkerPin], frame: u64, up: bool) -> Option<&MarkerPin> {
    if up {
        pins.iter().rev().find(|pin| pin.frame < frame)
    } else {
        pins.iter().find(|pin| pin.frame > frame)
    }
}

/// v5 `adjacent_marker_slot`: from the selected slot, else the slot under the
/// frame, else the first empty slot; no wrapping around.
pub fn neighbour_slot(slots: &[Slot], frame: u64, up: bool) -> Option<&Slot> {
    let from = slots
        .iter()
        .position(|slot| slot.selected)
        .or_else(|| {
            slots
                .iter()
                .position(|slot| (slot.start_frame..slot.end_frame).contains(&frame))
        })
        .or_else(|| {
            first_empty_slot(slots).and_then(|empty| slots.iter().position(|slot| slot == empty))
        })
        .unwrap_or(0);
    if up {
        from.checked_sub(1).map(|index| &slots[index])
    } else {
        slots.get(from + 1)
    }
}

/// Previous or next segment from the one under a frame.
pub fn neighbour_segment(view: &SegmentsView, frame: u64, up: bool) -> Option<&SegmentRow> {
    let at = view.segment_at(frame)?;
    let index = view.rows.iter().position(|row| row == at)?;
    if up {
        index.checked_sub(1).map(|before| &view.rows[before])
    } else {
        view.rows.get(index + 1)
    }
}
