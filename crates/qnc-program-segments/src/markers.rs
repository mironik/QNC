//! M markers and M-M slots of the program (docs/93 R15-R23, docs/94).
//!
//! Pure functions, program frames only. By default the program has two markers:
//! the start of the first segment (frame 0) and the end of the last segment
//! (program length). They are markers by position: never stored, never moved,
//! and a segment border is no marker. User markers may cross segment borders.
//! A slot is named after the two markers around it, so moving a marker keeps
//! the identity of both of its slots (docs/94 7a).

use qnc_content_store::ProgramMarker;

use crate::{SegmentRow, SegmentsView};

/// Name of the locked start marker in slot ids.
pub const PROGRAM_START: &str = "program_start";
/// Name of the locked end marker in slot ids.
pub const PROGRAM_END: &str = "program_end";

/// A user marker on the program axis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerPin {
    pub marker_id: String,
    pub frame: u64,
    pub selected: bool,
}

/// The program between two neighbouring M markers, `[start, end)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub slot_id: String,
    pub start_frame: u64,
    pub end_frame: u64,
    pub selected: bool,
}

/// The stored user markers strictly inside the program, one per frame (v5 keeps
/// them on their program frame when segments move).
pub fn resolve(
    view: &SegmentsView,
    stored: &[ProgramMarker],
    selected: Option<&str>,
) -> Vec<MarkerPin> {
    let mut pins = stored
        .iter()
        .filter(|marker| marker.program_frame > 0 && marker.program_frame < view.total_frames)
        .map(|marker| MarkerPin {
            marker_id: marker.marker_id.clone(),
            frame: marker.program_frame,
            selected: selected == Some(marker.marker_id.as_str()),
        })
        .collect::<Vec<_>>();
    pins.sort_by(|a, b| a.frame.cmp(&b.frame).then(a.marker_id.cmp(&b.marker_id)));
    pins.dedup_by_key(|pin| pin.frame);
    pins
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

/// M-M slots between the start, the markers and the end; empty ones are skipped.
pub fn slots(view: &SegmentsView, pins: &[MarkerPin], selected: Option<&str>) -> Vec<Slot> {
    if view.total_frames == 0 {
        return Vec::new();
    }
    let mut bounds = vec![(PROGRAM_START, 0u64)];
    bounds.extend(pins.iter().map(|pin| (pin.marker_id.as_str(), pin.frame)));
    bounds.push((PROGRAM_END, view.total_frames));
    bounds
        .windows(2)
        .filter(|pair| pair[1].1 > pair[0].1)
        .map(|pair| {
            let slot_id = format!("{}|{}", pair[0].0, pair[1].0);
            Slot {
                selected: selected == Some(slot_id.as_str()),
                slot_id,
                start_frame: pair[0].1,
                end_frame: pair[1].1,
            }
        })
        .collect()
}

/// The slot under a frame: `[start, end)`, and the last slot also at its end (docs/94 3.5).
pub fn slot_at(slots: &[Slot], frame: u64) -> Option<&Slot> {
    slots
        .iter()
        .find(|slot| (slot.start_frame..slot.end_frame).contains(&frame))
        .or_else(|| slots.last().filter(|slot| slot.end_frame == frame))
}

/// A new marker must lie strictly inside the program and on a free frame
/// (docs/94 section 1); the start and the end are locked.
pub fn check_new(view: &SegmentsView, pins: &[MarkerPin], frame: u64) -> Result<(), String> {
    if frame == 0 || frame >= view.total_frames {
        return Err("Pocetni i zavrsni M marker su zakljucani.".into());
    }
    if pins.iter().any(|pin| pin.frame == frame) {
        return Err("Na tom frameu vec postoji M marker.".into());
    }
    Ok(())
}

/// A moved marker stays between its neighbours (docs/94 7a).
pub fn check_move(
    view: &SegmentsView,
    pins: &[MarkerPin],
    marker_id: &str,
    frame: u64,
) -> Result<(), String> {
    let index = pins
        .iter()
        .position(|pin| pin.marker_id == marker_id)
        .ok_or("M marker nije pronadjen.")?;
    let low = index.checked_sub(1).map_or(0, |before| pins[before].frame);
    let high = pins
        .get(index + 1)
        .map_or(view.total_frames, |after| after.frame);
    if frame <= low || frame >= high {
        return Err("M marker se pomice samo izmedu susjednih markera.".into());
    }
    Ok(())
}

/// Previous or next marker from a frame (docs/94 3.2).
pub fn neighbour_marker(pins: &[MarkerPin], frame: u64, up: bool) -> Option<&MarkerPin> {
    if up {
        pins.iter().rev().find(|pin| pin.frame < frame)
    } else {
        pins.iter().find(|pin| pin.frame > frame)
    }
}

/// Previous or next slot from the selected one, else from the slot under the frame
/// (docs/94 3.3); no wrapping around.
pub fn neighbour_slot<'a>(slots: &'a [Slot], frame: u64, up: bool) -> Option<&'a Slot> {
    let from = slots
        .iter()
        .position(|slot| slot.selected)
        .or_else(|| slot_at(slots, frame).and_then(|at| slots.iter().position(|s| s == at)))?;
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
