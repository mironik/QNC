//! The project settings executed as a background process.
//!
//! Uvezi writes the selection to the database and puts the selected clips in the import
//! queue; then it makes sure `qnc-ingest-worker`, a separate application, is running.
//! The worker has no rules of its own. It reads the settings of the active project once,
//! takes the queued clips and does what the settings say for each (link: nothing is
//! copied; proxy or original: a copy, and the poster with it), and ends. Everything the
//! form and the worker need to know about each other lives in the project database:
//! the worker lease (only one runs), whether a player works (the copy waits) and what
//! the worker did last. The form never waits for the worker.

use crate::{run_next, ConfigMediaOpener, ImportQueue, TransportQueue};
use qnc_ingest_store::content::{ImportedCopy, StoredClip};
use qnc_media_metadata::MediaRepresentation;
use qnc_active_project_read::ActiveProjectReader;
use qnc_playback_activity::{Lease, Writer, WORKER, WORKER_FRESH_SECONDS, WORKER_RESULT};
use qnc_ingest_select::selection_config::SelectionConfig;
use qnc_ingest_store::content::ContentTarget;
use qnc_ingest_work_plan::IngestWorkPlan;
use qnc_work_settings::SettingsReader;
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

/// The name of the executable that runs beside the application.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSummary {
    pub imported: usize,
    pub failed: usize,
}

/// What one project needs to import.
struct Project {
    dir: PathBuf,
    queue: TransportQueue,
    opener: ConfigMediaOpener,
}

impl Project {
    fn open(
        root: &Path,
        reader: &SettingsReader,
        plan: &IngestWorkPlan,
        target: &ContentTarget,
        lease: Option<qnc_ingest_store::content::WriteLease>,
    ) -> Result<Self, String> {
        let dir = reader
            .local_workspace_dir(&plan.settings)
            .map_err(|e| e.to_string())?
            .ok_or(qnc_ui_text::text("import.import_needs_local_access_project_folder"))?;
        // Link needs no source; a copy from an unconfigured source fails per clip.
        let sources = SelectionConfig::load(root)
            .map(|config| config.sources)
            .unwrap_or_default();
        Ok(Self {
            dir,
            queue: TransportQueue::start_leased(target.clone(), lease)?,
            opener: ConfigMediaOpener::new(
                sources,
                qnc_playback_activity::playback_pause(target.clone()),
            ),
        })
    }
}

/// The import of one Uvezi: the settings of the active project are read once, then the
/// queued clips are taken one by one until the queue is empty. Nothing happens when
/// another worker already holds the lease of this project.
pub fn run_service(root: &Path) -> Result<ImportSummary, String> {
    let active_project = ActiveProjectReader::from_root(root).map_err(|e| e.to_string())?;
    let snapshot = active_project.read().map_err(|e| e.to_string())?;
    let reader = active_project.settings_reader().clone();
    let target = ContentTarget::for_project(&reader, &snapshot.settings)?;
    let Some(lease) = Lease::claim(target, WORKER, WORKER_FRESH_SECONDS)? else {
        return Ok(ImportSummary {
            imported: 0,
            failed: 0,
        });
    };
    run_import(root, &ImportRun { project_id: &snapshot.settings.project_id, lease: &lease, cancel: lease.lost() })
}

/// The run an import belongs to (external audit 2026-10-17): the project its lease was
/// taken for, the lease, and the cancel of the whole run.
pub struct ImportRun<'a> {
    pub project_id: &'a str,
    pub lease: &'a Lease,
    pub cancel: &'a AtomicBool,
}

impl ImportRun<'_> {
    /// The run may still work and publish: not cancelled and the lease still its own.
    fn allowed(&self) -> bool {
        !self.cancel.load(std::sync::atomic::Ordering::Relaxed) && self.lease.confirm()
    }
}

/// The queue of a run: it takes no clip and publishes no result once the run may no
/// longer (cancelled, or its lease is another's).
struct RunQueue<'q, 'r> {
    queue: &'q mut dyn ImportQueue,
    run: &'q ImportRun<'r>,
}

impl ImportQueue for RunQueue<'_, '_> {
    fn claim_next(&mut self) -> Result<Option<StoredClip>, String> {
        if !self.run.allowed() {
            return Ok(None);
        }
        self.queue.claim_next()
    }

    fn heartbeat(&mut self, clip_id: &str) -> Result<(), String> {
        self.queue.heartbeat(clip_id)
    }

    fn finish_import(
        &mut self,
        clip_id: String,
        media_uri: Option<String>,
        thumbnail_uri: Option<String>,
        copy_of: Option<ImportedCopy>,
        optimized: Option<MediaRepresentation>,
        error: Option<String>,
    ) -> Result<(), String> {
        if !self.run.allowed() {
            return Err(qnc_ui_text::text("import.lease_lost").into());
        }
        self.queue.finish_import(clip_id, media_uri, thumbnail_uri, copy_of, optimized, error)
    }
}

/// The import of one Uvezi under the lease the caller holds (the background application
/// also builds the artifacts under it), for the project the lease was taken for only.
pub fn run_import(root: &Path, run: &ImportRun<'_>) -> Result<ImportSummary, String> {
    let active_project = ActiveProjectReader::from_root(root).map_err(|e| e.to_string())?;
    let snapshot = active_project.read().map_err(|e| e.to_string())?;
    if snapshot.settings.project_id != run.project_id {
        return Ok(ImportSummary { imported: 0, failed: 0 }); // another project is active now
    }
    let reader = active_project.settings_reader().clone();
    let plan = IngestWorkPlan::from_settings(snapshot.settings)?;
    let target = ContentTarget::for_project(&reader, &plan.settings)?;
    let mut summary = ImportSummary {
        imported: 0,
        failed: 0,
    };
    let mut project = Project::open(root, &reader, &plan, &target, Some(run.lease.write_lease()))?;
    let outcome = loop {
        let mut queue = RunQueue { queue: &mut project.queue, run };
        match run_next(&mut queue, &plan, &project.dir, &project.opener, run.cancel) {
            Ok(Some(outcome)) if outcome.result.is_ok() => summary.imported += 1,
            Ok(Some(_)) => summary.failed += 1,
            Ok(None) => break Ok(()),
            Err(error) => break Err(error),
        }
    };
    let result = match &outcome {
        Ok(()) => qnc_ui_text::fill("import.result_done", &[("imported", &summary.imported.to_string()), ("failed", &summary.failed.to_string())]),
        Err(error) => qnc_ui_text::fill("import.result_error", &[("error", error)]),
    };
    if run.lease.confirm() {
        if let Ok(mut writer) = Writer::start(target) {
            let _ = writer.set(WORKER_RESULT, &result);
        }
    }
    outcome.map(|()| summary)
}

/// Makes sure the worker runs (the wake block's; no copy of it here).
pub use qnc_worker_wake::{wake as launch_worker, WORKER_EXECUTABLE};

