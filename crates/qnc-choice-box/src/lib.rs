//! An info box with a choice (user 2026-10-23: "when a project setting is in conflict with
//! the state of the card, say so in an info box and offer a solution with a choice"). It
//! says what is in the way, names what it is about, how it can be solved, and offers the
//! options as catalog actions; the caller runs the chosen one. Passive paint only: the box
//! decides and remembers nothing and knows no form, card or database.

use eframe::egui::{self, Align2, RichText, ScrollArea, Stroke, Vec2};

pub const MODULE_ID: &str = "qnc.module.choice-box";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const WIDTH: f32 = 560.0;
const ITEMS_HEIGHT: f32 = 160.0;

/// What the box shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Choice {
    pub title: String,
    /// What is in the way.
    pub message: String,
    /// What it is about (clip names), one per line.
    pub items: Vec<String>,
    /// How it can be solved.
    pub solution: String,
    pub options: Vec<ChoiceOption>,
}

/// One option: a catalog action and its label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceOption {
    pub action_id: String,
    pub label: String,
    /// The proposed one.
    pub strong: bool,
}

/// Shows `choice` in the middle of the screen over everything; the action of the option the
/// user clicked.
pub fn show(ctx: &egui::Context, theme: &qnc_desktop_theme::Theme, choice: &Choice) -> Option<String> {
    let mut chosen = None;
    egui::Area::new(egui::Id::new("qnc_choice_box"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(theme.panel)
                .stroke(Stroke::new(2.0, theme.focus))
                .corner_radius(6.0)
                .inner_margin(16.0)
                .show(ui, |ui| {
                    ui.set_width(WIDTH);
                    ui.label(RichText::new(&choice.title).color(theme.text).size(theme.font_ui + 4.0).strong());
                    ui.add_space(8.0);
                    ui.label(RichText::new(&choice.message).color(theme.text).size(theme.font_ui));
                    if !choice.items.is_empty() {
                        ui.add_space(6.0);
                        ScrollArea::vertical().max_height(ITEMS_HEIGHT).show(ui, |ui| {
                            for item in &choice.items {
                                ui.label(RichText::new(item).color(theme.text_muted).size(theme.font_ui));
                            }
                        });
                    }
                    if !choice.solution.is_empty() {
                        ui.add_space(8.0);
                        ui.label(RichText::new(&choice.solution).color(theme.text).size(theme.font_ui));
                    }
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        for option in &choice.options {
                            let clicked = if option.strong {
                                qnc_ui_kit::strong_button(ui, &option.label, true, theme.accent, theme.chrome_control_height).clicked()
                            } else {
                                qnc_ui_kit::chrome_button(ui, &option.label, true, false, qnc_theme_styles::button(theme)).clicked()
                            };
                            if clicked {
                                chosen = Some(option.action_id.clone());
                            }
                        }
                    });
                });
        });
    chosen
}
