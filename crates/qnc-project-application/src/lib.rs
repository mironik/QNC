//! Project application component. Owns the Project store; the desktop form
//! only shows state and sends intents through this crate.

mod project_component;

use std::path::PathBuf;

pub use project_component::{
    ProjectBrowserTarget, ProjectComponent, ProjectCreated, ProjectDeleted, ProjectsState,
    TemplatesState, UserTemplateCreated,
};
pub use qnc_project_store::{ProjectNavigationStep, ProjectRow, ProjectTemplateRow};

use qnc_project_store::ProjectStore;

pub fn validate_embedded_contracts() -> Result<(), String> {
    ProjectStore::validate_embedded_contracts()
}

pub fn open_project_component(project_root: impl Into<PathBuf>) -> Result<ProjectComponent, String> {
    let root = project_root.into();
    let store = ProjectStore::open(&root)?;
    Ok(ProjectComponent::new(store, &root))
}
