//! The blocks of the Project application on the one desktop board (user rule
//! 2026-09-30): the project list in the body of the left column, the settings (with
//! their sub-blocks: template picker, project name, AI switches, location fields with
//! their browsers, template actions, advanced settings, own template) in the right
//! panel, and the delete questions. Blocks only draw and return [`ProjectIntent`]s; the
//! session carries them out.

pub mod layout_contract;
mod list;
mod project_advanced;
mod settings;
pub mod theme;
use qnc_settings_rows as widgets;

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
    frame: &mut qnc_board::Frame<'_>,
    contracts: &AppContracts,
    session: &mut ProjectSession,
) -> Vec<ProjectIntent> {
    let actions: Vec<&str> = qnc_keyboard_shortcut::egui_shortcut_events(ctx)
        .iter()
        .flat_map(|event| contracts.shortcuts.action_ids_for_event("project", event))
        .collect();
    // While a field is typed in its keys move between fields; while a dialog, picker or
    // browser is open no Project key works.
    let typing = ctx.wants_keyboard_input();
    let next_field = typing && actions.contains(&"form_next_field");
    let open = !typing && !session.keys_held() && actions.contains(&"project_open_selected");
    let intents = open.then_some(ProjectIntent::OpenSelectedProject).into_iter().collect();
    let mut blocks = Blocks { contracts, session, intents };
    blocks.board(ui, frame);
    blocks.delete_questions(ctx);
    // The story sheet over the board (piece `qnc-newsroom-sheet`): name and story data.
    if let Some(overlay) = blocks.session.sheet.as_mut() {
        if let Some(answer) = qnc_newsroom_sheet::show(ctx, overlay, &contracts.theme, next_field) {
            blocks.intents.push(ProjectIntent::AnswerStorySheet { answer });
        }
    }
    blocks.intents
}

impl Blocks<'_> {
    /// The one desktop board: the project list in the body of the left column, the
    /// settings in the right panel; monitor, head row and dock are one pixel.
    fn board(&mut self, ui: &mut egui::Ui, frame: &mut qnc_board::Frame<'_>) {
        let t = Theme::from_contract(&self.contracts.shell.colors);
        let faces = qnc_board::BoardFaces { bg: t.bg, left: t.bg, right: t.bg, divider: t.border };
        let sizes = self.contracts.project.board;
        let names = qnc_board::BoardNames {
            monitor: "",
            head: "",
            body: "project-list",
            right: "project-settings",
            dock: "",
        };
        qnc_board::show_in_frame(ui, frame, &sizes.standard_layout(&names), |name| faces.named(name), |ui, block, rect| {
            match block {
                "project-list" => self.project_list(ui, rect.width(), rect.height()),
                "project-settings" => self.settings_panel(ui, rect.width(), rect.height()),
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
            let asked = question("delete_project", &self.contracts.project.texts.delete_project_question, &project.name);
            match qnc_confirm_dialog::show(ctx, &style, asked) {
                Some(true) => self.intents.push(ProjectIntent::ConfirmDeleteProject),
                Some(false) => self.intents.push(ProjectIntent::CancelDeleteProject),
                None => {}
            }
        }
        if let Some(template) = self.session.delete_template_candidate.clone() {
            let asked = question("delete_template", &self.contracts.project.texts.delete_template_question, &template.name);
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
    qnc_desktop_theme::apply_settings_look(ctx, &contracts.theme);
}
