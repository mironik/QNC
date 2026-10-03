//! The applications the desktop launch bar shows (v5 `workspace.tabs`, user rule
//! 2026-10-01). Without an active project only the first priority group is shown (the
//! project application, chosen by its group, not by its name). With an active project
//! the bar shows only the applications its template chose, in the order of its
//! sequence, read from the project database through the public read-only ports. The
//! database stays the truth: every `read` reads it again; this block keeps nothing.

use std::path::Path;

use qnc_shell_desktop_api::DesktopApplicationRef;

pub const MODULE_ID: &str = "qnc.module.desktop-tabs";

/// The tabs to show and, when the project could not be read, why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tabs {
    pub tab_ids: Vec<String>,
    /// The database names an active project.
    pub project_open: bool,
    pub error: Option<String>,
}

/// The tabs for the applications `available` in this desktop.
pub fn read(root: &Path, available: &[DesktopApplicationRef]) -> Tabs {
    match project_sequence(root) {
        Ok(None) => Tabs { tab_ids: first_group(available), project_open: false, error: None },
        Ok(Some(sequence)) => Tabs { tab_ids: of_sequence(&sequence, available), project_open: true, error: None },
        Err(error) => Tabs { tab_ids: first_group(available), project_open: false, error: Some(error) },
    }
}

/// Reads the tabs again on a thread of its own, every `every` or when woken, so the
/// desktop never waits on the database (a reader waits while a background job writes
/// the project database, up to its busy timeout). It keeps only the last read, as a
/// mailbox; the database stays the truth.
pub struct Watcher {
    latest: std::sync::Arc<std::sync::Mutex<Option<Tabs>>>,
    wake: Option<std::sync::mpsc::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Watcher {
    /// Stops the reading thread and waits for it, so nothing keeps the database open.
    fn drop(&mut self) {
        self.wake.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Watcher {
    pub fn start(root: std::path::PathBuf, available: Vec<DesktopApplicationRef>, every: std::time::Duration) -> Self {
        let latest = std::sync::Arc::new(std::sync::Mutex::new(None));
        let (wake, woken) = std::sync::mpsc::channel::<()>();
        let mailbox = latest.clone();
        let thread = std::thread::Builder::new().name("qnc-desktop-tabs".into()).spawn(move || loop {
            let tabs = read(&root, &available);
            if let Ok(mut slot) = mailbox.lock() {
                *slot = Some(tabs);
            }
            match woken.recv_timeout(every) {
                Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        });
        Self { latest, wake: Some(wake), thread: thread.ok() }
    }

    /// The tabs read since the last call, if any.
    pub fn take(&self) -> Option<Tabs> {
        self.latest.lock().ok()?.take()
    }

    /// Read again now.
    pub fn reread(&self) {
        if let Some(wake) = &self.wake {
            let _ = wake.send(());
        }
    }
}

/// The application ids of the active project's sequence; `None` without an active
/// project.
fn project_sequence(root: &Path) -> Result<Option<Vec<String>>, String> {
    let reader = qnc_active_project_read::ActiveProjectReader::from_root(root).map_err(|error| error.to_string())?;
    let snapshot = match reader.read() {
        Ok(snapshot) => snapshot,
        Err(error) if error.code == "no_active_project" => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let steps = qnc_application_sequence::read(reader.settings_reader(), &snapshot.settings)?;
    Ok(Some(steps.into_iter().map(|step| step.application_id).collect()))
}

/// The available applications of the first priority group.
pub fn first_group(available: &[DesktopApplicationRef]) -> Vec<String> {
    let Some(first) = available.iter().map(|app| app.priority_group.as_str()).min() else {
        return Vec::new();
    };
    available
        .iter()
        .filter(|app| app.priority_group == first)
        .map(|app| app.tab_id.clone())
        .collect()
}

/// The available applications of the sequence, in its order.
pub fn of_sequence(sequence: &[String], available: &[DesktopApplicationRef]) -> Vec<String> {
    sequence
        .iter()
        .filter_map(|id| available.iter().find(|app| &app.application_id == id))
        .map(|app| app.tab_id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, tab: &str, group: &str) -> DesktopApplicationRef {
        DesktopApplicationRef {
            application_id: id.into(),
            tab_id: tab.into(),
            priority_group: group.into(),
        }
    }

    fn available() -> Vec<DesktopApplicationRef> {
        vec![
            app("qnc.story", "storyboard", "o"),
            app("qnc.project", "project", "a"),
            app("qnc.ingest", "ingest", "b"),
            app("qnc.media-assist-video", "ma_video", "l"),
        ]
    }

    #[test]
    fn without_a_project_only_the_first_group() {
        assert_eq!(first_group(&available()), ["project"]);
        assert!(first_group(&[]).is_empty());
    }

    #[test]
    fn with_a_project_only_its_applications_in_its_order() {
        let sequence = ["qnc.project", "qnc.ingest", "qnc.story", "qnc.not-installed"].map(String::from);
        assert_eq!(of_sequence(&sequence, &available()), ["project", "ingest", "storyboard"]);
    }

    #[test]
    fn the_watcher_reads_on_its_own_thread_and_again_when_woken() {
        let root = std::env::temp_dir().join(format!("qnc_desktop_tabs_watch_{}", std::process::id()));
        let watcher = Watcher::start(root, available(), std::time::Duration::from_secs(60));
        let wait = |watcher: &Watcher| {
            let start = std::time::Instant::now();
            loop {
                if let Some(tabs) = watcher.take() {
                    return tabs;
                }
                assert!(start.elapsed() < std::time::Duration::from_secs(5), "no read");
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        };
        assert_eq!(wait(&watcher).tab_ids, ["project"]);
        assert!(watcher.take().is_none(), "a read is taken once");
        watcher.reread();
        assert_eq!(wait(&watcher).tab_ids, ["project"]);
    }

    #[test]
    fn a_root_without_a_project_database_shows_the_first_group_and_why() {
        let root = std::env::temp_dir().join(format!("qnc_desktop_tabs_{}", std::process::id()));
        let tabs = read(&root, &available());
        assert_eq!(tabs.tab_ids, ["project"]);
        assert!(tabs.error.is_some());
    }
}
