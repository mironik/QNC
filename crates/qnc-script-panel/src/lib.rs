//! The script panel (user 2026-10-28, Media Assist Audio: writing the offs and putting the
//! story together as text). On top the length of the script against the story's target, then
//! its blocks in reading order (OFF text and statements, each with its length, up, down and
//! remove), the statements of the transcript to put in, and at the bottom the TextSourceEditor
//! where OFF text is written and corrected; Add OFF puts it into the table above. It paints the
//! script view and gives catalog actions; it holds no state, reads nothing and knows no form.

use eframe::egui::{self, RichText, Sense};
use qnc_action_event::ActionEvent;
use qnc_script::{seconds_label, BlockKind, ScriptView, ADD_INTRO, ADD_OFF, ADD_OUTRO, ADD_QUOTE, ADD_SOT, CLEAR_EDITOR, EDITOR_TEXT, MOVE_DOWN, MOVE_UP, OPEN, PANEL, RECORD, REMOVE, REPLACE_OFF, SELECT};
use qnc_text_source_editor::{Answer, Editor, EditorButton};
use qnc_transcript::TranscriptView;

pub const MODULE_ID: &str = "qnc.module.script-panel";

/// The share of the panel the editor takes at the bottom (and its least height).
const EDITOR_SHARE: f32 = 0.32;
const EDITOR_LEAST: f32 = 150.0;
/// The share of the panel the statements may take.
const QUOTES_SHARE: f32 = 0.22;

/// Paints the script over the whole of `ui`; `transcript` gives the statements to put in.
pub fn show(ui: &mut egui::Ui, view: &ScriptView, transcript: &TranscriptView, theme: &qnc_desktop_theme::Theme) -> Option<ActionEvent> {
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(rect, 0.0, theme.bg);
    let inner = rect.shrink(8.0);
    let editor_height = (inner.height() * EDITOR_SHARE).max(EDITOR_LEAST).min(inner.height() * 0.6);
    let (top, bottom) = inner.split_top_bottom_at_y(inner.bottom() - editor_height);
    let mut asked = None;
    let mut ui_top = ui.new_child(egui::UiBuilder::new().max_rect(top).layout(egui::Layout::top_down(egui::Align::Min)));
    ui_top.set_clip_rect(top.intersect(ui.clip_rect()));
    if let Some(event) = paint_script(&mut ui_top, view, transcript, theme) {
        asked = Some(event);
    }
    let editor_rect = bottom.with_min_y(bottom.top() + 6.0);
    let mut ui_bottom = ui.new_child(egui::UiBuilder::new().max_rect(editor_rect).layout(egui::Layout::top_down(egui::Align::Min)));
    ui_bottom.set_clip_rect(editor_rect.intersect(ui.clip_rect()));
    if let Some(event) = paint_editor(&mut ui_bottom, view, theme) {
        asked = Some(event);
    }
    asked
}

fn paint_script(ui: &mut egui::Ui, view: &ScriptView, transcript: &TranscriptView, theme: &qnc_desktop_theme::Theme) -> Option<ActionEvent> {
    let mut asked = None;
    let style = qnc_ui_kit::ChromeButtonStyle { text: theme.text, border: theme.border, focus: theme.focus, height: theme.chrome_control_height };
    let muted = |text: &str| RichText::new(text).color(theme.text_muted).size(theme.font_ui);
    ui.horizontal(|ui| {
        ui.label(RichText::new(qnc_ui_text::text("script.title")).color(theme.text).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("script.copy"), !view.blocks.is_empty(), false, style).clicked() {
                ui.ctx().copy_text(view.text());
            }
            if qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("script.add_sot"), view.marks, false, style).on_hover_text(qnc_ui_text::text("script.add_sot_hint")).clicked() {
                asked = Some(ActionEvent::key(ADD_SOT).at(PANEL));
            }
        });
    });
    // The length of the script against the target of the story sheet (red when over).
    let total = view.total_seconds();
    let lengths = qnc_ui_text::fill("script.lengths", &[("off", &seconds_label(view.off_seconds)), ("sot", &seconds_label(view.sot_seconds)), ("total", &seconds_label(total)), ("anchor", &seconds_label(view.anchor_seconds))]);
    ui.horizontal(|ui| {
        ui.label(muted(&lengths));
        if let Some(target) = view.target_seconds {
            let over = total > target;
            let text = qnc_ui_text::fill("script.target", &[("target", &seconds_label(target))]);
            ui.label(RichText::new(text).color(if over { theme.danger } else { theme.text_muted }).size(theme.font_ui));
        }
    });
    if !view.message.is_empty() {
        ui.label(RichText::new(&view.message).color(theme.danger).size(theme.font_ui));
    }
    ui.add_space(4.0);
    let quotes_height = if transcript.quotes.is_empty() { 0.0 } else { (ui.available_height() * QUOTES_SHARE).max(60.0) };
    let blocks_height = (ui.available_height() - quotes_height - 8.0).max(60.0);
    egui::ScrollArea::vertical().id_salt("qnc_script_blocks").max_height(blocks_height).auto_shrink([false, false]).show(ui, |ui| {
        if view.blocks.is_empty() {
            ui.label(muted(qnc_ui_text::text("script.empty")));
        }
        for row in &view.blocks {
            if let Some(event) = paint_block(ui, row, view, theme, style) {
                asked = Some(event);
            }
            ui.add_space(3.0);
        }
    });
    if !transcript.quotes.is_empty() {
        ui.add_space(6.0);
        ui.label(muted(qnc_ui_text::text("script.quotes")));
        egui::ScrollArea::vertical().id_salt("qnc_script_quotes").max_height(quotes_height).auto_shrink([false, true]).show(ui, |ui| {
            for quote in &transcript.quotes {
                ui.horizontal(|ui| {
                    if qnc_ui_kit::chrome_button(ui, "+", true, false, style).on_hover_text(qnc_ui_text::text("script.add_quote")).clicked() {
                        asked = Some(ActionEvent::key(ADD_QUOTE).with_target(quote.id.to_string()).at(PANEL));
                    }
                    ui.add(egui::Label::new(muted(&format!("{} {}  {}", quote.clip_name, quote.start_tc, quote.text))).truncate());
                });
            }
        });
    }
    asked
}

/// One block: its kind, length and buttons on top, its text below; a click chooses it (an OFF
/// block's text goes into the editor), a statement can open its clip.
fn paint_block(ui: &mut egui::Ui, row: &qnc_script::BlockRow, view: &ScriptView, theme: &qnc_desktop_theme::Theme, style: qnc_ui_kit::ChromeButtonStyle) -> Option<ActionEvent> {
    let mut asked = None;
    let block = &row.block;
    let id = block.id.to_string();
    let chosen = view.selected == Some(block.id);
    let stroke = if chosen { egui::Stroke::new(1.5, theme.focus) } else { egui::Stroke::new(1.0, theme.border) };
    let fill = if block.kind == BlockKind::Sot { theme.surface_alt } else { theme.bg };
    let frame = egui::Frame::new().fill(fill).stroke(stroke).inner_margin(egui::Margin::symmetric(6, 4)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (badge, color) = match block.kind {
                BlockKind::Off => (qnc_ui_text::text("script.off"), theme.focus),
                BlockKind::Sot => (qnc_ui_text::text("script.sot"), theme.text),
                BlockKind::Intro => (qnc_ui_text::text("script.intro"), theme.text_muted),
                BlockKind::Outro => (qnc_ui_text::text("script.outro"), theme.text_muted),
            };
            ui.label(RichText::new(badge).color(color).strong().size(theme.font_ui));
            let detail = match block.kind {
                BlockKind::Off if !block.voice_uri.is_empty() => qnc_ui_text::fill("script.recorded", &[("length", &seconds_label(row.seconds)), ("tc", &block.voice_tc)]),
                BlockKind::Off | BlockKind::Intro | BlockKind::Outro => seconds_label(row.seconds),
                BlockKind::Sot => format!("{}  {} - {}  {}", block.clip_name, block.start_tc, block.end_tc, block.length),
            };
            ui.label(RichText::new(detail).color(theme.text_muted).size(theme.font_ui));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for (label, action, hint) in [("X", REMOVE, "script.remove"), ("v", MOVE_DOWN, "script.move_down"), ("^", MOVE_UP, "script.move_up")] {
                    if qnc_ui_kit::chrome_button(ui, label, true, false, style).on_hover_text(qnc_ui_text::text(hint)).clicked() {
                        asked = Some(ActionEvent::key(action).with_target(id.clone()).at(PANEL));
                    }
                }
                if block.kind == BlockKind::Sot && qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("script.open"), true, false, style).clicked() {
                    asked = Some(ActionEvent::key(OPEN).with_target(id.clone()).at(PANEL));
                }
                // Rec on an OFF block; while it records, Stop (user 2026-10-28).
                if block.kind == BlockKind::Off {
                    let this = view.recording.is_some_and(|(recording, _, _)| recording == block.id);
                    let other = view.recording.is_some() && !this;
                    let label = if this { qnc_ui_text::text("script.stop") } else { qnc_ui_text::text("script.record") };
                    let red = qnc_ui_kit::ChromeButtonStyle { text: theme.danger, border: theme.danger, focus: theme.danger, ..style };
                    if qnc_ui_kit::chrome_button(ui, label, !other && !view.record_busy, this, red).on_hover_text(qnc_ui_text::text("script.record_hint")).clicked() {
                        asked = Some(ActionEvent::key(RECORD).with_target(id.clone()).at(PANEL));
                    }
                }
            });
        });
        // The take being recorded: its time and the microphone's level.
        if let Some((_, seconds, peak)) = view.recording.filter(|(recording, _, _)| *recording == block.id) {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("● {}", seconds_label(seconds))).color(theme.danger).strong().size(theme.font_ui));
                let (bar, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().min(240.0), 8.0), Sense::hover());
                ui.painter().rect_filled(bar, 2.0, theme.surface_alt);
                let level = if peak > 0.0 { ((20.0 * peak.log10() + 48.0) / 48.0).clamp(0.0, 1.0) } else { 0.0 };
                let mut lit = bar;
                lit.set_width(bar.width() * level);
                ui.painter().rect_filled(lit, 2.0, if peak > 0.7 { theme.danger } else { theme.focus });
            });
        }
        let text = if block.text.is_empty() { qnc_ui_text::text("script.no_words").to_string() } else { block.text.clone() };
        let color = if block.text.is_empty() { theme.text_muted } else { theme.text };
        ui.add(egui::Label::new(RichText::new(text).color(color).size(theme.font_ui + 1.0)).wrap());
    });
    let response = ui.interact(frame.response.rect, egui::Id::new(("qnc_script_block", block.id)), Sense::click());
    if response.clicked() && asked.is_none() {
        asked = Some(ActionEvent::key(SELECT).with_target(id).at(PANEL));
    }
    asked
}

/// The TextSourceEditor at the bottom: OFF text is written here; Add OFF puts it after the
/// chosen block, Replace puts it back into the block it was taken from.
fn paint_editor(ui: &mut egui::Ui, view: &ScriptView, theme: &qnc_desktop_theme::Theme) -> Option<ActionEvent> {
    let written = !view.editor.trim().is_empty();
    let buttons = [
        EditorButton { label: qnc_ui_text::text("script.clear"), action_id: CLEAR_EDITOR, enabled: written || view.editing.is_some() },
        EditorButton { label: qnc_ui_text::text("script.replace"), action_id: REPLACE_OFF, enabled: written && view.editing.is_some() },
        EditorButton { label: qnc_ui_text::text("script.add_intro"), action_id: ADD_INTRO, enabled: written },
        EditorButton { label: qnc_ui_text::text("script.add_outro"), action_id: ADD_OUTRO, enabled: written },
        EditorButton { label: qnc_ui_text::text("script.add_off"), action_id: ADD_OFF, enabled: written },
    ];
    let info = qnc_ui_text::fill("script.editor_info", &[("words", &view.editor.split_whitespace().count().to_string()), ("length", &seconds_label(view.editor_seconds))]);
    let title = if view.editing.is_some() { qnc_ui_text::text("script.editor_editing") } else { qnc_ui_text::text("script.editor") };
    let editor = Editor { id: "qnc_script_editor", title, text: &view.editor, info: &info, hint: qnc_ui_text::text("script.editor_hint"), buttons: &buttons };
    match qnc_text_source_editor::show(ui, &editor, theme)? {
        Answer::Text(text) => Some(ActionEvent::key(EDITOR_TEXT).with_target(text).at(PANEL)),
        Answer::Button(action) => Some(ActionEvent::key(action).at(PANEL)),
    }
}
