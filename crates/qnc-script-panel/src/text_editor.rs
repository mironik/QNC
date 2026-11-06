//! A statement's part edited as text (user 2026-11-06: "maybe the best option is a plain
//! classic text edit: what is deleted in the text is deleted in the sound"). The part's words are
//! the text of an ordinary editor: the user deletes words or letters as in any editor (the
//! keyboard, the mouse, its own select and undo); Enter cuts out of the part's picture and sound
//! what the text no longer says, Esc gives the text back. A click with the text unchanged puts
//! the playhead on the word clicked. What is typed in is not said and is not kept.

use eframe::egui::{self, text::LayoutJob, TextFormat};
use qnc_action_event::ActionEvent;
use qnc_script::{TimedWord, PANEL, TEXT_CUT, WORD_CUE};

/// The catalog's keys of the editor: Enter applies, Esc gives the text back.
const SCOPE: &str = "script_text";
const APPLY: &str = "script_text_cut";
const REVERT: &str = "script_text_revert";

fn catalog() -> Option<&'static qnc_keyboard_shortcut::ShortcutCatalog> {
    static CATALOG: std::sync::OnceLock<Option<qnc_keyboard_shortcut::ShortcutCatalog>> = std::sync::OnceLock::new();
    CATALOG.get_or_init(|| qnc_keyboard_shortcut::ShortcutCatalog::from_json_str(include_str!("../../../contracts/qnc-keyboard-shortcuts.json")).ok()).as_ref()
}

/// The text as said and as edited (kept while the part's words stay the same).
#[derive(Clone, Default)]
struct Draft {
    said: String,
    edited: String,
}

/// The editor of part `index` of block `id`; `heard` the clip frame under the playhead (its word
/// lit while the text is as said), `chosen` the part is the open SubSegment's chosen slot and
/// `bring` brings it into sight once.
pub(crate) fn show(
    ui: &mut egui::Ui,
    theme: &qnc_desktop_theme::Theme,
    (id, index): (i64, usize),
    words: &[TimedWord],
    heard: Option<u64>,
    (chosen, bring): (bool, bool),
) -> Option<ActionEvent> {
    let said = qnc_script::text_of(words);
    let key = egui::Id::new(("qnc_script_text", id, index));
    let mut draft = ui.ctx().data(|data| data.get_temp::<Draft>(key)).filter(|draft| draft.said == said).unwrap_or_else(|| Draft { said: said.clone(), edited: said.clone() });
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
    let as_said = draft.edited == said;
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
    let output = qnc_ui_kit::text_area(&mut draft.edited)
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
        draft.edited = said.clone();
    } else if apply && draft.edited != said {
        asked = Some(ActionEvent::key(TEXT_CUT).with_target(format!("{id}:{index}:{}", draft.edited)).at(PANEL));
    } else if output.response.clicked() && draft.edited == said {
        // A click on a word, the text as said: the playhead on it.
        let cursor = output.cursor_range.map(|range| range.primary.ccursor.index);
        let word = cursor.and_then(|at| places.iter().position(|(from, to)| (*from..=*to).contains(&at)));
        if let Some(word) = word.and_then(|word| words.get(word)) {
            asked = Some(ActionEvent::key(WORD_CUE).with_target(format!("{id}:{index}:{}:{}", word.from, word.to)).at(PANEL));
        }
    }
    if draft.edited != said {
        ui.label(egui::RichText::new(qnc_ui_text::text("script.text_edit_hint")).color(theme.text_muted).size(theme.font_ui - 1.0));
    } else if output.response.hovered() {
        output.response.on_hover_text(qnc_ui_text::text("script.word_hint"));
    }
    ui.ctx().data_mut(|data| data.insert_temp(key, draft));
    asked
}
