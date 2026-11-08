//! The controls of an open SubSegment in its text (user 2026-11-08). The text stays readable
//! ("a forest of data, confusing"): the sound of a part is not here, it is already chosen and
//! is changed on the timeline (Ctrl+1 or A1). Between two parts stands only how they join (cut
//! or crossfade, smooth); the part chosen on the timeline also shows its place and its leaving
//! and the pause of its joins. They ask for the same actions the SubSegment's edit takes.

use eframe::egui::{self, RichText};
use qnc_action_event::ActionEvent;
use qnc_script::{SubsegmentPart, SubsegmentView, JOIN_CUT, JOIN_GAP, JOIN_SMOOTH, PANEL, PART_MOVE, PART_REMOVE};

fn event(open: &SubsegmentView, action: &str, index: usize, step: Option<i64>) -> ActionEvent {
    let target = match step {
        Some(step) => format!("{}:{index}:{step}", open.block_id),
        None => format!("{}:{index}", open.block_id),
    };
    ActionEvent::key(action).with_target(target).at(PANEL)
}

/// The clip and timecodes of a part; the part chosen on the timeline is marked and has its place
/// among the parts and its leaving.
pub fn part_row(ui: &mut egui::Ui, open: &SubsegmentView, index: usize, detail: Option<&str>, theme: &qnc_desktop_theme::Theme) -> Option<ActionEvent> {
    let mut asked = None;
    let chosen = open.chosen == Some(index);
    ui.horizontal_wrapped(|ui| {
        if let Some(detail) = detail {
            ui.label(RichText::new(detail).color(if chosen { theme.focus } else { theme.text_muted }).size(theme.font_ui));
        }
        if chosen {
            if qnc_ui_kit::small_button(ui, qnc_ui_text::text("subsegment.move_earlier"), index > 0, false).on_hover_text(qnc_ui_text::text("subsegment.move_hint")).clicked() {
                asked = Some(event(open, PART_MOVE, index, Some(-1)));
            }
            if qnc_ui_kit::small_button(ui, qnc_ui_text::text("subsegment.move_later"), index + 1 < open.parts.len(), false).on_hover_text(qnc_ui_text::text("subsegment.move_hint")).clicked() {
                asked = Some(event(open, PART_MOVE, index, Some(1)));
            }
            if qnc_ui_kit::small_button(ui, qnc_ui_text::text("subsegment.remove"), open.parts.len() > 1, false).on_hover_text(qnc_ui_text::text("subsegment.remove_hint")).clicked() {
                asked = Some(event(open, PART_REMOVE, index, None));
            }
        }
    });
    asked
}

/// The join before part `index`: cut or crossfade and smooth; the numbers only beside the part
/// chosen on the timeline.
pub fn join_row(ui: &mut egui::Ui, open: &SubsegmentView, index: usize, part: &SubsegmentPart, theme: &qnc_desktop_theme::Theme) -> Option<ActionEvent> {
    let mut asked = None;
    let near = open.chosen == Some(index) || open.chosen == Some(index - 1);
    ui.horizontal_wrapped(|ui| {
        ui.add_space(24.0);
        if near {
            ui.label(RichText::new(qnc_ui_text::fill("subsegment.join", &[("frames", &part.gap_before.to_string())])).color(theme.text_muted));
            if ui.small_button("-").clicked() && part.gap_before > 0 {
                asked = Some(event(open, JOIN_GAP, index, Some(-1)));
            }
            if ui.small_button("+").clicked() {
                asked = Some(event(open, JOIN_GAP, index, Some(1)));
            }
        }
        let kind = qnc_ui_text::text(if part.cut { "subsegment.cut" } else { "subsegment.crossfade" });
        if ui.small_button(kind).on_hover_text(qnc_ui_text::text("subsegment.join_hint")).clicked() {
            asked = Some(event(open, JOIN_CUT, index, None));
        }
        // AI in-between pictures around the cut; what it is doing beside it.
        let on = part.smooth_frames > 0;
        let label = if on { qnc_ui_text::fill("subsegment.smooth_on", &[("frames", &part.smooth_frames.to_string())]) } else { qnc_ui_text::text("subsegment.smooth").to_string() };
        if qnc_ui_kit::small_button(ui, &label, true, on).on_hover_text(qnc_ui_text::text("subsegment.smooth_hint")).clicked() {
            asked = Some(event(open, JOIN_SMOOTH, index, None));
        }
        if on && near {
            if ui.small_button("-").clicked() {
                asked = Some(event(open, JOIN_SMOOTH, index, Some(-1)));
            }
            if ui.small_button("+").clicked() {
                asked = Some(event(open, JOIN_SMOOTH, index, Some(1)));
            }
        }
        if on && part.smooth_ready != Some(true) {
            let state = if part.smooth_ready.is_some() { "subsegment.smooth_making" } else { "subsegment.smooth_cannot" };
            ui.label(RichText::new(qnc_ui_text::text(state)).color(theme.text_muted));
        }
    });
    asked
}
