//! The work of the Project form on the project registry and folders (audit 2026-10-25: it
//! ran on the form thread, 100-300 ms a time): each request runs in one background worker
//! that owns the Project store (`qnc-store-jobs`), in the order sent, and answers with the
//! state the form shows. Opening or creating a project also reads its application sequence
//! there, for the desktop to move on.

use std::path::PathBuf;

use qnc_application_catalog::SelectionSnapshot;
use qnc_project_store::{ProjectNavigationStep, ProjectRow, ProjectStore, ProjectTemplateRow};
use qnc_store_jobs::StoreJobs;
use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.project-jobs";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
pub struct ProjectsState {
    pub projects: Vec<ProjectRow>,
    pub selected_project: Option<usize>,
    /// Where new projects go, as shown.
    pub projects_root: String,
}

#[derive(Debug, Clone)]
pub struct TemplatesState {
    pub templates: Vec<ProjectTemplateRow>,
    pub selected_template_id: String,
    pub draft_settings: Value,
}

/// What the form asks of the registry.
pub enum Request {
    LoadProjects,
    LoadTemplates,
    Create { name: String, template_id: String, settings: Value, selection: SelectionSnapshot },
    Open { project_id: String },
    Delete { project_id: String },
    SelectTemplate { template_id: String },
    CreateTemplate { name: String, description: String, base_template_id: String, settings: Value, selection: SelectionSnapshot },
    DeleteTemplate { template_id: String, name: String },
    SetProjectsRoot { path: PathBuf },
}

/// What came of a request.
pub enum Reply {
    Projects(Result<ProjectsState, String>),
    Templates(Result<TemplatesState, String>),
    Created(Result<(ProjectRow, ProjectsState), String>),
    Opened(Result<ProjectsState, String>),
    Deleted(Result<(ProjectRow, ProjectsState), String>),
    TemplateSelected(Result<TemplatesState, String>),
    TemplateCreated(Result<(ProjectTemplateRow, TemplatesState), String>),
    TemplateDeleted { name: String, result: Result<TemplatesState, String> },
    ProjectsRoot(Result<String, String>),
}

/// A reply and, for an opened or created project, its id and application sequence.
pub type Answer = (Reply, Option<(String, Result<Vec<ProjectNavigationStep>, String>)>);

/// The worker of the Project store of `root` (opened in the worker).
pub fn start(root: PathBuf) -> StoreJobs<Result<ProjectStore, String>, Answer> {
    StoreJobs::start("qnc-project-jobs", move || ProjectStore::open(&root))
}

/// Runs a request on the store (in the worker).
pub fn run(store: &mut Result<ProjectStore, String>, request: Request) -> Answer {
    let store = match store {
        Ok(store) => store,
        Err(error) => return (fail(request, error.clone()), None),
    };
    match request {
        Request::LoadProjects => (Reply::Projects(projects(store)), None),
        Request::LoadTemplates => (Reply::Templates(templates(store)), None),
        Request::Create { name, template_id, settings, selection } => {
            let created = store
                .create_project(&name, &template_id, Some(&settings), &selection)
                .and_then(|project| Ok((project, projects(store)?)));
            let navigation = created.as_ref().ok().map(|(project, _)| (project.project_id.clone(), store.navigation_sequence(&project.project_id)));
            (Reply::Created(created), navigation)
        }
        Request::Open { project_id } => {
            let opened = store.open_project(&project_id).and_then(|()| projects(store));
            let navigation = opened.is_ok().then(|| (project_id.clone(), store.navigation_sequence(&project_id)));
            (Reply::Opened(opened), navigation)
        }
        Request::Delete { project_id } => {
            (Reply::Deleted(store.delete_project(&project_id).and_then(|project| Ok((project, projects(store)?)))), None)
        }
        Request::SelectTemplate { template_id } => {
            (Reply::TemplateSelected(store.set_selected_template(&template_id).and_then(|()| templates(store))), None)
        }
        Request::CreateTemplate { name, description, base_template_id, settings, selection } => {
            let created = store
                .create_user_template(&name, &description, &base_template_id, Some(&settings), &selection)
                .and_then(|template| Ok((template, templates(store)?)));
            (Reply::TemplateCreated(created), None)
        }
        Request::DeleteTemplate { template_id, name } => {
            (Reply::TemplateDeleted { name, result: store.delete_user_template(&template_id).and_then(|()| templates(store)) }, None)
        }
        Request::SetProjectsRoot { path } => {
            (Reply::ProjectsRoot(store.set_projects_root(path).map(|()| store.projects_root_display())), None)
        }
    }
}

/// The reply of a request when the store could not be opened.
fn fail(request: Request, error: String) -> Reply {
    match request {
        Request::LoadProjects => Reply::Projects(Err(error)),
        Request::LoadTemplates => Reply::Templates(Err(error)),
        Request::Create { .. } => Reply::Created(Err(error)),
        Request::Open { .. } => Reply::Opened(Err(error)),
        Request::Delete { .. } => Reply::Deleted(Err(error)),
        Request::SelectTemplate { .. } => Reply::TemplateSelected(Err(error)),
        Request::CreateTemplate { .. } => Reply::TemplateCreated(Err(error)),
        Request::DeleteTemplate { name, .. } => Reply::TemplateDeleted { name, result: Err(error) },
        Request::SetProjectsRoot { .. } => Reply::ProjectsRoot(Err(error)),
    }
}

fn projects(store: &ProjectStore) -> Result<ProjectsState, String> {
    let projects = store.list_projects()?;
    let selected_project = projects.iter().position(|project| project.active);
    Ok(ProjectsState { projects, selected_project, projects_root: store.projects_root_display() })
}

fn templates(store: &ProjectStore) -> Result<TemplatesState, String> {
    let templates = store.list_project_templates()?;
    let selected_template_id = templates
        .iter()
        .find(|template| template.selected)
        .or_else(|| templates.first())
        .map(|template| template.template_id.clone())
        .unwrap_or_default();
    let draft_settings = if selected_template_id.trim().is_empty() {
        Value::Object(Default::default())
    } else {
        store.selected_template_settings()?.settings
    };
    Ok(TemplatesState { templates, selected_template_id, draft_settings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn the_registry_answers_from_the_worker_in_the_order_asked() {
        let root = std::env::temp_dir().join(format!("qnc_project_jobs_{}", std::process::id()));
        let mut jobs = start(root.clone());
        for request in [Request::LoadProjects, Request::LoadTemplates, Request::Open { project_id: "missing".into() }] {
            assert!(jobs.submit(move |store| run(store, request)));
        }
        let mut answers = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(20);
        while jobs.busy() && Instant::now() < deadline {
            answers.extend(jobs.poll());
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(answers.len(), 3);
        assert!(matches!(&answers[0].0, Reply::Projects(Ok(state)) if state.projects.is_empty()));
        assert!(matches!(&answers[1].0, Reply::Templates(Ok(state)) if !state.templates.is_empty()), "the system templates are seeded");
        assert!(matches!(&answers[2], (Reply::Opened(Err(_)), None)), "a project that is not there opens nothing");
        drop(jobs);
        let _ = std::fs::remove_dir_all(root);
    }
}
