//! Bottom source dock: a header row (clip label, IN / OUT / Trajanje and action
//! buttons) above a slot for the timeline. Passive paint (mirrors qnc_v4
//! `qnc_source_dock::show`, edit-actions variant). The caller draws the
//! timeline into the slot, supplies colours, labels and the button captions
//! from the UI contract, and gets back one intent (the button index). The
//! module keeps no state and knows no application.

mod timeline_dock;
pub use timeline_dock::{show_chrome_row, show_timeline_dock, SourceTimeline, TimelineDockStyle};

use eframe::egui::{self, Color32, RichText, Vec2};

/// Which timecode is highlighted (the focused mark of the timeline).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarkFocus {
    #[default]
    None,
    In,
    Out,
}

/// Colours and metrics from the UI contract (`source_dock` / theme).
#[derive(Debug, Clone, Copy)]
pub struct DockStyle {
    /// Face behind the header and the timeline.
    pub fill: Color32,
    pub chrome_fill: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    /// Highlight of the focused timecode.
    pub focus: Color32,
    /// Timecode value colour.
    pub timecode_value: Color32,
    pub font_ui: f32,
    pub font_timecode: f32,
    pub chrome_row_height: f32,
    pub chrome_control_height: f32,
    pub chrome_pad_x: i8,
    pub chrome_pad_y: i8,
    /// Horizontal inset shared with the workspace columns.
    pub inset_x: i8,
    pub header_timeline_gap: f32,
    pub header_item_gap: f32,
}

pub struct DockHeader<'a> {
    pub clip_label: &'a str,
    pub in_label: &'a str,
    pub out_label: &'a str,
    pub duration_label: &'a str,
    pub focus: MarkFocus,
    /// Button captions, right-to-left (first is rightmost), as in the contract.
    pub actions_rtl: &'a [&'a str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockAction {
    None,
    /// Index into `actions_rtl` of the clicked button.
    Button(usize),
}

/// Exact dock height: timeline outer height plus, when shown, the header row
/// and the gap under it.
pub fn dock_height(timeline_outer_height: f32, show_header: bool, style: &DockStyle) -> f32 {
    if show_header {
        timeline_outer_height + style.chrome_row_height + style.header_timeline_gap
    } else {
        timeline_outer_height
    }
}

/// Paints the dock. `add_timeline` draws the timeline into the remaining space.
pub fn show_dock(
    ui: &mut egui::Ui,
    style: &DockStyle,
    header: Option<&DockHeader<'_>>,
    add_timeline: impl FnOnce(&mut egui::Ui),
) -> DockAction {
    let mut action = DockAction::None;
    let framed = egui::Frame::NONE
        .fill(style.fill)
        .inner_margin(egui::Margin {
            left: style.inset_x,
            right: style.inset_x,
            top: 0,
            bottom: 0,
        })
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if let Some(header) = header {
                chrome_row(ui, style, |ui| {
                    // Right-to-left first: buttons keep their hit targets and
                    // the label group takes what is left.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = style.header_item_gap;
                        for (index, caption) in header.actions_rtl.iter().enumerate() {
                            if action_btn(ui, style, caption).clicked() {
                                action = DockAction::Button(index);
                            }
                        }
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(header.clip_label)
                                    .color(style.text)
                                    .strong()
                                    .size(style.font_ui),
                            );
                            ui.add_space(10.0);
                            timecode_label(
                                ui,
                                style,
                                "IN",
                                header.in_label,
                                header.focus == MarkFocus::In,
                            );
                            timecode_label(
                                ui,
                                style,
                                "OUT",
                                header.out_label,
                                header.focus == MarkFocus::Out,
                            );
                            timecode_label(ui, style, "Trajanje", header.duration_label, false);
                        });
                    });
                });
                ui.add_space(style.header_timeline_gap);
            }
            add_timeline(ui);
        });
    // Separator line on the top edge of the dock.
    let dock_rect = framed.response.rect;
    ui.painter().hline(
        dock_rect.x_range(),
        dock_rect.top(),
        egui::Stroke::new(1.0, style.border),
    );
    action
}

/// Fixed-height chrome strip with a bottom rule.
fn chrome_row(ui: &mut egui::Ui, style: &DockStyle, add_contents: impl FnOnce(&mut egui::Ui)) {
    let width = ui.available_width();
    let out = ui.allocate_ui_with_layout(
        Vec2::new(width, style.chrome_row_height),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().button_padding = Vec2::new(8.0, 2.0);
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
            egui::Frame::NONE
                .fill(style.chrome_fill)
                .inner_margin(egui::Margin {
                    left: style.chrome_pad_x,
                    right: style.chrome_pad_x,
                    top: style.chrome_pad_y,
                    bottom: style.chrome_pad_y,
                })
                .show(ui, |ui| {
                    ui.set_min_size(Vec2::new(ui.available_width(), style.chrome_control_height));
                    ui.set_max_height(style.chrome_control_height);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_height(style.chrome_control_height);
                        add_contents(ui);
                    });
                });
        },
    );
    let r = out.response.rect;
    ui.painter().hline(
        r.x_range(),
        r.bottom() - 0.5,
        egui::Stroke::new(1.0, style.border),
    );
}

/// Ghost text action button.
fn action_btn(ui: &mut egui::Ui, style: &DockStyle, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).color(style.text).size(style.font_ui))
            .min_size(Vec2::new(0.0, style.chrome_control_height))
            .fill(Color32::TRANSPARENT)
            .stroke(egui::Stroke::new(1.0, style.border)),
    )
}

fn timecode_label(ui: &mut egui::Ui, style: &DockStyle, label: &str, value: &str, focused: bool) {
    let label_color = if focused { style.focus } else { style.muted };
    let value_color = if focused {
        style.focus
    } else {
        style.timecode_value
    };
    ui.label(
        RichText::new(label)
            .size(style.font_timecode)
            .color(label_color),
    );
    ui.label(
        RichText::new(value)
            .monospace()
            .strong()
            .size(style.font_timecode)
            .color(value_color),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style() -> DockStyle {
        DockStyle {
            fill: Color32::BLACK,
            chrome_fill: Color32::BLACK,
            border: Color32::GRAY,
            text: Color32::WHITE,
            muted: Color32::GRAY,
            focus: Color32::YELLOW,
            timecode_value: Color32::GOLD,
            font_ui: 14.0,
            font_timecode: 13.0,
            chrome_row_height: 28.0,
            chrome_control_height: 24.0,
            chrome_pad_x: 8,
            chrome_pad_y: 2,
            inset_x: 8,
            header_timeline_gap: 4.0,
            header_item_gap: 8.0,
        }
    }

    #[test]
    fn dock_height_adds_header_row_and_gap() {
        assert_eq!(dock_height(100.0, true, &style()), 132.0);
        assert_eq!(dock_height(100.0, false, &style()), 100.0);
    }
}
