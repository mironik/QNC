//! The reach of an M marker for the mouse: one wide zone for the red mark under the pointer,
//! a click and a drag (user 2026-10-18: the user must not look for a marker; as soon as the
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
