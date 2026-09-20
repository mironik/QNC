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
    let reader = SettingsReader::from_root(root).map_err(|e| e.to_string())?;
    let plan = IngestWorkPlan::from_settings(reader.read().map_err(|e| e.to_string())?)?;
    let target = ContentTarget::for_project(&reader, &plan.settings)?;
    let mut summary = ImportSummary {
        imported: 0,
        failed: 0,
    };
    if qnc_ingest_runtime::is_fresh(&target, WORKER, WORKER_FRESH_SECONDS) {
        return Ok(summary);
    }
    let _lease = Beat::start(target.clone(), WORKER)?;
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
