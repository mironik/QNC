//! The search field of the transcript (user 2026-11-05, MA Script media overlay: "Potraži: and
//! a field for the term"). The term and the finds are the transcript's (`qnc-transcript`,
//! docs/99 step 4): the field sends what is typed and Enter, the transcript panel lights what the
//! transcript found and brings the current find into view once.

use eframe::egui::{self, RichText};
use qnc_action_event::ActionEvent;
use qnc_transcript::{SearchView, PANEL, SEARCH_NEXT, SEARCH_TERM};

/// The catalog scope of Enter in the field.
const SCOPE: &str = "transcript_search";

fn catalog() -> Option<&'static qnc_keyboard_shortcut::ShortcutCatalog> {
    static CATALOG: std::sync::OnceLock<Option<qnc_keyboard_shortcut::ShortcutCatalog>> = std::sync::OnceLock::new();
    CATALOG.get_or_init(|| qnc_keyboard_shortcut::ShortcutCatalog::from_json_str(include_str!("../../../contracts/qnc-keyboard-shortcuts.json")).ok()).as_ref()
}

/// The search field: `Search:`, the term and, when the transcript has it, which find of how
/// many is current. Gives what was typed or Enter.
pub fn search_field(ui: &mut egui::Ui, search: &SearchView, theme: &qnc_desktop_theme::Theme) -> Option<ActionEvent> {
    let ctx = ui.ctx().clone();
    let mut term = search.term.clone();
    ui.label(RichText::new(qnc_ui_text::text("transcript.search")).color(theme.text).size(theme.font_ui));
    let field = qnc_ui_kit::search_field(&mut term, 260.0, qnc_ui_text::text("transcript.search_hint"), egui::FontId::proportional(theme.font_ui));
    // Asked while the field still has the keyboard (it lets it go on Enter).
    let next = catalog().is_some_and(|catalog| qnc_keyboard_shortcut::pressed_while_typing(&ctx, catalog, SCOPE, SEARCH_NEXT));
    let response = ui.add(field);
    let mut asked = None;
    if next && (response.lost_focus() || response.has_focus()) {
        asked = Some(ActionEvent::key(SEARCH_NEXT).at(PANEL));
        response.request_focus();
    } else if term != search.term {
        asked = Some(ActionEvent::key(SEARCH_TERM).with_target(term.clone()).at(PANEL));
    }
    if !search.term.trim().is_empty() {
        let (text, color) = if search.count == 0 {
            (qnc_ui_text::text("transcript.search_none").to_string(), theme.danger)
        } else {
            (qnc_ui_text::fill("transcript.search_found", &[("at", &(search.at + 1).to_string()), ("count", &search.count.to_string())]), theme.text_muted)
        };
        ui.label(RichText::new(text).color(color).size(theme.font_ui));
    }
    asked
}

/// The search as the transcript panel sees it this frame.
pub(crate) struct Shown<'a> {
    /// The words of every find.
    pub lit: &'a [(u64, u64)],
    /// The words of the current find.
    pub current: &'a [(u64, u64)],
    /// Whether the current find is to be brought into view this frame (once per `bring`).
    pub bring: bool,
}

/// What the transcript found, and whether its current find was asked into view since this panel
/// last brought it (scrolling once is this panel's own; nothing else reads it).
pub(crate) fn shown<'a>(ctx: &egui::Context, search: &'a SearchView) -> Shown<'a> {
    let seen = egui::Id::new("qnc-transcript-search-brought");
    let bring = ctx.data(|data| data.get_temp::<u64>(seen)) != Some(search.bring) && !search.current.is_empty();
    ctx.data_mut(|data| data.insert_temp(seen, search.bring));
    Shown { lit: &search.lit, current: &search.current, bring }
}
