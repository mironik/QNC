//! The reach of an M marker for the mouse, one rule for a click, a drag and the red mark
//! under the pointer (user 2026-10-18: markers virtually wider, so the mouse need not be
//! precise; red while the pointer is in reach). A marker is taken within `MARKER_REACH` of
//! its line, the nearest one; a taken (or chosen) marker within `TAKEN_MARKER_REACH`.

use eframe::egui::{self, Rect};

use crate::{x_for_frame, TimelineMarkerPin};

/// How far from its line a marker is taken by a click.
pub(crate) const MARKER_REACH: f32 = 8.0;
/// How far from a taken marker a drag still takes it.
pub(crate) const TAKEN_MARKER_REACH: f32 = 40.0;

/// The marker the pointer at `pos` reaches: a taken one from further away, else the nearest.
pub(crate) fn marker_in_reach<'a>(track: Rect, range_start: u64, duration: u64, pos: egui::Pos2, markers: &'a [TimelineMarkerPin<'a>]) -> Option<&'a str> {
    taken_marker_near(track, range_start, duration, pos, markers).or_else(|| marker_hit(track, range_start, duration, pos, markers))
}

/// A taken marker within `TAKEN_MARKER_REACH` of the press: the drag moves it.
pub(crate) fn taken_marker_near<'a>(track: Rect, range_start: u64, duration: u64, pos: egui::Pos2, markers: &'a [TimelineMarkerPin<'a>]) -> Option<&'a str> {
    markers
        .iter()
        .filter(|marker| marker.taken && !marker.id.trim().is_empty())
        .find(|marker| (pos.x - x_for_frame(track, range_start, duration, marker.frame)).abs() <= TAKEN_MARKER_REACH)
        .map(|marker| marker.id)
}

pub(crate) fn marker_hit<'a>(
    track: Rect,
    range_start: u64,
    duration: u64,
    pos: egui::Pos2,
    markers: &'a [TimelineMarkerPin<'a>],
) -> Option<&'a str> {
    // Wide enough to take a marker by mouse (live 2026-10-18: a marker on the edge of a Wrap
    // row, one frame away, was a cue); on the row's edge the whole strip beside it counts.
    let tolerance = MARKER_REACH;
    markers
        .iter()
        .filter(|marker| !marker.id.trim().is_empty())
        .map(|marker| (marker.id, (pos.x - x_for_frame(track, range_start, duration, marker.frame)).abs()))
        .filter(|(_, distance)| *distance <= tolerance)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

