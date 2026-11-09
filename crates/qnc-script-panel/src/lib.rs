//! The script panel (user 2026-10-28, Media Assist Audio: writing the offs and putting the
//! story together as text). On top the length of the script against the story's target and a
//! button per clip kind of the project's script profile (its IN-OUT goes in as that kind), then
//! the blocks in reading order (each with its mark from the profile, its length, up, down and
//! remove; Rec on a recorded kind), the statements of the transcript to put in, and at the
//! bottom the TextSourceEditor where text is written and corrected, with a button per text kind
//! of the profile. It paints the script view and gives catalog actions; it holds no state,
//! reads nothing and knows no form or profile of its own.

use eframe::egui::{self, RichText, Sense};
use qnc_action_event::ActionEvent;
use qnc_script::{seconds_label, FieldEdit, DRAFT, EXPORT, SEND_STORY, OPEN_EXPORT, Role, FIELD_COMMIT, FIELD_SET, FIELD_TEXT, ScriptView, ADD_CLIP, ADD_QUOTE, ADD_TEXT, CLEAR_EDITOR, EDITOR_TEXT, MOVE_DOWN, MOVE_UP, OPEN, PANEL, RECORD, REMOVE, REPLACE_TEXT, SELECT, VOICE_TAKE};

mod part_controls;
mod read_marks;
mod text_editor;
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
    recording_press(ui, rect, view, &mut asked);
    asked
}

/// While a recording runs a mouse click or a tap on the pad anywhere on the panel marks a new
/// reading (user 2026-11-09); only the Stop button still works.
pub fn recording_press(ui: &egui::Ui, rect: egui::Rect, view: &ScriptView, asked: &mut Option<ActionEvent>) {
    if view.session.is_none() && view.recording.is_none() {
        return;
    }
    let pressed = ui.input(|i| i.pointer.primary_clicked() && i.pointer.interact_pos().is_some_and(|pos| rect.contains(pos)));
    let stop = asked.take().filter(|event| event.action_id == qnc_script::SESSION || event.action_id == RECORD);
    *asked = stop.or(pressed.then(|| ActionEvent::key(qnc_script::SESSION_MARK).at(PANEL)));
}

fn paint_script(ui: &mut egui::Ui, view: &ScriptView, transcript: &TranscriptView, theme: &qnc_desktop_theme::Theme) -> Option<ActionEvent> {
    let mut asked = None;
    let style = qnc_ui_kit::ChromeButtonStyle { text: theme.text, border: theme.border, focus: theme.focus, height: theme.chrome_control_height };
    let muted = |text: &str| RichText::new(text).color(theme.text_muted).size(theme.font_ui);
    // Buttons in logical groups (user 2026-11-01): the first row is the script and what goes out
    // of it (copy, export, draft, send to S-Story, in that order), the second what goes into it
    // from the shown clip's IN-OUT.
    let out = !view.blocks.is_empty() && !view.exporting;
    ui.horizontal(|ui| {
        ui.label(RichText::new(qnc_ui_text::text("script.title")).color(theme.text).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("script.send_story"), out, false, style).on_hover_text(qnc_ui_text::text("script.send_story_hint")).clicked() {
                asked = Some(ActionEvent::key(SEND_STORY).at(PANEL));
            }
            if qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("script.draft"), out, false, style).on_hover_text(qnc_ui_text::text("script.draft_hint")).clicked() {
                asked = Some(ActionEvent::key(DRAFT).at(PANEL));
            }
            if qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("script.export"), out, false, style).on_hover_text(qnc_ui_text::text("script.export_hint")).clicked() {
                asked = Some(ActionEvent::key(EXPORT).at(PANEL));
            }
            if qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("script.copy"), !view.blocks.is_empty(), false, style).clicked() {
                ui.ctx().copy_text(view.text());
            }
            // The profile's name takes what is left and never runs under the buttons.
            if let Some(profile) = &view.profile {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add(egui::Label::new(muted(&qnc_ui_text::fill("script.profile", &[("name", &profile.name)]))).truncate());
                });
            }
        });
    });
    ui.horizontal(|ui| {
        ui.label(muted(qnc_ui_text::text("script.from_clip")));
        // One button per clip kind: the shown clip's IN-OUT goes in as that kind.
        for kind in view.clip_kinds() {
            let label = qnc_ui_text::fill("script.add_clip", &[("kind", &kind.short)]);
            let hint = qnc_ui_text::fill("script.add_clip_hint", &[("kind", &kind.label)]);
            if qnc_ui_kit::chrome_button(ui, &label, view.marks, false, style).on_hover_text(hint).clicked() {
                asked = Some(ActionEvent::key(ADD_CLIP).with_target(kind.id.clone()).at(PANEL));
            }
        }
    });
    // The length of the script against the target of the story sheet (red when over).
    let total = view.total_seconds();
    let lengths = qnc_ui_text::fill("script.lengths", &[("off", &seconds_label(view.read_seconds)), ("sot", &seconds_label(view.clip_seconds)), ("total", &seconds_label(total)), ("anchor", &seconds_label(view.anchor_seconds))]);
    ui.horizontal(|ui| {
        ui.label(muted(&lengths));
        if let Some(target) = view.target_seconds {
            let over = total > target;
            let text = qnc_ui_text::fill("script.target", &[("target", &seconds_label(target))]);
            ui.label(RichText::new(text).color(if over { theme.danger } else { theme.text_muted }).size(theme.font_ui));
        }
    });
    // The story's in-cue and out-cue (the director's cue that it ends).
    if !view.in_cue.is_empty() {
        ui.label(muted(&qnc_ui_text::fill("script.story_cues", &[("in", &view.in_cue), ("out", &view.out_cue)])));
    }
    if !view.message.is_empty() {
        ui.label(RichText::new(&view.message).color(theme.danger).size(theme.font_ui));
    }
    if let Some(file) = view.exported.as_deref().filter(|_| view.message.is_empty()) {
        let name = std::path::Path::new(file).file_name().map_or_else(|| file.to_string(), |name| name.to_string_lossy().into_owned());
        let link = egui::Label::new(RichText::new(qnc_ui_text::fill("script.exported", &[("file", &name)])).color(theme.focus).underline().size(theme.font_ui)).sense(Sense::click());
        if ui.add(link).on_hover_text(file).clicked() {
            asked = Some(ActionEvent::key(OPEN_EXPORT).at(PANEL));
        }
    }
    ui.add_space(4.0);
    let quotes_height = if transcript.quotes.is_empty() { 0.0 } else { (ui.available_height() * QUOTES_SHARE).max(60.0) };
    let blocks_height = (ui.available_height() - quotes_height - 8.0).max(60.0);
    egui::ScrollArea::vertical().id_salt("qnc_script_blocks").max_height(blocks_height).auto_shrink([false, false]).show(ui, |ui| {
        if view.blocks.is_empty() {
            ui.label(muted(qnc_ui_text::text("script.empty")));
        }
        for row in &view.blocks {
            if let Some(event) = paint_block(ui, "script", row, view, theme, style) {
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

/// The statement open as a SubSegment on the source timeline's place, alone (user 2026-11-04:
/// the overlay where statements are put together shows its text, interactive): the same block
/// as in the script, its parts' words with their times, the chosen slot's words marked. `None`
/// when no statement of the script is open.
pub fn show_open_statement(ui: &mut egui::Ui, view: &ScriptView, theme: &qnc_desktop_theme::Theme) -> Option<Option<ActionEvent>> {
    let open = view.subsegment.as_ref()?;
    let row = view.blocks.iter().find(|row| row.block.id == open.block_id)?;
    let style = qnc_ui_kit::ChromeButtonStyle { text: theme.text, border: theme.border, focus: theme.focus, height: theme.chrome_control_height };
    let mut asked = None;
    egui::ScrollArea::vertical().id_salt("qnc_script_open_statement").auto_shrink([false, false]).show(ui, |ui| {
        asked = paint_block(ui, "open", row, view, theme, style);
    });
    Some(asked)
}

/// One block: its mark, length and buttons on top, its text below; a click chooses it (a text
/// block's text goes into the editor), a clip block can open its clip. Public: every panel of
/// the script draws its blocks with it (MA Audio's and MA Script's).
pub fn paint_block(ui: &mut egui::Ui, place: &'static str, row: &qnc_script::BlockRow, view: &ScriptView, theme: &qnc_desktop_theme::Theme, style: qnc_ui_kit::ChromeButtonStyle) -> Option<ActionEvent> {
    let mut asked = None;
    let block = &row.block;
    let id = block.id.to_string();
    let chosen = view.selected == Some(block.id);
    let mut picked = false;
    let stroke = if chosen { egui::Stroke::new(1.5, theme.focus) } else { egui::Stroke::new(1.0, theme.border) };
    let fill = if row.is_clip() { theme.surface_alt } else { theme.bg };
    egui::Frame::new().fill(fill).stroke(stroke).inner_margin(egui::Margin::symmetric(6, 4)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let color = match row.role {
                Some(Role::Spoken) => theme.focus,
                Some(Role::Clip) => theme.text,
                Some(Role::Anchor | Role::Note) => theme.text_muted,
                None => theme.danger,
            };
            if ui.add(egui::Label::new(RichText::new(&row.mark).color(color).strong().size(theme.font_ui)).sense(Sense::click())).clicked() {
                picked = true;
            }
            // A SubSegment of several parts: how many and how long all of them are.
            let detail = if row.is_clip() && block.pieces.len() > 1 { format!("{}  {}", block.length, seconds_label(row.seconds)) } else { String::new() };
            ui.label(RichText::new(detail).color(theme.text_muted).size(theme.font_ui));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for (label, action, hint) in [("X", REMOVE, "script.remove"), ("v", MOVE_DOWN, "script.move_down"), ("^", MOVE_UP, "script.move_up")] {
                    if qnc_ui_kit::chrome_button(ui, label, true, false, style).on_hover_text(qnc_ui_text::text(hint)).clicked() {
                        asked = Some(ActionEvent::key(action).with_target(id.clone()).at(PANEL));
                    }
                }
                let open = if row.block.pieces.len() > 1 { "script.open_subsegment" } else { "script.open" };
                if row.is_clip() && qnc_ui_kit::chrome_button(ui, qnc_ui_text::text(open), true, false, style).clicked() {
                    asked = Some(ActionEvent::key(OPEN).with_target(id.clone()).at(PANEL));
                }
                // Rec on a block of a recorded kind; while it records, Stop (user 2026-10-28).
                if row.recorded {
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
        // The voice record of a recorded block: its status and its takes, one of them used
        // (user 2026-10-30: chosen as A1 of a camera clip).
        if row.recorded && view.recording.is_none_or(|(recording, _, _)| recording != block.id) {
            ui.horizontal_wrapped(|ui| {
                let (status, color) = match row.voice.as_ref().map(|voice| voice.status) {
                    None => (qnc_ui_text::text("script.voice_none"), theme.text_muted),
                    Some(qnc_script::VoiceStatus::Recorded) => (qnc_ui_text::text("script.voice_recorded"), theme.focus),
                    Some(qnc_script::VoiceStatus::TextChanged) => (qnc_ui_text::text("script.voice_text_changed"), theme.danger),
                    Some(qnc_script::VoiceStatus::Missing) => (qnc_ui_text::text("script.voice_missing"), theme.danger),
                };
                ui.label(RichText::new(status).color(color).size(theme.font_ui));
                for take in row.voice.iter().flat_map(|voice| voice.takes.iter().map(move |take| (voice.chosen, take))) {
                    let (chosen, take) = take;
                    let hint = qnc_ui_text::fill("script.voice_take_hint", &[("take", &take.take.to_string()), ("length", &seconds_label(take.seconds)), ("tc", &take.tc), ("text", &take.text)]);
                    // How much of the text the take read; ".." while it is being heard.
                    let label = match row.voice.as_ref().and_then(|voice| voice.check_of(take.take)) {
                        Some(check) => format!("{}{} {}%", row.mark, qnc_script_db::take_letter(take.take), check.percent),
                        None if view.read_note.is_none() => format!("{}{} ..", row.mark, qnc_script_db::take_letter(take.take)),
                        None => format!("{}{}", row.mark, qnc_script_db::take_letter(take.take)),
                    };
                    if qnc_ui_kit::chrome_button(ui, &label, view.recording.is_none(), take.take == chosen, style).on_hover_text(hint).clicked() {
                        asked = Some(ActionEvent::key(VOICE_TAKE).with_target(format!("{}:{}", block.id, take.take)).at(PANEL));
                    }
                }
                if let Some(note) = view.read_note.as_ref().filter(|_| row.voice.is_some()) {
                    ui.label(RichText::new(note).color(theme.text_muted).size(theme.font_ui));
                }
            });
        }
        // The take being recorded: its time and the microphone's level.
        if let Some((_, seconds, peak)) = view.recording.filter(|(recording, _, _)| *recording == block.id) {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("● {}  ·  {}", seconds_label(seconds), view.recording_marks + 1)).color(theme.danger).strong().size(theme.font_ui));
                let (bar, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().min(240.0), 8.0), Sense::hover());
                ui.painter().rect_filled(bar, 2.0, theme.surface_alt);
                let level = if peak > 0.0 { ((20.0 * peak.log10() + 48.0) / 48.0).clamp(0.0, 1.0) } else { 0.0 };
                let mut lit = bar;
                lit.set_width(bar.width() * level);
                ui.painter().rect_filled(lit, 2.0, if peak > 0.7 { theme.danger } else { theme.focus });
            });
        }
        // A SubSegment of several parts: each part's clip and timecodes, its words under it, one
        // part under another (user 2026-10-29).
        // Every block is this table (user 2026-11-09: as the composite statement): the time on
        // the left, the text in the middle.
        let parts: Vec<(String, &str)> = if block.pieces.len() > 1 {
            block.pieces.iter().map(|piece| (format!("{} - {}  {}", piece.start_tc, piece.end_tc, piece.length), piece.text.as_str())).collect()
        } else {
            let time = if row.is_clip() {
                format!("{} - {}  {}", block.start_tc, block.end_tc, block.length)
            } else if !block.voice_uri.is_empty() {
                format!("{}  {}", block.voice_tc, seconds_label(row.seconds))
            } else if row.role == Some(Role::Note) {
                String::new()
            } else {
                seconds_label(row.seconds)
            };
            vec![(time, block.text.as_str())]
        };
        for (index, (detail, text)) in parts.into_iter().enumerate() {
            // The text of the part (the middle column of a SubSegment's table).
            let paint_text = |ui: &mut egui::Ui| -> (Option<ActionEvent>, bool) {
            let mut asked = None;
            // The words of the part with their times (user 2026-11-04: the transcript in the
            // script has its timecodes, to edit by its text): the timecode in front of every line
            // of speech, a click on a word puts the playhead there, a double click on two words
            // of the part is its IN and OUT.
            if let Some(words) = row.words.get(index).filter(|words| !words.is_empty()) {
                // The slot chosen on the open SubSegment's timeline is this part (user
                // 2026-11-04: the slot and its text go together): its words are marked, the
                // word under the playhead lit, and the text is brought into sight once.
                let open = view.subsegment.as_ref().filter(|open| open.block_id == block.id);
                let part_chosen = open.is_some_and(|open| open.chosen == Some(index));
                let heard = open.and_then(|open| {
                    let part = open.parts.get(index)?;
                    let from = block.pieces.get(index).map_or(block.from_frame, |piece| piece.from_frame);
                    let at = open.playhead.filter(|at| (part.start..part.end).contains(at))?;
                    Some(from + (at - part.start))
                });
                let seen = egui::Id::new(("qnc_script_chosen_part", block.id));
                let bring = part_chosen && ui.ctx().data(|data| data.get_temp::<usize>(seen)) != Some(index);
                ui.ctx().data_mut(|data| if part_chosen { data.insert_temp(seen, index) } else if open.is_some_and(|open| open.chosen.is_none()) { data.remove::<usize>(seen) });
                // The words are a text edited as in any editor (user 2026-11-06: "a plain classic
                // text edit: what is deleted in the text is deleted in the sound"): Enter cuts out
                // what the text no longer says, Esc gives the text back; a click puts the playhead
                // on the word clicked.
                if let Some(event) = text_editor::show(ui, theme, place, (block.id, index), words, view.text_draft(block.id, index, &qnc_script::text_of(words)), heard, (part_chosen, bring)) {
                    asked = Some(event);
                }
                return (asked, false);
            }
            // A recorded text shows how the take used was read (user 2026-11-08).
            let recording_here = view.recording.is_some_and(|(recording, _, _)| recording == block.id);
            if row.recorded && !text.is_empty() && !recording_here {
                let check = row.voice.as_ref().and_then(|voice| voice.check_of(voice.chosen));
                let taken = view.taken_mark.filter(|(taken, _)| *taken == block.id).map(|(_, word)| word);
                let (clicked, right) = read_marks::show(ui, text, &read_marks::Shown { check, marks: &block.marks, taken, lengths: &row.mark_lengths }, theme);
                if let Some(word) = right {
                    asked = Some(ActionEvent::key(qnc_script::MARK_AT).with_target(format!("{}:{word}", block.id)).at(PANEL));
                }
                return (asked, clicked);
            }
            let shown = if text.is_empty() { qnc_ui_text::text("script.no_words").to_string() } else { text.to_string() };
            let color = if text.is_empty() { theme.text_muted } else { theme.text };
            let clicked = ui.add(egui::Label::new(RichText::new(shown).color(color).size(theme.font_ui + 1.0)).wrap().sense(Sense::click())).clicked();
            (asked, clicked)
            };
            // Timecode, the text, and on the right the smooth cut and pause of the join before the
            // part (user 2026-11-08).
            let open_part = view.subsegment.as_ref().filter(|open| open.block_id == block.id && index > 0).and_then(|open| Some((open, open.parts.get(index)?)));
            let answer = ui
                .horizontal_top(|ui| {
                    let height = ui.spacing().interact_size.y;
                    let middle = (ui.available_width() - TC_COLUMN - JOIN_COLUMN - 2.0 * ui.spacing().item_spacing.x).max(120.0);
                    ui.allocate_ui(egui::vec2(TC_COLUMN, height), |ui| {
                        ui.set_width(TC_COLUMN);
                        ui.add(egui::Label::new(RichText::new(detail).color(theme.text_muted).size(theme.font_ui)).extend());
                    });
                    let text = ui.allocate_ui(egui::vec2(middle, height), |ui| {
                        ui.set_width(middle);
                        paint_text(ui)
                    });
                    let join = ui.allocate_ui(egui::vec2(JOIN_COLUMN, height), |ui| {
                        ui.set_width(JOIN_COLUMN);
                        open_part.and_then(|(open, part)| part_controls::join_cell(ui, open, index, part, theme))
                    });
                    (join.inner.or(text.inner.0), text.inner.1)
                })
                .inner;
            ui.separator();
            if answer.0.is_some() {
                asked = answer.0;
            }
            picked |= answer.1;
        }
        if let Some(event) = paint_fields(ui, row, view, theme) {
            asked = Some(event);
        }
    });
    // The mark and the text choose the block (the whole frame would take the clicks of its
    // buttons and fields).
    if picked && asked.is_none() {
        asked = Some(ActionEvent::key(SELECT).with_target(id).at(PANEL));
    }
    asked
}

/// The fields of a block's kind (the caption of a statement, the template and lines of a
/// graphic), one row each: typed text goes into the script when the field is left or Enter is
/// pressed, a choice at once.
fn paint_fields(ui: &mut egui::Ui, row: &qnc_script::BlockRow, view: &ScriptView, theme: &qnc_desktop_theme::Theme) -> Option<ActionEvent> {
    let profile = view.profile.as_ref()?;
    let kind = profile.kind(&row.block.kind)?;
    let mut asked = None;
    let id = row.block.id;
    for field in &kind.fields {
        ui.horizontal(|ui| {
            ui.add_sized([LABEL_WIDTH, theme.chrome_control_height], egui::Label::new(RichText::new(&field.label).color(theme.text_muted).size(theme.font_ui)));
            let stored = view.field_value(id, &field.path);
            let choices = field.options.as_ref().and_then(|set| profile.options.get(set));
            match choices.filter(|_| field.kind == qnc_settings_form::FieldKind::Choice) {
                Some(choices) => {
                    let value = if stored.is_empty() { field.default.clone() } else { stored };
                    let shown = choices.iter().find(|(choice, _)| *choice == value).map_or(value.clone(), |(_, name)| name.clone());
                    egui::ComboBox::from_id_salt(("qnc_script_field", id, &field.path)).selected_text(shown).show_ui(ui, |ui| {
                        for (choice, name) in choices {
                            if ui.selectable_label(*choice == value, name).clicked() {
                                asked = Some(ActionEvent::key(FIELD_SET).with_target(FieldEdit::target(id, &field.path, choice)).at(PANEL));
                            }
                        }
                    });
                }
                None => {
                    let mut text = stored;
                    let edit = ui.add(qnc_ui_kit::text_field(&mut text).hint_text(&field.hint).desired_width(ui.available_width()));
                    if edit.changed() {
                        asked = Some(ActionEvent::key(FIELD_TEXT).with_target(FieldEdit::target(id, &field.path, &text)).at(PANEL));
                    } else if edit.lost_focus() {
                        asked = Some(ActionEvent::key(FIELD_COMMIT).with_target(id.to_string()).at(PANEL));
                    }
                }
            }
        });
    }
    asked
}

/// The width of a field's name in front of it.
const LABEL_WIDTH: f32 = 120.0;
/// The columns of a SubSegment's table: its timecode on the left, its join on the right.
const TC_COLUMN: f32 = 310.0;
const JOIN_COLUMN: f32 = 230.0;

/// The TextSourceEditor at the bottom: text is written here; a kind's button puts it after the
/// chosen block as that kind, Replace puts it back into the block it was taken from.
fn paint_editor(ui: &mut egui::Ui, view: &ScriptView, theme: &qnc_desktop_theme::Theme) -> Option<ActionEvent> {
    let written = !view.editor.trim().is_empty();
    let kinds = view.text_kinds();
    let labels: Vec<String> = kinds.iter().map(|kind| qnc_ui_text::fill("script.add_text", &[("kind", &kind.short)])).collect();
    // Two groups (user 2026-11-01): the text goes into the script as a kind, then the text is
    // corrected (Replace into the block it came from, Clear).
    let mut buttons: Vec<EditorButton> = kinds.iter().zip(&labels).map(|(kind, label)| EditorButton { label, action_id: ADD_TEXT, target: &kind.id, enabled: written, group: 0 }).collect();
    buttons.push(EditorButton { label: qnc_ui_text::text("script.replace"), action_id: REPLACE_TEXT, target: "", enabled: written && view.editing.is_some(), group: 1 });
    buttons.push(EditorButton { label: qnc_ui_text::text("script.clear"), action_id: CLEAR_EDITOR, target: "", enabled: written || view.editing.is_some(), group: 1 });
    let info = qnc_ui_text::fill("script.editor_info", &[("words", &view.editor.split_whitespace().count().to_string()), ("length", &seconds_label(view.editor_seconds))]);
    let title = match view.editing.and_then(|id| view.kind_of(id)) {
        Some(kind) => qnc_ui_text::fill("script.editor_editing", &[("kind", &kind.short)]),
        None => qnc_ui_text::text("script.editor").to_string(),
    };
    let editor = Editor { id: "qnc_script_editor", title: &title, text: &view.editor, info: &info, hint: qnc_ui_text::text("script.editor_hint"), buttons: &buttons };
    match qnc_text_source_editor::show(ui, &editor, theme)? {
        Answer::Text(text) => Some(ActionEvent::key(EDITOR_TEXT).with_target(text).at(PANEL)),
        Answer::Button(action, Some(kind)) => Some(ActionEvent::key(action).with_target(kind).at(PANEL)),
        Answer::Button(action, None) => Some(ActionEvent::key(action).at(PANEL)),
    }
}
