//! The blocks of the Project application on the one desktop board (user rule
//! 2026-09-30): the project list in the body of the left column, the settings (with
//! their sub-blocks: template picker, project name, AI switches, location fields with
//! their browsers, template actions, advanced settings, own template) in the right
//! panel, and the delete questions. Blocks only draw and return [`ProjectIntent`]s; the
//! session carries them out.

pub mod layout_contract;
mod list;
pub mod location_browser;
mod project_advanced;
mod settings;
pub mod theme;
mod widgets;

use eframe::egui;
use qnc_project_session::{ProjectIntent, ProjectSession};

use layout_contract::AppContracts;
use theme::Theme;

/// One frame of the Project blocks over the session.
struct Blocks<'a> {
    contracts: &'a AppContracts,
    session: &'a mut ProjectSession,
    intents: Vec<ProjectIntent>,
}

/// Draws Project on the desktop board and returns what its blocks and keys ask for.
pub fn show(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    contracts: &AppContracts,
    session: &mut ProjectSession,
) -> Vec<ProjectIntent> {
    let intents = key_intents(ctx, contracts, session);
    let mut blocks = Blocks { contracts, session, intents };
    blocks.board(ui);
    blocks.delete_questions(ctx);
    blocks.intents
}

/// The catalog keys of the Project scope; none while a dialog, picker or browser is open.
fn key_intents(ctx: &egui::Context, contracts: &AppContracts, session: &ProjectSession) -> Vec<ProjectIntent> {
    if session.keys_held() {
        return Vec::new();
    }
    qnc_keyboard_shortcut::egui_shortcut_events(ctx)
        .iter()
        .filter(|event| {
            contracts
                .shortcuts
                .action_ids_for_event("project", event)
                .contains(&"project_open_selected")
        })
        .map(|_| ProjectIntent::OpenSelectedProject)
        .collect()
}

impl Blocks<'_> {
    /// The one desktop board: the project list in the body of the left column, the
    /// settings in the right panel; monitor, head row and dock are one pixel.
    fn board(&mut self, ui: &mut egui::Ui) {
        let t = Theme::from_contract(&self.contracts.shell.colors);
        let faces = qnc_board::BoardFaces { bg: t.bg, left: t.bg, right: t.bg, divider: t.border };
        let sizes = self.contracts.project.board;
        qnc_board::show(ui, &sizes, &faces, |ui, place, rect| {
            match place {
                qnc_board::Place::Body => self.project_list(ui, rect.width(), rect.height()),
                qnc_board::Place::Right => self.settings_panel(ui, rect.width(), rect.height()),
                _ => {}
            }
            None::<()>
        });
    }

    fn delete_questions(&mut self, ctx: &egui::Context) {
        let t = Theme::from_contract(&self.contracts.shell.colors);
        let style = qnc_confirm_dialog::DialogStyle {
            surface: t.surface,
            border: t.border,
            text: t.text,
            muted: t.muted,
            accent: t.accent,
            font_size: self.contracts.shell.theme_metrics.font_ui,
            button_height: self.contracts.shell.theme_metrics.chrome_control_height,
        };
        if let Some(project) = self.session.delete_candidate.clone() {
            let asked = question("delete_project", "Želite ukloniti projekt?", &project.name);
            match qnc_confirm_dialog::show(ctx, &style, asked) {
                Some(true) => self.intents.push(ProjectIntent::ConfirmDeleteProject),
                Some(false) => self.intents.push(ProjectIntent::CancelDeleteProject),
                None => {}
            }
        }
        if let Some(template) = self.session.delete_template_candidate.clone() {
            let asked = question("delete_template", "Želite ukloniti template?", &template.name);
            match qnc_confirm_dialog::show(ctx, &style, asked) {
                Some(true) => self.intents.push(ProjectIntent::ConfirmDeleteTemplate),
                Some(false) => self.intents.push(ProjectIntent::CancelDeleteTemplate),
                None => {}
            }
        }
    }
}

/// A Project delete question (NE / DA).
fn question<'a>(id: &'a str, title: &'a str, subject: &'a str) -> qnc_confirm_dialog::Question<'a> {
    qnc_confirm_dialog::Question { id, title, subject, no: "NE", yes: "DA" }
}

/// Fonts and colours of the Project contract applied to the window.
pub fn apply_style(ctx: &egui::Context, contracts: &AppContracts) {
    theme::apply_app_fonts(ctx, &contracts.shell);
    theme::apply_visuals(ctx, &contracts.shell);
}
