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

use qnc_choice_box::{Choice, ChoiceOption};
use qnc_clip_row::{ClipRow, SaveState};
use qnc_import_fit::{Conflict, Needs, Playback};
use qnc_ingest_store::content::{Access, ContentTarget, ContentWriteTransport};
use std::{
    sync::mpsc::{self, Receiver, TryRecvError},
    thread::JoinHandle,
    time::Duration,
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

/// The selected clips that do not serve what the project uses.
fn in_the_way(request: &Request) -> Result<Vec<InTheWay>, String> {
    let mut client = request.target.open(Access::ReadOnly).map_err(|e| e.to_string())?;
    let mut found = Vec::new();
    let mut after = None;
    loop {
        let page = client.list(after.clone()).map_err(|e| e.to_string())?;
        let Some(last) = page.last() else { break };
        after = Some(last.clip.id().to_string());
        for stored in &page {
            let id = stored.clip.id().to_string();
            if request.selected.contains(&id) {
                if let Some(conflict) = qnc_import_fit::conflict(&stored.clip.snapshot, request.needs) {
                    found.push((id, stored.clip.name.clone(), conflict));
                }
            }
        }
    }
    Ok(found)
}

fn write(mut request: Request) -> Result<Applied, Stop> {
    let found = in_the_way(&request).map_err(Stop::Failed)?;
    if !found.is_empty() {
        if !request.skip_conflicts {
            return Err(Stop::Conflicts(found));
        }
        request.selected.retain(|id| !found.iter().any(|(clip, _, _)| clip == id));
        request.unselected.extend(found.into_iter().map(|(id, _, _)| id));
    }
    let Request { target, selected, unselected, needs, .. } = request;
    let mut transport = ContentWriteTransport::start(target).map_err(|e| Stop::Failed(e.to_string()))?;
    if !unselected.is_empty() {
        run(&mut transport, "unselect", |t, key| t.select(key, unselected.clone(), false)).map_err(Stop::Failed)?;
    }
    if !selected.is_empty() {
        run(&mut transport, "select", |t, key| t.select(key, selected.clone(), true)).map_err(Stop::Failed)?;
        // The selected clips wait in the queue for the background application.
        run(&mut transport, "queue", |t, key| t.queue_selected(key, needs)).map_err(Stop::Failed)?;
    }
    Ok(Applied { selected, unselected })
}

/// The info box for the clips in the way: what is in the way, how it is solved in the
/// project's settings, and the answers.
fn choice_of(found: &[InTheWay], needs: Needs, selected: usize) -> Choice {
    let key = |conflict: &Conflict| match conflict {
        Conflict::ProxyMissing => "ingest.choice.proxy_missing",
        Conflict::ProxyIncomplete { .. } => "ingest.choice.proxy_incomplete",
        Conflict::OriginalIncomplete { .. } => "ingest.choice.original_incomplete",
    };
    let mut keys: Vec<&str> = found.iter().map(|(_, _, conflict)| key(conflict)).collect();
    keys.sort_unstable();
    keys.dedup();
    let message = keys.iter().map(|key| qnc_ui_text::text(key).to_string()).collect::<Vec<_>>().join(" ");
    let proxy_in_the_way = found.iter().any(|(_, _, c)| !matches!(c, Conflict::OriginalIncomplete { .. }));
    let solution = match needs.playback {
        _ if !proxy_in_the_way => "ingest.choice.solve_original_incomplete",
        Playback::Proxy => "ingest.choice.solve_proxy",
        Playback::ProxyIfAvailable => "ingest.choice.solve_proxy_if_available",
        Playback::Original => "ingest.choice.solve_copy_proxy",
    };
    let items = found
        .iter()
        .map(|(_, name, conflict)| match conflict {
            Conflict::ProxyMissing => name.clone(),
            Conflict::ProxyIncomplete { missing } | Conflict::OriginalIncomplete { missing } => format!("{name}: {}", missing.join(", ")),
        })
        .collect();
    let mut options = Vec::new();
    if selected > found.len() {
        let others = (selected - found.len()).to_string();
        options.push(ChoiceOption { action_id: SKIP_CONFLICTS.into(), label: qnc_ui_text::fill("ingest.choice.import_others", &[("count", &others)]), strong: true });
    }
    options.push(ChoiceOption { action_id: CANCEL_IMPORT.into(), label: qnc_ui_text::text("ingest.choice.cancel").into(), strong: options.is_empty() });
    Choice { title: qnc_ui_text::text("ingest.choice.title").into(), message, items, solution: qnc_ui_text::text(solution).into(), options }
}

fn run<E: ToString>(
    transport: &mut ContentWriteTransport,
    name: &str,
    send: impl FnOnce(&mut ContentWriteTransport, String) -> Result<(), E>,
) -> Result<(), String> {
    let key = format!("selection-{name}");
    send(transport, key.clone()).map_err(|e| e.to_string())?;
    loop {
        for completion in transport.poll() {
            if completion.key == key {
                completion.result.map_err(|e| e.to_string())?;
                return Ok(());
            }
        }
        if !transport.has_pending() {
            return Err(qnc_ui_text::text("db.content_writer_returned_result").into());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_idle_writer_has_no_outcome_and_can_be_cancelled_repeatedly() {
        let mut writer = SelectionWriter::new();
        assert!(!writer.is_busy());
        assert!(writer.poll().is_none());
        writer.cancel();
        writer.cancel();
        assert!(!writer.is_busy());
    }

    #[test]
    fn clips_in_the_way_give_a_choice_to_import_the_others_or_cancel() {
        let proxy = Needs { playback: Playback::Proxy, copies_proxy: false };
        let found = vec![("c1".to_string(), "C0012.MP4".to_string(), Conflict::ProxyMissing)];
        let choice = choice_of(&found, proxy, 3);
        assert_eq!(choice.items, ["C0012.MP4"]);
        assert_eq!(choice.options.iter().map(|o| o.action_id.as_str()).collect::<Vec<_>>(), ANSWERS);
        assert!(choice.options[0].strong && choice.options[0].label.contains('2'), "the proposed answer imports the other two");
        assert!(choice.solution.contains("Proxy if available"));
        let only = choice_of(&found, proxy, 1);
        assert_eq!(only.options.len(), 1, "nothing else to import: only cancel");
        assert_eq!(only.options[0].action_id, CANCEL_IMPORT);
    }

    #[test]
    fn an_answer_without_a_choice_drops_nothing_and_another_action_is_not_an_answer() {
        let mut writer = SelectionWriter::new();
        assert_eq!(writer.answer("ingest_import_selected"), None);
        assert_eq!(writer.answer(SKIP_CONFLICTS), None, "no choice is waiting");
        assert!(writer.choice().is_none());
    }
}
