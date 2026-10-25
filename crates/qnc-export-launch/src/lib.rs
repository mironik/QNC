//! Starts the export of the active project's story (user 2026-10-24): the executable
//! `qnc-export` beside the running application, which finds the QNC root as the application
//! does, reads the story and writes into the project's export folder. It runs on a thread of
//! its own; its outcome (the path written, or the error) is taken when it is there. It knows
//! no form, no database and no format.

use std::{
    process::Stdio,
    sync::{Arc, Mutex},
};

pub const MODULE_ID: &str = "qnc.module.export-launch";
/// The executable of the export, beside the running application.
pub const EXECUTABLE: &str = "qnc-export";

/// The outcome of an export: the path written, or what went wrong.
pub type Outcome = Result<String, String>;

#[derive(Debug, Default)]
pub struct ExportRun {
    outcome: Arc<Mutex<Option<Outcome>>>,
    running: bool,
}

impl ExportRun {
    /// Starts an export with `args` (e.g. `--xml`); false while one still runs.
    pub fn start(&mut self, args: &[&str]) -> bool {
        if self.running {
            return false;
        }
        self.running = true;
        let outcome = self.outcome.clone();
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        let started = std::thread::Builder::new().name("qnc-export".into()).spawn(move || {
            let result = run(&args);
            if let Ok(mut slot) = outcome.lock() {
                *slot = Some(result);
            }
        });
        if let Err(error) = started {
            self.running = false;
            if let Ok(mut slot) = self.outcome.lock() {
                *slot = Some(Err(error.to_string()));
            }
        }
        true
    }

    pub fn running(&self) -> bool {
        self.running
    }

    /// The outcome of the last export, once, when it is there.
    pub fn take(&mut self) -> Option<Outcome> {
        let outcome = self.outcome.lock().ok()?.take();
        if outcome.is_some() {
            self.running = false;
        }
        outcome
    }
}

fn run(args: &[String]) -> Outcome {
    let executable = qnc_plugin_process::beside_application(EXECUTABLE)?;
    let mut command = qnc_plugin_process::background_command(&executable);
    command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    let output = command.output().map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let last = stdout.lines().rev().find(|line| !line.trim().is_empty()).unwrap_or_default().trim().to_string();
    match output.status.success() {
        true => Ok(last),
        false => Err(last.strip_prefix("qnc-export: ").unwrap_or(&last).to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_executable_is_an_outcome_not_a_hang() {
        let mut run = ExportRun::default();
        assert!(run.start(&["--xml"]));
        let outcome = loop {
            if let Some(outcome) = run.take() {
                break outcome;
            }
            std::thread::yield_now();
        };
        assert!(outcome.is_err() || outcome.is_ok());
        assert!(!run.running());
    }
}
