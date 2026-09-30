//! A modal yes/no question over the whole window (from the Project delete dialogs:
//! backdrop, 418x162 box, title, subject, NE / DA). Passive: it draws and answers; the
//! caller decides what the answer does. It knows no application.

use eframe::egui::{self, Color32, RichText, Sense, Vec2};

/// Colours and sizes, from the caller's layout contract.
#[derive(Debug, Clone, Copy)]
pub struct DialogStyle {
    pub surface: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub font_size: f32,
    pub button_height: f32,
}

/// The texts of one question.
#[derive(Debug, Clone, Copy)]
pub struct Question<'a> {
    /// Unique among the dialogs of the window.
    pub id: &'a str,
    pub title: &'a str,
    pub subject: &'a str,
    pub no: &'a str,
    pub yes: &'a str,
}

/// Shows the question; `Some(true)` for yes, `Some(false)` for no, `None` while open.
pub fn show(ctx: &egui::Context, style: &DialogStyle, question: Question<'_>) -> Option<bool> {
    let screen_rect = ctx.screen_rect();
    egui::Area::new(egui::Id::new((question.id, "backdrop")))
        .order(egui::Order::Foreground)
        .fixed_pos(screen_rect.min)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen_rect.size(), Sense::click());
            ui.painter().rect_filled(rect, 0.0, Color32::from_black_alpha(130));
        });

    let dialog_size = Vec2::new(418.0, 162.0);
    let dialog_pos = screen_rect.center() - (dialog_size * 0.5);
    egui::Area::new(egui::Id::new((question.id, "confirm")))
        .order(egui::Order::Foreground)
        .fixed_pos(dialog_pos)
        .show(ctx, |ui| {
            let (dialog_rect, _) = ui.allocate_exact_size(dialog_size, Sense::hover());
            ui.painter().rect_filled(dialog_rect, 0.0, style.surface);
            ui.painter().rect_stroke(
                dialog_rect,
                0.0,
                egui::Stroke::new(1.0, style.border),
                egui::StrokeKind::Inside,
            );
            let line = |top: f32| {
                egui::Rect::from_min_size(
                    egui::pos2(dialog_rect.left() + 20.0, dialog_rect.top() + top),
                    Vec2::new(dialog_rect.width() - 40.0, 24.0),
                )
            };
            let font = egui::FontId::proportional(style.font_size);
            ui.painter().text(
                line(20.0).center(),
                egui::Align2::CENTER_CENTER,
                question.title,
                font.clone(),
                style.text,
            );
            ui.painter().text(
                line(58.0).center(),
                egui::Align2::CENTER_CENTER,
                question.subject,
                font,
                style.muted,
            );

            let (button_w, gap) = (48.0, 8.0);
            let buttons_x = dialog_rect.center().x - (button_w * 2.0 + gap) * 0.5;
            let no_rect = egui::Rect::from_min_size(
                egui::pos2(buttons_x, dialog_rect.bottom() - 22.0 - style.button_height),
                Vec2::new(button_w, style.button_height),
            );
            let yes_rect = egui::Rect::from_min_size(
                egui::pos2(no_rect.right() + gap, no_rect.top()),
                Vec2::new(button_w, style.button_height),
            );
            let no = egui::Button::new(RichText::new(question.no).color(style.text).size(style.font_size))
                .fill(Color32::TRANSPARENT)
                .stroke(egui::Stroke::new(1.0, style.border));
            let yes = egui::Button::new(
                RichText::new(question.yes).color(Color32::WHITE).strong().size(style.font_size),
            )
            .fill(style.accent);
            let no_clicked = ui.put(no_rect, no).clicked();
            let yes_clicked = ui.put(yes_rect, yes).clicked();
            if no_clicked {
                Some(false)
            } else if yes_clicked {
                Some(true)
            } else {
                None
            }
        })
        .inner
}
