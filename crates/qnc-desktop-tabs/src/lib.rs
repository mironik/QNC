//! The applications the desktop launch bar shows (v5 `workspace.tabs`, user rule
//! 2026-10-01). Without an active project only the first priority group is shown (the
//! project application, chosen by its group, not by its name). With an active project
//! the bar shows only the applications its template chose, in the order of its
//! sequence, read from the project database through the public read-only ports. The
//! database stays the truth: every `read` reads it again; this block keeps nothing.

use std::path::Path;

use qnc_shell_desktop_api::DesktopApplicationRef;

pub const MODULE_ID: &str = "qnc.module.desktop-tabs";

/// The tabs to show and, when the project could not be read, why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tabs {
    pub tab_ids: Vec<String>,
    /// The database names an active project.
    pub project_open: bool,
    pub error: Option<String>,
}

/// The tabs for the applications `available` in this desktop.
pub fn read(root: &Path, available: &[DesktopApplicationRef]) -> Tabs {
    match project_sequence(root) {
        Ok(None) => Tabs { tab_ids: first_group(available), project_open: false, error: None },
        Ok(Some(sequence)) => Tabs { tab_ids: of_sequence(&sequence, available), project_open: true, error: None },
        Err(error) => Tabs { tab_ids: first_group(available), project_open: false, error: Some(error) },
    }
}

/// The application ids of the active project's sequence; `None` without an active
/// project.
fn project_sequence(root: &Path) -> Result<Option<Vec<String>>, String> {
    let reader = qnc_active_project_read::ActiveProjectReader::from_root(root).map_err(|error| error.to_string())?;
    let snapshot = match reader.read() {
        Ok(snapshot) => snapshot,
        Err(error) if error.code == "no_active_project" => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let steps = qnc_application_sequence::read(reader.settings_reader(), &snapshot.settings)?;
    Ok(Some(steps.into_iter().map(|step| step.application_id).collect()))
}

/// The available applications of the first priority group.
pub fn first_group(available: &[DesktopApplicationRef]) -> Vec<String> {
    let Some(first) = available.iter().map(|app| app.priority_group.as_str()).min() else {
        return Vec::new();
    };
    available
        .iter()
        .filter(|app| app.priority_group == first)
        .map(|app| app.tab_id.clone())
        .collect()
}

/// The available applications of the sequence, in its order.
pub fn of_sequence(sequence: &[String], available: &[DesktopApplicationRef]) -> Vec<String> {
    sequence
        .iter()
        .filter_map(|id| available.iter().find(|app| &app.application_id == id))
        .map(|app| app.tab_id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, tab: &str, group: &str) -> DesktopApplicationRef {
        DesktopApplicationRef {
            application_id: id.into(),
            tab_id: tab.into(),
            priority_group: group.into(),
        }
    }

    fn available() -> Vec<DesktopApplicationRef> {
        vec![
            app("qnc.story", "storyboard", "o"),
            app("qnc.project", "project", "a"),
            app("qnc.ingest", "ingest", "b"),
            app("qnc.media-assist-video", "ma_video", "l"),
        ]
    }

    #[test]
    fn without_a_project_only_the_first_group() {
        assert_eq!(first_group(&available()), ["project"]);
        assert!(first_group(&[]).is_empty());
    }

    #[test]
    fn with_a_project_only_its_applications_in_its_order() {
        let sequence = ["qnc.project", "qnc.ingest", "qnc.story", "qnc.not-installed"].map(String::from);
        assert_eq!(of_sequence(&sequence, &available()), ["project", "ingest", "storyboard"]);
    }

    #[test]
    fn a_root_without_a_project_database_shows_the_first_group_and_why() {
        let root = std::env::temp_dir().join(format!("qnc_desktop_tabs_{}", std::process::id()));
        let tabs = read(&root, &available());
        assert_eq!(tabs.tab_ids, ["project"]);
        assert!(tabs.error.is_some());
    }
}
