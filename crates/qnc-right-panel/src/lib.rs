//! The right panel of an editorial board (user 2026-10-26: each group says in its layout
//! contract what its right panel is). It paints the public panel the kind names and gives its
//! answer; it holds no state and knows no form. A kind it does not know stays empty.

use eframe::egui;
use qnc_action_event::ActionEvent;

pub const MODULE_ID: &str = "qnc.module.right-panel";
pub const SEGMENT_PANEL: &str = "segment_panel";
pub const TRANSCRIPT_PANEL: &str = "transcript_panel";

/// What the right panels show.
pub struct Shown<'a> {
    pub segments: &'a qnc_program_view::SegmentsView,
    pub cover_lengths: &'a [qnc_segment_panel::CoverLengthBand],
    pub levels: [Option<f32>; 2],
    pub transcript: &'a qnc_transcript::TranscriptView,
}

/// The panel of `kind`, and whether it is the segment panel (it takes the keyboard focus).
pub fn show(ui: &mut egui::Ui, kind: &str, theme: &qnc_desktop_theme::Theme, shown: &Shown<'_>) -> (Option<ActionEvent>, bool) {
    match kind {
        SEGMENT_PANEL => (qnc_segment_panel::show(ui, shown.segments, theme, shown.cover_lengths, shown.levels), true),
        TRANSCRIPT_PANEL => (qnc_transcript_panel::show(ui, shown.transcript, theme), false),
        _ => (None, false),
    }
}
