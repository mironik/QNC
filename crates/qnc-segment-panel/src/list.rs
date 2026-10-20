//! The Segment tab of the clip menu (v5 `media_pool::segment_cards`): kind, id and
//! duration per row. The selected active row carries Edit, Replace, Up, Down and Isključi; an
//! excluded segment stays listed greyed where it was with Uključi and Izbriši
//! (user rule 2026-09-25) and cannot be picked.

use eframe::egui::{self, Color32, RichText};
use qnc_action_event::ActionEvent;
use qnc_program_view::SegmentsView;
use qnc_timeline::TimelineTheme;


pub fn show_segment_list(
    ui: &mut egui::Ui,
    segments: &SegmentsView,
    theme: TimelineTheme,
    select: Color32,
) -> Option<ActionEvent> {
    let mut command: Option<(&'static str, String)> = None;
    egui::ScrollArea::vertical()
        .id_salt("qnc_segment_list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if segments.parts.is_empty() {
                ui.colored_label(theme.muted, qnc_ui_text::text("story.ui.no_segments_list"));
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
                                ui.label(RichText::new(qnc_ui_text::text("story.ui.excluded")).color(theme.muted).small());
                            }
                            if row.selected || !row.active {
                                let layout = egui::Layout::right_to_left(egui::Align::Center);
                                ui.with_layout(layout, |ui| {
                                    buttons = Some(row_buttons(ui, row, segments.editing.as_deref() == Some(row.segment_id.as_str()), &mut command));
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
                    command = Some(("segment_select", row.segment_id.clone()));
                }
                ui.add_space(6.0);
            }
        });
    // The row's segment is the target; the panel is the segment panel's (user 2026-10-18).
    command.map(|(action_id, id)| ActionEvent::key(action_id).with_target(id).at(crate::PANEL))
}

/// The buttons of a row: Isključi, Down, Up on the selected active one; Izbriši,
/// Uključi on an excluded one. Returns the area they take.
fn row_buttons(
    ui: &mut egui::Ui,
    row: &qnc_program_view::SegmentPart,
    editing: bool,
    command: &mut Option<(&'static str, String)>,
) -> egui::Rect {
    let mut area = egui::Rect::NOTHING;
    let mut small = |ui: &mut egui::Ui, text: &str, hint: &str| {
        let response = ui.add(egui::Button::new(RichText::new(text).small()));
        area = area.union(response.rect);
        response.on_hover_text(hint).clicked()
    };
    let id = || row.segment_id.clone();
    if !row.active {
        if small(ui, qnc_ui_text::text("story.ui.delete"), qnc_ui_text::text("story.ui.delete_hint")) {
            *command = Some(("segment_purge", id()));
        }
        if small(ui, qnc_ui_text::text("story.ui.include"), qnc_ui_text::text("story.ui.include_hint")) {
            *command = Some(("segment_include", id()));
        }
        return area;
    }
    if small(
        ui,
        qnc_ui_text::text("story.ui.exclude"),
        qnc_ui_text::text("story.ui.exclude_hint"),
    ) {
        *command = Some(("segment_exclude", id()));
    }
    if small(ui, qnc_ui_text::text("story.ui.down"), qnc_ui_text::text("story.ui.down_hint")) {
        *command = Some(("segment_move_down", id()));
    }
    if small(ui, qnc_ui_text::text("story.ui.up"), qnc_ui_text::text("story.ui.up_hint")) {
        *command = Some(("segment_move_up", id()));
    }
    if small(
        ui,
        qnc_ui_text::text("story.ui.replace"),
        qnc_ui_text::text("story.ui.replace_hint"),
    ) {
        *command = Some(("segment_replace", id()));
    }
    // Edit (user rule 2026-10-01): IN/OUT of the segment on the source timeline.
    let edit = ui.add(egui::Button::new(RichText::new(qnc_ui_text::text("story.ui.edit")).small()).selected(editing));
    area = area.union(edit.rect);
    if edit.on_hover_text(qnc_ui_text::text("story.ui.edit_hint")).clicked() {
        *command = Some(("segment_edit", id()));
    }
    area
}
