//! The import bar of the source timeline's header (user 2026-10-20, rule 11: it was drawn
//! by the Ingest form). The status of the import, the New | All switch of the pool, the
//! import, select-all and clear buttons, the editing buttons a layout may show off, and the
//! archive and AI ticks. It paints what the form gives and returns what the user asked; it
//! knows no catalog, no Select and no database.

use eframe::egui::{Align, Color32, Layout, RichText, Ui};
use qnc_source_timeline::HeaderAction;

pub const MODULE_ID: &str = "qnc.module.ingest-import-bar";

/// What the layout contract says of the bar.
pub struct ImportBarSpec<'a> {
    pub show_import_actions: bool,
    pub show_edit_actions: bool,
    /// New | All: labels, colours and width of the switch.
    pub filter_labels: [&'a str; 2],
    pub filter_colors: [Color32; 2],
    pub filter_width: f32,
    /// The import actions right to left (the strong one first).
    pub actions: Vec<HeaderAction<'a>>,
    pub edit_actions: &'a [String],
    pub archive_label: &'a str,
    pub ai_label: &'a str,
}

/// What the bar shows now (from the form's view).
pub struct ImportBarNow<'a> {
    pub status: &'a str,
    pub status_detail: &'a str,
    /// All (`true`) or New (`false`) is chosen.
    pub filter_all: bool,
    pub focused: Option<&'a str>,
    pub archive_available: bool,
    pub archive: bool,
    pub ai_mining: bool,
}

/// What the user asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportBarAction {
    /// New (`false`) or All (`true`).
    Filter(bool),
    Action(String),
    Archive(bool),
    AiMining(bool),
}

pub fn show(
    ui: &mut Ui,
    theme: &qnc_desktop_theme::Theme,
    spec: &ImportBarSpec<'_>,
    now: &ImportBarNow<'_>,
    enabled: impl Fn(&str) -> bool,
) -> Option<ImportBarAction> {
    let mut asked = None;
    ui.add_space(10.0);
    if spec.show_import_actions && !now.status.is_empty() {
        ui.label(RichText::new(now.status).color(theme.text_muted)).on_hover_text(now.status_detail);
    }
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if spec.show_import_actions {
            let mut style = qnc_theme_styles::action_bar(theme);
            style.button_width = spec.filter_width / 2.0;
            if let Some(index) = qnc_ui_kit::show_two_way_switch(ui, spec.filter_labels, usize::from(now.filter_all), spec.filter_colors, &style) {
                asked = Some(ImportBarAction::Filter(index == 1));
            }
            let actions = spec.actions.iter().map(|action| HeaderAction { label: action.label, action_id: action.action_id, strong: action.strong });
            if let Some(action) = qnc_source_timeline::header_actions(ui, theme, actions, &enabled, |_| false, now.focused) {
                asked = Some(ImportBarAction::Action(action.to_string()));
            }
        }
        if spec.show_edit_actions {
            for label in spec.edit_actions {
                let _ = qnc_ui_kit::chrome_button(ui, label, false, false, qnc_theme_styles::button(theme));
            }
        }
        let tick = |text: &str| RichText::new(text).color(theme.text).size(theme.font_ui);
        if now.archive_available {
            let mut archive = now.archive;
            if ui.checkbox(&mut archive, tick(spec.archive_label)).changed() {
                asked = Some(ImportBarAction::Archive(archive));
            }
        }
        let mut ai = now.ai_mining;
        if ui.add_enabled(false, eframe::egui::Checkbox::new(&mut ai, tick(spec.ai_label))).changed() {
            asked = Some(ImportBarAction::AiMining(ai));
        }
    });
    asked
}
