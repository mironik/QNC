use std::path::{Path, PathBuf};

use qnc_application_selection::ApplicationSelection;
use qnc_dir_browser::{BrowserState, DirectoryBrowserSession};
use qnc_project_jobs::{Answer, Reply, Request};
use qnc_project_store::{ProjectNavigationStep, ProjectStore};
use qnc_store_jobs::StoreJobs;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectBrowserTarget {
    ProjectsRoot,
    ExportDir,
}

/// The Project component: its work on the registry and folders runs in the background
/// (piece `qnc-project-jobs`, audit 2026-10-25); the form takes the replies with `poll`.
pub struct ProjectComponent {
    pub applications: ApplicationSelection,
    jobs: StoreJobs<Result<ProjectStore, String>, Answer>,
    projects_root_browser: DirectoryBrowserSession,
    export_dir_browser: DirectoryBrowserSession,
    navigation: Option<(String, Result<Vec<ProjectNavigationStep>, String>)>,
    navigation_pending: bool,
}

impl ProjectComponent {
    pub fn new(root: &Path) -> Self {
        Self {
            applications: ApplicationSelection::new(root),
            jobs: qnc_project_jobs::start(root.to_path_buf()),
            projects_root_browser: DirectoryBrowserSession::default(),
            export_dir_browser: DirectoryBrowserSession::default(),
            navigation: None,
            navigation_pending: false,
        }
    }

    /// Sends a request to the background worker.
    pub fn request(&mut self, request: Request) {
        if matches!(request, Request::Create { .. } | Request::Open { .. }) {
            self.navigation_pending = false;
        }
        self.jobs.submit(move |store| qnc_project_jobs::run(store, request));
    }

    /// A new project from the template, with the applications chosen now.
    pub fn create_project(&mut self, name: &str, template_id: &str, settings: &Value) -> Result<(), String> {
        let selection = self.applications.snapshot()?;
        self.request(Request::Create { name: name.into(), template_id: template_id.into(), settings: settings.clone(), selection });
        Ok(())
    }

    /// A user template from the template, with the applications chosen now.
    pub fn create_user_template(&mut self, name: &str, description: &str, base_template_id: &str, settings: &Value) -> Result<(), String> {
        let selection = self.applications.snapshot()?;
        self.request(Request::CreateTemplate {
            name: name.into(),
            description: description.into(),
            base_template_id: base_template_id.into(),
            settings: settings.clone(),
            selection,
        });
        Ok(())
    }

    /// The replies that are there; an opened or created project arms the desktop's move on.
    pub fn poll(&mut self) -> Vec<Reply> {
        let answers = self.jobs.poll();
        let mut replies = Vec::with_capacity(answers.len());
        for (reply, navigation) in answers {
            if let Some(navigation) = navigation {
                self.navigation = Some(navigation);
                self.navigation_pending = true;
            }
            replies.push(reply);
        }
        replies
    }

    /// Whether a request still waits for its reply.
    pub fn busy(&self) -> bool {
        self.jobs.busy()
    }

    pub fn take_navigation_trigger(&mut self) -> bool {
        std::mem::take(&mut self.navigation_pending)
    }

    /// The applications of the project last opened or created, read with it in the worker.
    pub fn navigation_sequence(&self) -> Result<Vec<ProjectNavigationStep>, String> {
        match &self.navigation {
            Some((_, sequence)) => sequence.clone(),
            None => Err(qnc_ui_text::text("project.msg.no_open_project").to_string()),
        }
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
