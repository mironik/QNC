mod app;

use std::path::PathBuf;

use eframe::egui;

pub use app::ProjectApp;

use qnc_project_blocks::layout_contract::AppContracts;

pub fn check_contracts_message() -> Result<String, String> {
    let contracts = AppContracts::load_embedded()?;
    qnc_project_application::validate_embedded_contracts()?;
    Ok(format!(
        "qnc-project contracts OK: shell={} layout={} shortcut_preset={} open_hint={}",
        contracts.shell.layout_id,
        contracts.project.layout_id,
        contracts.shortcuts.active_preset,
        contracts.project_open_hint.as_deref().unwrap_or("")
    ))
}

pub fn create_project_app(project_root: impl Into<PathBuf>) -> Result<ProjectApp, String> {
    let contracts = AppContracts::load_embedded()?;
    let component = qnc_project_application::open_project_component(project_root)?;
    let ready_text = contracts.project.left_project_list.ready_text.clone();
    let known_actions = contracts.shortcuts.actions.keys().cloned().collect();
    let session = qnc_project_session::ProjectSession::new(component, ready_text, known_actions);
    Ok(ProjectApp::new(contracts, session))
}

pub fn apply_project_style(ctx: &egui::Context) -> Result<(), String> {
    qnc_project_blocks::apply_style(ctx, &AppContracts::load_embedded()?);
    Ok(())
}
