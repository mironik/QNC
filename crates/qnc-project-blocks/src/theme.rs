use eframe::egui::{self, Color32, RichText};

use crate::layout_contract::ThemeColors;

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub bg: Color32,
    pub surface: Color32,
    pub raised: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub focus: Color32,
}

impl Theme {
    pub fn from_contract(colors: &ThemeColors) -> Self {
        Self {
            bg: rgb(colors.bg),
            surface: rgb(colors.surface),
            raised: rgb(colors.raised),
            border: rgb(colors.border),
            text: rgb(colors.text),
            muted: rgb(colors.muted),
            accent: rgb(colors.accent),
            focus: rgb(colors.focus),
        }
    }
}

fn rgb(value: [u8; 3]) -> Color32 {
    Color32::from_rgb(value[0], value[1], value[2])
}

pub fn label(ui: &mut egui::Ui, text: impl Into<String>, size: f32, color: Color32) {
    ui.label(RichText::new(text.into()).size(size).color(color));
}

pub fn strong_label(ui: &mut egui::Ui, text: impl Into<String>, size: f32, color: Color32) {
    ui.label(RichText::new(text.into()).size(size).strong().color(color));
}
