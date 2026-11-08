//! The right column of an open SubSegment's table (user 2026-11-08: timecode, text, and on the
//! right the smooth cut and the pause): the join before a part. It asks for the actions the
//! SubSegment's edit takes.

use eframe::egui::{self, RichText};
use qnc_action_event::ActionEvent;
use qnc_script::{SubsegmentPart, SubsegmentView, JOIN_GAP, JOIN_SMOOTH, PANEL};

/// The smooth cut of the join before part `index`, on or off, and its pause a frame shorter or
/// longer; while the smooth cut is not ready, why.
pub fn join_cell(ui: &mut egui::Ui, open: &SubsegmentView, index: usize, part: &SubsegmentPart, theme: &qnc_desktop_theme::Theme) -> Option<ActionEvent> {
    let step = |by: i64| ActionEvent::key(JOIN_GAP).with_target(format!("{}:{index}:{by}", open.block_id)).at(PANEL);
    let mut asked = None;
    ui.horizontal_wrapped(|ui| {
        let on = part.smooth_frames > 0;
        if qnc_ui_kit::small_button(ui, qnc_ui_text::text("subsegment.smooth"), true, on).on_hover_text(qnc_ui_text::text("subsegment.smooth_hint")).clicked() {
            asked = Some(ActionEvent::key(JOIN_SMOOTH).with_target(format!("{}:{index}", open.block_id)).at(PANEL));
        }
        ui.label(RichText::new(qnc_ui_text::fill("subsegment.join", &[("frames", &part.gap_before.to_string())])).color(theme.text_muted));
        if ui.small_button("-").clicked() && part.gap_before > 0 {
            asked = Some(step(-1));
        }
        if ui.small_button("+").clicked() {
            asked = Some(step(1));
        }
        if on && part.smooth_ready != Some(true) {
            let state = if part.smooth_ready.is_some() { "subsegment.smooth_making" } else { "subsegment.smooth_cannot" };
            ui.label(RichText::new(qnc_ui_text::text(state)).color(theme.text_muted));
        }
    });
    asked
}
