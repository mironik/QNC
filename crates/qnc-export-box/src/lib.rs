//! The export box (user 2026-10-24, 2026-10-25): over everything in the middle of the screen,
//! Export to XML and Express apart at the top, then the tabs PAL and NTSC with the export
//! settings of that tab in groups; the default one marked, the chosen one lit, one that is
//! not made yet greyed; Export and Cancel. In the colours and pieces of the desktop theme
//! (the info box, the pool tabs, the chrome buttons; user 2026-10-25: "stick to our palette
//! and style"). Passive paint: it keeps nothing and answers what the user did.

use eframe::egui::{self, Align2, RichText, Sense, Stroke, Vec2};
use qnc_desktop_theme::Theme;
use qnc_export_choice::{ExportChoice, Kind, Standard};

pub const MODULE_ID: &str = "qnc.module.export-box";

const WIDTH: f32 = 440.0;
const ROW_HEIGHT: f32 = 22.0;

/// What the user did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Choose(usize),
    Tab(Standard),
    Export,
    Cancel,
}

pub fn show(ctx: &egui::Context, theme: &Theme, choice: &ExportChoice) -> Option<Answer> {
    let mut answer = None;
    egui::Area::new(egui::Id::new("qnc_export_box"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            // The frame of the info box (`qnc-choice-box`).
            egui::Frame::NONE
                .fill(theme.panel)
                .stroke(Stroke::new(2.0, theme.focus))
                .corner_radius(6.0)
                .inner_margin(16.0)
                .show(ui, |ui| {
                    ui.set_width(WIDTH);
                    ui.label(RichText::new(qnc_ui_text::text("common.export")).color(theme.text).size(theme.font_ui + 4.0).strong());
                    ui.add_space(8.0);
                    let mut group = None;
                    let mut tabs_drawn = false;
                    for index in choice.shown() {
                        let option = &choice.options[index];
                        if option.standard.is_some() && !tabs_drawn {
                            tabs_drawn = true;
                            ui.add_space(8.0);
                            if let Some(tab) = tabs(ui, theme, choice.tab) {
                                answer = Some(Answer::Tab(tab));
                            }
                            ui.add_space(4.0);
                        }
                        if option.standard.is_some() && group.as_ref() != Some(&option.group) {
                            group = Some(option.group.clone());
                            ui.add_space(6.0);
                            ui.label(RichText::new(&option.group).color(theme.text_muted).size(theme.font_ui));
                        }
                        if let Some(line) = row(ui, theme, choice, index) {
                            answer = Some(line);
                        }
                    }
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if qnc_ui_kit::strong_button(ui, qnc_ui_text::text("common.export"), choice.picked().is_some(), theme.accent, theme.chrome_control_height).clicked() {
                            answer = Some(Answer::Export);
                        }
                        if qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("common.cancel"), true, false, qnc_theme_styles::button(theme)).clicked() {
                            answer = Some(Answer::Cancel);
                        }
                    });
                });
        });
    answer
}

/// The PAL | NTSC tabs as the tabs of the pool head; the one clicked.
fn tabs(ui: &mut egui::Ui, theme: &Theme, shown: Standard) -> Option<Standard> {
    let style = qnc_theme_styles::row(theme, 12.0);
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = style.tab_gap;
        for tab in [Standard::Pal, Standard::Ntsc] {
            if qnc_media_pool_head::tab(ui, tab.label(), tab == shown, &style).clicked() {
                clicked = Some(tab);
            }
        }
    });
    clicked
}

/// One line: chosen lit in the focus colour, default marked, not made greyed and deaf to
/// clicks.
fn row(ui: &mut egui::Ui, theme: &Theme, choice: &ExportChoice, index: usize) -> Option<Answer> {
    let option = &choice.options[index];
    let sense = if option.available { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(WIDTH, ROW_HEIGHT), sense);
    let chosen = index == choice.chosen;
    if chosen {
        ui.painter().rect_filled(rect, 0.0, theme.surface_alt);
        ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, theme.focus), egui::StrokeKind::Inside);
    } else if option.available && response.hovered() {
        ui.painter().rect_filled(rect, 0.0, theme.surface);
    }
    let mut label = option.label.clone();
    if option.default {
        label = format!("{label}  ({})", qnc_ui_text::text("export.default"));
    }
    if !option.available {
        label = format!("{label}  ({})", qnc_ui_text::text("export.not_made"));
    }
    // What is not made is greyed (user 2026-10-25).
    let color = match (option.available, chosen || option.kind == Kind::Original) {
        (false, _) => theme.text_muted.gamma_multiply(0.5),
        (true, true) => theme.text,
        (true, false) => theme.text_muted,
    };
    ui.painter().text(rect.left_center() + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, label, egui::FontId::proportional(theme.font_ui), color);
    if !option.available {
        return None;
    }
    if response.double_clicked() {
        Some(Answer::Export)
    } else if response.clicked() {
        Some(Answer::Choose(index))
    } else {
        None
    }
}
