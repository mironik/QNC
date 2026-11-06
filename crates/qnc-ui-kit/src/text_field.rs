//! The text fields of the gui-kit (docs/99 C15, user 2026-11-06): every piece that lets the user
//! type takes its field here, so a field looks and behaves the same in every form. The field is
//! handed back as egui's `TextEdit`, so a piece still adds what is its own (an id, a hint, a
//! layouter); what every field shares is decided here once.

use eframe::egui::{self, Align, TextEdit, Vec2};

/// A field of one line.
pub fn text_field(text: &mut String) -> TextEdit<'_> {
    TextEdit::singleline(text)
}

/// A field of one line beside a label: as tall as the row (`height`), its text in the middle.
pub fn row_field(text: &mut String, width: f32, height: f32) -> TextEdit<'_> {
    text_field(text).desired_width(width).min_size(Vec2::new(0.0, height)).vertical_align(Align::Center)
}

/// A field of several lines (a text that is written, not a value).
pub fn text_area(text: &mut String) -> TextEdit<'_> {
    TextEdit::multiline(text)
}

/// A field of one line that does not take more of the row than the `width` it is given.
pub fn search_field<'t>(text: &'t mut String, width: f32, hint: &str, font: egui::FontId) -> TextEdit<'t> {
    text_field(text).desired_width(width).hint_text(hint.to_string()).font(font)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_field_is_taller_than_a_plain_field() {
        let ctx = egui::Context::default();
        let (mut row_text, mut plain_text) = (String::from("x"), String::from("x"));
        let (mut row, mut plain) = (0.0, 0.0);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                row = ui.add(row_field(&mut row_text, 200.0, 32.0)).rect.height();
                plain = ui.add(text_field(&mut plain_text)).rect.height();
            });
        });
        assert!(row > plain, "row {row}, plain {plain}");
    }
}
