//! The export box (user 2026-10-24, 2026-10-25): over everything in the middle of the screen,
//! Export to XML and Express apart at the top, then the tabs PAL and NTSC with the export
//! settings of that tab in groups; the default one marked, the chosen one lit, one that is
//! not made yet greyed; Export and Cancel. Passive paint: it keeps nothing and answers what
//! the user did.

use eframe::egui::{self, Align2, Color32, RichText, Sense, Stroke, Vec2};
use qnc_export_choice::{ExportChoice, Kind, Standard};

pub const MODULE_ID: &str = "qnc.module.export-box";

const WIDTH: f32 = 440.0;
const ROW_HEIGHT: f32 = 24.0;
const TAB_HEIGHT: f32 = 28.0;

/// The colours of the box.
#[derive(Debug, Clone, Copy)]
pub struct ExportBoxStyle {
    pub background: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub chosen: Color32,
    pub font: f32,
}

/// What the user did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Choose(usize),
    Tab(Standard),
    Export,
    Cancel,
}

pub fn show(ctx: &egui::Context, choice: &ExportChoice, style: ExportBoxStyle) -> Option<Answer> {
    let mut answer = None;
    egui::Area::new(egui::Id::new("qnc_export_box"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(style.background)
                .stroke(Stroke::new(2.0, style.chosen))
                .corner_radius(6.0)
                .inner_margin(16.0)
                .show(ui, |ui| {
                    ui.set_width(WIDTH);
                    ui.label(RichText::new(qnc_ui_text::text("common.export")).color(style.text).size(style.font + 4.0).strong());
                    ui.add_space(8.0);
                    let mut group = None;
                    let mut tabs_drawn = false;
                    for index in choice.shown() {
                        let option = &choice.options[index];
                        if option.standard.is_some() && !tabs_drawn {
                            tabs_drawn = true;
                            ui.add_space(6.0);
                            if let Some(tab) = tabs(ui, choice.tab, style) {
                                answer = Some(Answer::Tab(tab));
                            }
                        }
                        if option.standard.is_some() && group.as_ref() != Some(&option.group) {
                            group = Some(option.group.clone());
                            ui.add_space(4.0);
                            ui.label(RichText::new(&option.group).color(style.muted).size(style.font - 1.0).strong());
                        }
                        if let Some(line) = row(ui, choice, index, style) {
                            answer = Some(line);
                        }
                    }
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let button = |text: &str, strong: bool| {
                            egui::Button::new(RichText::new(text).color(style.text).size(style.font))
                                .fill(if strong { style.chosen.gamma_multiply(0.5) } else { Color32::TRANSPARENT })
                                .stroke(Stroke::new(1.0, style.border))
                        };
                        if ui.add(button(qnc_ui_text::text("common.export"), true)).clicked() {
                            answer = Some(Answer::Export);
                        }
                        if ui.add(button(qnc_ui_text::text("common.cancel"), false)).clicked() {
                            answer = Some(Answer::Cancel);
                        }
                    });
                });
        });
    answer
}

/// The PAL | NTSC tabs, the shown one lit; the one clicked.
fn tabs(ui: &mut egui::Ui, shown: Standard, style: ExportBoxStyle) -> Option<Standard> {
    let mut clicked = None;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(WIDTH, TAB_HEIGHT), Sense::hover());
    ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, style.border));
    let width = WIDTH / 2.0;
    for (at, tab) in [Standard::Pal, Standard::Ntsc].into_iter().enumerate() {
        let tab_rect = egui::Rect::from_min_size(rect.min + Vec2::new(at as f32 * width, 0.0), Vec2::new(width, TAB_HEIGHT));
        let response = ui.interact(tab_rect, ui.id().with(("qnc_export_tab", at)), Sense::click());
        let lit = tab == shown;
        if lit {
            ui.painter().rect_filled(tab_rect.shrink(1.0), 3.0, style.chosen.gamma_multiply(0.35));
            ui.painter().line_segment([tab_rect.left_bottom(), tab_rect.right_bottom()], Stroke::new(2.0, style.chosen));
        } else if response.hovered() {
            ui.painter().rect_filled(tab_rect.shrink(1.0), 3.0, style.border);
        }
        let color = if lit { style.text } else { style.muted };
        ui.painter().text(tab_rect.center(), Align2::CENTER_CENTER, tab.label(), egui::FontId::proportional(style.font), color);
        if response.clicked() {
            clicked = Some(tab);
        }
    }
    clicked
}

/// One line: chosen lit, default marked, not made greyed and deaf to clicks.
fn row(ui: &mut egui::Ui, choice: &ExportChoice, index: usize, style: ExportBoxStyle) -> Option<Answer> {
    let option = &choice.options[index];
    let sense = if option.available { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(WIDTH, ROW_HEIGHT), sense);
    let chosen = index == choice.chosen;
    if chosen {
        ui.painter().rect_filled(rect, 3.0, style.chosen.gamma_multiply(0.35));
    } else if option.available && response.hovered() {
        ui.painter().rect_filled(rect, 3.0, style.border);
    }
    let mut label = option.label.clone();
    if option.default {
        label = format!("{label}  ({})", qnc_ui_text::text("export.default"));
    }
    if !option.available {
        label = format!("{label}  ({})", qnc_ui_text::text("export.not_made"));
    }
    // Original is stressed (user 2026-10-24); what is not made is greyed (user 2026-10-25).
    let stressed = option.kind == Kind::Original;
    let color = match (option.available, chosen || stressed) {
        (false, _) => style.muted.gamma_multiply(0.5),
        (true, true) => style.text,
        (true, false) => style.muted,
    };
    let font = egui::FontId::proportional(if stressed { style.font + 1.0 } else { style.font });
    ui.painter().text(rect.left_center() + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, label, font, color);
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
