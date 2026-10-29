//! What the mouse takes of the M markers on a Wrap row, and of IN and OUT on a source timeline.
//! The one place of these rules (user rule 2026-10-20, locked; each rule has its test below;
//! the actions they stand for are the mouse map's, `qnc-mouse-map`):
//!
//! - A left click places the playhead; on a cover's colour strip (the strip that shows the
//!   cover's length) it selects the slot. It takes no marker.
//! - A right click takes the NEAREST marker, however far, never the locked start or end
//!   marker, and the playhead goes to it (`marker_for_right_click`); on the colour strip it
//!   takes the slot instead.
//! - Only the taken marker has a zone for the mouse: within `MARKER_REACH` it is red under
//!   the pointer and a drag moves it (`marker_to_drag`). There is no zone before a marker is
//!   taken (live 2026-10-20: a zone around every marker took the clicks on a short cover's
//!   strip). The same for IN and OUT: only a taken one is dragged.

use eframe::egui::{self, Rect};

use crate::{x_for_frame, TimelineMarkerPin};

/// The zone of the chosen marker: red under the pointer and dragged within this distance of
/// its line (live 2026-10-18: 8, then 16, then 24 pixels were too narrow).
pub(crate) const MARKER_REACH: f32 = 40.0;


/// The marker a RIGHT click at `pos` takes: none on a cover's colour strip (the click takes
/// the slot), elsewhere the nearest one that is not locked, however far.
pub(crate) fn marker_for_right_click<'a>(track: Rect, range_start: u64, duration: u64, pos: egui::Pos2, markers: &'a [TimelineMarkerPin<'a>], on_strip: bool) -> Option<&'a str> {
    if on_strip {
        return None;
    }
    markers
        .iter()
        .filter(|marker| !marker.locked && !marker.id.trim().is_empty())
        .map(|marker| (marker.id, (pos.x - x_for_frame(track, range_start, duration, marker.frame)).abs()))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

/// The taken marker when it is the nearest one to `pos` (a second right click on it takes its
/// picture or its sound alone, user 2026-10-22).
pub(crate) fn taken_marker_for_right_click<'a>(track: Rect, range_start: u64, duration: u64, pos: egui::Pos2, markers: &'a [TimelineMarkerPin<'a>]) -> Option<&'a str> {
    let nearest = marker_for_right_click(track, range_start, duration, pos, markers, false)?;
    markers.iter().any(|marker| marker.taken && marker.id == nearest).then_some(nearest)
}

/// The marker a drag at `pos` takes, and the one red under the pointer: only the taken
/// marker, within `MARKER_REACH` of its line.
pub(crate) fn marker_to_drag<'a>(track: Rect, range_start: u64, duration: u64, pos: egui::Pos2, markers: &'a [TimelineMarkerPin<'a>]) -> Option<&'a str> {
    markers
        .iter()
        .find(|marker| marker.taken && !marker.id.trim().is_empty())
        .filter(|marker| (pos.x - x_for_frame(track, range_start, duration, marker.frame)).abs() <= MARKER_REACH)
        .map(|marker| marker.id)
}

/// The marker nearest the pointer and its distance in pixels.
pub(crate) fn nearest<'a>(track: Rect, range_start: u64, duration: u64, pos: egui::Pos2, markers: &'a [TimelineMarkerPin<'a>]) -> Option<(&'a str, f32)> {
    markers
        .iter()
        .filter(|marker| !marker.id.trim().is_empty())
        .map(|marker| (marker.id, (pos.x - x_for_frame(track, range_start, duration, marker.frame)).abs()))
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

/// IN (`true`) or OUT the pointer at `pos` reaches on a source timeline, the nearer within
/// `MARKER_REACH` (user 2026-10-18: the same zone as the M markers). Frames are the row's
/// own; a mark that is not set is not reached.
pub(crate) fn source_mark_in_reach(track: Rect, range_start: u64, duration: u64, pos: egui::Pos2, (in_frame, in_set): (u64, bool), (out_frame, out_set): (u64, bool)) -> Option<bool> {
    let distance = |frame: u64| (pos.x - x_for_frame(track, range_start, duration, range_start + frame)).abs();
    let marks = [(true, in_frame, in_set), (false, out_frame, out_set)];
    marks
        .into_iter()
        .filter(|(_, _, set)| *set)
        .map(|(is_in, frame, _)| (is_in, distance(frame)))
        .filter(|(_, distance)| *distance <= MARKER_REACH)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(is_in, _)| is_in)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Locked (user 2026-10-20): a right click on a cover's colour strip is its slot, never a
    /// marker; elsewhere the nearest marker, however far.
    #[test]
    fn a_right_click_takes_the_nearest_marker_however_far_but_not_on_a_cover_strip() {
        let track = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 20.0)); // 10 px a frame
        let pin = |id, frame, locked| TimelineMarkerPin { id, frame, taken: false, locked, part: Default::default(), origin: None, sound_tail: None, sound_lead: None };
        let markers = [pin("a", 10, false), pin("b", 90, false)];
        assert_eq!(marker_for_right_click(track, 0, 100, egui::pos2(101.0, 5.0), &markers, true), None, "on the strip: the slot");
        assert_eq!(marker_for_right_click(track, 0, 100, egui::pos2(400.0, 5.0), &markers, false), Some("a"), "300 px away, the nearer");
        assert_eq!(marker_for_right_click(track, 0, 100, egui::pos2(700.0, 5.0), &markers, false), Some("b"));
        // The locked start and end markers are never taken: the nearest open one is.
        let edges = [pin("start", 0, true), pin("m", 50, false), pin("end", 100, true)];
        assert_eq!(marker_for_right_click(track, 0, 100, egui::pos2(20.0, 5.0), &edges, false), Some("m"), "beside the start marker");
        assert_eq!(marker_for_right_click(track, 0, 100, egui::pos2(990.0, 5.0), &edges, false), Some("m"), "beside the end marker");
    }

    /// Locked (user 2026-10-20): only the taken marker has a zone; it is dragged within it.
    #[test]
    fn only_the_taken_marker_has_a_zone_for_the_drag() {
        let track = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 20.0));
        let unchosen = [TimelineMarkerPin { id: "a", frame: 10, taken: false, locked: false, part: Default::default(), origin: None, sound_tail: None, sound_lead: None }];
        assert_eq!(marker_to_drag(track, 0, 100, egui::pos2(110.0, 5.0), &unchosen), None, "no zone before it is taken");
        let chosen = [TimelineMarkerPin { id: "a", frame: 10, taken: true, locked: false, part: Default::default(), origin: None, sound_tail: None, sound_lead: None }];
        assert_eq!(marker_to_drag(track, 0, 100, egui::pos2(130.0, 5.0), &chosen), Some("a"), "within the zone");
        assert_eq!(marker_to_drag(track, 0, 100, egui::pos2(160.0, 5.0), &chosen), None, "past it");
    }

    #[test]
    fn in_and_out_are_reached_in_the_zone_of_the_markers_the_nearer_one() {
        let track = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 20.0)); // 10 px a frame
        let marks = ((10, true), (20, true)); // IN at 100 px, OUT at 200 px
        assert_eq!(source_mark_in_reach(track, 0, 100, egui::pos2(130.0, 5.0), marks.0, marks.1), Some(true), "30 px from IN");
        assert_eq!(source_mark_in_reach(track, 0, 100, egui::pos2(160.0, 5.0), marks.0, marks.1), Some(false), "nearer OUT");
        assert_eq!(source_mark_in_reach(track, 0, 100, egui::pos2(300.0, 5.0), marks.0, marks.1), None, "out of the zone");
        assert_eq!(source_mark_in_reach(track, 0, 100, egui::pos2(100.0, 5.0), (10, false), marks.1), None, "IN not set");
    }
}
