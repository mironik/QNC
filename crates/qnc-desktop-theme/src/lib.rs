//! The one desktop theme (user rule 2026-09-30: the Ingest and editorial copies became
//! one piece): the shell layout model (colours and chrome measures of
//! `contracts/ui/shell.layout.json`), the theme made from it, the fonts and visuals it
//! applies, and the faces of the desktop board. Passive: it knows no application.

use eframe::egui::{self, Color32, FontFamily, FontId, Stroke, TextStyle, Visuals};

use serde::Deserialize;

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
            danger: danger(),
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

/// The colour of an error line (one for every form; the Project form had its own).
pub fn danger() -> Color32 {
    Color32::from_rgb(239, 68, 68)
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

/// The look of a settings form (the Project form had it as its own copy, audit 2026-10-27):
/// raised controls, accent hover and selection, the text colour on every widget.
pub fn apply_settings_look(ctx: &egui::Context, theme: &Theme) {
    let mut style = (*ctx.style()).clone();
    let font = |size: f32| FontId::new(size, FontFamily::Proportional);
    style.text_styles = [
        (TextStyle::Small, font(theme.font_ui - 1.0)),
        (TextStyle::Body, font(theme.font_ui)),
        (TextStyle::Button, font(theme.font_ui)),
        (TextStyle::Heading, font(20.0)),
        (TextStyle::Monospace, font(theme.font_ui)),
    ]
    .into();
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    let visuals = &mut style.visuals;
    visuals.panel_fill = theme.bg;
    visuals.window_fill = theme.surface;
    visuals.extreme_bg_color = theme.bg;
    visuals.faint_bg_color = theme.surface;
    visuals.code_bg_color = theme.surface_alt;
    visuals.override_text_color = Some(theme.text);
    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_fill = theme.surface;
    widgets.noninteractive.weak_bg_fill = theme.surface_alt;
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, theme.text_muted);
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, theme.border);
    widgets.inactive.bg_fill = theme.surface_alt;
    widgets.inactive.weak_bg_fill = theme.surface;
    widgets.inactive.fg_stroke = Stroke::new(1.0, theme.text);
    widgets.inactive.bg_stroke = Stroke::new(1.0, theme.border);
    widgets.hovered.bg_fill = theme.surface_alt;
    widgets.hovered.weak_bg_fill = theme.surface_alt;
    widgets.hovered.fg_stroke = Stroke::new(1.0, theme.text);
    widgets.hovered.bg_stroke = Stroke::new(1.0, theme.accent);
    widgets.active.bg_fill = theme.accent;
    widgets.active.weak_bg_fill = theme.accent;
    widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    widgets.active.bg_stroke = Stroke::new(1.0, theme.accent);
    visuals.selection.bg_fill = theme.accent.linear_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, theme.accent);
    visuals.hyperlink_color = theme.accent;
    ctx.set_style(style);
}

fn rgb(value: [u8; 3]) -> Color32 {
    Color32::from_rgb(value[0], value[1], value[2])
}


impl Theme {
    /// The faces of the desktop board: the board, the left column, the right panel and
    /// the divider between them.
    pub fn board_faces(&self) -> qnc_board::BoardFaces {
        qnc_board::BoardFaces { bg: self.bg, left: self.surface, right: self.bg, divider: self.border_soft }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShellLayoutContract {
    pub layout_id: String,
    pub colors: ShellColors,
    pub theme_metrics: ShellThemeMetrics,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShellThemeMetrics {
    pub font_ui: f32,
    pub font_timecode: f32,
    pub chrome_row_height: f32,
    pub chrome_control_height: f32,
    pub chrome_pad_x: i8,
    pub chrome_pad_y: i8,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShellColors {
    pub bg: [u8; 3],
    pub surface: [u8; 3],
    pub raised: [u8; 3],
    pub border: [u8; 3],
    pub text: [u8; 3],
    pub muted: [u8; 3],
    pub accent: [u8; 3],
    pub focus: [u8; 3],
}
