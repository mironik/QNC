//! Searching the transcript (user 2026-11-05, MA Script media overlay: "Potraži: and a field
//! for the term"). The field and the transcript are two panels of one window, so what is typed
//! is a choice of this process's view (egui memory), never written anywhere; the transcript
//! panel finds the words, lights them and brings the current find into view. Enter in the
//! field goes to the next find and asks the playhead there.

use eframe::egui::{self, RichText};

/// What the field holds: the term and how many times Enter was pressed on it.
#[derive(Clone, Default, PartialEq)]
struct Asked {
    term: String,
    step: u32,
}

/// What the transcript panel found for the term: the current find and how many there are.
#[derive(Clone, Copy, Default, PartialEq)]
struct Found {
    at: usize,
    count: usize,
}

/// The catalog action and scope of Enter in the field.
const NEXT: &str = "transcript_search_next";
const SCOPE: &str = "transcript_search";

fn catalog() -> Option<&'static qnc_keyboard_shortcut::ShortcutCatalog> {
    static CATALOG: std::sync::OnceLock<Option<qnc_keyboard_shortcut::ShortcutCatalog>> = std::sync::OnceLock::new();
    CATALOG.get_or_init(|| qnc_keyboard_shortcut::ShortcutCatalog::from_json_str(include_str!("../../../contracts/qnc-keyboard-shortcuts.json")).ok()).as_ref()
}

fn asked_id() -> egui::Id {
    egui::Id::new("qnc-transcript-search-asked")
}

fn found_id() -> egui::Id {
    egui::Id::new("qnc-transcript-search-found")
}

fn seen_id() -> egui::Id {
    egui::Id::new("qnc-transcript-search-seen")
}

/// The search field: `Search:`, the term and, when the transcript has it, which find of how
/// many is current.
pub fn search_field(ui: &mut egui::Ui, theme: &qnc_desktop_theme::Theme) {
    let ctx = ui.ctx().clone();
    let mut asked = ctx.data(|data| data.get_temp::<Asked>(asked_id())).unwrap_or_default();
    ui.label(RichText::new(qnc_ui_text::text("transcript.search")).color(theme.text).size(theme.font_ui));
    let field = egui::TextEdit::singleline(&mut asked.term)
        .hint_text(qnc_ui_text::text("transcript.search_hint"))
        .desired_width(260.0)
        .font(egui::FontId::proportional(theme.font_ui));
    // Asked while the field still has the keyboard (it lets it go on Enter).
    let next = catalog().is_some_and(|catalog| qnc_keyboard_shortcut::pressed_while_typing(&ctx, catalog, SCOPE, NEXT));
    let response = ui.add(field);
    if next && (response.lost_focus() || response.has_focus()) {
        asked.step = asked.step.wrapping_add(1);
        response.request_focus();
    }
    if !asked.term.trim().is_empty() {
        let found = ctx.data(|data| data.get_temp::<Found>(found_id())).unwrap_or_default();
        let (text, color) = if found.count == 0 {
            (qnc_ui_text::text("transcript.search_none").to_string(), theme.danger)
        } else {
            (qnc_ui_text::fill("transcript.search_found", &[("at", &(found.at + 1).to_string()), ("count", &found.count.to_string())]), theme.text_muted)
        };
        ui.label(RichText::new(text).color(color).size(theme.font_ui));
    }
    ctx.data_mut(|data| data.insert_temp(asked_id(), asked));
}

/// The finds of the term in `words` (every word of the transcript in order, as (text, from,
/// to)): the words of each find, in order. A term of several words is found where they follow
/// one another; a word matches when it holds its piece of the term, whatever the case and
/// punctuation.
pub(crate) fn finds(term: &str, words: &[(&str, u64, u64)]) -> Vec<Vec<(u64, u64)>> {
    let pieces: Vec<String> = term.split_whitespace().map(plain).filter(|piece| !piece.is_empty()).collect();
    if pieces.is_empty() {
        return Vec::new();
    }
    let said: Vec<String> = words.iter().map(|(text, _, _)| plain(text)).collect();
    (0..words.len().saturating_sub(pieces.len() - 1))
        .filter(|&at| pieces.iter().enumerate().all(|(k, piece)| said[at + k].contains(piece.as_str())))
        .map(|at| words[at..at + pieces.len()].iter().map(|(_, from, to)| (*from, *to)).collect())
        .collect()
}

fn plain(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// The search as the transcript panel sees it this frame.
pub(crate) struct Shown {
    /// The words of every find.
    pub lit: Vec<(u64, u64)>,
    /// The words of the current find.
    pub current: Vec<(u64, u64)>,
    /// Whether the current find is to be brought into view this frame.
    pub bring: bool,
    /// The first frame and the end of the current find's first word when Enter asked it.
    pub cue: Option<(u64, u64)>,
}

/// Finds the term in `words`, keeps the current find and tells the field what was found.
pub(crate) fn shown(ctx: &egui::Context, words: &[(&str, u64, u64)]) -> Shown {
    let asked = ctx.data(|data| data.get_temp::<Asked>(asked_id())).unwrap_or_default();
    let all = finds(&asked.term, words);
    let (seen, mut found) = ctx.data(|data| (data.get_temp::<Asked>(seen_id()), data.get_temp::<Found>(found_id()).unwrap_or_default()));
    let new_term = seen.as_ref().is_none_or(|seen| seen.term != asked.term);
    let stepped = !new_term && seen.as_ref().is_some_and(|seen| seen.step != asked.step);
    found.count = all.len();
    found.at = if new_term || all.is_empty() {
        0
    } else if stepped {
        (found.at + 1) % all.len()
    } else {
        found.at.min(all.len() - 1)
    };
    ctx.data_mut(|data| {
        data.insert_temp(seen_id(), asked.clone());
        data.insert_temp(found_id(), found);
    });
    let current = all.get(found.at).cloned().unwrap_or_default();
    Shown {
        lit: all.iter().flatten().copied().collect(),
        bring: (new_term || stepped) && !current.is_empty(),
        cue: stepped.then(|| current.first().copied()).flatten(),
        current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_term_of_one_or_more_words_is_found_whatever_the_case_and_punctuation() {
        let words = [("Ovaj", 0, 10), ("skandal", 10, 20), ("sa", 20, 30), ("Skandalom,", 30, 40), ("sa", 40, 50)];
        assert_eq!(finds("skandal", &words), vec![vec![(10, 20)], vec![(30, 40)]]);
        assert_eq!(finds("Skandal sa", &words), vec![vec![(10, 20), (20, 30)], vec![(30, 40), (40, 50)]]);
        assert_eq!(finds("  ", &words), Vec::<Vec<(u64, u64)>>::new());
        assert_eq!(finds("nema", &words), Vec::<Vec<(u64, u64)>>::new());
    }
}
