//! The reach of an M marker, and of IN and OUT on a source timeline, for the mouse: one wide
//! zone for the red mark under the pointer, a click and a drag (user 2026-10-18: the user must not look for a marker; as soon as the
//! pointer enters its zone it turns red and can be dragged at once, the zone is what the
//! red shows). Of two markers in reach the nearer is taken.

use eframe::egui::{self, Rect};

use crate::{x_for_frame, TimelineMarkerPin};

/// How far from its line a marker is reached: red under the pointer, taken by a click or a
/// drag (live 2026-10-18: 8, then 16, then 24 pixels were too narrow).
pub(crate) const MARKER_REACH: f32 = 40.0;

/// The marker the pointer at `pos` reaches: the nearest within `MARKER_REACH`.
pub(crate) fn marker_in_reach<'a>(track: Rect, range_start: u64, duration: u64, pos: egui::Pos2, markers: &'a [TimelineMarkerPin<'a>]) -> Option<&'a str> {
    nearest(track, range_start, duration, pos, markers).filter(|(_, distance)| *distance <= MARKER_REACH).map(|(id, _)| id)
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
