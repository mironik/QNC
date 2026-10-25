use super::*;
use std::time::{Duration, Instant};

/// Work that runs until it is cancelled.
fn until_cancelled() -> Option<Work<&'static str>> {
    Some(Box::new(|cancel: Arc<AtomicBool>| {
        while !cancel.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(2));
        }
        Err("cancelled".into())
    }))
}

fn done() -> Option<Work<&'static str>> {
    Some(Box::new(|_| Ok("done")))
}

fn wait_finished(jobs: &mut Jobs<&'static str>) -> Vec<Finished<&'static str>> {
    let start = Instant::now();
    loop {
        let (any, finished) = jobs.finished();
        if any || start.elapsed() > Duration::from_secs(5) {
            return finished;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn nothing_starts_without_a_project() {
    let mut jobs = Jobs::new("test", Preferred::Beside);
    jobs.enqueue("a", false);
    jobs.start(4, |_| done());
    assert!(!jobs.has_workers());
    assert!(jobs.has_queue());
}

#[test]
fn a_finished_clip_comes_back_and_a_done_clip_is_not_queued() {
    let mut jobs = Jobs::new("test", Preferred::Beside);
    jobs.set_project("p");
    jobs.enqueue("a", false);
    jobs.start(4, |_| done());
    let finished = wait_finished(&mut jobs);
    assert_eq!(finished.len(), 1);
    assert_eq!(finished[0].clip_id, "a");
    assert_eq!(finished[0].result.as_ref().unwrap().as_ref().unwrap(), &"done");
    jobs.mark_ready("a");
    assert!(!jobs.enqueue("a", false));
    assert!(!jobs.is_busy());
}

#[test]
fn another_project_ends_the_workers_and_forgets() {
    let mut jobs = Jobs::new("test", Preferred::Beside);
    jobs.set_project("p");
    jobs.mark_ready("done");
    jobs.enqueue("a", false);
    jobs.enqueue("b", false);
    jobs.start(1, |_| until_cancelled());
    assert!(jobs.has_workers());
    assert!(jobs.set_project("q"));
    assert!(!jobs.is_busy());
    assert!(!jobs.is_ready("done"));
}

#[test]
fn playback_priority_holds_the_queue() {
    let mut jobs = Jobs::new("test", Preferred::Beside);
    jobs.set_project("p");
    jobs.set_playback_priority(true);
    jobs.enqueue("a", false);
    jobs.start(4, |_| done());
    assert!(!jobs.has_workers());
    assert!(jobs.has_queue());
}

#[test]
fn playback_priority_cancels_the_workers_and_queues_them_again() {
    let mut jobs = Jobs::new("test", Preferred::Beside);
    jobs.set_project("p");
    jobs.enqueue("a", false);
    jobs.start(4, |_| until_cancelled());
    assert!(jobs.has_workers());
    jobs.set_playback_priority(true);
    let finished = wait_finished(&mut jobs);
    assert!(finished.is_empty(), "a worker that gave way is not given back");
    assert!(jobs.has_queue(), "it waits again");
}

#[test]
fn while_a_player_plays_only_the_preferred_clip_runs_alone() {
    let mut jobs = Jobs::new("test", Preferred::Beside);
    jobs.set_project("p");
    jobs.set_playback_priority(true);
    jobs.enqueue("other", false);
    jobs.prefer("open");
    jobs.start(4, |_| until_cancelled());
    assert!(jobs.is_running("open"));
    assert!(!jobs.is_running("other"));
}

#[test]
fn the_preferred_clip_runs_beside_the_others() {
    let mut jobs = Jobs::new("test", Preferred::Beside);
    jobs.set_project("p");
    jobs.enqueue("a", false);
    jobs.enqueue("b", false);
    jobs.start(2, |_| until_cancelled());
    jobs.prefer("open");
    jobs.start(2, |_| until_cancelled());
    assert!(jobs.is_running("a") && jobs.is_running("b") && jobs.is_running("open"));
}

#[test]
fn the_preferred_clip_reads_the_medium_alone() {
    let mut jobs = Jobs::new("test", Preferred::Alone);
    jobs.set_project("p");
    jobs.enqueue("long", false);
    jobs.start(1, |_| until_cancelled());
    jobs.enqueue("other", false);
    jobs.enqueue("open", false);
    jobs.prefer("open");
    jobs.start(4, |_| until_cancelled());
    assert!(!jobs.is_running("open"), "nothing starts while the other one stops");
    let finished = wait_finished(&mut jobs);
    assert_eq!(finished.len(), 1, "the one that gave way comes back to its owner");
    jobs.start(4, |_| until_cancelled());
    assert!(jobs.is_running("open"));
    assert!(!jobs.is_running("other"), "the preferred clip runs alone");
}

#[test]
fn the_cap_holds_how_many_run() {
    let mut jobs = Jobs::new("test", Preferred::Beside);
    jobs.set_project("p");
    jobs.set_cap(Some(1));
    for clip in ["a", "b", "c"] {
        jobs.enqueue(clip, false);
    }
    jobs.start(4, |_| until_cancelled());
    assert_eq!(["a", "b", "c"].iter().filter(|clip| jobs.is_running(clip)).count(), 1);
}

#[test]
fn a_clip_without_work_is_passed_over() {
    let mut jobs = Jobs::new("test", Preferred::Beside);
    jobs.set_project("p");
    jobs.enqueue("nothing", false);
    jobs.enqueue("a", false);
    jobs.start(1, |clip| if clip == "nothing" { None } else { until_cancelled() });
    assert!(jobs.is_running("a"));
}
