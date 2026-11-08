//! The controls of an open SubSegment in its text (user 2026-11-08): the row of a part carries
//! its A1, place and leaving; the row between two parts is their join (pause, cut or crossfade,
//! smooth). They ask for the same actions the SubSegment's edit takes; nothing is drawn over
//! the timeline or its header.

use eframe::egui::{self, RichText};
use qnc_action_event::ActionEvent;
use qnc_script::{SubsegmentPart, SubsegmentView, JOIN_CUT, JOIN_GAP, JOIN_SMOOTH, PANEL, PART_CHANNEL, PART_LEVEL, PART_MOVE, PART_OFF, PART_REMOVE};

fn event(open: &SubsegmentView, action: &str, index: usize, step: Option<i64>) -> ActionEvent {
    let target = match step {
        Some(step) => format!("{}:{index}:{step}", open.block_id),
        None => format!("{}:{index}", open.block_id),
    };
    ActionEvent::key(action).with_target(target).at(PANEL)
}

/// Its A1 (camera channel, level, on or off), its place among the parts and its leaving, beside
/// its `detail` (clip and timecodes).
pub fn part_row(ui: &mut egui::Ui, open: &SubsegmentView, index: usize, part: &SubsegmentPart, detail: Option<&str>, theme: &qnc_desktop_theme::Theme) -> Option<ActionEvent> {
    let mut asked = None;
    ui.horizontal_wrapped(|ui| {
        // The part whose A1 is taken (Ctrl+1) is marked as a taken lane is.
        let color = if open.sound == Some(index) { theme.focus } else { theme.text_muted };
        if let Some(detail) = detail {
            ui.label(RichText::new(detail).color(color).size(theme.font_ui));
        }
        ui.label(RichText::new("A1").color(color)).on_hover_text(qnc_ui_text::text("subsegment.channel_hint"));
        for channel in 0..part.channels.unwrap_or(2).max(1) {
            if ui.selectable_label(part.a1_channel == Some(channel), (channel + 1).to_string()).clicked() {
                asked = Some(event(open, PART_CHANNEL, index, Some(i64::from(channel))));
            }
        }
        if ui.small_button("-").clicked() {
            asked = Some(event(open, PART_LEVEL, index, Some(-1)));
        }
        ui.label(RichText::new(format!("{:+} dB", part.a1_level_db)).color(theme.text));
        if ui.small_button("+").clicked() {
            asked = Some(event(open, PART_LEVEL, index, Some(1)));
        }
        let off = qnc_ui_text::text(if part.a1_off { "subsegment.a1_off" } else { "subsegment.a1_on" });
        if ui.small_button(off).clicked() {
            asked = Some(event(open, PART_OFF, index, None));
        }
        ui.separator();
        // Its place among the parts and its leaving (user 2026-10-29).
        if qnc_ui_kit::small_button(ui, qnc_ui_text::text("subsegment.move_earlier"), index > 0, false).on_hover_text(qnc_ui_text::text("subsegment.move_hint")).clicked() {
            asked = Some(event(open, PART_MOVE, index, Some(-1)));
        }
        if qnc_ui_kit::small_button(ui, qnc_ui_text::text("subsegment.move_later"), index + 1 < open.parts.len(), false).on_hover_text(qnc_ui_text::text("subsegment.move_hint")).clicked() {
            asked = Some(event(open, PART_MOVE, index, Some(1)));
        }
        if qnc_ui_kit::small_button(ui, qnc_ui_text::text("subsegment.remove"), open.parts.len() > 1, false).on_hover_text(qnc_ui_text::text("subsegment.remove_hint")).clicked() {
            asked = Some(event(open, PART_REMOVE, index, None));
        }
    });
    asked
}

/// The join before part `index`: its pause, cut or crossfade and smooth cut.
pub fn join_row(ui: &mut egui::Ui, open: &SubsegmentView, index: usize, part: &SubsegmentPart, theme: &qnc_desktop_theme::Theme) -> Option<ActionEvent> {
    let mut asked = None;
    ui.horizontal_wrapped(|ui| {
        ui.add_space(24.0);
        ui.label(RichText::new(qnc_ui_text::fill("subsegment.join", &[("frames", &part.gap_before.to_string())])).color(theme.text_muted));
        if ui.small_button("-").clicked() && part.gap_before > 0 {
            asked = Some(event(open, JOIN_GAP, index, Some(-1)));
        }
        if ui.small_button("+").clicked() {
            asked = Some(event(open, JOIN_GAP, index, Some(1)));
        }
        let kind = qnc_ui_text::text(if part.cut { "subsegment.cut" } else { "subsegment.crossfade" });
        if ui.small_button(kind).on_hover_text(qnc_ui_text::text("subsegment.join_hint")).clicked() {
            asked = Some(event(open, JOIN_CUT, index, None));
        }
        // AI in-between pictures around the cut, two more or fewer; what it is doing beside it.
        let on = part.smooth_frames > 0;
        let label = if on { qnc_ui_text::fill("subsegment.smooth_on", &[("frames", &part.smooth_frames.to_string())]) } else { qnc_ui_text::text("subsegment.smooth").to_string() };
        if qnc_ui_kit::small_button(ui, &label, true, on).on_hover_text(qnc_ui_text::text("subsegment.smooth_hint")).clicked() {
            asked = Some(event(open, JOIN_SMOOTH, index, None));
        }
        if on {
            if ui.small_button("-").clicked() {
                asked = Some(event(open, JOIN_SMOOTH, index, Some(-1)));
            }
            if ui.small_button("+").clicked() {
                asked = Some(event(open, JOIN_SMOOTH, index, Some(1)));
            }
            let state = match part.smooth_ready {
                Some(true) => "subsegment.smooth_ready",
                Some(false) => "subsegment.smooth_making",
                None => "subsegment.smooth_cannot",
            };
            ui.label(RichText::new(qnc_ui_text::text(state)).color(theme.text_muted));
        }
    });
    asked
}
