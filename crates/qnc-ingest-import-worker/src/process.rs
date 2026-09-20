//! The project settings executed as a background process.
//!
//! Uvezi writes the selection to the database and puts the selected clips in the import
//! queue; then it makes sure `qnc-ingest-worker`, a separate application, is running.
//! The worker has no rules of its own. In a loop it reads the settings of the active
//! project again, takes the next queued clip and does what the settings say for it
//! (link: nothing is copied; proxy or original: a copy, and the poster with it), and
//! records the outcome through the content write transport. It ends after it has been
//! idle for a while; the next Uvezi starts it again. The form never waits for it.

use crate::{run_next, ConfigMediaOpener, TransportQueue};
use qnc_ingest_select::selection_config::SelectionConfig;
use qnc_ingest_store::content::ContentTarget;
use qnc_ingest_work_plan::IngestWorkPlan;
use qnc_work_settings::SettingsReader;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant, SystemTime},
};

/// The name of the executable that runs beside the application.
pub const WORKER_EXECUTABLE: &str = "qnc-ingest-worker";

/// How often the worker looks at the queue and at the settings while it is idle.
const IDLE_POLL: Duration = Duration::from_secs(2);

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
    project_id: String,
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
            project_id: plan.settings.project_id.clone(),
            dir,
            queue: TransportQueue::start(ContentTarget::for_project(reader, &plan.settings)?)?,
            opener: ConfigMediaOpener::new(sources, pause.clone()),
        })
    }
}

/// The worker loop: until it has had nothing to do for `idle_exit`, read the settings of
/// the active project, take the next queued clip and execute the settings for it. While
/// `pause` is true the copy waits (a player works on this machine).
pub fn run_service(
    root: &Path,
    pause: Arc<AtomicBool>,
    idle_exit: Duration,
) -> Result<ImportSummary, String> {
    let cancel = AtomicBool::new(false);
    let mut summary = ImportSummary {
        imported: 0,
        failed: 0,
    };
    let mut project: Option<Project> = None;
    let mut idle_since = Instant::now();
    loop {
        match cycle(root, &pause, &cancel, &mut project) {
            Ok(Some(succeeded)) => {
                if succeeded {
                    summary.imported += 1;
                } else {
                    summary.failed += 1;
                }
                idle_since = Instant::now();
                continue;
            }
            Ok(None) => {}
            Err(error) if idle_since.elapsed() >= idle_exit => return Err(error),
            Err(_) => {}
        }
        if idle_since.elapsed() >= idle_exit {
            return Ok(summary);
        }
        std::thread::sleep(IDLE_POLL);
    }
}

/// One turn: settings of the active project, then at most one clip. `Some(true)` when a
/// clip was imported, `Some(false)` when it failed, `None` when the queue is empty.
fn cycle(
    root: &Path,
    pause: &Arc<AtomicBool>,
    cancel: &AtomicBool,
    project: &mut Option<Project>,
) -> Result<Option<bool>, String> {
    let reader = SettingsReader::from_root(root).map_err(|e| e.to_string())?;
    let plan = IngestWorkPlan::from_settings(reader.read().map_err(|e| e.to_string())?)?;
    if project
        .as_ref()
        .is_none_or(|open| open.project_id != plan.settings.project_id)
    {
        *project = Some(Project::open(root, &reader, &plan, pause)?);
    }
    let open = project.as_mut().expect("opened above");
    let outcome = run_next(&mut open.queue, &plan, &open.dir, &open.opener, cancel)?;
    Ok(outcome.map(|outcome| outcome.result.is_ok()))
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
