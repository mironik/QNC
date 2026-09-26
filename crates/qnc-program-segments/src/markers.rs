//! M markers and M-M slots of the program as stored by the content store (v5
//! `story_markers`, `story_marker_slots`; rule `qnc-story-segment-timeline.mdc`).
//!
//! Pure functions, program frames only. The locked start (frame 0) and end
//! (program length) are stored markers; segment borders are no markers. Slots
//! come from the database, named by their marker pair. Navigation follows v5
//! `editorial/segment_program.rs`.

use qnc_program_db::{ProgramMarker, ProgramSlot};

use crate::{SegmentRow, SegmentsView};

/// An M marker on the program axis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerPin {
    pub marker_id: String,
    pub frame: u64,
    /// The locked program start or end marker.
    pub locked: bool,
    pub selected: bool,
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
            frame: marker.program_frame,
            locked: !marker.system_role.is_empty(),
            selected: selected == Some(marker.marker_id.as_str()),
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

/// Program frame of a picture of a segment, if the picture is inside it.
pub fn program_frame(segment: &SegmentRow, source_frame: u64) -> Option<u64> {
    (segment.source_in_frame..segment.source_out_frame)
        .contains(&source_frame)
        .then(|| segment.start_frame + (source_frame - segment.source_in_frame))
}

/// The segment and picture at a program frame; the last frame belongs to the last segment.
pub fn source_at(view: &SegmentsView, frame: u64) -> Option<(&SegmentRow, u64)> {
    let segment = view.segment_at(frame)?;
    let local = frame.min(segment.end_frame.saturating_sub(1)) - segment.start_frame;
    Some((segment, segment.source_in_frame + local))
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
        .ok_or("M marker nije pronadjen.")?;
    if pins[index].locked {
        return Err("Početni i završni M marker su zaključani.".into());
    }
    let low = index.checked_sub(1).map_or(0, |before| pins[before].frame);
    let high = pins.get(index + 1).map_or(u64::MAX, |after| after.frame);
    if frame <= low || frame >= high {
        return Err("M marker se pomice samo izmedu susjednih markera.".into());
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
