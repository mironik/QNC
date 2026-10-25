//! Project application component. Owns the Project store; the desktop form
//! only shows state and sends intents through this crate.

mod project_component;

use std::path::PathBuf;

pub use project_component::{ProjectBrowserTarget, ProjectComponent};
pub use qnc_dir_browse_jobs::BrowseStep;
pub use qnc_project_jobs::{ProjectsState, Reply, Request, TemplatesState};
pub use qnc_project_store::{ProjectNavigationStep, ProjectRow, ProjectTemplateRow};

use qnc_project_store::ProjectStore;

pub fn validate_embedded_contracts() -> Result<(), String> {
    ProjectStore::validate_embedded_contracts()
}

pub fn open_project_component(
    project_root: impl Into<PathBuf>,
) -> Result<ProjectComponent, String> {
    // The store opens in the background worker; a failure is the reply of every request.
    ProjectStore::validate_embedded_contracts()?;
    Ok(ProjectComponent::new(&project_root.into()))
}

/// Footer/status label for the selected project row.
pub fn selected_project_label(projects: &[ProjectRow], selected: Option<usize>) -> &str {
    selected
        .and_then(|index| projects.get(index))
        .map(|project| project.name.as_str())
        .unwrap_or(qnc_ui_text::text("project.msg.no_project_chosen"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_follows_selection_not_active_flag_and_never_keeps_removed_name() {
        let mut rows = vec![
            ProjectRow {
                project_id: "p1".into(),
                name: "First project".into(),
                created_date: String::new(),
                project_uri: "qnc://local/project/p1".into(),
                active: true,
            },
            ProjectRow {
                project_id: "p2".into(),
                name: "Selected project".into(),
                created_date: String::new(),
                project_uri: "qnc://local/project/p2".into(),
                active: false,
            },
        ];
        assert_eq!(selected_project_label(&rows, Some(0)), "First project");
        assert_eq!(selected_project_label(&rows, Some(1)), "Selected project");
        assert_eq!(selected_project_label(&rows, None), qnc_ui_text::text("project.msg.no_project_chosen"));
        rows.clear();
        assert_eq!(
            selected_project_label(&rows, Some(1)),
            qnc_ui_text::text("project.msg.no_project_chosen")
        );
    }
}
