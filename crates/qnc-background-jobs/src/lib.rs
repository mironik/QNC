//! QNC public piece: the queue and the workers of one kind of background job over the clips of
//! one project: which clip waits, which runs and which is done, the clip open in a preview
//! first, the player's priority and a cap on how many run (user 2026-10-26: one scheduler
//! instead of the mirrored copies in the filmstrip and wave workers, before transcripts,
//! scenes and image vectors come as further jobs). What a job makes, and how it is read and
//! written, belongs to its worker: this piece only schedules and runs the threads.

use std::collections::{BTreeSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;

pub const MODULE_ID: &str = "qnc.module.background-jobs";

/// How the clip open in a preview runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preferred {
    /// It goes first and one more worker runs for it beside the others (filmstrip: the work
    /// is the processor's).
    Beside,
    /// It runs alone: the others give way (they are cancelled and queued again) and nothing
    /// else starts until it is done (wave: two long passes on one card halve each other).
    Alone,
}

/// The work of one clip, run on its own thread with the job's cancel flag.
pub type Work<O> = Box<dyn FnOnce(Arc<AtomicBool>) -> Result<O, String> + Send>;

/// A worker that ended. `result` is `None` when its thread ended without an answer.
#[derive(Debug)]
pub struct Finished<O> {
    pub clip_id: String,
    pub result: Option<Result<O, String>>,
}

struct Worker<O> {
    clip_id: String,
    result: Receiver<Result<O, String>>,
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

pub struct Jobs<O> {
    thread_name: &'static str,
    preferred_runs: Preferred,
    project: Option<String>,
    ready: BTreeSet<String>,
    queue: VecDeque<String>,
    queued: BTreeSet<String>,
    workers: Vec<Worker<O>>,
    playback: bool,
    /// At most this many workers (user 2026-10-20: two while the user edits, one beside a
    /// playing player).
    cap: Option<usize>,
    preferred: Option<String>,
}

impl<O: Send + 'static> Jobs<O> {
    pub fn new(thread_name: &'static str, preferred_runs: Preferred) -> Self {
        Self {
            thread_name,
            preferred_runs,
            project: None,
            ready: BTreeSet::new(),
            queue: VecDeque::new(),
            queued: BTreeSet::new(),
            workers: Vec::new(),
            playback: false,
            cap: None,
            preferred: None,
        }
    }

    pub fn project(&self) -> Option<&str> {
        self.project.as_deref()
    }

    /// The project whose clips the jobs are. Another project ends its workers and forgets
    /// its queue and what was done; true when it changed.
    pub fn set_project(&mut self, project: &str) -> bool {
        if self.project.as_deref() == Some(project) {
            return false;
        }
        self.forget_all();
        self.project = Some(project.to_string());
        true
    }

    /// No project: every worker ends, nothing is remembered.
    pub fn clear(&mut self) {
        self.forget_all();
        self.project = None;
    }

    fn forget_all(&mut self) {
        self.cancel_and_join();
        self.ready.clear();
        self.queue.clear();
        self.queued.clear();
    }

    pub fn is_ready(&self, clip_id: &str) -> bool {
        self.ready.contains(clip_id)
    }

    pub fn mark_ready(&mut self, clip_id: &str) {
        self.ready.insert(clip_id.to_string());
    }

    pub fn is_running(&self, clip_id: &str) -> bool {
        self.workers.iter().any(|worker| worker.clip_id == clip_id)
    }

    /// Clips no longer in the project: out of the queue and no longer done.
    pub fn forget(&mut self, clip_ids: &[String]) {
        self.queue.retain(|queued| !clip_ids.contains(queued));
        for clip_id in clip_ids {
            self.queued.remove(clip_id);
            self.ready.remove(clip_id);
        }
    }

    /// Queues a clip (first when `front`); false when it is done or runs.
    pub fn enqueue(&mut self, clip_id: &str, front: bool) -> bool {
        if self.ready.contains(clip_id) || self.is_running(clip_id) {
            return false;
        }
        if !self.queued.insert(clip_id.to_string()) {
            if front {
                self.to_front(clip_id);
            }
            return true;
        }
        if front {
            self.queue.push_front(clip_id.to_string());
        } else {
            self.queue.push_back(clip_id.to_string());
        }
        true
    }

    fn to_front(&mut self, clip_id: &str) -> bool {
        let Some(index) = self.queue.iter().position(|queued| queued == clip_id) else {
            return false;
        };
        let queued = self.queue.remove(index).expect("index exists");
        self.queue.push_front(queued);
        true
    }

    /// While a player plays only the preferred clip is worked on, alone; the other workers
    /// are cancelled (user 2026-10-20). True when it changed.
    pub fn set_playback_priority(&mut self, active: bool) -> bool {
        if self.playback == active {
            return false;
        }
        self.playback = active;
        if active {
            for worker in self.workers.iter().filter(|worker| Some(&worker.clip_id) != self.preferred.as_ref()) {
                worker.cancel.store(true, Ordering::Release);
            }
        }
        true
    }

    pub fn playback_priority(&self) -> bool {
        self.playback
    }

    /// At most `cap` workers (`None`: as many as the job allows). True when it changed.
    pub fn set_cap(&mut self, cap: Option<usize>) -> bool {
        if self.cap == cap {
            return false;
        }
        self.cap = cap;
        true
    }

    /// The clip open in a preview goes first (user 2026-10-19: "priority of the clicked clip").
    pub fn prefer(&mut self, clip_id: &str) {
        match self.preferred_runs {
            Preferred::Beside => {
                if self.ready.contains(clip_id) || self.is_running(clip_id) {
                    return;
                }
                self.preferred = Some(clip_id.to_string());
                self.enqueue(clip_id, true);
            }
            Preferred::Alone => {
                self.preferred = Some(clip_id.to_string());
                if self.to_front(clip_id) {
                    for worker in self.workers.iter().filter(|worker| worker.clip_id != clip_id) {
                        worker.cancel.store(true, Ordering::Release);
                    }
                }
            }
        }
    }

    pub fn preferred(&self) -> Option<&str> {
        self.preferred.as_deref()
    }

    /// Starts workers: at most `most` (before the cap), the preferred clip as its kind runs.
    /// `work` gives the work of a clip, or `None` when the clip has nothing to do now.
    pub fn start(&mut self, most: usize, mut work: impl FnMut(&str) -> Option<Work<O>>) {
        if self.project.is_none() {
            return;
        }
        let preferred_waits = match self.preferred.clone() {
            Some(preferred) if !self.is_running(&preferred) => self.to_front(&preferred),
            _ => false,
        };
        if self.playback {
            // A player plays: only the preferred clip, alone.
            if preferred_waits && self.workers.is_empty() {
                self.start_one(&mut work);
            }
            return;
        }
        let most = match self.preferred_runs {
            Preferred::Alone => {
                let preferred_runs = self.preferred.as_deref().is_some_and(|preferred| self.is_running(preferred));
                if preferred_runs {
                    return; // the medium is its own
                }
                if preferred_waits {
                    // It starts once the others have given way.
                    if self.workers.is_empty() {
                        self.start_one(&mut work);
                    }
                    return;
                }
                most.min(self.cap.unwrap_or(usize::MAX))
            }
            Preferred::Beside => most.min(self.cap.unwrap_or(usize::MAX)) + usize::from(preferred_waits),
        };
        while self.workers.len() < most {
            if !self.start_one(&mut work) {
                return;
            }
        }
    }

    /// Starts the first queued clip that has work; false when none.
    fn start_one(&mut self, work: &mut impl FnMut(&str) -> Option<Work<O>>) -> bool {
        loop {
            let Some(clip_id) = self.queue.pop_front() else {
                return false;
            };
            self.queued.remove(&clip_id);
            if self.ready.contains(&clip_id) {
                continue;
            }
            let Some(job) = work(&clip_id) else {
                continue;
            };
            let (send, receive) = mpsc::sync_channel(1);
            let cancel = Arc::new(AtomicBool::new(false));
            let worker_cancel = cancel.clone();
            match std::thread::Builder::new()
                .name(self.thread_name.into())
                .spawn(move || {
                    let _ = send.send(job(worker_cancel));
                }) {
                Ok(thread) => self.workers.push(Worker { clip_id, result: receive, cancel, thread }),
                Err(error) => eprintln!("{} start failed: {error}", self.thread_name),
            }
            return true;
        }
    }

    /// Workers that ended since the last call, and whether any did. A worker that ended while
    /// a player plays and is not the preferred clip is queued again first and not given back
    /// (it gave way).
    pub fn finished(&mut self) -> (bool, Vec<Finished<O>>) {
        let mut any = false;
        let mut finished = Vec::new();
        let mut index = 0;
        while index < self.workers.len() {
            let result = match self.workers[index].result.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => {
                    index += 1;
                    continue;
                }
                Err(TryRecvError::Disconnected) => None,
            };
            let worker = self.workers.swap_remove(index);
            let _ = worker.thread.join();
            any = true;
            if self.playback && Some(&worker.clip_id) != self.preferred.as_ref() {
                self.enqueue(&worker.clip_id, true);
                continue;
            }
            finished.push(Finished { clip_id: worker.clip_id, result });
        }
        (any, finished)
    }

    pub fn has_workers(&self) -> bool {
        !self.workers.is_empty()
    }

    pub fn has_queue(&self) -> bool {
        !self.queue.is_empty()
    }

    pub fn is_busy(&self) -> bool {
        self.has_workers() || self.has_queue()
    }

    fn cancel_and_join(&mut self) {
        for worker in &self.workers {
            worker.cancel.store(true, Ordering::Release);
        }
        for worker in self.workers.drain(..) {
            let _ = worker.thread.join();
        }
    }
}

impl<O> Drop for Jobs<O> {
    fn drop(&mut self) {
        for worker in &self.workers {
            worker.cancel.store(true, Ordering::Release);
        }
        for worker in self.workers.drain(..) {
            let _ = worker.thread.join();
        }
    }
}

#[cfg(test)]
mod tests;
