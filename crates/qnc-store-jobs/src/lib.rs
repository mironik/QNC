//! A store worked on in the background (audit 2026-10-25: the Project form waited for the
//! project registry and folders on the form thread, 100-300 ms a time). One thread owns the
//! store, made by `init` in that thread; jobs run one after another in the order sent, so a
//! write is followed by the read that shows it; each job's reply is taken by `poll`.
//! It knows no store and no application.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;

pub const MODULE_ID: &str = "qnc.module.store-jobs";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

type Job<S, R> = Box<dyn FnOnce(&mut S) -> R + Send>;

/// The worker of one store `S`, answering `R`.
pub struct StoreJobs<S, R> {
    jobs: Option<Sender<Job<S, R>>>,
    replies: Receiver<R>,
    waiting: usize,
    worker: Option<JoinHandle<()>>,
}

impl<S: 'static, R: Send + 'static> StoreJobs<S, R> {
    /// Starts the thread `name`; the store is made there by `init`.
    pub fn start(name: &str, init: impl FnOnce() -> S + Send + 'static) -> Self {
        let (jobs, inbox) = mpsc::channel::<Job<S, R>>();
        let (reply, replies) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                let mut store = init();
                for job in inbox {
                    if reply.send(job(&mut store)).is_err() {
                        break;
                    }
                }
            })
            .ok();
        Self { jobs: worker.is_some().then_some(jobs), replies, waiting: 0, worker }
    }

    /// Sends a job; false when the worker is gone (its reply never comes).
    pub fn submit(&mut self, job: impl FnOnce(&mut S) -> R + Send + 'static) -> bool {
        let sent = self.jobs.as_ref().is_some_and(|jobs| jobs.send(Box::new(job)).is_ok());
        self.waiting += usize::from(sent);
        sent
    }

    /// The replies that are there, in the order of their jobs.
    pub fn poll(&mut self) -> Vec<R> {
        let replies: Vec<R> = self.replies.try_iter().collect();
        self.waiting = self.waiting.saturating_sub(replies.len());
        replies
    }

    /// Whether a job still waits for its reply.
    pub fn busy(&self) -> bool {
        self.waiting > 0
    }
}

impl<S, R> Drop for StoreJobs<S, R> {
    fn drop(&mut self) {
        // The worker ends after the jobs already sent.
        self.jobs = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn jobs_run_in_order_on_the_store_made_in_the_worker() {
        let mut jobs: StoreJobs<Vec<u32>, usize> = StoreJobs::start("test-store", Vec::new);
        for value in 0..5 {
            assert!(jobs.submit(move |store| {
                store.push(value);
                store.len()
            }));
        }
        let mut replies = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        while jobs.busy() && Instant::now() < deadline {
            replies.extend(jobs.poll());
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(replies, vec![1, 2, 3, 4, 5]);
        assert!(!jobs.busy());
    }
}
