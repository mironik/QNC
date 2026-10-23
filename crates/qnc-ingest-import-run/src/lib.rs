//! Uvezi in Ingest (user 2026-10-23: "Ingest into blocks"): the marked clips are checked
//! against what the project uses and written to the database once (`qnc-ingest-selection-write`,
//! off the caller's thread; clips in the way give the user a choice), then the background
//! application is woken and does the import the project settings say. Nothing here copies
//! media: that is the worker's; nothing here is a source of truth, the database is.

use std::{path::Path, sync::Arc};

use qnc_clip_row::ClipRow;
use qnc_ingest_selection_write::SelectionWriter;
use qnc_ingest_store::content::ContentTarget;
use qnc_work_settings::WorkSettings;

pub use qnc_choice_box::Choice;
pub use qnc_ingest_selection_write::ANSWERS;

pub const MODULE_ID: &str = "qnc.module.ingest-import-run";

type Wake = dyn Fn(&Path, &ContentTarget) -> Result<(), String> + Send + Sync;

/// What a finished write of the selection brought.
#[derive(Debug, Clone, PartialEq)]
pub enum Written {
    /// The selection is in the database and the worker runs the import.
    HandedOver,
    /// Written, but the worker could not be woken.
    WorkerFailed(String),
    /// Not written; `choice` when clips are in the way and the user chooses.
    Failed { error: String, choice: Option<Choice> },
}

pub struct ImportRun {
    writer: SelectionWriter,
    wake: Arc<Wake>,
    /// Nothing ticked to write, but other work waits (ticked Ingest SubClip parts): the
    /// background is woken at the next poll.
    wake_only: bool,
}

impl Default for ImportRun {
    fn default() -> Self {
        Self { writer: SelectionWriter::default(), wake: Arc::new(qnc_ingest_import_worker::launch_worker), wake_only: false }
    }
}

impl std::fmt::Debug for ImportRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportRun").field("busy", &self.is_busy()).finish()
    }
}

impl ImportRun {
    /// Another way to wake the background application (tests, other compositions).
    pub fn with_wake(mut self, wake: impl Fn(&Path, &ContentTarget) -> Result<(), String> + Send + Sync + 'static) -> Self {
        self.wake = Arc::new(wake);
        self
    }

    pub fn is_busy(&self) -> bool {
        self.writer.is_busy()
    }

    pub fn cancel(&mut self) {
        self.writer.cancel();
    }

    /// Uvezi with the clips the form shows: `Ok(true)` while the selection is written,
    /// `Ok(false)` when nothing was to be written. `others`: other work waits for Uvezi
    /// (ticked Ingest SubClip parts), so with no clip ticked the background is only woken.
    pub fn start(&mut self, settings: &WorkSettings, target: Option<ContentTarget>, clips: &[ClipRow], others: bool) -> Result<bool, String> {
        let needs = qnc_import_fit::Needs::of(settings)?;
        if self.writer.is_busy() {
            return Err(qnc_ui_text::text("ingest.msg.import_running").into());
        }
        if others && !clips.iter().any(|clip| clip.selected) {
            self.wake_only = true;
            return Ok(true);
        }
        let target = target.ok_or(qnc_ui_text::text("ingest.msg.catalog_unavailable"))?;
        self.writer.start_import(target, clips, needs).map(|()| true)
    }

    /// The answer to the choice (import the others, or cancel); `Ok(true)` while writing.
    pub fn answer(&mut self, action_id: &str) -> Result<bool, String> {
        self.writer.answer(action_id).unwrap_or(Ok(false))
    }

    /// Wakes the background application of the project (import, records, filmstrip, wave).
    pub fn wake(&self, root: Option<&Path>, target: Option<&ContentTarget>) -> Result<(), String> {
        let root = root.ok_or(qnc_ui_text::text("ingest.msg.import_no_root"))?;
        let target = target.ok_or(qnc_ui_text::text("ingest.msg.catalog_unavailable"))?;
        (self.wake)(root, target)
    }

    /// The finished write of the selection, `None` while it runs or none runs.
    pub fn poll(&mut self, root: Option<&Path>, target: Option<&ContentTarget>) -> Option<Written> {
        if std::mem::take(&mut self.wake_only) {
            return Some(match self.wake(root, target) {
                Ok(()) => Written::HandedOver,
                Err(error) => Written::WorkerFailed(error),
            });
        }
        Some(match self.writer.poll()? {
            Ok(_) => match self.wake(root, target) {
                Ok(()) => Written::HandedOver,
                Err(error) => Written::WorkerFailed(error),
            },
            Err(error) => Written::Failed { error, choice: self.writer.choice().cloned() },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waking_the_worker_needs_the_project_and_uses_the_given_wake() {
        let woken = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let seen = woken.clone();
        let run = ImportRun::default().with_wake(move |_, _| {
            seen.store(true, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        });
        assert_eq!(run.wake(None, None).unwrap_err(), qnc_ui_text::text("ingest.msg.import_no_root"));
        assert!(!woken.load(std::sync::atomic::Ordering::Relaxed));
        let mut run = run;
        assert_eq!(run.poll(None, None), None, "nothing written, nothing to hand over");
        assert!(!run.is_busy());
    }
}
