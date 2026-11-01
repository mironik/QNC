//! The choice of the interface language (user 2026-11-01: "in Project there should be an
//! option to choose the language of the interface"; a setting of this computer, as decided
//! 2026-10-14; it applies when QNC starts again). A settings panel places it as its
//! `ui_language` section: the languages QNC has dictionaries for, the one this computer
//! chose, and a note while that is not the language QNC runs in. It writes only this
//! computer's setting (`qnc-ui-text`), never a project or its database.

use eframe::egui;
use qnc_settings_form::GridStyle;

pub const MODULE_ID: &str = "qnc.module.language-choice";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The name of the section this block draws.
pub const SECTION: &str = "ui_language";

/// Draws the choice when `block` is this block's section; any other section is not its own.
pub fn section(ui: &mut egui::Ui, block: Option<&str>, width: f32, label: &str, style: &GridStyle) {
    if block != Some(SECTION) {
        return;
    }
    let Some(root) = qnc_dev_diagnostics::locate_qnc_root() else { return };
    let chosen = qnc_ui_text::chosen_language(&root);
    if let Some(language) = qnc_settings_form::choice_cell(ui, width, label, "qnc_ui_language", &chosen, &qnc_ui_text::languages(), style) {
        if let Err(error) = qnc_ui_text::choose_language(&root, &language) {
            qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("ui-language refused: {error}"));
        }
    }
    let running = qnc_ui_text::host().map(|texts| texts.language.clone()).unwrap_or_default();
    if qnc_ui_text::chosen_language(&root) != running {
        ui.label(egui::RichText::new(qnc_ui_text::text("project.language_after_restart")).size(style.label_size).color(style.label_color));
    }
}
