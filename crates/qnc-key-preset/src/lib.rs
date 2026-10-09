//! The keyboard preset of the active project (user 2026-10-09: chosen in Project, Advanced
//! settings, so users of one template keep their own keys; it holds for every form, present
//! and future). A thread of its own reads it read-only from the project database once a
//! second (the active project may change at any time); a caller only takes the latest value,
//! so no paint ever waits for the database. Without an active project, or when the project
//! chose none, there is no preset and the keyboard catalog keeps its own.

use qnc_active_project_read::ActiveProjectReader;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

const READ_EVERY: Duration = Duration::from_secs(1);

/// The latest keyboard preset of the active project, kept up to date in the background.
pub struct KeyPresetWatcher {
    latest: Arc<Mutex<Option<String>>>,
}

impl KeyPresetWatcher {
    /// Starts reading the active project of the QNC `root`; the thread lives as long as
    /// the process.
    pub fn start(root: PathBuf) -> Self {
        let latest = Arc::new(Mutex::new(None));
        let shared = latest.clone();
        let _ = thread::Builder::new()
            .name("qnc-key-preset".into())
            .spawn(move || loop {
                let preset = read_preset(&root);
                *shared.lock().unwrap() = preset;
                thread::sleep(READ_EVERY);
            });
        Self { latest }
    }

    /// The preset the active project chose, as last read.
    pub fn preset(&self) -> Option<String> {
        self.latest.lock().unwrap().clone()
    }
}

/// The preset the active project of `root` chose; `None` without one.
pub fn read_preset(root: &std::path::Path) -> Option<String> {
    let project = ActiveProjectReader::from_root(root).ok()?.read().ok()?;
    project.settings.keyboard_preset().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_qnc_root_means_no_preset() {
        let dir = std::env::temp_dir().join("qnc-key-preset-none");
        assert_eq!(read_preset(&dir), None);
        let watcher = KeyPresetWatcher::start(dir);
        assert_eq!(watcher.preset(), None);
    }
}

#[cfg(test)]
mod live {
    /// Reads the preset of the active project of `QNC_KEY_PRESET_ROOT` (a QNC root, for a
    /// check on a protected copy); run with `--ignored --nocapture`.
    #[test]
    #[ignore = "reads the QNC root given in QNC_KEY_PRESET_ROOT"]
    fn preset_of_a_given_root() {
        let root = std::env::var("QNC_KEY_PRESET_ROOT").expect("QNC_KEY_PRESET_ROOT");
        println!("preset={:?}", super::read_preset(std::path::Path::new(&root)));
    }
}
