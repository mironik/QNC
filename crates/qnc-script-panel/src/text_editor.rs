//! A statement's part edited as text (user 2026-11-06: "maybe the best option is a plain
//! classic text edit: what is deleted in the text is deleted in the sound"). The part's words are
//! the text of an ordinary editor: the user deletes words or letters as in any editor (the
//! keyboard, the mouse, its own select and undo); Enter cuts out of the part's picture and sound
//! what the text no longer says, Esc gives the text back. A click with the text unchanged puts
//! the playhead on the word clicked. What is typed in is not said and is not kept.

use eframe::egui::{self, text::LayoutJob, TextFormat};
use qnc_action_event::ActionEvent;
use qnc_script::{TimedWord, PANEL, TEXT_CUT, TEXT_DRAFT, TEXT_REVERT, WORD_CUE};

/// The catalog's keys of the editor: Enter applies, Esc gives the text back.
const SCOPE: &str = "script_text";
const APPLY: &str = TEXT_CUT;
const REVERT: &str = TEXT_REVERT;

fn catalog() -> Option<&'static qnc_keyboard_shortcut::ShortcutCatalog> {
    static CATALOG: std::sync::OnceLock<Option<qnc_keyboard_shortcut::ShortcutCatalog>> = std::sync::OnceLock::new();
    CATALOG.get_or_init(|| qnc_keyboard_shortcut::ShortcutCatalog::from_json_str(include_str!("../../../contracts/qnc-keyboard-shortcuts.json")).ok()).as_ref()
}

/// The editor of part `index` of block `id`; `drafted` the text as the user edits it, held by
/// the script (docs/99 step 4: not egui memory); `heard` the clip frame under the playhead (its word
/// lit while the text is as said), `chosen` the part is the open SubSegment's chosen slot and
/// `bring` brings it into sight once.
pub(crate) fn show(
    ui: &mut egui::Ui,
    theme: &qnc_desktop_theme::Theme,
    place: &'static str,
    (id, index): (i64, usize),
    words: &[TimedWord],
    drafted: Option<&str>,
    heard: Option<u64>,
    (chosen, bring): (bool, bool),
) -> Option<ActionEvent> {
    let said = qnc_script::text_of(words);
    // The same statement is drawn in the script and over it in the media window: each editor
    // its own id, or the one behind takes the clicks and the keyboard of the one in front.
    let key = egui::Id::new(("qnc_script_text", place, id, index));
    let mut edited = drafted.unwrap_or(&said).to_string();
    // The words' places in the text, for the word under the playhead and the one clicked.
    let mut places = Vec::with_capacity(words.len());
    let mut at = 0;
    for word in words {
        let length = word.text.chars().count();
        places.push((at, at + length));
        at += length + 1;
    }
    let lit = heard.and_then(|frame| words.iter().position(|word| (word.from..word.to.max(word.from + 1)).contains(&frame)));
    let font = egui::FontId::proportional(theme.font_ui + 1.0);
    let as_said = edited == said;
    let (text_color, focus, surface, bg) = (theme.text, theme.focus, theme.surface_alt, theme.bg);
    let mut layouter = |ui: &egui::Ui, text: &str, width: f32| {
        let mut job = LayoutJob::default();
        job.wrap.max_width = width;
        let lit_place = lit.filter(|_| text == said).and_then(|lit| places.get(lit).copied());
        for (at, c) in text.chars().enumerate() {
            let mut format = TextFormat { font_id: font.clone(), color: text_color, ..Default::default() };
            if lit_place.is_some_and(|(from, to)| (from..to).contains(&at)) {
                (format.color, format.background) = (bg, focus);
            } else if chosen && text == said {
                format.background = surface;
            }
            job.append(&c.to_string(), 0.0, format);
        }
        ui.fonts(|fonts| fonts.layout_job(job))
    };
    // Enter and Esc of the catalog, asked while the editor still has the keyboard.
    let editor_id = key.with("editor");
    let has_focus = ui.ctx().memory(|memory| memory.has_focus(editor_id));
    let pressed = |action: &str| has_focus && catalog().is_some_and(|catalog| qnc_keyboard_shortcut::pressed_while_typing(ui.ctx(), catalog, SCOPE, action));
    let (apply, revert) = (pressed(APPLY), pressed(REVERT));
    let before = edited.clone();
    let output = qnc_ui_kit::text_area(&mut edited)
        .id(editor_id)
        .desired_width(f32::INFINITY)
        .desired_rows(1)
        .return_key(None)
        .frame(!as_said)
        .layouter(&mut layouter)
        .show(ui);
    if bring {
        output.response.scroll_to_me(Some(egui::Align::Center));
    }
    let mut asked = None;
    if revert {
        asked = Some(ActionEvent::key(TEXT_REVERT).with_target(format!("{id}:{index}")).at(PANEL));
    } else if apply && edited != said {
        asked = Some(ActionEvent::key(TEXT_CUT).with_target(format!("{id}:{index}:{}", edited)).at(PANEL));
    } else if output.response.clicked() && edited == said {
        // A click on a word, the text as said: the playhead on it.
        let cursor = output.cursor_range.map(|range| range.primary.ccursor.index);
        let word = cursor.and_then(|at| places.iter().position(|(from, to)| (*from..=*to).contains(&at)));
        if let Some(word) = word.and_then(|word| words.get(word)) {
            asked = Some(ActionEvent::key(WORD_CUE).with_target(format!("{id}:{index}:{}:{}", word.from, word.to)).at(PANEL));
        }
    }
    if edited != said {
        ui.label(egui::RichText::new(qnc_ui_text::text("script.text_edit_hint")).color(theme.text_muted).size(theme.font_ui - 1.0));
    } else if output.response.hovered() {
        output.response.on_hover_text(qnc_ui_text::text("script.word_hint"));
    }
    // What was typed goes to the script, which keeps it until Enter or Esc.
    if asked.is_none() && edited != before {
        asked = Some(ActionEvent::key(TEXT_DRAFT).with_target(format!("{id}:{index}:{edited}")).at(PANEL));
    }
    asked
}
