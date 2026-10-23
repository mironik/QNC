//! The project an Ingest surface works on (user 2026-10-23: "Ingest into blocks"). It
//! reads the active project from the database, loads its work plan and clip catalog off
//! the caller's thread and holds what follows from them: the catalog and Select targets of
//! the project database, the catalog statistics, the project folder and the order of the
//! project's applications. Nothing here is a source of truth: every load reads the
//! database again; a failure leaves nothing of the previous project behind.

use qnc_active_project_read::ActiveProjectReader;
use qnc_ingest_catalog::{CatalogClipRow, CatalogLoader, CatalogSource};
use qnc_ingest_select::SelectTarget;
use qnc_ingest_store::content::{CatalogStats, ContentTarget};
use qnc_ingest_work_plan::IngestWorkPlan;
use qnc_work_settings::SettingsReader;

pub const MODULE_ID: &str = "qnc.module.ingest-project";

/// Where the loading of the project stands, as the form shows it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectStatus {
    pub loading: bool,
    pub ready: bool,
    pub error: Option<String>,
}

/// What a finished load brought.
#[derive(Debug)]
pub struct Loaded {
    /// Another project than the one loaded before: nothing of that one may stay shown.
    pub project_changed: bool,
    /// The clip rows, when the catalog was read (it is not while it did not change).
    pub clips: Option<Vec<CatalogClipRow>>,
    pub source: Option<CatalogSource>,
}

/// How a request to load the project went.
#[derive(Debug, Clone, PartialEq)]
pub enum Start {
    /// Loading; `keep_view`: the shown project stays while it is checked again.
    Started { keep_view: bool },
    /// A check while a load already runs: that load answers it.
    AlreadyLoading,
    Refused(String),
}

#[derive(Debug, Default)]
pub struct IngestProject {
    reader: Option<SettingsReader>,
    active: Option<ActiveProjectReader>,
    loader: CatalogLoader,
    plan: Option<IngestWorkPlan>,
    catalog: Option<ContentTarget>,
    select: Option<SelectTarget>,
    stats: Option<CatalogStats>,
    status: ProjectStatus,
}

impl IngestProject {
    /// The project of the QNC root's active project database; an error is its status.
    pub fn from_root(root: &std::path::Path) -> Self {
        let mut project = Self::default();
        match ActiveProjectReader::from_root(root) {
            Ok(active) => {
                project.reader = Some(active.settings_reader().clone());
                project.active = Some(active);
            }
            Err(error) => _ = project.fail(error.to_string()),
        }
        project
    }

    pub fn status(&self) -> &ProjectStatus {
        &self.status
    }

    pub fn reader(&self) -> Option<&SettingsReader> {
        self.reader.as_ref()
    }

    /// The work plan of the loaded project, only once it is ready.
    pub fn plan(&self) -> Option<&IngestWorkPlan> {
        self.plan.as_ref().filter(|_| self.status.ready)
    }

    pub fn catalog_target(&self) -> Option<&ContentTarget> {
        self.catalog.as_ref()
    }

    pub fn select_target(&self) -> Option<&SelectTarget> {
        self.select.as_ref()
    }

    pub fn is_loading(&self) -> bool {
        self.loader.is_busy()
    }

    /// The folder of the loaded project on this machine (posters read from it).
    pub fn project_folder(&self) -> Option<qnc_media_thumbnail::ProjectFolder> {
        let (reader, plan) = self.reader.as_ref().zip(self.plan.as_ref())?;
        qnc_ingest_catalog::project_folder(reader, plan)
    }

    /// The sources Select can read: those of this computer and, once the project is loaded,
    /// its folder (user 2026-10-23: the card of the Ingest SubClip parts is a disk of its own
    /// in the browser, named without any operating system's drive name).
    pub fn selection_config(&self, root: &std::path::Path) -> Result<qnc_ingest_select::SelectionConfig, Box<dyn std::error::Error + Send + Sync>> {
        match self.reader.as_ref().zip(self.plan()) {
            Some((reader, plan)) => qnc_ingest_select::SelectionConfig::for_project(root, reader, &plan.settings),
            None => qnc_ingest_select::SelectionConfig::load(root),
        }
    }

    /// The order of the project's applications, from its public database view.
    pub fn application_sequence(&self) -> Result<Vec<qnc_application_sequence::SequenceStep>, String> {
        let reader = self.reader.as_ref().ok_or(qnc_ui_text::text("ingest.msg.no_settings_reader"))?;
        let plan = self.plan.as_ref().ok_or(qnc_ui_text::text("ingest.msg.settings_unavailable"))?;
        qnc_application_sequence::read(reader, &plan.settings)
    }

    /// Whether the active project in the database is still the loaded one; `None` when the
    /// database cannot say.
    pub fn is_current(&self) -> Option<bool> {
        let snapshot = self.active.as_ref()?.read().ok()?;
        Some(self.plan.as_ref().map(|plan| plan.settings.project_id.as_str()) == Some(snapshot.project_id.as_str()))
    }

    /// Loads the project again. `retain`: a check of the shown project, which keeps it on
    /// view; `others_busy`: Select or the import writes and the project must not change.
    pub fn start(&mut self, retain: bool, others_busy: bool) -> Start {
        if self.loader.is_busy() || others_busy {
            if retain && self.loader.is_busy() {
                return Start::AlreadyLoading;
            }
            return Start::Refused(qnc_ui_text::text("ingest.msg.settings_reading").into());
        }
        let Some(active) = self.active.clone() else {
            let error = qnc_ui_text::text("ingest.msg.no_settings_reader_configured").to_string();
            self.fail(error.clone());
            return Start::Refused(error);
        };
        let keep_view = retain && self.plan.is_some();
        let workspace = keep_view.then(|| self.plan.as_ref().map(|plan| plan.settings.workspace_db_uri.clone())).flatten();
        let stats = keep_view.then(|| self.stats.clone()).flatten();
        if !keep_view {
            self.status.loading = true;
            self.status.ready = false;
        }
        self.status.error = None;
        if let Err(error) = self.loader.start(active, workspace, stats) {
            self.fail(error);
        }
        Start::Started { keep_view }
    }

    /// The finished load, `None` while it runs; an error is also the status.
    pub fn poll(&mut self) -> Option<Result<Loaded, String>> {
        let result = self.loader.poll()?;
        self.status.loading = false;
        let loaded = match result {
            Ok(loaded) => loaded,
            Err(error) => return Some(Err(self.fail(error))),
        };
        let project_changed = self.plan.as_ref().map(|plan| &plan.settings.project_id) != Some(&loaded.plan.settings.project_id);
        if project_changed {
            self.stats = None;
        }
        self.status.ready = true;
        let select = match self.reader.as_ref().map(|reader| SelectTarget::for_project(reader, &loaded.plan.settings)) {
            Some(Err(error)) => return Some(Err(self.fail(error))),
            select => select.and_then(Result::ok),
        };
        self.select = select;
        self.catalog = Some(loaded.target);
        self.stats = Some(loaded.stats);
        self.plan = Some(loaded.plan);
        Some(Ok(Loaded { project_changed, clips: loaded.clips, source: loaded.source }))
    }

    /// Nothing of the loaded project stays: the error is the status. Returns the error.
    pub fn fail(&mut self, error: String) -> String {
        self.plan = None;
        self.catalog = None;
        self.select = None;
        self.stats = None;
        self.loader.cancel();
        self.status = ProjectStatus { loading: false, ready: false, error: Some(error.clone()) };
        error
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_project_without_an_active_project_reader_refuses_to_load_and_says_why() {
        let mut project = IngestProject::default();
        let Start::Refused(error) = project.start(false, false) else { panic!("refused") };
        assert_eq!(project.status().error.as_deref(), Some(error.as_str()));
        assert!(project.plan().is_none());
        assert!(project.catalog_target().is_none());
    }

    #[test]
    fn a_failed_project_keeps_nothing_and_a_busy_select_refuses_a_change() {
        let mut project = IngestProject::default();
        project.status.ready = true;
        assert!(matches!(project.start(false, true), Start::Refused(_)), "Select writes: the project does not change");
        project.fail("gone".into());
        assert_eq!(project.status(), &ProjectStatus { loading: false, ready: false, error: Some("gone".into()) });
        assert!(project.plan().is_none() && project.select_target().is_none());
    }
}
