//! The pool head row as the layout contract gives it (user request 2026-10-02: one
//! public piece, not a private drawing per application): text tabs on the left, small
//! transport buttons on the right. The labels come from the caller's layout contract,
//! which tab is chosen from its view; a click is answered with the index, the caller
//! turns it into its own `action_id`. Same look as the copies it replaces.

use eframe::egui::{self, Button, Color32, CornerRadius, Label, RichText, Sense, Stroke, Ui, Vec2};

/// Colours and sizes from the caller's theme.
#[derive(Debug, Clone, Copy)]
pub struct RowStyle {
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub border: Color32,
    pub font_ui: f32,
    pub control_height: f32,
    pub tab_gap: f32,
}

/// A tab: its label, whether it is the chosen one and whether it answers a click.
#[derive(Debug, Clone, Copy)]
pub struct RowTab<'a> {
    pub label: &'a str,
    pub selected: bool,
    pub enabled: bool,
}

/// A command button: its label and whether it is the one on of a switch (Monitor
/// Auto | On); a plain command is never on.
#[derive(Debug, Clone, Copy)]
pub struct RowCommand<'a> {
    pub label: &'a str,
    pub on: bool,
}

/// What was clicked: the index of a tab or of a command, in the order given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowClick {
    Tab(usize),
    Command(usize),
}

/// Draws the row contents in `ui` (inside the caller's chrome row).
pub fn show_row(ui: &mut Ui, style: &RowStyle, tabs: &[RowTab<'_>], commands: &[RowCommand<'_>]) -> Option<RowClick> {
    ui.spacing_mut().button_padding = Vec2::new(8.0, 2.0);
    ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
    let mut click = None;
    for (index, row_tab) in tabs.iter().enumerate() {
        if tab(ui, row_tab.label, row_tab.selected, style).clicked() && row_tab.enabled {
            click = Some(RowClick::Tab(index));
        }
        ui.add_space(style.tab_gap);
    }
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        for (index, row_command) in commands.iter().enumerate().rev() {
            if command(ui, row_command.label, row_command.on, style).clicked() {
                click = Some(RowClick::Command(index));
            }
        }
    });
    click
}

/// A text tab: the chosen one strong with an accent line under it.
pub fn tab(ui: &mut Ui, text: &str, selected: bool, style: &RowStyle) -> egui::Response {
    let label = if selected {
        RichText::new(text).color(style.text).strong().size(style.font_ui)
    } else {
        RichText::new(text).color(style.muted).size(style.font_ui)
    };
    let response = ui.add(Label::new(label).sense(Sense::click()).selectable(false));
    if selected {
        let y = response.rect.bottom() + 2.0;
        ui.painter().line_segment(
            [egui::pos2(response.rect.left(), y), egui::pos2(response.rect.right(), y)],
            Stroke::new(2.0, style.accent),
        );
    }
    response
}

fn command(ui: &mut Ui, text: &str, on: bool, style: &RowStyle) -> egui::Response {
    let colour = if on { style.accent } else { style.text };
    ui.add(
        Button::new(RichText::new(text).color(colour))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, if on { style.accent } else { style.border }))
            .corner_radius(CornerRadius::same(0))
            .min_size(Vec2::new(40.0, style.control_height)),
    )
}
