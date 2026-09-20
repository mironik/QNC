//! The project settings executed as a background process.
//!
//! Uvezi writes the selection to the database and puts the selected clips in the import
//! queue; then it makes sure `qnc-ingest-worker`, a separate application, is running.
//! The worker has no rules of its own. It reads the settings of the active project once,
//! takes the queued clips and does what the settings say for each (link: nothing is
//! copied; proxy or original: a copy, and the poster with it, unless that copy already
//! exists), records every outcome through the content write transport and ends. The form
//! never waits for it.

use crate::{run_next, ConfigMediaOpener, TransportQueue};
use qnc_ingest_select::selection_config::SelectionConfig;
use qnc_ingest_store::content::ContentTarget;
use qnc_ingest_work_plan::IngestWorkPlan;
use qnc_work_settings::SettingsReader;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, SystemTime},
};

/// The name of the executable that runs beside the application.
pub const WORKER_EXECUTABLE: &str = "qnc-ingest-worker";

/// The lock is alive while it was touched this recently.
const LOCK_FRESH_FOR: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSummary {
    pub imported: usize,
    pub failed: usize,
}

fn lock_path(root: &Path) -> PathBuf {
    root.join("data").join("ingest-worker.lock")
}

/// The worker keeps this file fresh while it runs, so there is only one of it.
pub fn touch_lock(root: &Path) {
    let _ = std::fs::write(lock_path(root), b"");
}

pub fn remove_lock(root: &Path) {
    let _ = std::fs::remove_file(lock_path(root));
}

/// A worker is running for this root.
pub fn is_running(root: &Path) -> bool {
    std::fs::metadata(lock_path(root))
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age <= LOCK_FRESH_FOR)
}

/// What one project needs to import: the plan comes from the settings read this cycle.
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
        pause: &Arc<AtomicBool>,
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
            queue: TransportQueue::start(ContentTarget::for_project(reader, &plan.settings)?)?,
            opener: ConfigMediaOpener::new(sources, pause.clone()),
        })
    }
}

/// The import of one Uvezi: the settings of the active project are read once, then the
/// queued clips are taken one by one until the queue is empty. For each clip the settings
/// decide: link copies nothing; a copy that already exists is not copied again; what is
/// missing is copied. While `pause` is true the copy waits (a player works here).
pub fn run_service(root: &Path, pause: Arc<AtomicBool>) -> Result<ImportSummary, String> {
    let reader = SettingsReader::from_root(root).map_err(|e| e.to_string())?;
    let plan = IngestWorkPlan::from_settings(reader.read().map_err(|e| e.to_string())?)?;
    let mut project = Project::open(root, &reader, &plan, &pause)?;
    let cancel = AtomicBool::new(false);
    let mut summary = ImportSummary {
        imported: 0,
        failed: 0,
    };
    while let Some(outcome) = run_next(
        &mut project.queue,
        &plan,
        &project.dir,
        &project.opener,
        &cancel,
    )? {
        if outcome.result.is_ok() {
            summary.imported += 1;
        } else {
            summary.failed += 1;
        }
    }
    Ok(summary)
}

/// Makes sure the worker runs: nothing happens when it does already, otherwise the
/// executable beside the running application is started and this returns at once.
pub fn launch_worker(root: &Path) -> Result<(), String> {
    if is_running(root) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("data")).unwrap();
        dir
    }

    #[test]
    fn a_worker_is_running_only_while_its_lock_is_fresh() {
        let root = root();
        assert!(!is_running(root.path()));
        touch_lock(root.path());
        assert!(is_running(root.path()));
        remove_lock(root.path());
        assert!(!is_running(root.path()));
    }

    #[test]
    fn launching_while_a_worker_runs_starts_nothing() {
        let root = root();
        touch_lock(root.path());
        // No executable beside the test binary: this would fail if it tried to start one.
        assert!(launch_worker(root.path()).is_ok());
    }

    #[test]
    fn without_a_worker_and_without_its_executable_launching_says_so() {
        let root = root();
        assert!(launch_worker(root.path()).unwrap_err().contains("Nedostaje"));
    }
}
