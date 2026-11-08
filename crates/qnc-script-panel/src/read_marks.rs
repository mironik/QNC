//! The text of a recorded block as it was read (user 2026-11-08): a word read is green, a word
//! read wrong red, a word missed red and struck through.

use eframe::egui::{self, text::LayoutJob, FontId, Stroke, TextFormat};
use qnc_script::{ReadCheck, Verdict};

pub(crate) fn job(text: &str, check: &ReadCheck, theme: &qnc_desktop_theme::Theme, width: f32) -> LayoutJob {
    let font = FontId::proportional(theme.font_ui + 1.0);
    let plain = TextFormat { font_id: font.clone(), color: theme.text, ..Default::default() };
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    let mut from = 0;
    for word in &check.words {
        job.append(&text[from..word.at.start], 0.0, plain.clone());
        let format = match word.verdict {
            Verdict::Read => TextFormat { color: qnc_desktop_theme::done(), ..plain.clone() },
            Verdict::Wrong => TextFormat { color: theme.danger, ..plain.clone() },
            Verdict::Missed => TextFormat { color: theme.danger, strikethrough: Stroke::new(1.0, theme.danger), ..plain.clone() },
        };
        job.append(&text[word.at.clone()], 0.0, format);
        from = word.at.end;
    }
    job.append(&text[from..], 0.0, plain);
    job
}

/// The block's text marked; its response, and under it what was said beyond the text.
pub(crate) fn show(ui: &mut egui::Ui, text: &str, check: &ReadCheck, theme: &qnc_desktop_theme::Theme) -> bool {
    let job = job(text, check, theme, ui.available_width());
    let hint = qnc_ui_text::fill("script.read_heard", &[("text", &check.heard)]);
    let clicked = ui.add(egui::Label::new(job).wrap().sense(egui::Sense::click())).on_hover_text(hint).clicked();
    if !check.extra.is_empty() {
        let extra = qnc_ui_text::fill("script.read_extra", &[("words", &check.extra.join(" "))]);
        ui.label(egui::RichText::new(extra).color(theme.text_muted).size(theme.font_ui));
    }
    clicked
}
