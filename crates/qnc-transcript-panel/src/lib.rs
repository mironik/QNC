//! The transcript panel (user 2026-10-26: Media Assist Audio AI, what was said in the clip on
//! the preview; a click on a word takes the playhead there). It paints the transcript view and
//! gives the click as the catalog action `transcript_cue_word` with the word's first frame; it
//! holds no state, reads nothing and knows no form.

use eframe::egui::{self, Color32, RichText, Sense};
use qnc_action_event::ActionEvent;
use qnc_transcript::{Line, TranscriptView, CUE_WORD, PANEL};

pub const MODULE_ID: &str = "qnc.module.transcript-panel";

/// Paints the transcript of the shown clip on the whole of `ui`.
pub fn show(ui: &mut egui::Ui, view: &TranscriptView, theme: &qnc_desktop_theme::Theme) -> Option<ActionEvent> {
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(rect, 0.0, theme.bg);
    let inner = rect.shrink(8.0);
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
    ui.set_clip_rect(inner.intersect(ui.clip_rect()));
    ui.label(RichText::new(qnc_ui_text::text("transcript.title")).color(theme.text).strong());
    if !view.message.is_empty() {
        ui.label(RichText::new(&view.message).color(theme.text_muted).size(theme.font_ui));
    }
    ui.add_space(6.0);
    if view.lines.is_empty() {
        let empty = if view.clip_id.is_some() { "transcript.empty" } else { "transcript.no_clip" };
        ui.label(RichText::new(qnc_ui_text::text(empty)).color(theme.text_muted).size(theme.font_ui));
        return None;
    }
    let mut asked = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(&mut ui, |ui| {
        for (index, line) in view.lines.iter().enumerate() {
            if let Some(frame) = paint_line(ui, line, view, theme, index) {
                asked = Some(frame);
            }
            ui.add_space(4.0);
        }
    });
    asked.map(|frame| ActionEvent::key(CUE_WORD).with_value(frame).at(PANEL))
}

/// One line: its words in a wrapped row, the one said at the playhead lit; returns the first
/// frame of a clicked word.
fn paint_line(ui: &mut egui::Ui, line: &Line, view: &TranscriptView, theme: &qnc_desktop_theme::Theme, index: usize) -> Option<u64> {
    let running = view.running.is_some_and(|(from, to)| line.from >= from && line.to <= to);
    let base = if running { theme.text_muted } else { theme.text };
    let mut clicked = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if line.words.is_empty() {
            clicked = word(ui, &line.text, line.from, said(view, line.from, line.to), base, theme, (index, 0));
            return;
        }
        for (at, said_word) in line.words.iter().enumerate() {
            let lit = said(view, said_word.from, said_word.to.max(said_word.from + 1));
            if let Some(frame) = word(ui, &said_word.text, said_word.from, lit, base, theme, (index, at)) {
                clicked = Some(frame);
            }
        }
    });
    clicked
}

fn said(view: &TranscriptView, from: u64, to: u64) -> bool {
    view.playhead.is_some_and(|playhead| (from..to).contains(&playhead))
}

fn word(ui: &mut egui::Ui, text: &str, frame: u64, lit: bool, base: Color32, theme: &qnc_desktop_theme::Theme, salt: (usize, usize)) -> Option<u64> {
    let mut rich = RichText::new(text).size(theme.font_ui).color(if lit { theme.black } else { base });
    if lit {
        rich = rich.background_color(theme.focus);
    }
    let response = ui.push_id(salt, |ui| ui.add(egui::Label::new(rich).sense(Sense::click()))).inner;
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked().then_some(frame)
}
