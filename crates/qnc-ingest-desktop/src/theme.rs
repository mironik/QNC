use eframe::egui::{self, Color32, FontFamily, FontId, Stroke, TextStyle, Visuals};

use crate::layout_contract::{ShellColors, ShellLayoutContract};

#[derive(Debug, Clone)]
pub struct Theme {
    pub bg: Color32,
    pub panel: Color32,
    pub panel_alt: Color32,
    pub border: Color32,
    pub border_soft: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub accent: Color32,
    pub focus: Color32,
    pub danger: Color32,
    pub input_bg: Color32,
    pub surface: Color32,
    pub surface_alt: Color32,
    pub black: Color32,
    pub font_ui: f32,
    pub font_timecode: f32,
    pub chrome_row_height: f32,
    pub chrome_control_height: f32,
    pub chrome_pad_x: f32,
    pub chrome_pad_y: f32,
}

impl Theme {
    pub fn from_contract(shell: &ShellLayoutContract) -> Self {
        let colors = &shell.colors;
        Self {
            bg: rgb(colors.bg),
            panel: rgb(colors.surface),
            panel_alt: rgb(colors.surface),
            border: rgb(colors.border),
            border_soft: rgb(colors.border).linear_multiply(0.65),
            text: rgb(colors.text),
            text_muted: rgb(colors.muted),
            accent: rgb(colors.accent),
            focus: rgb(colors.focus),
            danger: Color32::from_rgb(239, 68, 68),
            input_bg: rgb(colors.surface),
            surface: rgb(colors.surface),
            surface_alt: rgb(colors.raised),
            black: Color32::BLACK,
            font_ui: shell.theme_metrics.font_ui,
            font_timecode: shell.theme_metrics.font_timecode,
            chrome_row_height: shell.theme_metrics.chrome_row_height,
            chrome_control_height: shell.theme_metrics.chrome_control_height,
            chrome_pad_x: f32::from(shell.theme_metrics.chrome_pad_x),
            chrome_pad_y: f32::from(shell.theme_metrics.chrome_pad_y),
        }
    }
}

pub fn apply(ctx: &egui::Context, theme: &Theme) {
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(theme.font_ui + 4.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Body,
            FontId::new(theme.font_ui, FontFamily::Proportional),
        ),
        (
            TextStyle::Button,
            FontId::new(theme.font_ui, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(theme.font_ui - 1.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(theme.font_timecode, FontFamily::Proportional),
        ),
    ]
    .into();

    let mut visuals = Visuals::dark();
    visuals.panel_fill = theme.bg;
    visuals.window_fill = theme.panel;
    visuals.extreme_bg_color = theme.black;
    visuals.faint_bg_color = theme.input_bg;
    visuals.hyperlink_color = theme.accent;
    visuals.window_stroke = Stroke::new(1.0, theme.focus);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, theme.text);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, theme.text_muted);
    visuals.widgets.inactive.bg_fill = theme.input_bg;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, theme.border);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, theme.text);
    visuals.widgets.hovered.bg_fill = theme.surface_alt;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, theme.border);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, theme.text);
    visuals.widgets.active.bg_fill = theme.accent;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, theme.accent);
    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(theme.chrome_pad_x, theme.chrome_pad_y);
    ctx.set_style(style);
}

fn rgb(value: [u8; 3]) -> Color32 {
    Color32::from_rgb(value[0], value[1], value[2])
}

#[allow(dead_code)]
fn _shell_colors_type(_: &ShellColors) {}
