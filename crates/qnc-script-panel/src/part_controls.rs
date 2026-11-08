//! The pause between two parts of an open SubSegment, in its text (user 2026-11-08: "leave only
//! the option of adding a gap"; everything else of a part is on the timeline). It asks for the
//! action the SubSegment's edit takes.

use eframe::egui::{self, RichText};
use qnc_action_event::ActionEvent;
use qnc_script::{SubsegmentPart, SubsegmentView, JOIN_GAP, JOIN_SMOOTH, PANEL};

/// The pause before part `index`, a frame shorter or longer.
pub fn gap_row(ui: &mut egui::Ui, open: &SubsegmentView, index: usize, part: &SubsegmentPart, theme: &qnc_desktop_theme::Theme) -> Option<ActionEvent> {
    let step = |by: i64| ActionEvent::key(JOIN_GAP).with_target(format!("{}:{index}:{by}", open.block_id)).at(PANEL);
    let mut asked = None;
    ui.horizontal(|ui| {
        ui.add_space(24.0);
        ui.label(RichText::new(qnc_ui_text::fill("subsegment.join", &[("frames", &part.gap_before.to_string())])).color(theme.text_muted));
        if ui.small_button("-").clicked() && part.gap_before > 0 {
            asked = Some(step(-1));
        }
        if ui.small_button("+").clicked() {
            asked = Some(step(1));
        }
        // The smooth cut on or off and nothing more (user 2026-11-08: "only what is needed");
        // while it is not ready, why.
        let on = part.smooth_frames > 0;
        if qnc_ui_kit::small_button(ui, qnc_ui_text::text("subsegment.smooth"), true, on).on_hover_text(qnc_ui_text::text("subsegment.smooth_hint")).clicked() {
            asked = Some(ActionEvent::key(JOIN_SMOOTH).with_target(format!("{}:{index}", open.block_id)).at(PANEL));
        }
        if on && part.smooth_ready != Some(true) {
            let state = if part.smooth_ready.is_some() { "subsegment.smooth_making" } else { "subsegment.smooth_cannot" };
            ui.label(RichText::new(qnc_ui_text::text(state)).color(theme.text_muted));
        }
    });
    asked
}
