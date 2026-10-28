//! The transcription queue of a project, run in the background (user 2026-10-26: "tick the
//! clips to transcribe; while that is done in the background, or waits there, the trim zoom
//! helps cut the statements"). The queue is in the project database (`qnc-transcript-db`):
//! what is asked outlives the application and every process sees it. One runner per project
//! holds a lease in its database; it takes the first waiting clip (one left running by a run
//! that died counts as waiting), transcribes it with the engine it was asked with, yielding to
//! the player (`qnc-transcribe-run`), keeps the transcript and marks the clip done or failed,
//! and stops when nothing waits. The lines heard come out as they are made.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread::JoinHandle,
};

use qnc_db_broker::ProjectDbTarget;
use qnc_still_locator::StillLocator;
use qnc_transcribe_run::{Ask, Line, Pace, Quality};
pub use qnc_transcribe_run::Progress;
pub use qnc_transcript_db::{QueueQuality, QueueState, Queued};

pub const MODULE_ID: &str = "qnc.module.transcribe-queue";
/// The lease of the one runner of a project.
pub const LEASE: &str = "transcribe_lease";

/// What the runner tells while it works.
#[derive(Debug, Clone, PartialEq)]
pub enum Heard {
    /// The clip's range `[from, to)` is being transcribed now.
    Started { clip_id: String, from: u64, to: u64 },
    /// A line of the clip being transcribed.
    Line { clip_id: String, line: Line },
    /// How far the clip being transcribed is.
    Progress { clip_id: String, progress: Progress },
    /// The clip is done (kept) or failed.
    Finished { clip_id: String, result: Result<(), String> },
}

/// The runner of one project's queue, if it runs here.
#[derive(Default)]
pub struct QueueRunner {
    running: Option<Running>,
}

struct Running {
    project_uri: String,
    cancel: Arc<AtomicBool>,
    heard: mpsc::Receiver<Heard>,
    thread: Option<JoinHandle<()>>,
}

impl QueueRunner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a runner of this process works.
    pub fn busy(&self) -> bool {
        self.running.as_ref().is_some_and(|run| run.thread.as_ref().is_some_and(|t| !t.is_finished()))
    }

    /// Starts the runner of `target`'s project here unless one works (here, or in another
    /// process: its lease is held). A runner of another project is stopped first.
    pub fn ensure(&mut self, locator: &StillLocator, target: &ProjectDbTarget) {
        if self.running.as_ref().is_some_and(|run| run.project_uri != target.uri()) {
            self.stop();
        }
        if self.busy() {
            return;
        }
        let (send, heard) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let (locator, db, stop) = (locator.clone(), target.clone(), cancel.clone());
        let thread = std::thread::Builder::new().name("qnc-transcribe-queue".into()).spawn(move || run(&locator, &db, &stop, &send));
        if let Ok(thread) = thread {
            self.running = Some(Running { project_uri: target.uri().to_string(), cancel, heard, thread: Some(thread) });
        }
    }

    /// What the runner heard since the last call.
    pub fn take(&mut self) -> Vec<Heard> {
        self.running.as_ref().map(|run| run.heard.try_iter().collect()).unwrap_or_default()
    }

    /// Stops the runner (another project, the form closes); a clip it was doing waits again.
    pub fn stop(&mut self) {
        if let Some(run) = self.running.take() {
            run.cancel.store(true, Ordering::Relaxed);
        }
    }
}

impl Drop for QueueRunner {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run(locator: &StillLocator, db: &ProjectDbTarget, cancel: &AtomicBool, send: &mpsc::Sender<Heard>) {
    let lease = match qnc_content_store::ContentTarget::for_project(locator.reader(), locator.settings())
        .map_err(|e| e.to_string())
        .and_then(|target| qnc_playback_activity::Lease::claim(target, LEASE, qnc_playback_activity::WORKER_FRESH_SECONDS))
    {
        Ok(Some(lease)) => lease,
        Ok(None) => return, // another process runs this project's queue
        Err(error) => return log(&format!("transcribe-queue lease: {error}")),
    };
    loop {
        if cancel.load(Ordering::Relaxed) || !lease.held() {
            return;
        }
        let queue = match qnc_transcript_db::read_queue(db) {
            Ok(queue) => queue,
            Err(error) => return log(&format!("transcribe-queue read: {error}")),
        };
        // A clip left running by a run that died waits again; this runner holds the lease.
        let Some(next) = queue.into_iter().find(|q| matches!(q.state, QueueState::Waiting | QueueState::Running)) else {
            // The queue is done: the lease goes now and the background worker is woken, so the
            // shot analysis that waited for the card starts at once (user 2026-10-28).
            drop(lease);
            return wake_worker(locator);
        };
        if let Err(error) = qnc_transcript_db::set_queued(db, &next.clip_id, QueueState::Running, "", 0) {
            return log(&format!("transcribe-queue mark {}: {error}", next.clip_id));
        }
        let _ = send.send(Heard::Started { clip_id: next.clip_id.clone(), from: next.from_frame, to: next.to_frame });
        let stop = StopWhen { cancel, lost: lease.lost() };
        let ask = Ask { clip_id: next.clip_id.clone(), from: next.from_frame, to: next.to_frame };
        let quality = match next.quality {
            QueueQuality::Fast => Quality::Fast,
            QueueQuality::Best => Quality::Best,
        };
        let started = std::time::Instant::now();
        let made = stop.run(|flag| {
            qnc_transcribe_run::transcribe(
                locator,
                &ask,
                quality,
                Pace::YieldToPlayer,
                flag,
                &mut |line| {
                    let _ = send.send(Heard::Line { clip_id: next.clip_id.clone(), line: line.clone() });
                },
                &mut |progress| {
                    let _ = send.send(Heard::Progress { clip_id: next.clip_id.clone(), progress });
                },
            )
        });
        if cancel.load(Ordering::Relaxed) || !lease.held() {
            // Stopped, not failed: it waits for the next runner.
            let _ = qnc_transcript_db::set_queued(db, &next.clip_id, QueueState::Waiting, "", 0);
            return;
        }
        let kept = made.and_then(|transcript| qnc_transcript_db::save(db, transcript));
        let (state, error) = match &kept {
            Ok(()) => (QueueState::Done, String::new()),
            Err(error) => (QueueState::Failed, error.clone()),
        };
        log(&format!("transcribe-queue {} {}..{} {:?} {:?} ms={} {error}", next.clip_id, next.from_frame, next.to_frame, next.quality, state, started.elapsed().as_millis()));
        let _ = qnc_transcript_db::set_queued(db, &next.clip_id, state, &error, started.elapsed().as_millis() as u64);
        let _ = send.send(Heard::Finished { clip_id: next.clip_id, result: kept });
    }
}

/// Wakes the background worker of this project (piece `qnc-worker-wake`).
fn wake_worker(locator: &StillLocator) {
    let Some(root) = qnc_dev_diagnostics::resolve_qnc_root() else {
        return log("transcribe-queue done; no QNC root to wake the worker from");
    };
    let woken = qnc_content_store::ContentTarget::for_project(locator.reader(), locator.settings())
        .map_err(|e| e.to_string())
        .and_then(|target| qnc_worker_wake::wake(&root, &target));
    match woken {
        Ok(()) => log("transcribe-queue done; background worker woken"),
        Err(error) => log(&format!("transcribe-queue done; background worker not woken: {error}")),
    }
}

/// One cancel flag set by the runner's cancel or the loss of its lease.
struct StopWhen<'a> {
    cancel: &'a AtomicBool,
    lost: &'a AtomicBool,
}

impl StopWhen<'_> {
    fn run<T>(&self, work: impl FnOnce(&AtomicBool) -> T) -> T {
        let flag = AtomicBool::new(false);
        let done = AtomicBool::new(false);
        std::thread::scope(|scope| {
            scope.spawn(|| {
                while !done.load(Ordering::Relaxed) {
                    if self.cancel.load(Ordering::Relaxed) || self.lost.load(Ordering::Relaxed) {
                        flag.store(true, Ordering::Relaxed);
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            });
            let made = work(&flag);
            done.store(true, Ordering::Relaxed);
            made
        })
    }
}

fn log(line: &str) {
    if qnc_dev_diagnostics::player_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line.to_string());
    }
}
