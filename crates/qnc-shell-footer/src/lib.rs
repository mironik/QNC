//! The shell footer as a block (user rule 2026-09-30: the footer is a place of every
//! board): theme picker, one tab per registered application, Close project and the
//! status. Moved unchanged out of `qnc-app`. Passive: it draws and answers with a
//! [`FooterIntent`]; the desktop decides what the answer does.

use eframe::egui::{self, Color32, Rect, RichText, Sense, Vec2};

/// The shell palette of one theme.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub raised: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub focus: Color32,
}

/// The themes the footer offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeId {
    Dark,
    Soft,
    HighContrast,
}

impl ThemeId {
    pub const ALL: [ThemeId; 3] = [ThemeId::Dark, ThemeId::Soft, ThemeId::HighContrast];

    pub fn label(self) -> &'static str {
        match self {
            ThemeId::Dark => "Dark",
            ThemeId::Soft => "Soft",
            ThemeId::HighContrast => "High contrast",
        }
    }

    /// The palette of this theme; Dark is the one of the shell contract.
    pub fn palette(self, contract: Palette) -> Palette {
        match self {
            ThemeId::Dark => contract,
            ThemeId::Soft => Palette {
                bg: Color32::from_rgb(22, 27, 38),
                surface: Color32::from_rgb(32, 40, 56),
                raised: Color32::from_rgb(45, 55, 74),
                border: Color32::from_rgb(75, 88, 110),
                text: Color32::from_rgb(236, 239, 244),
                muted: Color32::from_rgb(168, 178, 194),
                accent: Color32::from_rgb(52, 199, 148),
                focus: Color32::from_rgb(255, 196, 90),
            },
            ThemeId::HighContrast => Palette {
                bg: Color32::BLACK,
                surface: Color32::from_rgb(18, 18, 18),
                raised: Color32::from_rgb(36, 36, 36),
                border: Color32::from_rgb(180, 180, 180),
                text: Color32::WHITE,
                muted: Color32::from_rgb(200, 200, 200),
                accent: Color32::from_rgb(0, 255, 170),
                focus: Color32::from_rgb(255, 200, 0),
            },
        }
    }
}

/// Measures from the shell layout contract.
#[derive(Debug, Clone, Copy)]
pub struct FooterStyle {
    pub font_ui: f32,
    pub pad_x: i8,
    pub columns: usize,
}

/// What the footer shows.
pub struct FooterInput<'a> {
    /// `(tab id, label)` of every registered application, in order.
    pub tabs: &'a [(&'a str, &'a str)],
    pub active_tab: &'a str,
    pub theme: ThemeId,
    pub status: &'a str,
}

/// What a click in the footer asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FooterIntent {
    Activate(String),
    CloseProject,
    Theme(ThemeId),
}

/// Draws the footer in `rect`.
pub fn show(
    ui: &mut egui::Ui,
    rect: Rect,
    style: &FooterStyle,
    palette: &Palette,
    input: FooterInput<'_>,
) -> Option<FooterIntent> {
    ui.painter().rect_filled(rect, 0.0, palette.bg);
    // The separator line a bottom panel draws along its top edge, inside the panel.
    let separator = ui.visuals().widgets.noninteractive.bg_stroke;
    // Clipped to the footer, as the panel painter is.
    let painter = ui.painter().with_clip_rect(rect);
    painter.hline(rect.x_range(), rect.top() + 0.5 * separator.width, separator);
    let inner = rect.shrink2(Vec2::new(style.pad_x as f32, 0.0));
    let mut intent = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
        let h = ui.available_height();
        ui.columns(style.columns.max(3), |cols| {
            cols[0].with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_min_height(h);
                if let Some(theme) = theme_picker(ui, style, palette, input.theme) {
                    intent = Some(FooterIntent::Theme(theme));
                }
            });
            cols[1].with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                ui.set_min_height(h);
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    for (tab_id, label) in input.tabs {
                        let selected = input.active_tab == *tab_id;
                        if link_tab(ui, style, palette, label, selected).clicked() {
                            intent = Some(FooterIntent::Activate(tab_id.to_string()));
                        }
                    }
                });
            });
            cols[2].with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.set_min_height(h);
                let close_button = egui::Button::new(
                    RichText::new("Close project")
                        .size(style.font_ui)
                        .color(palette.text),
                )
                .min_size(Vec2::new(118.0, 24.0));
                if ui.add(close_button).on_hover_text("Zatvori aktivni projekt").clicked() {
                    intent = Some(FooterIntent::CloseProject);
                }
                ui.add(
                    egui::Label::new(
                        RichText::new(input.status).size(style.font_ui).color(palette.muted),
                    )
                    .truncate(),
                )
                .on_hover_text(input.status);
            });
        });
    });
    intent
}

fn theme_picker(
    ui: &mut egui::Ui,
    style: &FooterStyle,
    palette: &Palette,
    current: ThemeId,
) -> Option<ThemeId> {
    ui.label(RichText::new("Tema").size(style.font_ui).color(palette.muted));
    let mut selected = current;
    egui::ComboBox::from_id_salt("qnc_shell_theme")
        .selected_text(selected.label())
        .width(110.0)
        .show_ui(ui, |ui| {
            for id in ThemeId::ALL {
                ui.selectable_value(&mut selected, id, id.label());
            }
        });
    (selected != current).then_some(selected)
}

fn link_tab(
    ui: &mut egui::Ui,
    style: &FooterStyle,
    palette: &Palette,
    label: &str,
    selected: bool,
) -> egui::Response {
    let text = if selected {
        RichText::new(label).size(style.font_ui).strong().color(palette.text)
    } else {
        RichText::new(label).size(style.font_ui).color(palette.muted)
    };
    let response = ui.add(egui::Label::new(text).sense(Sense::click()).selectable(false));
    if selected {
        ui.painter().hline(
            response.rect.left()..=response.rect.right(),
            response.rect.bottom() + 1.0,
            egui::Stroke::new(2.0, palette.accent),
        );
    }
    response
}
