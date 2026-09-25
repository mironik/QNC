//! The Segment tab of the clip menu (v5 `media_pool::segment_cards`): kind, id and
//! duration per row. The selected active row carries Up, Down and Isključi; an
//! excluded segment stays listed greyed where it was with Uključi and Izbriši
//! (user rule 2026-09-25) and cannot be picked.

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
                let mut buttons: Option<egui::Rect> = None;
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
                                ui.label(RichText::new("isključen").color(theme.muted).small());
                            }
                            if row.selected || !row.active {
                                let layout = egui::Layout::right_to_left(egui::Align::Center);
                                ui.with_layout(layout, |ui| {
                                    buttons = Some(row_buttons(ui, row, &mut command));
                                });
                            }
                        });
                    })
                    .response;
                // The row is no widget of its own: registered over its buttons it would
                // swallow Del, Down and Up. A click on it away from them selects it.
                let pointer = ui.input(|i| i.pointer.interact_pos());
                let on_row = pointer.is_some_and(|pos| {
                    response.rect.contains(pos) && !buttons.is_some_and(|b| b.contains(pos))
                });
                if row.active
                    && on_row
                    && command.is_none()
                    && ui.input(|i| i.pointer.primary_clicked())
                {
                    command = Some(SegmentCommand::Select(row.segment_id.clone()));
                }
                ui.add_space(6.0);
            }
        });
    command
}

/// The buttons of a row: Isključi, Down, Up on the selected active one; Izbriši,
/// Uključi on an excluded one. Returns the area they take.
fn row_buttons(
    ui: &mut egui::Ui,
    row: &qnc_program_segments::SegmentPart,
    command: &mut Option<SegmentCommand>,
) -> egui::Rect {
    let mut area = egui::Rect::NOTHING;
    let mut small = |ui: &mut egui::Ui, text: &str, hint: &str| {
        let response = ui.add(egui::Button::new(RichText::new(text).small()));
        area = area.union(response.rect);
        response.on_hover_text(hint).clicked()
    };
    let id = || row.segment_id.clone();
    if !row.active {
        if small(ui, "Izbriši", "Trajno obriši isključeni segment") {
            *command = Some(SegmentCommand::Purge(id()));
        }
        if small(ui, "Uključi", "Vrati segment u program gdje je bio") {
            *command = Some(SegmentCommand::Include(id()));
        }
        return area;
    }
    if small(
        ui,
        "Isključi",
        "Makni segment iz programa (ostaje na popisu)",
    ) {
        *command = Some(SegmentCommand::Exclude(id()));
    }
    if small(ui, "Down", "Pomakni segment kasnije") {
        *command = Some(SegmentCommand::Move { up: false });
    }
    if small(ui, "Up", "Pomakni segment ranije") {
        *command = Some(SegmentCommand::Move { up: true });
    }
    area
}
