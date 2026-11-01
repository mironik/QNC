//! The panel of writing the story (user 2026-11-01, MA Script: the form is the script, the media
//! are in a window over it). On top the story's working title (the story sheet's slug), its
//! approved length, the length written so far and what is left, with the document buttons on
//! the right (the text for review, copy, draft XML, send to S-Story). Under them the blocks in
//! reading order, drawn as in the script panel. In the lower third a floating group of story
//! buttons, one per kind of the project's script profile (NAJAVA, OFF, IZJAVA, STAND-UP, NETON
//! ...): a text kind opens the editor above the group (a kind of the anchor once only, OFF
//! numbered), a clip kind opens the media window and, above the group, the row that puts the
//! marked IN-OUT in. Which kind is open is a view choice of this process (egui memory); the
//! panel holds no other state, reads nothing and gives catalog actions.

use eframe::egui::{self, Align, Id, Layout, Rect, RichText, Sense, Ui, UiBuilder, Vec2};
use qnc_action_event::ActionEvent;
use qnc_script::{seconds_label, KindSpec, Role, ScriptView, ADD_CLIP, CLEAR_EDITOR, DRAFT, EDITOR_TEXT, EXPORT, OPEN_EXPORT, PANEL, SAVE_TEXT, SEND_STORY};
use qnc_text_source_editor::{Answer, Editor, EditorButton};

pub const MODULE_ID: &str = "qnc.module.script-writer-panel";

/// The width of a story button and the gap between them.
const STORY_BUTTON: f32 = 132.0;
const STORY_GAP: f32 = 10.0;
/// The editor above the story buttons: its widest and its height.
const EDITOR_WIDTH: f32 = 860.0;
const EDITOR_HEIGHT: f32 = 230.0;

fn open_id() -> Id {
    Id::new("qnc-script-writer-open")
}

/// The kind whose editor (or clip row) is open.
fn open_kind(ui: &Ui) -> Option<String> {
    ui.ctx().data(|data| data.get_temp::<Option<String>>(open_id())).flatten()
}

fn set_open_kind(ui: &Ui, kind: Option<String>) {
    ui.ctx().data_mut(|data| data.insert_temp(open_id(), kind));
}

/// Paints the panel over the whole of `ui`.
pub fn show(ui: &mut Ui, view: &ScriptView, theme: &qnc_desktop_theme::Theme) -> Option<ActionEvent> {
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(rect, 0.0, theme.bg);
    let inner = rect.shrink(12.0);
    let style = qnc_ui_kit::ChromeButtonStyle { text: theme.text, border: theme.border, focus: theme.focus, height: theme.chrome_control_height };
    let mut asked = None;
    // A block taken into the editor by a click opens the editor of its kind.
    let editing = view.editing.and_then(|id| view.kind_of(id)).map(|kind| kind.id.clone());
    let open = editing.clone().or_else(|| open_kind(ui));
    let open_spec = open.as_deref().and_then(|code| view.profile.as_ref()?.kind(code));

    // The header: title, lengths and the document buttons.
    let header_height = theme.chrome_row_height * 2.0 + theme.font_ui + 30.0;
    let header = Rect::from_min_size(inner.min, Vec2::new(inner.width(), header_height));
    let mut top = ui.new_child(UiBuilder::new().max_rect(header).layout(Layout::top_down(Align::Min)));
    if let Some(event) = paint_header(&mut top, view, theme, style) {
        asked = Some(event);
    }

    // The story buttons in the middle of the lower third.
    let row_height = theme.chrome_control_height + 8.0;
    let kinds: Vec<&KindSpec> = view.profile.iter().flat_map(|profile| &profile.kinds).collect();
    let row_width = (kinds.len() as f32 * (STORY_BUTTON + STORY_GAP) - STORY_GAP).max(STORY_BUTTON).min(inner.width());
    let row_center_y = inner.bottom() - inner.height() / 6.0;
    let row = Rect::from_center_size(egui::pos2(inner.center().x, row_center_y), Vec2::new(row_width, row_height));
    let mut buttons = ui.new_child(UiBuilder::new().max_rect(row).layout(Layout::left_to_right(Align::Center)));
    buttons.spacing_mut().item_spacing.x = STORY_GAP;
    buttons.spacing_mut().interact_size = Vec2::new(STORY_BUTTON, theme.chrome_control_height);
    for kind in &kinds {
        // A kind of the anchor (NAJAVA, ODJAVA) goes in once; its button greys once it is there.
        let used = kind.role == Role::Anchor && view.blocks.iter().any(|row| row.block.kind == kind.id);
        let on = open.as_deref() == Some(kind.id.as_str());
        let enabled = !used && view.recording.is_none();
        // Every button as wide as the others, so the group is centred as measured.
        let cell = Vec2::new(STORY_BUTTON, theme.chrome_control_height);
        let pressed = buttons.allocate_ui_with_layout(cell, Layout::centered_and_justified(egui::Direction::LeftToRight), |ui| qnc_ui_kit::chrome_button(ui, &kind.short, enabled, on, style).on_hover_text(&kind.label).clicked()).inner;
        if pressed {
            if kind.role == Role::Clip {
                qnc_media_overlay::set_open(ui.ctx(), true);
            }
            set_open_kind(ui, Some(kind.id.clone()));
            // Another kind than the block in the editor: the editor starts empty.
            if editing.is_some() {
                asked = Some(ActionEvent::key(CLEAR_EDITOR).at(PANEL));
            }
        }
    }

    // Above the story buttons the editor of the open kind, or the row of a clip kind.
    let mut list_bottom = row.top() - 10.0;
    if let Some(kind) = open_spec {
        let height = if kind.role == Role::Clip { theme.chrome_row_height * 2.0 + 16.0 } else { EDITOR_HEIGHT };
        let width = EDITOR_WIDTH.min(inner.width());
        let area = Rect::from_min_size(egui::pos2(inner.center().x - width / 2.0, row.top() - 10.0 - height), Vec2::new(width, height));
        list_bottom = area.top() - 10.0;
        ui.painter().rect(area.expand(6.0), 6.0, theme.surface, egui::Stroke::new(1.0, theme.focus), egui::StrokeKind::Outside);
        let mut panel = ui.new_child(UiBuilder::new().max_rect(area).layout(Layout::top_down(Align::Min)));
        panel.set_clip_rect(area.expand(6.0).intersect(ui.clip_rect()));
        let answer = if kind.role == Role::Clip { clip_row(&mut panel, kind, view, theme, style) } else { editor(&mut panel, kind, view, theme) };
        if let Some((event, close)) = answer {
            if close {
                set_open_kind(ui, None);
            }
            asked = Some(event);
        }
    }

    // The blocks of the story between the header and what floats in the lower part.
    let list = Rect::from_min_max(egui::pos2(inner.left(), header.bottom() + 6.0), egui::pos2(inner.right(), list_bottom.max(header.bottom() + 40.0)));
    let mut blocks = ui.new_child(UiBuilder::new().max_rect(list).layout(Layout::top_down(Align::Min)));
    blocks.set_clip_rect(list.intersect(ui.clip_rect()));
    egui::ScrollArea::vertical().id_salt("qnc_script_writer_blocks").auto_shrink([false, false]).show(&mut blocks, |ui| {
        if view.blocks.is_empty() {
            ui.label(RichText::new(qnc_ui_text::text("writer.story_empty")).color(theme.text_muted).size(theme.font_ui));
        }
        for row in &view.blocks {
            if let Some(event) = qnc_script_panel::paint_block(ui, row, view, theme, style) {
                asked = Some(event);
            }
            ui.add_space(3.0);
        }
    });
    ui.allocate_rect(rect, Sense::hover());
    asked
}

/// Title and lengths on the left, the document buttons on the right; the message or the link
/// to the document written under them.
fn paint_header(ui: &mut Ui, view: &ScriptView, theme: &qnc_desktop_theme::Theme, style: qnc_ui_kit::ChromeButtonStyle) -> Option<ActionEvent> {
    let mut asked = None;
    let muted = |text: &str| RichText::new(text).color(theme.text_muted).size(theme.font_ui);
    let out = !view.blocks.is_empty() && !view.exporting;
    ui.horizontal(|ui| {
        let title = view.title.as_deref().unwrap_or(qnc_ui_text::text("writer.untitled"));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // The document group: what leaves the script (user 2026-11-01).
            if qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("script.send_story"), out, false, style).on_hover_text(qnc_ui_text::text("script.send_story_hint")).clicked() {
                asked = Some(ActionEvent::key(SEND_STORY).at(PANEL));
            }
            if qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("script.draft"), out, false, style).on_hover_text(qnc_ui_text::text("script.draft_hint")).clicked() {
                asked = Some(ActionEvent::key(DRAFT).at(PANEL));
            }
            if qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("script.copy"), !view.blocks.is_empty(), false, style).clicked() {
                ui.ctx().copy_text(view.text());
            }
            if qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("writer.review"), out, false, style).on_hover_text(qnc_ui_text::text("script.export_hint")).clicked() {
                asked = Some(ActionEvent::key(EXPORT).at(PANEL));
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.add(egui::Label::new(RichText::new(title).color(theme.text).strong().size(theme.font_ui + 5.0)).truncate());
            });
        });
    });
    // The lengths in the middle, large: approved, the length so far, and what is left or how much
    // it is over (user 2026-11-01: the information that matters).
    let total = view.total_seconds();
    let target = view.target_seconds;
    let left = target.map(|target| target - total);
    let big = theme.font_ui + 9.0;
    let dash = || "-".to_string();
    let mut pieces = vec![
        (qnc_ui_text::fill("writer.approved", &[("t", &target.map_or_else(dash, seconds_label))]), theme.text),
        (qnc_ui_text::fill("writer.length", &[("t", &seconds_label(total))]), theme.text),
    ];
    pieces.push(match left {
        Some(left) if left < 0.0 => (qnc_ui_text::fill("writer.over", &[("t", &seconds_label(-left))]), theme.danger),
        Some(left) => (qnc_ui_text::fill("writer.left", &[("t", &seconds_label(left))]), theme.focus),
        None => (qnc_ui_text::fill("writer.left", &[("t", &dash())]), theme.text_muted),
    });
    let mut job = egui::text::LayoutJob::default();
    for (index, (text, color)) in pieces.iter().enumerate() {
        if index > 0 {
            job.append("     ·     ", 0.0, egui::TextFormat { font_id: egui::FontId::proportional(big), color: theme.text_muted, ..Default::default() });
        }
        job.append(text, 0.0, egui::TextFormat { font_id: egui::FontId::proportional(big), color: *color, ..Default::default() });
    }
    ui.vertical_centered(|ui| {
        ui.label(job);
    });
    ui.horizontal(|ui| {
        if view.anchor_seconds > 0.0 {
            ui.label(muted(&qnc_ui_text::fill("writer.anchor", &[("anchor", &seconds_label(view.anchor_seconds))])));
        }
        if !view.message.is_empty() {
            ui.label(RichText::new(&view.message).color(theme.danger).size(theme.font_ui));
        } else if let Some(file) = view.exported.as_deref() {
            let name = std::path::Path::new(file).file_name().map_or_else(|| file.to_string(), |name| name.to_string_lossy().into_owned());
            let link = egui::Label::new(RichText::new(qnc_ui_text::fill("script.exported", &[("file", &name)])).color(theme.focus).underline().size(theme.font_ui)).sense(Sense::click());
            if ui.add(link).on_hover_text(file).clicked() {
                asked = Some(ActionEvent::key(OPEN_EXPORT).at(PANEL));
            }
        }
    });
    asked
}

/// The editor of a text kind: its title (the numbered cue of the next block of the kind, or the
/// block taken into it), Save and Cancel. Gives the action and whether the editor closes.
fn editor(ui: &mut Ui, kind: &KindSpec, view: &ScriptView, theme: &qnc_desktop_theme::Theme) -> Option<(ActionEvent, bool)> {
    let written = !view.editor.trim().is_empty();
    let title = match view.editing.and_then(|id| view.blocks.iter().find(|row| row.block.id == id)) {
        Some(row) => qnc_ui_text::fill("writer.editing", &[("kind", &row.mark)]),
        None => next_mark(kind, view),
    };
    let buttons = [
        EditorButton { label: qnc_ui_text::text("common.save"), action_id: SAVE_TEXT, target: &kind.id, enabled: written, group: 0 },
        EditorButton { label: qnc_ui_text::text("common.cancel"), action_id: CLEAR_EDITOR, target: "", enabled: true, group: 0 },
    ];
    let info = qnc_ui_text::fill("script.editor_info", &[("words", &view.editor.split_whitespace().count().to_string()), ("length", &seconds_label(view.editor_seconds))]);
    let editor = Editor { id: "qnc_script_writer_editor", title: &title, text: &view.editor, info: &info, hint: qnc_ui_text::text("script.editor_hint"), buttons: &buttons };
    match qnc_text_source_editor::show(ui, &editor, theme)? {
        Answer::Text(text) => Some((ActionEvent::key(EDITOR_TEXT).with_target(text).at(PANEL), false)),
        Answer::Button(action, target) => {
            let event = ActionEvent::key(&action).at(PANEL);
            Some((match target { Some(kind) => event.with_target(kind), None => event }, true))
        }
    }
}

/// The row of a clip kind: what goes in (the IN-OUT of the clip on the source timeline), the
/// media window to open again, Add and Cancel.
fn clip_row(ui: &mut Ui, kind: &KindSpec, view: &ScriptView, theme: &qnc_desktop_theme::Theme, style: qnc_ui_kit::ChromeButtonStyle) -> Option<(ActionEvent, bool)> {
    let mut asked = None;
    let say = if view.marks { qnc_ui_text::fill("writer.clip_title", &[("kind", &next_mark(kind, view))]) } else { qnc_ui_text::text("writer.clip_no_marks").to_string() };
    ui.label(RichText::new(say).color(if view.marks { theme.text } else { theme.text_muted }).size(theme.font_ui + 1.0));
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("ma_script.media"), true, false, style).clicked() {
            qnc_media_overlay::set_open(ui.ctx(), true);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("common.cancel"), true, false, style).clicked() {
                asked = Some((ActionEvent::key(CLEAR_EDITOR).at(PANEL), true));
            }
            if qnc_ui_kit::strong_button(ui, qnc_ui_text::text("writer.clip_add"), view.marks, theme.accent, theme.chrome_control_height).clicked() {
                asked = Some((ActionEvent::key(ADD_CLIP).with_target(kind.id.clone()).at(PANEL), true));
            }
        });
    });
    asked
}

/// The mark the next block of `kind` gets: its numbered cue (`OFF 3`) or its short mark.
fn next_mark(kind: &KindSpec, view: &ScriptView) -> String {
    if kind.cue.is_empty() {
        return kind.short.clone();
    }
    let n = view.blocks.iter().filter(|row| row.block.kind == kind.id).count() + 1;
    kind.cue.replace("{n}", &n.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(id: &str, cue: &str) -> KindSpec {
        serde_json::from_value(serde_json::json!({ "id": id, "label": id, "short": id.to_uppercase(), "role": "spoken", "counts": "story", "cue": cue })).unwrap()
    }

    #[test]
    fn the_next_block_of_a_numbered_kind_gets_the_next_number() {
        let view = ScriptView::default();
        assert_eq!(next_mark(&kind("off", "OFF {n}"), &view), "OFF 1");
        assert_eq!(next_mark(&kind("nat", ""), &view), "NAT");
    }
}
