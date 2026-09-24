//! The Segment tab of the clip menu (v5 `media_pool::segment_cards`): kind, id and
//! duration per row; the selected row carries Up, Down and Del. Deleted segments
//! stay listed greyed with "neaktivno" and cannot be picked.

use eframe::egui::{self, Color32, RichText};
use qnc_program_segments::{SegmentCommand, SegmentsView};
use qnc_timeline::TimelineTheme;

const EMPTY_LIST_MESSAGE: &str = "Nema segmenata — označi source IN/OUT pa dodaj TON/OFF.";

pub fn show_segment_list(
    ui: &mut egui::Ui,
    segments: &SegmentsView,
    theme: TimelineTheme,
    select: Color32,
) -> Option<SegmentCommand> {
    let mut command = None;
    egui::ScrollArea::vertical()
        .id_salt("qnc_segment_list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if segments.parts.is_empty() {
                ui.colored_label(theme.muted, EMPTY_LIST_MESSAGE);
                return;
            }
            for row in &segments.parts {
                let stroke = if row.selected {
                    egui::Stroke::new(2.0, select)
                } else {
                    egui::Stroke::new(1.0, theme.border)
                };
                let response = egui::Frame::NONE
                    .stroke(stroke)
                    .fill(if row.active {
                        theme.label_background
                    } else {
                        theme.label_background.linear_multiply(0.65)
                    })
                    .inner_margin(egui::Margin::symmetric(10, 7))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width().max(180.0));
                        ui.horizontal(|ui| {
                            let kind_color = if row.active {
                                theme.playhead
                            } else {
                                theme.muted
                            };
                            let kind = RichText::new(row.kind.label()).color(kind_color);
                            ui.label(kind.strong());
                            ui.label(RichText::new(&row.segment_id).color(theme.text).small());
                            let duration = RichText::new(&row.duration_label).color(theme.muted);
                            ui.label(duration.small());
                            if !row.active {
                                ui.label(RichText::new("neaktivno").color(theme.muted).small());
                            }
                            if row.selected && row.active {
                                let layout = egui::Layout::right_to_left(egui::Align::Center);
                                ui.with_layout(layout, |ui| row_buttons(ui, &mut command));
                            }
                        });
                    })
                    .response;
                if row.active
                    && command.is_none()
                    && response.interact(egui::Sense::click()).clicked()
                {
                    command = Some(SegmentCommand::Select(row.segment_id.clone()));
                }
                ui.add_space(6.0);
            }
        });
    command
}

fn row_buttons(ui: &mut egui::Ui, command: &mut Option<SegmentCommand>) {
    let small = |ui: &mut egui::Ui, text: &str, hint: &str| {
        ui.add(egui::Button::new(RichText::new(text).small()))
            .on_hover_text(hint)
            .clicked()
    };
    if small(ui, "Del", "Obriši segment") {
        *command = Some(SegmentCommand::DeleteSelected);
    }
    if small(ui, "Down", "Pomakni segment kasnije") {
        *command = Some(SegmentCommand::Move { up: false });
    }
    if small(ui, "Up", "Pomakni segment ranije") {
        *command = Some(SegmentCommand::Move { up: true });
    }
}
