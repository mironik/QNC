//! The pool head row of the desktop board (user rule 2026-10-01: one block per
//! responsibility; moved unchanged out of the Ingest and editorial blocks, which drew it
//! twice): tabs from the layout contract on the left, transport buttons on the right, in
//! the public chrome row of the dock. Answers with the action id of the click.

use eframe::egui::{self, Align, Button, Color32, CornerRadius, Label, Layout, RichText, Sense, Stroke, Ui, Vec2};
use qnc_source_dock::TimelineDockStyle;

/// One tab: selected, and the action it asks for when clicked (none for a tab that only
/// names the pool).
pub struct RowTab<'a> {
    pub label: &'a str,
    pub selected: bool,
    pub action_id: Option<&'a str>,
    pub enabled: bool,
}

/// One transport button.
pub struct RowCommand<'a> {
    pub label: &'a str,
    pub action_id: &'a str,
}

/// The colours the row takes from the theme besides the dock style.
#[derive(Debug, Clone, Copy)]
pub struct RowColors {
    pub fill: Color32,
    pub muted: Color32,
    pub accent: Color32,
}

/// Draws the row in the place of the pool head; transport is drawn right to left in
/// reverse, so it reads in contract order.
pub fn show_row<'a>(
    ui: &mut Ui,
    style: &TimelineDockStyle,
    colors: RowColors,
    tabs: &[RowTab<'a>],
    transport: &[RowCommand<'a>],
) -> Option<&'a str> {
    let rect = ui.available_rect_before_wrap();
    let mut answer = None;
    qnc_source_dock::show_chrome_row(ui, rect, style, colors.fill, true, |ui| {
        ui.spacing_mut().button_padding = Vec2::new(8.0, 2.0);
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
        for tab in tabs {
            if text_tab(ui, style, colors, tab.label, tab.selected).clicked() && tab.enabled {
                answer = tab.action_id.or(answer);
            }
            ui.add_space(10.0);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            for command in transport.iter().rev() {
                if small_button(ui, style, command.label).clicked() {
                    answer = Some(command.action_id);
                }
            }
        });
    });
    answer
}

fn text_tab(ui: &mut Ui, style: &TimelineDockStyle, colors: RowColors, text: &str, selected: bool) -> egui::Response {
    let label = if selected {
        RichText::new(text).color(style.text).strong().size(style.font_ui)
    } else {
        RichText::new(text).color(colors.muted).size(style.font_ui)
    };
    let response = ui.add(Label::new(label).sense(Sense::click()).selectable(false));
    if selected {
        let y = response.rect.bottom() + 2.0;
        ui.painter().line_segment(
            [egui::pos2(response.rect.left(), y), egui::pos2(response.rect.right(), y)],
            Stroke::new(2.0, colors.accent),
        );
    }
    response
}

fn small_button(ui: &mut Ui, style: &TimelineDockStyle, text: &str) -> egui::Response {
    ui.add(
        Button::new(RichText::new(text).color(style.text))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, style.border))
            .corner_radius(CornerRadius::same(0))
            .min_size(Vec2::new(40.0, style.chrome_control_height)),
    )
}
