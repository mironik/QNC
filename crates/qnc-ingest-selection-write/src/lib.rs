//! Writes which clips are selected for import.
//!
//! Selecting clips is only a mark in the form's cache. The database sees the selection
//! once, when the user starts the import: this component then writes the whole set
//! (selected on, the others off) and puts the selected clips in the import queue, on its
//! own thread through the public content write transport. It knows no form and no application, and works over a local, LAN or
//! intranet content target.
//!
//! Before writing it checks each selected clip against what the project uses
//! (`qnc-import-fit`, user 2026-10-23). When a clip is in the way it writes nothing and
//! offers a choice (`qnc-choice-box`): import the others, or cancel. The answer is one of
//! its catalog actions ([`ANSWERS`]).

use qnc_choice_box::Choice;
use qnc_clip_row::{ClipRow, SaveState};
use qnc_import_fit::{Conflict, Needs};
use qnc_ingest_store::content::ContentTarget;
use std::{
    sync::mpsc::{self, Receiver, TryRecvError},
    thread::JoinHandle,
};

pub const MODULE_ID: &str = "qnc.module.ingest-selection-write";
pub const VERSION: &str = "0.3.0";

/// Import the selected clips without those in the way.
pub const SKIP_CONFLICTS: &str = "ingest_import_skip_conflicts";
/// Import nothing.
pub const CANCEL_IMPORT: &str = "ingest_import_cancel";
/// The answers to the choice.
pub const ANSWERS: [&str; 2] = [SKIP_CONFLICTS, CANCEL_IMPORT];

/// The selection as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    pub selected: Vec<String>,
    pub unselected: Vec<String>,
}

/// A selected clip in the way: id, name, what is in the way.
type InTheWay = (String, String, Conflict);

/// What the write is asked to do.
#[derive(Debug, Clone)]
struct Request {
    target: ContentTarget,
    selected: Vec<String>,
    unselected: Vec<String>,
    needs: Needs,
    /// Clips in the way are left unselected instead of stopping the write.
    skip_conflicts: bool,
}

/// Why the write stopped.
enum Stop {
    Failed(String),
    /// Nothing was written.
    Conflicts(Vec<InTheWay>),
}

#[derive(Debug, Default)]
pub struct SelectionWriter {
    result: Option<Receiver<Result<Applied, Vec<InTheWay>>>>,
    failure: Option<Receiver<String>>,
    thread: Option<JoinHandle<()>>,
    request: Option<Request>,
    choice: Option<Choice>,
}

impl SelectionWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_busy(&self) -> bool {
        self.result.is_some()
    }

    /// The choice waiting for the user's answer, if any.
    pub fn choice(&self) -> Option<&Choice> {
        self.choice.as_ref()
    }

    /// Uvezi: the ticked rows of the form are the selection, the others are unselected. A row
    /// not yet saved in the database stops it.
    pub fn start_import(&mut self, target: ContentTarget, rows: &[ClipRow], needs: Needs) -> Result<(), String> {
        let selected: Vec<String> = rows.iter().filter(|row| row.selected).map(|row| row.clip_id.clone()).collect();
        if selected.is_empty() {
            return Err(qnc_ui_text::text("ingest.msg.no_clips_selected").into());
        }
        if rows.iter().any(|row| row.selected && row.save_state != SaveState::Saved) {
            return Err(qnc_ui_text::text("ingest.msg.clip_not_saved").into());
        }
        let unselected = rows.iter().filter(|row| !row.selected && row.save_state == SaveState::Saved).map(|row| row.clip_id.clone()).collect();
        self.launch(Request { target, selected, unselected, needs, skip_conflicts: false })
    }

    /// The user's answer to the choice: `Some(Ok(true))` writes again without the clips in
    /// the way, `Some(Ok(false))` drops it; `None` for an action that is not an answer.
    pub fn answer(&mut self, action_id: &str) -> Option<Result<bool, String>> {
        if !ANSWERS.contains(&action_id) {
            return None;
        }
        self.choice = None;
        let request = self.request.take()?;
        if action_id == CANCEL_IMPORT {
            return Some(Ok(false));
        }
        Some(self.launch(Request { skip_conflicts: true, ..request }).map(|()| true))
    }

    /// Starts writing the whole selection. Refused while a write is still running.
    pub fn start(&mut self, target: ContentTarget, selected: Vec<String>, unselected: Vec<String>, needs: Needs) -> Result<(), String> {
        self.launch(Request { target, selected, unselected, needs, skip_conflicts: false })
    }

    fn launch(&mut self, request: Request) -> Result<(), String> {
        if self.is_busy() {
            return Err(qnc_ui_text::text("ingest.selection_being_written").into());
        }
        self.choice = None;
        let (send, receive) = mpsc::sync_channel(1);
        let (send_failure, failure) = mpsc::sync_channel(1);
        let job = request.clone();
        let thread = std::thread::Builder::new()
            .name("ingest-selection-write".into())
            .spawn(move || {
                let _ = match write(job) {
                    Ok(applied) => send.send(Ok(applied)),
                    Err(Stop::Conflicts(found)) => send.send(Err(found)),
                    Err(Stop::Failed(error)) => {
                        let _ = send_failure.send(error);
                        Ok(())
                    }
                };
            })
            .map_err(|error| error.to_string())?;
        (self.request, self.result, self.failure, self.thread) = (Some(request), Some(receive), Some(failure), Some(thread));
        Ok(())
    }

    /// The outcome once the write has finished, `None` while it still runs. Clips in the way
    /// give an error whose message is the choice's; the choice then waits for an answer.
    pub fn poll(&mut self) -> Option<Result<Applied, String>> {
        let receiver = self.result.as_ref()?;
        let outcome = match receiver.try_recv() {
            Ok(outcome) => outcome,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => {
                let error = self.failure.as_ref().and_then(|f| f.try_recv().ok());
                self.finish();
                self.request = None;
                return Some(Err(error.unwrap_or_else(|| qnc_ui_text::text("ingest.writing_selection_interrupted").into())));
            }
        };
        self.finish();
        match outcome {
            Ok(applied) => {
                self.request = None;
                Some(Ok(applied))
            }
            Err(found) => {
                let request = self.request.as_ref()?;
                let choice = choice_of(&found, request.needs, request.selected.len());
                let message = choice.message.clone();
                self.choice = Some(choice);
                Some(Err(message))
            }
        }
    }

    /// Drops the outcome and the choice, and waits for the write thread.
    pub fn cancel(&mut self) {
        self.finish();
        (self.request, self.choice) = (None, None);
    }

    fn finish(&mut self) {
        (self.result, self.failure) = (None, None);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for SelectionWriter {
    fn drop(&mut self) {
        self.finish();
    }
}

mod conflicts;
mod write;

use conflicts::choice_of;
use write::write;

#[cfg(test)]
mod tests;
