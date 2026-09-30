//! The state of the Project application and everything its blocks and keys ask for
//! (user rule 2026-09-30: forms are boards of blocks, the work lives in public pieces).
//! Blocks only draw and return [`ProjectIntent`]s; [`ProjectSession::dispatch`] carries
//! them out through the Project component, the owner of the project registry.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use qnc_dir_browser::BrowserState;
use qnc_project_application::{
    ProjectBrowserTarget, ProjectComponent, ProjectRow, ProjectTemplateRow, ProjectsState,
    TemplatesState,
};
use serde_json::Value;

/// Where a location browser looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LocationSourceKind {
    #[default]
    Local,
    Lan,
    Internet,
}

impl LocationSourceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "Računalo",
            Self::Lan => "LAN",
            Self::Internet => "Internet",
        }
    }
}

pub struct ProjectSession {
    pub component: ProjectComponent,
    pub project_name: String,
    pub projects_root: String,
    pub export_dir: String,
    pub projects_root_dirty: bool,
    pub export_dir_dirty: bool,
    pub projects_root_browser_open: bool,
    pub projects_root_browser: LocationBrowserState,
    pub export_dir_browser_open: bool,
    pub export_dir_browser: LocationBrowserState,
    pub picker_open: bool,
    pub template_create_open: bool,
    pub advanced_open: bool,
    pub selected_project: Option<usize>,
    pub delete_candidate: Option<ProjectRow>,
    pub delete_template_candidate: Option<ProjectTemplateRow>,
    pub projects: Vec<ProjectRow>,
    pub selected_template_id: String,
    pub templates: Vec<ProjectTemplateRow>,
    pub draft_settings: Value,
    pub template_draft_name: String,
    pub template_draft_description: String,
    pub export_preset_draft_name: String,
    pub status: String,
    ready_text: String,
    known_actions: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub enum ProjectIntent {
    SelectWorkflowGroup {
        priority_group: String,
        application_id: Option<String>,
    },
    OpenSelectedProject,
    OpenProjectRow {
        index: usize,
    },
    CreateProject,
    RequestDeleteProject {
        index: usize,
    },
    CancelDeleteProject,
    ConfirmDeleteProject,
    SelectTemplate {
        template_id: String,
    },
    BeginTemplateCreate,
    CancelTemplateCreate,
    SaveUserTemplate,
    RequestDeleteTemplate {
        template_id: String,
    },
    CancelDeleteTemplate,
    ConfirmDeleteTemplate,
    PickProjectsRoot,
    SelectProjectsRootKind {
        kind: LocationSourceKind,
    },
    OpenProjectsRootUri {
        uri: String,
    },
    OpenProjectsRootParent,
    OpenProjectsRootRoots,
    ConfirmProjectsRootBrowser,
    CancelProjectsRootBrowser,
    PickExportDir,
    SelectExportDirKind {
        kind: LocationSourceKind,
    },
    OpenExportDirUri {
        uri: String,
    },
    OpenExportDirParent,
    OpenExportDirRoots,
    ConfirmExportDirBrowser,
    CancelExportDirBrowser,
}

impl ProjectIntent {
    pub fn action_id(&self) -> &'static str {
        match self {
            Self::SelectWorkflowGroup { .. } => "project_workflow_group_select",
            Self::OpenSelectedProject => "project_open_selected",
            Self::OpenProjectRow { .. } => "project_open_row",
            Self::CreateProject => "project_create",
            Self::RequestDeleteProject { .. } => "project_delete_request",
            Self::CancelDeleteProject => "project_delete_cancel",
            Self::ConfirmDeleteProject => "project_delete_confirm",
            Self::SelectTemplate { .. } => "project_template_select",
            Self::BeginTemplateCreate => "project_template_create_begin",
            Self::CancelTemplateCreate => "project_template_create_cancel",
            Self::SaveUserTemplate => "project_template_save",
            Self::RequestDeleteTemplate { .. } => "project_template_delete_request",
            Self::CancelDeleteTemplate => "project_template_delete_cancel",
            Self::ConfirmDeleteTemplate => "project_template_delete_confirm",
            Self::PickProjectsRoot => "project_pick_projects_root",
            Self::SelectProjectsRootKind { .. } => "project_projects_root_browser_select_source",
            Self::OpenProjectsRootUri { .. }
            | Self::OpenProjectsRootParent
            | Self::OpenProjectsRootRoots => "project_projects_root_browser_open_path",
            Self::ConfirmProjectsRootBrowser => "project_projects_root_browser_confirm",
            Self::CancelProjectsRootBrowser => "project_projects_root_browser_cancel",
            Self::PickExportDir => "project_pick_export_dir",
            Self::SelectExportDirKind { .. } => "project_export_dir_browser_select_source",
            Self::OpenExportDirUri { .. }
            | Self::OpenExportDirParent
            | Self::OpenExportDirRoots => "project_export_dir_browser_open_path",
            Self::ConfirmExportDirBrowser => "project_export_dir_browser_confirm",
            Self::CancelExportDirBrowser => "project_export_dir_browser_cancel",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LocationBrowserState {
    pub kind: LocationSourceKind,
    pub browser: BrowserState,
    pub error: Option<String>,
    pub busy: bool,
}

impl Default for LocationBrowserState {
    fn default() -> Self {
        Self {
            kind: LocationSourceKind::Local,
            browser: BrowserState {
                roots: true,
                ..BrowserState::default()
            },
            error: None,
            busy: false,
        }
    }
}

impl ProjectSession {
    /// `ready_text` is the list status of the layout contract; `known_actions` the Project
    /// action ids of the keyboard catalog (an intent outside it is refused).
    pub fn new(component: ProjectComponent, ready_text: String, known_actions: BTreeSet<String>) -> Self {
        let projects_root = component.projects_root_display();
        let mut session = Self {
            component,
            project_name: String::new(),
            projects_root,
            export_dir: "exports/projekti".to_string(),
            projects_root_dirty: false,
            export_dir_dirty: false,
            projects_root_browser_open: false,
            projects_root_browser: LocationBrowserState::default(),
            export_dir_browser_open: false,
            export_dir_browser: LocationBrowserState::default(),
            picker_open: false,
            template_create_open: false,
            advanced_open: false,
            selected_project: None,
            delete_candidate: None,
            delete_template_candidate: None,
            projects: Vec::new(),
            selected_template_id: String::new(),
            templates: Vec::new(),
            draft_settings: Value::Object(Default::default()),
            template_draft_name: String::new(),
            template_draft_description: String::new(),
            export_preset_draft_name: String::new(),
            status: ready_text.clone(),
            ready_text,
            known_actions,
        };
        session.refresh_projects();
        session.refresh_templates();
        session
    }

    pub fn footer_status(&self) -> &str {
        qnc_project_application::selected_project_label(&self.projects, self.selected_project)
    }

    pub fn take_navigation_trigger(&mut self) -> bool {
        self.component.take_navigation_trigger()
    }

    pub fn navigation_sequence(
        &self,
    ) -> Result<Vec<qnc_project_application::ProjectNavigationStep>, String> {
        self.component.navigation_sequence()
    }

    /// A dialog, picker or browser holds the keys.
    pub fn keys_held(&self) -> bool {
        self.delete_candidate.is_some()
            || self.delete_template_candidate.is_some()
            || self.picker_open
            || self.projects_root_browser_open
            || self.export_dir_browser_open
            || self.template_create_open
    }

    /// Carries out one intent of a Project block or key.
    pub fn dispatch(&mut self, action: ProjectIntent) {
        let action_id = action.action_id();
        if !self.known_actions.contains(action_id) {
            self.status = format!("Nepoznata Project akcija: {action_id}");
            return;
        }

        match action {
            ProjectIntent::SelectWorkflowGroup {
                priority_group,
                application_id,
            } => {
                self.status = match self
                    .component
                    .applications
                    .choose(&priority_group, application_id.as_deref())
                {
                    Ok(()) => "Postavke promijenjene.".into(),
                    Err(error) => error,
                };
            }
            ProjectIntent::OpenSelectedProject => {
                if let Some(index) = self.selected_project {
                    self.perform_open_project(index);
                }
            }
            ProjectIntent::OpenProjectRow { index } => {
                self.perform_open_project(index);
            }
            ProjectIntent::CreateProject => {
                self.perform_create_project();
            }
            ProjectIntent::RequestDeleteProject { index } => {
                self.request_delete_project(index);
            }
            ProjectIntent::CancelDeleteProject => {
                self.delete_candidate = None;
                self.status = "Brisanje otkazano.".to_string();
            }
            ProjectIntent::ConfirmDeleteProject => {
                if let Some(project) = self.delete_candidate.clone() {
                    self.perform_confirm_delete_project(&project);
                }
            }
            ProjectIntent::SelectTemplate { template_id } => {
                self.perform_select_template(&template_id);
            }
            ProjectIntent::BeginTemplateCreate => {
                let selected_name = self.selected_template_name();
                self.template_create_open = true;
                if self.template_draft_name.trim().is_empty() && selected_name != "—" {
                    self.template_draft_name = format!("{selected_name} custom");
                }
            }
            ProjectIntent::CancelTemplateCreate => {
                self.template_create_open = false;
                self.template_draft_name.clear();
                self.template_draft_description.clear();
                self.export_preset_draft_name.clear();
            }
            ProjectIntent::SaveUserTemplate => {
                self.perform_save_custom_template();
            }
            ProjectIntent::RequestDeleteTemplate { template_id } => {
                self.request_delete_template(&template_id);
            }
            ProjectIntent::CancelDeleteTemplate => {
                self.delete_template_candidate = None;
                self.status = "Brisanje templatea otkazano.".to_string();
            }
            ProjectIntent::ConfirmDeleteTemplate => {
                if let Some(template) = self.delete_template_candidate.clone() {
                    self.perform_confirm_delete_template(&template);
                }
            }
            ProjectIntent::PickProjectsRoot => {
                self.toggle_projects_root_browser();
            }
            ProjectIntent::SelectProjectsRootKind { kind } => {
                self.select_projects_root_kind(kind);
            }
            ProjectIntent::OpenProjectsRootUri { uri } => {
                self.open_projects_root_uri(&uri);
            }
            ProjectIntent::OpenProjectsRootParent => {
                self.open_projects_root_parent();
            }
            ProjectIntent::OpenProjectsRootRoots => {
                self.open_projects_root_roots();
            }
            ProjectIntent::ConfirmProjectsRootBrowser => {
                self.confirm_projects_root_browser();
            }
            ProjectIntent::CancelProjectsRootBrowser => {
                self.projects_root_browser_open = false;
            }
            ProjectIntent::PickExportDir => {
                self.toggle_export_dir_browser();
            }
            ProjectIntent::SelectExportDirKind { kind } => {
                self.select_export_dir_kind(kind);
            }
            ProjectIntent::OpenExportDirUri { uri } => {
                self.open_export_dir_uri(&uri);
            }
            ProjectIntent::OpenExportDirParent => {
                self.open_export_dir_parent();
            }
            ProjectIntent::OpenExportDirRoots => {
                self.open_export_dir_roots();
            }
            ProjectIntent::ConfirmExportDirBrowser => {
                self.confirm_export_dir_browser();
            }
            ProjectIntent::CancelExportDirBrowser => {
                self.export_dir_browser_open = false;
            }
        }
    }

    fn refresh_projects(&mut self) {
        match self.component.load_projects() {
            Ok(projects) => self.apply_projects_state(projects),
            Err(error) => {
                self.status = format!("DB: {error}");
            }
        }
    }

    fn perform_create_project(&mut self) {
        let settings = self.settings_draft_for_save();
        match self.component.create_project(
            &self.project_name,
            &self.selected_template_id,
            &settings,
        ) {
            Ok(created) => {
                self.project_name.clear();
                self.status = format!("Otvoren projekt: {}", created.project.name);
                self.apply_projects_state(created.projects);
            }
            Err(error) => {
                self.status = format!("DB: {error}");
            }
        }
    }

    fn refresh_templates(&mut self) {
        match self.component.load_templates() {
            Ok(templates) => self.apply_templates_state(templates),
            Err(error) => {
                self.status = format!("DB: {error}");
            }
        }
    }

    fn perform_select_template(&mut self, template_id: &str) {
        match self.component.select_template(template_id) {
            Ok(templates) => {
                self.apply_templates_state(templates);
                self.picker_open = false;
                self.status = format!("Template: {}", self.selected_template_name());
            }
            Err(error) => {
                self.status = format!("DB: {error}");
            }
        }
    }

    pub fn selected_template_name(&self) -> String {
        self.templates
            .iter()
            .find(|template| template.template_id == self.selected_template_id)
            .map(|template| template.name.clone())
            .unwrap_or_else(|| "—".to_string())
    }

    fn perform_save_custom_template(&mut self) {
        let settings = self.settings_draft_for_save();
        match self.component.create_user_template(
            &self.template_draft_name,
            &self.template_draft_description,
            &self.selected_template_id,
            &settings,
        ) {
            Ok(created) => {
                self.selected_template_id = created.template.template_id;
                self.template_create_open = false;
                self.template_draft_name.clear();
                self.template_draft_description.clear();
                self.export_preset_draft_name.clear();
                self.apply_templates_state(created.templates);
                self.status = format!("Template: {}", self.selected_template_name());
            }
            Err(error) => {
                self.status = format!("DB: {error}");
            }
        }
    }

    fn browser_start_path(path: &str) -> Option<PathBuf> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return None;
        }
        let path = PathBuf::from(trimmed);
        if path.is_absolute() {
            Some(path)
        } else {
            None
        }
    }

    fn toggle_projects_root_browser(&mut self) {
        let opened = qnc_ui_kit::toggle_exclusive_panel(
            &mut self.projects_root_browser_open,
            &mut self.export_dir_browser_open,
        );
        if opened && self.projects_root_browser.kind == LocationSourceKind::Local {
            let start = Self::browser_start_path(&self.projects_root);
            self.load_projects_root_browser(start);
        }
    }

    fn select_projects_root_kind(&mut self, kind: LocationSourceKind) {
        self.projects_root_browser.kind = kind;
        if kind == LocationSourceKind::Local
            && self.projects_root_browser.browser.entries.is_empty()
            && self
                .projects_root_browser
                .browser
                .path_label
                .trim()
                .is_empty()
        {
            self.load_projects_root_browser(None);
        }
    }

    fn open_projects_root_uri(&mut self, uri: &str) {
        if self.projects_root_browser.kind == LocationSourceKind::Local {
            self.projects_root_browser.busy = true;
            self.projects_root_browser.error = None;
            match self
                .component
                .open_browser_uri(ProjectBrowserTarget::ProjectsRoot, uri)
            {
                Ok(browser) => self.apply_projects_root_listing(browser),
                Err(error) => self.set_projects_root_browser_error(error),
            }
        }
    }

    fn open_projects_root_parent(&mut self) {
        if self.projects_root_browser.kind == LocationSourceKind::Local {
            self.projects_root_browser.busy = true;
            self.projects_root_browser.error = None;
            match self
                .component
                .open_browser_parent(ProjectBrowserTarget::ProjectsRoot)
            {
                Ok(browser) => self.apply_projects_root_listing(browser),
                Err(error) => self.set_projects_root_browser_error(error),
            }
        }
    }

    fn open_projects_root_roots(&mut self) {
        if self.projects_root_browser.kind == LocationSourceKind::Local {
            self.load_projects_root_browser(None);
        }
    }

    fn confirm_projects_root_browser(&mut self) {
        let Some(uri) = self.projects_root_browser.browser.current_uri.clone() else {
            return;
        };
        let Some(path) = self
            .component
            .browser_private_path_for_uri(ProjectBrowserTarget::ProjectsRoot, &uri)
        else {
            self.status = "Browser URI nema privatni Project path.".to_string();
            return;
        };
        let display_path = qnc_dir_browser::display_private_path(&path);
        if display_path.trim().is_empty() {
            return;
        }
        match self.component.set_projects_root(path.clone()) {
            Ok(projects_root) => {
                self.projects_root =
                    qnc_dir_browser::display_private_path(Path::new(&projects_root));
                self.projects_root_dirty = true;
                self.projects_root_browser_open = false;
                qnc_settings_path::set_string_path(
                    &mut self.draft_settings,
                    "storage.projects_root",
                    self.projects_root.clone(),
                );
                self.status = "Lokacija projekata postavljena.".to_string();
            }
            Err(error) => {
                self.status = format!("DB: {error}");
            }
        }
    }

    fn toggle_export_dir_browser(&mut self) {
        let opened = qnc_ui_kit::toggle_exclusive_panel(
            &mut self.export_dir_browser_open,
            &mut self.projects_root_browser_open,
        );
        if opened && self.export_dir_browser.kind == LocationSourceKind::Local {
            let start = Self::browser_start_path(&self.export_dir);
            self.load_export_dir_browser(start);
        }
    }

    fn select_export_dir_kind(&mut self, kind: LocationSourceKind) {
        self.export_dir_browser.kind = kind;
        if kind == LocationSourceKind::Local
            && self.export_dir_browser.browser.entries.is_empty()
            && self.export_dir_browser.browser.path_label.trim().is_empty()
        {
            self.load_export_dir_browser(None);
        }
    }

    fn open_export_dir_uri(&mut self, uri: &str) {
        if self.export_dir_browser.kind == LocationSourceKind::Local {
            self.export_dir_browser.busy = true;
            self.export_dir_browser.error = None;
            match self
                .component
                .open_browser_uri(ProjectBrowserTarget::ExportDir, uri)
            {
                Ok(browser) => self.apply_export_dir_listing(browser),
                Err(error) => self.set_export_dir_browser_error(error),
            }
        }
    }

    fn open_export_dir_parent(&mut self) {
        if self.export_dir_browser.kind == LocationSourceKind::Local {
            self.export_dir_browser.busy = true;
            self.export_dir_browser.error = None;
            match self
                .component
                .open_browser_parent(ProjectBrowserTarget::ExportDir)
            {
                Ok(browser) => self.apply_export_dir_listing(browser),
                Err(error) => self.set_export_dir_browser_error(error),
            }
        }
    }

    fn open_export_dir_roots(&mut self) {
        if self.export_dir_browser.kind == LocationSourceKind::Local {
            self.load_export_dir_browser(None);
        }
    }

    fn confirm_export_dir_browser(&mut self) {
        let Some(uri) = self.export_dir_browser.browser.current_uri.clone() else {
            return;
        };
        let Some(path) = self
            .component
            .browser_private_path_for_uri(ProjectBrowserTarget::ExportDir, &uri)
        else {
            self.status = "Browser URI nema privatni export path.".to_string();
            return;
        };
        let path = qnc_dir_browser::display_private_path(&path);
        if path.trim().is_empty() {
            return;
        }
        self.export_dir = path;
        self.export_dir_dirty = true;
        self.export_dir_browser_open = false;
        qnc_settings_path::set_string_path(
            &mut self.draft_settings,
            "export.directory",
            self.export_dir.clone(),
        );
        self.status = "Export direktorij postavljen.".to_string();
    }

    fn load_projects_root_browser(&mut self, start: Option<PathBuf>) {
        self.projects_root_browser.busy = true;
        self.projects_root_browser.error = None;
        let result = if let Some(path) = start {
            self.component
                .open_browser_private_path(ProjectBrowserTarget::ProjectsRoot, path)
        } else {
            self.component
                .load_browser_roots(ProjectBrowserTarget::ProjectsRoot)
        };
        match result {
            Ok(browser) => self.apply_projects_root_listing(browser),
            Err(error) => self.set_projects_root_browser_error(error),
        }
    }

    fn load_export_dir_browser(&mut self, start: Option<PathBuf>) {
        self.export_dir_browser.busy = true;
        self.export_dir_browser.error = None;
        let result = if let Some(path) = start {
            self.component
                .open_browser_private_path(ProjectBrowserTarget::ExportDir, path)
        } else {
            self.component
                .load_browser_roots(ProjectBrowserTarget::ExportDir)
        };
        match result {
            Ok(browser) => self.apply_export_dir_listing(browser),
            Err(error) => self.set_export_dir_browser_error(error),
        }
    }

    fn apply_projects_root_listing(&mut self, browser: BrowserState) {
        Self::apply_location_listing(&mut self.projects_root_browser, browser);
    }

    fn apply_export_dir_listing(&mut self, browser: BrowserState) {
        Self::apply_location_listing(&mut self.export_dir_browser, browser);
    }

    fn apply_location_listing(browser_state: &mut LocationBrowserState, browser: BrowserState) {
        browser_state.browser = browser;
        browser_state.error = None;
        browser_state.busy = false;
    }

    fn set_projects_root_browser_error(&mut self, error: String) {
        self.projects_root_browser.error = Some(error);
        self.projects_root_browser.browser.entries.clear();
        self.projects_root_browser.busy = false;
    }

    fn set_export_dir_browser_error(&mut self, error: String) {
        self.export_dir_browser.error = Some(error);
        self.export_dir_browser.browser.entries.clear();
        self.export_dir_browser.busy = false;
    }

    fn perform_open_project(&mut self, index: usize) {
        let Some(project) = self.projects.get(index).cloned() else {
            return;
        };
        match self.component.open_project(&project.project_id) {
            Ok(projects) => {
                self.selected_project = Some(index);
                self.status = self.ready_text.clone();
                self.apply_projects_state(projects);
            }
            Err(error) => {
                self.status = format!("DB: {error}");
            }
        }
    }

    fn request_delete_project(&mut self, index: usize) {
        let Some(project) = self.projects.get(index).cloned() else {
            return;
        };
        self.selected_project = Some(index);
        self.delete_candidate = Some(project);
        self.status = "Potvrdi uklanjanje projekta.".to_string();
    }

    fn perform_confirm_delete_project(&mut self, project: &ProjectRow) {
        match self.component.delete_project(&project.project_id) {
            Ok(deleted) => {
                self.delete_candidate = None;
                self.status = format!("Uklonjen projekt: {}", deleted.project.name);
                self.apply_projects_state(deleted.projects);
            }
            Err(error) => {
                self.status = format!("DB: {error}");
            }
        }
    }

    fn request_delete_template(&mut self, template_id: &str) {
        if let Some(template) = self
            .templates
            .iter()
            .find(|template| template.template_id == template_id)
            .cloned()
        {
            self.delete_template_candidate = Some(template);
        }
    }

    fn perform_confirm_delete_template(&mut self, template: &ProjectTemplateRow) {
        match self.component.delete_user_template(&template.template_id) {
            Ok(templates) => {
                self.delete_template_candidate = None;
                self.apply_templates_state(templates);
                self.status = format!("Template obrisan: {}", template.name);
            }
            Err(error) => {
                self.delete_template_candidate = None;
                self.status = format!("DB: {error}");
            }
        }
    }

    fn apply_projects_state(&mut self, state: ProjectsState) {
        self.selected_project = state.selected_project;
        self.projects = state.projects;
    }

    fn apply_templates_state(&mut self, state: TemplatesState) {
        self.component
            .applications
            .set_settings(&state.draft_settings);
        self.selected_template_id = state.selected_template_id;
        self.templates = state.templates;
        self.draft_settings = state.draft_settings;
        self.projects_root_dirty = false;
        self.export_dir_dirty = false;
    }

    pub fn settings_draft_for_save(&self) -> Value {
        let mut settings = self.draft_settings.clone();
        qnc_settings_path::set_string_path(
            &mut settings,
            "storage.projects_root",
            self.projects_root.clone(),
        );
        if self.export_dir_dirty && !self.export_dir.trim().is_empty() {
            qnc_settings_path::set_string_path(
                &mut settings,
                "export.directory",
                self.export_dir.clone(),
            );
        }
        settings
    }
}
