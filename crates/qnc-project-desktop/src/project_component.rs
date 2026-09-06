use std::path::{Path, PathBuf};

use crate::application_selection::ApplicationSelection;
use qnc_dir_browser::{BrowserState, DirectoryBrowserSession};
use serde_json::Value;

use qnc_project_store::{ProjectRow, ProjectStore, ProjectTemplateRow};

#[derive(Debug, Clone)]
pub struct ProjectsState {
    pub projects: Vec<ProjectRow>,
    pub selected_project: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct TemplatesState {
    pub templates: Vec<ProjectTemplateRow>,
    pub selected_template_id: String,
    pub draft_settings: Value,
}

#[derive(Debug, Clone)]
pub struct ProjectCreated {
    pub project: ProjectRow,
    pub projects: ProjectsState,
}

#[derive(Debug, Clone)]
pub struct ProjectDeleted {
    pub project: ProjectRow,
    pub projects: ProjectsState,
}

#[derive(Debug, Clone)]
pub struct UserTemplateCreated {
    pub template: ProjectTemplateRow,
    pub templates: TemplatesState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectBrowserTarget {
    ProjectsRoot,
    ExportDir,
}

pub struct ProjectComponent {
    pub applications: ApplicationSelection,
    store: ProjectStore,
    projects_root_browser: DirectoryBrowserSession,
    export_dir_browser: DirectoryBrowserSession,
    navigation_project_id: Option<String>,
    navigation_pending: bool,
}

impl ProjectComponent {
    pub fn new(store: ProjectStore, root: &Path) -> Self {
        Self {
            applications: ApplicationSelection::new(root),
            store,
            projects_root_browser: DirectoryBrowserSession::default(),
            export_dir_browser: DirectoryBrowserSession::default(),
            navigation_project_id: None,
            navigation_pending: false,
        }
    }

    pub fn projects_root_display(&self) -> String {
        self.store.projects_root_display()
    }

    pub fn load_projects(&self) -> Result<ProjectsState, String> {
        let projects = self.store.list_projects()?;
        let selected_project = projects.iter().position(|project| project.active);
        Ok(ProjectsState {
            projects,
            selected_project,
        })
    }

    pub fn load_templates(&self) -> Result<TemplatesState, String> {
        let templates = self.store.list_project_templates()?;
        let selected_template_id = templates
            .iter()
            .find(|template| template.selected)
            .map(|template| template.template_id.clone())
            .or_else(|| {
                templates
                    .first()
                    .map(|template| template.template_id.clone())
            })
            .unwrap_or_default();
        let draft_settings = if selected_template_id.trim().is_empty() {
            Value::Object(Default::default())
        } else {
            self.store.selected_template_settings()?.settings
        };
        Ok(TemplatesState {
            templates,
            selected_template_id,
            draft_settings,
        })
    }

    pub fn create_project(
        &mut self,
        name: &str,
        template_id: &str,
        settings: &Value,
    ) -> Result<ProjectCreated, String> {
        self.navigation_pending = false;
        let project = self.store.create_project(
            name,
            template_id,
            Some(settings),
            &self.applications.snapshot()?,
        )?;
        let projects = self.load_projects()?;
        self.navigation_project_id = Some(project.project_id.clone());
        self.navigation_pending = true;
        Ok(ProjectCreated { project, projects })
    }

    pub fn open_project(&mut self, project_id: &str) -> Result<ProjectsState, String> {
        self.navigation_pending = false;
        self.store.open_project(project_id)?;
        let projects = self.load_projects()?;
        self.navigation_project_id = Some(project_id.to_string());
        self.navigation_pending = true;
        Ok(projects)
    }

    pub fn take_navigation_trigger(&mut self) -> bool {
        std::mem::take(&mut self.navigation_pending)
    }

    pub fn navigation_sequence(
        &self,
    ) -> Result<Vec<qnc_project_store::ProjectNavigationStep>, String> {
        let id = self
            .navigation_project_id
            .as_deref()
            .ok_or("Nema otvorenog projekta za navigaciju.")?;
        self.store.navigation_sequence(id)
    }

    pub fn delete_project(&self, project_id: &str) -> Result<ProjectDeleted, String> {
        let project = self.store.delete_project(project_id)?;
        let projects = self.load_projects()?;
        Ok(ProjectDeleted { project, projects })
    }

    pub fn select_template(&self, template_id: &str) -> Result<TemplatesState, String> {
        self.store.set_selected_template(template_id)?;
        self.load_templates()
    }

    pub fn create_user_template(
        &self,
        name: &str,
        description: &str,
        base_template_id: &str,
        settings: &Value,
    ) -> Result<UserTemplateCreated, String> {
        let template = self.store.create_user_template(
            name,
            description,
            base_template_id,
            Some(settings),
            &self.applications.snapshot()?,
        )?;
        let templates = self.load_templates()?;
        Ok(UserTemplateCreated {
            template,
            templates,
        })
    }

    pub fn delete_user_template(&self, template_id: &str) -> Result<TemplatesState, String> {
        self.store.delete_user_template(template_id)?;
        self.load_templates()
    }

    pub fn set_projects_root(&mut self, path: PathBuf) -> Result<String, String> {
        self.store.set_projects_root(path)?;
        Ok(self.store.projects_root_display())
    }

    pub fn load_browser_roots(
        &mut self,
        target: ProjectBrowserTarget,
    ) -> Result<BrowserState, String> {
        self.browser_session_mut(target).load_roots()
    }

    pub fn open_browser_private_path(
        &mut self,
        target: ProjectBrowserTarget,
        path: impl AsRef<Path>,
    ) -> Result<BrowserState, String> {
        self.browser_session_mut(target).open_private_path(path)
    }

    pub fn open_browser_uri(
        &mut self,
        target: ProjectBrowserTarget,
        uri: &str,
    ) -> Result<BrowserState, String> {
        self.browser_session_mut(target).open_uri(uri)
    }

    pub fn open_browser_parent(
        &mut self,
        target: ProjectBrowserTarget,
    ) -> Result<BrowserState, String> {
        self.browser_session_mut(target).open_parent()
    }

    pub fn browser_private_path_for_uri(
        &self,
        target: ProjectBrowserTarget,
        uri: &str,
    ) -> Option<PathBuf> {
        self.browser_session(target).path_for_uri(uri)
    }

    fn browser_session_mut(
        &mut self,
        target: ProjectBrowserTarget,
    ) -> &mut DirectoryBrowserSession {
        match target {
            ProjectBrowserTarget::ProjectsRoot => &mut self.projects_root_browser,
            ProjectBrowserTarget::ExportDir => &mut self.export_dir_browser,
        }
    }

    fn browser_session(&self, target: ProjectBrowserTarget) -> &DirectoryBrowserSession {
        match target {
            ProjectBrowserTarget::ProjectsRoot => &self.projects_root_browser,
            ProjectBrowserTarget::ExportDir => &self.export_dir_browser,
        }
    }
}
