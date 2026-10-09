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

use crate::{run_next, ConfigMediaOpener, TransportQueue};
use qnc_active_project_read::ActiveProjectReader;
use qnc_ingest_runtime::{Beat, Writer, WORKER, WORKER_FRESH_SECONDS, WORKER_RESULT};
use qnc_ingest_select::selection_config::SelectionConfig;
use qnc_ingest_store::content::ContentTarget;
use qnc_ingest_work_plan::IngestWorkPlan;
use qnc_work_settings::SettingsReader;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::AtomicBool,
};

/// The name of the executable that runs beside the application.
pub const WORKER_EXECUTABLE: &str = "qnc-ingest-worker";

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
    ) -> Result<Self, String> {
        let dir = reader
            .local_workspace_dir(&plan.settings)
            .map_err(|e| e.to_string())?
            .ok_or("Uvoz trazi lokalni pristup direktoriju projekta na ovom stroju.")?;
        // Link needs no source; a copy from an unconfigured source fails per clip.
        let sources = SelectionConfig::load(root)
            .map(|config| config.sources)
            .unwrap_or_default();
        Ok(Self {
            dir,
            queue: TransportQueue::start(target.clone())?,
            opener: ConfigMediaOpener::new(
                sources,
                qnc_ingest_runtime::playback_pause(target.clone()),
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
    if qnc_ingest_runtime::is_fresh(&target, WORKER, WORKER_FRESH_SECONDS) {
        return Ok(ImportSummary {
            imported: 0,
            failed: 0,
        });
    }
    let _lease = Beat::start(target, WORKER)?;
    run_import(root)
}

/// The import of one Uvezi without taking the worker lease: for a caller that already
/// holds it for its whole run (the background application also builds the artifacts).
pub fn run_import(root: &Path) -> Result<ImportSummary, String> {
    let active_project = ActiveProjectReader::from_root(root).map_err(|e| e.to_string())?;
    let snapshot = active_project.read().map_err(|e| e.to_string())?;
    let reader = active_project.settings_reader().clone();
    let plan = IngestWorkPlan::from_settings(snapshot.settings)?;
    let target = ContentTarget::for_project(&reader, &plan.settings)?;
    let mut summary = ImportSummary {
        imported: 0,
        failed: 0,
    };
    let mut project = Project::open(root, &reader, &plan, &target)?;
    let cancel = AtomicBool::new(false);
    let outcome = loop {
        match run_next(
            &mut project.queue,
            &plan,
            &project.dir,
            &project.opener,
            &cancel,
        ) {
            Ok(Some(outcome)) if outcome.result.is_ok() => summary.imported += 1,
            Ok(Some(_)) => summary.failed += 1,
            Ok(None) => break Ok(()),
            Err(error) => break Err(error),
        }
    };
    let result = match &outcome {
        Ok(()) => format!("uvezeno {}, neuspjelo {}", summary.imported, summary.failed),
        Err(error) => format!("greska: {error}"),
    };
    if let Ok(mut writer) = Writer::start(target) {
        let _ = writer.set(WORKER_RESULT, &result);
    }
    outcome.map(|()| summary)
}

/// Makes sure the worker runs: nothing happens when its lease in the project database is
/// alive, otherwise the executable beside the running application is started and this
/// returns at once.
pub fn launch_worker(root: &Path, target: &ContentTarget) -> Result<(), String> {
    if qnc_ingest_runtime::is_fresh(target, WORKER, WORKER_FRESH_SECONDS) {
        return Ok(());
    }
    let name = format!("{WORKER_EXECUTABLE}{}", std::env::consts::EXE_SUFFIX);
    let executable = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name(&name);
    if !executable.is_file() {
        return Err(format!(
            "Nedostaje {name} pored aplikacije. Izgradi -p qnc-ingest-worker istim profilom."
        ));
    }
    let mut command = Command::new(executable);
    command
        .arg("--root")
        .arg(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window for a background process.
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Posters for clips whose source has none (a single file without a card picture): made
/// after Select, before Uvezi, so Ingest shows the clip with a picture (user 2026-10-09;
/// v5 shows a coloured card instead). The poster is the start of the clip, written in the
/// project thumbnails folder (never into the source) and recorded through the write
/// transport only for a clip that has no poster yet. Returns how many were made.
pub fn make_missing_posters(root: &Path) -> Result<usize, String> {
    use qnc_ingest_store::content::Access;
    let active_project = ActiveProjectReader::from_root(root).map_err(|e| e.to_string())?;
    let snapshot = active_project.read().map_err(|e| e.to_string())?;
    let reader = active_project.settings_reader().clone();
    let plan = IngestWorkPlan::from_settings(snapshot.settings)?;
    let target = ContentTarget::for_project(&reader, &plan.settings)?;
    // The catalog is read through its one public reader.
    let wanted =
        qnc_content_read::ContentReader::for_project(&reader, &plan.settings)?.clips_without_poster()?;
    let mut client = target.open(Access::ReadOnly)?;
    if wanted.is_empty() {
        return Ok(0);
    }
    let mut project = Project::open(root, &reader, &plan, &target)?;
    let cancel = AtomicBool::new(false);
    let mut made = 0;
    for clip_id in wanted {
        let Some(clip) = client.read(&clip_id)? else { continue };
        let output = plan
            .settings
            .product_local_dir(&project.dir, qnc_work_settings::ProductArea::Thumbnails)
            .join(&clip_id)
            .join("poster.jpg");
        crate::inside_project(&project.dir, &output)?;
        match crate::create_missing_poster(&clip, &plan, &output, &project.opener, &cancel) {
            Ok(()) => {
                let uri = format!("{}/{clip_id}/poster.jpg", plan.thumbnails_uri.trim_end_matches('/'));
                project.queue.set_poster(clip_id, uri)?;
                made += 1;
            }
            Err(error) => eprintln!("poster {clip_id}: {error}"),
        }
    }
    Ok(made)
}
