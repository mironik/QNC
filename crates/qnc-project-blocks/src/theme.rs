use eframe::egui::{self, Color32, FontFamily, FontId, RichText, TextStyle, Vec2};

use crate::layout_contract::{ShellLayoutContract, ThemeColors};

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

pub fn apply_app_fonts(ctx: &egui::Context, shell: &ShellLayoutContract) {
    let mut style = (*ctx.style()).clone();
    let font_ui = shell.theme_metrics.font_ui;
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(font_ui - 1.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(font_ui, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(font_ui, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(20.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(font_ui, FontFamily::Proportional),
    );
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    ctx.set_style(style);
}

pub fn apply_visuals(ctx: &egui::Context, shell: &ShellLayoutContract) {
    let theme = Theme::from_contract(&shell.colors);
    let _focus_color = theme.focus;
    let mut style = (*ctx.style()).clone();
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.visuals.panel_fill = theme.bg;
    style.visuals.window_fill = theme.surface;
    style.visuals.extreme_bg_color = theme.bg;
    style.visuals.faint_bg_color = theme.surface;
    style.visuals.code_bg_color = theme.raised;
    style.visuals.override_text_color = Some(theme.text);
    style.visuals.widgets.noninteractive.bg_fill = theme.surface;
    style.visuals.widgets.noninteractive.weak_bg_fill = theme.raised;
    style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, theme.muted);
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, theme.border);
    style.visuals.widgets.inactive.bg_fill = theme.raised;
    style.visuals.widgets.inactive.weak_bg_fill = theme.surface;
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, theme.text);
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, theme.border);
    style.visuals.widgets.hovered.bg_fill = theme.raised;
    style.visuals.widgets.hovered.weak_bg_fill = theme.raised;
    style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, theme.text);
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, theme.accent);
    style.visuals.widgets.active.bg_fill = theme.accent;
    style.visuals.widgets.active.weak_bg_fill = theme.accent;
    style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, theme.accent);
    style.visuals.selection.bg_fill = theme.accent.linear_multiply(0.35);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, theme.accent);
    style.visuals.hyperlink_color = theme.accent;
    ctx.set_style(style);
}

pub fn label(ui: &mut egui::Ui, text: impl Into<String>, size: f32, color: Color32) {
    ui.label(RichText::new(text.into()).size(size).color(color));
}

pub fn strong_label(ui: &mut egui::Ui, text: impl Into<String>, size: f32, color: Color32) {
    ui.label(RichText::new(text.into()).size(size).strong().color(color));
}
