//! Wakes the background worker of the active project (records, posters, waves, filmstrips,
//! import). Its own block (user 2026-10-18: a clip whose record waited was never completed
//! because nothing woke the worker; "solve things at the level of the blocks"). Nothing
//! happens while the worker's lease in the project database is alive; otherwise the worker
//! executable beside the running application is started and this returns at once. It knows
//! no form: the import, the source preview and any other module may call it.

use std::{
    path::Path,
    process::{Command, Stdio},
};

use qnc_content_store::ContentTarget;

pub const MODULE_ID: &str = "qnc.module.worker-wake";

/// The background worker's executable, beside the running application.
pub const WORKER_EXECUTABLE: &str = "qnc-ingest-worker";

/// Makes sure the worker of the project `target` runs.
pub fn wake(root: &Path, target: &ContentTarget) -> Result<(), String> {
    if qnc_playback_activity::is_fresh(target, qnc_playback_activity::WORKER, qnc_playback_activity::WORKER_FRESH_SECONDS) {
        return Ok(());
    }
    let name = format!("{WORKER_EXECUTABLE}{}", std::env::consts::EXE_SUFFIX);
    let executable = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name(&name);
    if !executable.is_file() {
        return Err(qnc_ui_text::fill(
            "app.helper_missing",
            &[("name", &name.to_string()), ("package", "-p qnc-ingest-worker")],
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

/// Makes sure the worker of the active project of `root` runs (read from its database now).
pub fn wake_active(root: &Path) -> Result<(), String> {
    let active = qnc_active_project_read::ActiveProjectReader::from_root(root).map_err(|e| e.to_string())?;
    let snapshot = active.read().map_err(|e| e.to_string())?;
    let target = ContentTarget::for_project(active.settings_reader(), &snapshot.settings)?;
    wake(root, &target)
}
