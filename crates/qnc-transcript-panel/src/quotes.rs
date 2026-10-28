//! The statements of the transcript panel (user 2026-10-26): the text between IN and OUT
//! added to a table of start and end timecode, length, clip and text; a row opens its clip on
//! that statement, Copy puts the table on the clipboard for the journalist's script.

use eframe::egui::{self, RichText, Sense};
use qnc_action_event::ActionEvent;
use qnc_transcript::{TranscriptView, ADD_QUOTE, DELETE_QUOTE, OPEN_QUOTE, PANEL};

/// The share of the panel's height the table may take; the transcript gets the rest.
const TABLE_SHARE: f32 = 0.35;

pub(crate) fn show(ui: &mut egui::Ui, view: &TranscriptView, theme: &qnc_desktop_theme::Theme) -> Option<ActionEvent> {
    let mut asked = None;
    let style = qnc_ui_kit::ChromeButtonStyle { text: theme.text, border: theme.border, focus: theme.focus, height: theme.chrome_control_height };
    ui.horizontal(|ui| {
        ui.label(RichText::new(qnc_ui_text::text("transcript.quotes")).color(theme.text_muted).size(theme.font_ui));
        if qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("transcript.quote_add"), view.marks.is_some(), false, style).clicked() {
            asked = Some(ActionEvent::key(ADD_QUOTE).at(PANEL));
        }
        if qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("transcript.quote_copy"), !view.quotes.is_empty(), false, style).clicked() {
            ui.ctx().copy_text(view.quotes_text());
        }
    });
    if view.quotes.is_empty() {
        ui.label(RichText::new(qnc_ui_text::text("transcript.quotes_none")).color(theme.text_muted).size(theme.font_ui));
        return asked;
    }
    let height = (ui.ctx().screen_rect().height() * TABLE_SHARE).min(ui.available_height() * 0.5);
    egui::ScrollArea::vertical().id_salt("qnc_transcript_quotes").max_height(height).auto_shrink([false, true]).show(ui, |ui| {
        // Each statement (user 2026-10-27): clip, length and Delete on one line, then its start
        // timecode, the said text the whole width of the panel, and its end timecode.
        let font = |text: &str| RichText::new(text).color(theme.text).size(theme.font_ui);
        let muted = |text: &str| RichText::new(text).color(theme.text_muted).size(theme.font_ui);
        for (index, quote) in view.quotes.iter().enumerate() {
            let id = quote.id.to_string();
            let fill = if index % 2 == 0 { theme.surface_alt } else { egui::Color32::TRANSPARENT };
            egui::Frame::new().fill(fill).inner_margin(egui::Margin::symmetric(4, 3)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let mut row = ui
                    .horizontal(|ui| {
                        let row = ui.add(egui::Label::new(muted(&format!("{}  {}", quote.clip_name, quote.length))).sense(Sense::click()));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let delete = qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("transcript.quote_remove"), true, false, style);
                            if delete.on_hover_text(qnc_ui_text::text("transcript.quote_delete")).clicked() {
                                asked = Some(ActionEvent::key(DELETE_QUOTE).with_target(id.clone()).at(PANEL));
                            }
                        });
                        row
                    })
                    .inner;
                row |= ui.add(egui::Label::new(muted(&quote.start_tc)).sense(Sense::click()));
                row |= ui.add(egui::Label::new(font(&quote.text)).wrap().sense(Sense::click()));
                row |= ui.add(egui::Label::new(muted(&quote.end_tc)).sense(Sense::click()));
                if row.clicked() {
                    asked = Some(ActionEvent::key(OPEN_QUOTE).with_target(id.clone()).at(PANEL));
                }
                if row.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            });
        }
    });
    asked
}
