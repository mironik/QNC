//! The state of the Project application and everything its blocks and keys ask for
//! (user rule 2026-09-30: forms are boards of blocks, the work lives in public pieces).
//! Blocks only draw and return [`ProjectIntent`]s; [`ProjectSession::dispatch`] carries
//! them out through the Project component, the owner of the project registry.

use std::collections::BTreeSet;
use std::path::Path;

use qnc_dir_browser::BrowserState;
use qnc_project_application::{
    ProjectComponent, ProjectRow, ProjectTemplateRow, ProjectsState, Reply,
    Request, StorySheet, TemplatesState,
};
use qnc_newsroom_sheet::{SheetOverlay, SheetSpec};
pub use qnc_newsroom_sheet::Answer as SheetAnswer;
use serde_json::Value;

pub use qnc_location_field::{Answer as LocationAnswer, FieldStep, LocationField, SourceKind as LocationSourceKind};
pub use qnc_project_application::ProjectBrowserTarget as LocationTarget;

pub struct ProjectSession {
    pub component: ProjectComponent,
    pub project_name: String,
    /// Where new projects go and where exports go (piece `qnc-location-field`).
    pub projects_root: LocationField,
    pub export_dir: LocationField,
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
    /// The story sheet over the form (piece `qnc-newsroom-sheet`): of a new project (its name is
    /// typed in it) or of the open one; none while it is closed.
    pub sheet: Option<SheetOverlay>,
    /// The open project and its story sheet as kept in its database.
    pub story_project: Option<String>,
    story_kept: Option<StorySheet>,
    /// What the project list is searched by (name, date, story sheet values).
    pub project_search: String,
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
    /// Opens the story sheet of a new project (its name is typed there).
    CreateProject,
    /// Opens the story sheet of the open project.
    OpenStorySheet,
    /// What the user did with the story sheet: Create or Save, or Cancel.
    AnswerStorySheet {
        answer: SheetAnswer,
    },
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
    /// Opens or closes the browser of a location field.
    PickLocation {
        target: LocationTarget,
    },
    /// What the user asked of the open browser of a location field.
    AnswerLocation {
        target: LocationTarget,
        answer: LocationAnswer,
    },
}

impl ProjectIntent {
    pub fn action_id(&self) -> &'static str {
        match self {
            Self::SelectWorkflowGroup { .. } => "project_workflow_group_select",
            Self::OpenSelectedProject => "project_open_selected",
            Self::OpenProjectRow { .. } => "project_open_row",
            Self::CreateProject => "project_create",
            Self::OpenStorySheet => "project_story_open",
            Self::AnswerStorySheet { answer: SheetAnswer::Confirm } => "project_story_confirm",
            Self::AnswerStorySheet { answer: SheetAnswer::Cancel } => "project_story_cancel",
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
            Self::PickLocation { target } => location_action(*target, "pick"),
            Self::AnswerLocation { target, answer } => location_action(*target, match answer {
                LocationAnswer::Browser(qnc_dir_browser_panel::BrowserAction::Kind(_)) => "browser_select_source",
                LocationAnswer::Browser(_) => "browser_open_path",
                LocationAnswer::Confirm => "browser_confirm",
                LocationAnswer::Cancel => "browser_cancel",
            }),
        }
    }
}

impl ProjectSession {
    /// `ready_text` is the list status of the layout contract; `known_actions` the Project
    /// action ids of the keyboard catalog (an intent outside it is refused).
    pub fn new(component: ProjectComponent, ready_text: String, known_actions: BTreeSet<String>) -> Self {
        // Shown once the registry answers (piece `qnc-project-jobs`).
        let mut session = Self {
            component,
            project_name: String::new(),
            projects_root: LocationField::new(String::new()),
            export_dir: LocationField::new("exports/projekti"),
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
            sheet: None,
            story_project: None,
            story_kept: None,
            project_search: String::new(),
            status: ready_text.clone(),
            ready_text,
            known_actions,
        };
        session.refresh_projects();
        session.refresh_templates();
        session
    }

    /// The rows of the project list its search finds (all without a search).
    pub fn shown_projects(&self) -> Vec<usize> {
        (0..self.projects.len()).filter(|&index| self.projects[index].matches(&self.project_search)).collect()
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
            || self.projects_root.open
            || self.export_dir.open
            || self.template_create_open
            || self.sheet.is_some()
    }

    /// Carries out one intent of a Project block or key.
    pub fn dispatch(&mut self, action: ProjectIntent) {
        let action_id = action.action_id();
        if !self.known_actions.contains(action_id) {
            self.status = qnc_ui_text::fill("project.msg.unknown_action", &[("action", &action_id.to_string())]);
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
                    Ok(()) => qnc_ui_text::text("project.settings_changed").into(),
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
                let confirm = qnc_ui_text::text("project.create_project").to_string();
                self.sheet = Some(SheetOverlay::new(&self.component.root, self.project_name.clone(), true, Value::Null, None, confirm));
            }
            ProjectIntent::OpenStorySheet => self.open_story_sheet(),
            ProjectIntent::AnswerStorySheet { answer: SheetAnswer::Cancel } => self.sheet = None,
            ProjectIntent::AnswerStorySheet { answer: SheetAnswer::Confirm } => self.confirm_story_sheet(),
            ProjectIntent::RequestDeleteProject { index } => {
                self.request_delete_project(index);
            }
            ProjectIntent::CancelDeleteProject => {
                self.delete_candidate = None;
                self.status = qnc_ui_text::text("project.msg.delete_cancelled").to_string();
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
                self.status = qnc_ui_text::text("project.msg.template_delete_cancelled").to_string();
            }
            ProjectIntent::ConfirmDeleteTemplate => {
                if let Some(template) = self.delete_template_candidate.clone() {
                    self.perform_confirm_delete_template(&template);
                }
            }
            ProjectIntent::PickLocation { target } => {
                let (field, other) = self.locations_mut(target);
                let step = field.toggle(other);
                self.location_step(target, step);
            }
            ProjectIntent::AnswerLocation { target, answer } => {
                let step = self.locations_mut(target).0.answer(&answer);
                self.location_step(target, step);
            }
        }
    }

    fn refresh_projects(&mut self) {
        self.component.request(Request::LoadProjects);
    }

    /// The sheet of the open project as kept in it (or the sheet of this computer, empty).
    fn open_story_sheet(&mut self) {
        let Some(project_id) = self.story_project.clone() else { return };
        let name = self.projects.iter().find(|project| project.project_id == project_id).map(|project| project.name.clone()).unwrap_or_default();
        let kept = self.story_kept.clone().unwrap_or_default();
        let sheet = serde_json::from_value::<SheetSpec>(kept.sheet).ok();
        let values = Value::Object(kept.values.into_iter().collect());
        let confirm = qnc_ui_text::text("sheet.save_values").to_string();
        self.sheet = Some(SheetOverlay::new(&self.component.root, name, false, values, sheet, confirm));
    }

    /// Create (a new project with the name and sheet typed) or Save (the open project's sheet).
    /// The overlay stays open until the registry answers.
    fn confirm_story_sheet(&mut self) {
        let Some(overlay) = self.sheet.as_ref() else { return };
        let Some((sheet, values)) = overlay.filled() else { return };
        let (new_project, name) = (overlay.name_editable, overlay.name.trim().to_string());
        let sheet = StorySheet {
            sheet: serde_json::to_value(sheet).unwrap_or(Value::Null),
            values: values.as_object().cloned().unwrap_or_default().into_iter().collect(),
        };
        if !new_project {
            if let Some(project_id) = self.story_project.clone() {
                self.component.request(Request::SaveStory { project_id, sheet });
            }
            return;
        }
        let settings = self.settings_draft_for_save();
        let template_id = self.selected_template_id.clone();
        if let Err(error) = self.component.create_project(&name, &template_id, &settings, sheet) {
            self.sheet_message(db_error(error));
        }
    }

    /// A message in the open sheet, or in the status line when it is closed.
    fn sheet_message(&mut self, message: String) {
        match self.sheet.as_mut() {
            Some(overlay) => overlay.message = message,
            None => self.status = message,
        }
    }

    fn refresh_templates(&mut self) {
        self.component.request(Request::LoadTemplates);
    }

    fn perform_select_template(&mut self, template_id: &str) {
        self.component.request(Request::SelectTemplate { template_id: template_id.into() });
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
        let (name, description) = (self.template_draft_name.clone(), self.template_draft_description.clone());
        let base = self.selected_template_id.clone();
        if let Err(error) = self.component.create_user_template(&name, &description, &base, &settings) {
            self.status = db_error(error);
        }
    }

    /// The replies of the registry that are there (its work runs in the background).
    /// Returns whether one came.
    pub fn poll(&mut self) -> bool {
        let replies = self.component.poll();
        let mut came = !replies.is_empty();
        for reply in replies {
            self.apply_reply(reply);
        }
        for (target, listing) in self.component.poll_browsers() {
            self.apply_browser_listing(target, listing);
            came = true;
        }
        came
    }

    /// Whether the registry is still working on a request.
    pub fn busy(&self) -> bool {
        self.component.busy()
    }

    fn apply_reply(&mut self, reply: Reply) {
        match reply {
            Reply::Projects(Ok(state)) => self.apply_projects_state(state),
            Reply::Templates(Ok(state)) => self.apply_templates_state(state),
            Reply::Created(Ok((project, state))) => {
                self.project_name.clear();
                self.sheet = None;
                self.status = qnc_ui_text::fill("project.msg.opened", &[("name", &project.name)]);
                self.apply_projects_state(state);
            }
            Reply::Opened(Ok(state)) => {
                self.status = self.ready_text.clone();
                self.apply_projects_state(state);
            }
            Reply::Deleted(Ok((project, state))) => {
                self.delete_candidate = None;
                self.status = qnc_ui_text::fill("project.msg.removed", &[("v0", &project.name)]);
                self.apply_projects_state(state);
            }
            Reply::TemplateSelected(Ok(state)) => {
                self.apply_templates_state(state);
                self.picker_open = false;
                self.status = qnc_ui_text::fill("project.msg.template_chosen", &[("name", &self.selected_template_name())]);
            }
            Reply::TemplateCreated(Ok((template, state))) => {
                self.selected_template_id = template.template_id;
                self.template_create_open = false;
                self.template_draft_name.clear();
                self.template_draft_description.clear();
                self.export_preset_draft_name.clear();
                self.apply_templates_state(state);
                self.status = qnc_ui_text::fill("project.msg.template_chosen", &[("name", &self.selected_template_name())]);
            }
            Reply::TemplateDeleted { name, result } => {
                self.delete_template_candidate = None;
                match result {
                    Ok(state) => {
                        self.apply_templates_state(state);
                        self.status = qnc_ui_text::fill("project.msg.template_deleted", &[("v0", &name)]);
                    }
                    Err(error) => self.status = db_error(error),
                }
            }
            Reply::Story { project_id, saved, result } => match result {
                Ok(sheet) if self.story_project.as_deref() == Some(project_id.as_str()) => {
                    self.story_kept = Some(sheet);
                    if saved {
                        self.sheet = None;
                        self.status = qnc_ui_text::text("project.msg.story_saved").to_string();
                        // The list searches the new values.
                        self.refresh_projects();
                    }
                }
                Ok(_) => {}
                Err(error) => self.sheet_message(db_error(error)),
            },
            Reply::Created(Err(error)) => {
                // A project whose sheet was not kept is there all the same: the list shows it.
                self.sheet_message(db_error(error));
                self.refresh_projects();
            }
            Reply::ProjectsRoot(Ok(projects_root)) => {
                self.projects_root.chosen(qnc_dir_browser::display_private_path(Path::new(&projects_root)));
                qnc_settings_path::set_string_path(&mut self.draft_settings, "storage.projects_root", self.projects_root.text.clone());
                self.status = qnc_ui_text::text("project.msg.projects_root_set").to_string();
            }
            Reply::Projects(Err(error))
            | Reply::Templates(Err(error))
            | Reply::Opened(Err(error))
            | Reply::Deleted(Err(error))
            | Reply::TemplateSelected(Err(error))
            | Reply::TemplateCreated(Err(error))
            | Reply::ProjectsRoot(Err(error)) => self.status = db_error(error),
        }
    }

/// The field `target` and the other one (only one browser is open at a time).
    fn locations_mut(&mut self, target: LocationTarget) -> (&mut LocationField, &mut LocationField) {
        match target {
            LocationTarget::ProjectsRoot => (&mut self.projects_root, &mut self.export_dir),
            LocationTarget::ExportDir => (&mut self.export_dir, &mut self.projects_root),
        }
    }

    /// What a location field asks: a browser step runs in the background (its listing comes
    /// with `poll`); a confirmed folder becomes the projects root (through the registry) or
    /// the export folder of the draft.
    fn location_step(&mut self, target: LocationTarget, step: FieldStep) {
        let uri = match step {
            FieldStep::Nothing => return,
            FieldStep::Browse(step) => return self.component.browse(target, step),
            FieldStep::Confirm(uri) => uri,
        };
        let Some(path) = self.component.browser_private_path_for_uri(target, &uri) else {
            let missing = if target == LocationTarget::ProjectsRoot { "project.msg.browser_no_project_path" } else { "project.msg.browser_no_export_path" };
            self.status = qnc_ui_text::text(missing).to_string();
            return;
        };
        let shown = qnc_dir_browser::display_private_path(&path);
        if shown.trim().is_empty() {
            return;
        }
        if target == LocationTarget::ProjectsRoot {
            return self.component.request(Request::SetProjectsRoot { path });
        }
        self.export_dir.chosen(shown);
        qnc_settings_path::set_string_path(&mut self.draft_settings, "export.directory", self.export_dir.text.clone());
        self.status = qnc_ui_text::text("project.msg.export_dir_set").to_string();
    }

    /// The path of a location field was typed: it goes into the draft of the new project.
    pub fn location_typed(&mut self, target: LocationTarget) {
        let (field, _) = self.locations_mut(target);
        field.dirty = true;
        let text = field.text.clone();
        let path = if target == LocationTarget::ProjectsRoot { "storage.projects_root" } else { "export.directory" };
        qnc_settings_path::set_string_path(&mut self.draft_settings, path, text);
    }

    fn apply_browser_listing(&mut self, target: LocationTarget, listing: Result<BrowserState, String>) {
        self.locations_mut(target).0.apply_listing(listing);
    }

    fn perform_open_project(&mut self, index: usize) {
        let Some(project) = self.projects.get(index).cloned() else {
            return;
        };
        self.selected_project = Some(index);
        self.component.request(Request::Open { project_id: project.project_id });
    }

    fn request_delete_project(&mut self, index: usize) {
        let Some(project) = self.projects.get(index).cloned() else {
            return;
        };
        self.selected_project = Some(index);
        self.delete_candidate = Some(project);
        self.status = qnc_ui_text::text("project.msg.confirm_remove").to_string();
    }

    fn perform_confirm_delete_project(&mut self, project: &ProjectRow) {
        self.component.request(Request::Delete { project_id: project.project_id.clone() });
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
        self.component.request(Request::DeleteTemplate { template_id: template.template_id.clone(), name: template.name.clone() });
    }

    fn apply_projects_state(&mut self, state: ProjectsState) {
        self.selected_project = state.selected_project;
        self.projects = state.projects;
        let open = self.projects.iter().find(|project| project.active).map(|project| project.project_id.clone());
        if open != self.story_project {
            self.story_project = open.clone();
            self.story_kept = None;
            if let Some(project_id) = open {
                self.component.request(Request::LoadStory { project_id });
            }
        }
        if !self.projects_root.dirty {
            self.projects_root.text = qnc_dir_browser::display_private_path(Path::new(&state.projects_root));
        }
    }

    fn apply_templates_state(&mut self, state: TemplatesState) {
        self.component
            .applications
            .set_settings(&state.draft_settings);
        self.selected_template_id = state.selected_template_id;
        self.templates = state.templates;
        self.draft_settings = state.draft_settings;
        self.projects_root.dirty = false;
        self.export_dir.dirty = false;
    }

    pub fn settings_draft_for_save(&self) -> Value {
        let mut settings = self.draft_settings.clone();
        qnc_settings_path::set_string_path(
            &mut settings,
            "storage.projects_root",
            self.projects_root.text.clone(),
        );
        if self.export_dir.dirty && !self.export_dir.text.trim().is_empty() {
            qnc_settings_path::set_string_path(
                &mut settings,
                "export.directory",
                self.export_dir.text.clone(),
            );
        }
        settings
    }
}

/// The status line of a registry error.
fn db_error(error: String) -> String {
    qnc_ui_text::fill("project.msg.db_error", &[("error", &error)])
}

/// The catalog action of a location field's `what`, by the field (the ids the form had).
fn location_action(target: LocationTarget, what: &str) -> &'static str {
    match (target, what) {
        (LocationTarget::ProjectsRoot, "pick") => "project_pick_projects_root",
        (LocationTarget::ProjectsRoot, "browser_select_source") => "project_projects_root_browser_select_source",
        (LocationTarget::ProjectsRoot, "browser_open_path") => "project_projects_root_browser_open_path",
        (LocationTarget::ProjectsRoot, "browser_confirm") => "project_projects_root_browser_confirm",
        (LocationTarget::ProjectsRoot, _) => "project_projects_root_browser_cancel",
        (LocationTarget::ExportDir, "pick") => "project_pick_export_dir",
        (LocationTarget::ExportDir, "browser_select_source") => "project_export_dir_browser_select_source",
        (LocationTarget::ExportDir, "browser_open_path") => "project_export_dir_browser_open_path",
        (LocationTarget::ExportDir, "browser_confirm") => "project_export_dir_browser_confirm",
        (LocationTarget::ExportDir, _) => "project_export_dir_browser_cancel",
    }
}
