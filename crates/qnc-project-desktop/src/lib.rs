mod app;
mod layout_contract;
mod location_browser;
mod project_advanced;
mod project_component;
mod theme;
mod widgets;

use std::path::PathBuf;

use eframe::egui;

pub use app::ProjectApp;

use layout_contract::AppContracts;
use project_component::ProjectComponent;
use qnc_project_store::ProjectStore;

pub fn check_contracts_message() -> Result<String, String> {
    let contracts = AppContracts::load_embedded()?;
    ProjectStore::validate_embedded_contracts()?;
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
    let store = ProjectStore::open(project_root)?;
    Ok(ProjectApp::new(contracts, ProjectComponent::new(store)))
}

pub fn apply_project_style(ctx: &egui::Context) -> Result<(), String> {
    let contracts = AppContracts::load_embedded()?;
    theme::apply_app_fonts(ctx, &contracts.shell);
    theme::apply_visuals(ctx, &contracts.shell);
    Ok(())
}
