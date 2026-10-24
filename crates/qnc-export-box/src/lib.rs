//! The export box (user 2026-10-24): over everything in the middle of the screen, Export to XML
//! apart at the top, then the export settings of the project, the default one marked, the
//! chosen one lit; Export and Cancel. Passive paint: it keeps nothing and answers what the
//! user did.

use eframe::egui::{self, Align2, Color32, RichText, Sense, Stroke, Vec2};
use qnc_export_choice::ExportChoice;

pub const MODULE_ID: &str = "qnc.module.export-box";

const WIDTH: f32 = 440.0;
const ROW_HEIGHT: f32 = 24.0;

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
                    for (index, option) in choice.options.iter().enumerate() {
                        if option.kind == qnc_export_choice::Kind::Original {
                            // The settings rendered from the originals, apart from XML and Express.
                            ui.add_space(4.0);
                            ui.separator();
                            ui.label(RichText::new(qnc_ui_text::text("export.settings")).color(style.muted).size(style.font));
                        }
                        let (rect, response) = ui.allocate_exact_size(Vec2::new(WIDTH, ROW_HEIGHT), Sense::click());
                        let chosen = index == choice.chosen;
                        if chosen {
                            ui.painter().rect_filled(rect, 3.0, style.chosen.gamma_multiply(0.35));
                        } else if response.hovered() {
                            ui.painter().rect_filled(rect, 3.0, style.border);
                        }
                        let mut label = option.label.clone();
                        if option.default {
                            label = format!("{label}  ({})", qnc_ui_text::text("export.default"));
                        }
                        let color = if chosen { style.text } else { style.muted };
                        // Original is stressed (user 2026-10-24).
                        let stressed = option.kind == qnc_export_choice::Kind::Original;
                        let color = if stressed { style.text } else { color };
                        let font = egui::FontId::proportional(if stressed { style.font + 1.0 } else { style.font });
                        ui.painter().text(rect.left_center() + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, label, font, color);
                        if response.double_clicked() {
                            answer = Some(Answer::Export);
                        } else if response.clicked() {
                            answer = Some(Answer::Choose(index));
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
