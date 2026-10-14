//! Wakes the background prerender of the active project (user 2026-10-12: always on).
//! The prerender is its own executable beside the application (`qnc-prerender`); its
//! lease in the project database keeps it single. Waking reads that lease and starts the
//! executable on a thread of its own, so a form never waits for the database or a
//! process start. It knows no form and no application.

use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use qnc_content_store::ContentTarget;

/// The lease of the one prerender of a project in its database.
pub const LEASE: &str = "prerender_lease";
/// The executable of the prerender, beside the running application.
pub const EXECUTABLE: &str = "qnc-prerender";
/// Wakes closer together than this are one wake.
const EVERY: Duration = Duration::from_secs(3);

#[derive(Debug, Default)]
pub struct PrerenderWaker {
    last: Option<Instant>,
}

impl PrerenderWaker {
    /// Makes sure the prerender of the project of `target` runs (the story changed or the
    /// project opened). Returns at once. The prerender finds the QNC root as the
    /// application does (`QNC_ROOT` or beside its executable).
    pub fn wake(&mut self, target: Option<&ContentTarget>) {
        let Some(target) = target.cloned() else {
            return;
        };
        if self.last.is_some_and(|last| last.elapsed() < EVERY) {
            return;
        }
        self.last = Some(Instant::now());
        let _ = std::thread::Builder::new()
            .name("qnc-prerender-wake".into())
            .spawn(move || {
                if let Err(error) = start(&target) {
                    if qnc_dev_diagnostics::player_diagnostics_enabled() {
                        qnc_dev_diagnostics::log_line(
                            qnc_dev_diagnostics::DiagnosticsStream::Player,
                            format!("prerender wake failed: {error}"),
                        );
                    }
                }
            });
    }
}

fn start(target: &ContentTarget) -> Result<(), String> {
    if qnc_playback_activity::is_fresh(target, LEASE, qnc_playback_activity::WORKER_FRESH_SECONDS) {
        return Ok(());
    }
    let name = format!("{EXECUTABLE}{}", std::env::consts::EXE_SUFFIX);
    let executable = std::env::current_exe().map_err(|e| e.to_string())?.with_file_name(&name);
    if !executable.is_file() {
        return Err(qnc_ui_text::fill("app.helper_missing_short", &[("name", &name.to_string())]));
    }
    let mut command = Command::new(executable);
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window for a background process.
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let _ = child.wait();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_project_nothing_starts() {
        let mut waker = PrerenderWaker::default();
        waker.wake(None);
        assert!(waker.last.is_none());
    }
}
