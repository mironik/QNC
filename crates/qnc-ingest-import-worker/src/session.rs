//! A background import: one thread that drains the queue and reports events.
//! The application owns one session and polls it, as it does for Select.

use crate::{run_next, MediaOpener, Outcome};
use qnc_ingest_store::content::ContentTarget;
use qnc_ingest_work_plan::IngestWorkPlan;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
        Arc,
    },
    thread::JoinHandle,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSummary {
    pub imported: usize,
    pub failed: usize,
}

#[derive(Debug)]
pub enum ImportEvent {
    /// One clip finished (imported or failed).
    Clip(Outcome),
    Finished(Result<ImportSummary, String>),
}

#[derive(Default)]
pub struct ImportSession {
    result: Option<Receiver<ImportEvent>>,
    cancel: Option<Arc<AtomicBool>>,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for ImportSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportSession")
            .field("pending", &self.has_pending_work())
            .finish()
    }
}

impl ImportSession {
    pub fn has_pending_work(&self) -> bool {
        self.result.is_some()
    }

    /// Starts draining the queue of `target`. A running import is cancelled first.
    pub fn start(
        &mut self,
        plan: IngestWorkPlan,
        project_dir: PathBuf,
        opener: Arc<dyn MediaOpener>,
        target: ContentTarget,
    ) -> Result<(), String> {
        self.cancel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (send, receive) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("qnc-ingest-import".into())
            .spawn(move || {
                let result = run(&plan, &project_dir, opener.as_ref(), &target, &send, &worker_cancel);
                let _ = send.send(ImportEvent::Finished(result));
            })
            .map_err(|error| error.to_string())?;
        self.cancel = Some(cancel);
        self.result = Some(receive);
        self.thread = Some(thread);
        Ok(())
    }

    pub fn cancel(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.result = None;
        drop(self.thread.take());
    }

    pub fn poll(&mut self, limit: usize) -> Vec<ImportEvent> {
        let mut events = Vec::new();
        for _ in 0..limit {
            let Some(receiver) = self.result.as_ref() else {
                break;
            };
            let event = match receiver.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    ImportEvent::Finished(Err("Uvoz je prekinut.".into()))
                }
            };
            let finished = matches!(event, ImportEvent::Finished(_));
            events.push(event);
            if finished {
                self.result = None;
                self.cancel = None;
                drop(self.thread.take());
                break;
            }
        }
        events
    }
}

impl Drop for ImportSession {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn run(
    plan: &IngestWorkPlan,
    project_dir: &std::path::Path,
    opener: &dyn MediaOpener,
    target: &ContentTarget,
    send: &mpsc::Sender<ImportEvent>,
    cancel: &AtomicBool,
) -> Result<ImportSummary, String> {
    let mut client = crate::TransportQueue::start(target.clone())?;
    let mut summary = ImportSummary {
        imported: 0,
        failed: 0,
    };
    while !cancel.load(Ordering::Relaxed) {
        let Some(outcome) = run_next(&mut client, plan, project_dir, opener, cancel)? else {
            break;
        };
        if outcome.result.is_ok() {
            summary.imported += 1;
        } else {
            summary.failed += 1;
        }
        if send.send(ImportEvent::Clip(outcome)).is_err() {
            break;
        }
    }
    Ok(summary)
}
