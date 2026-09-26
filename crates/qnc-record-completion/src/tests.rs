use super::*;
use qnc_camera_adapter::CameraRegistry;
use std::sync::{atomic::AtomicUsize, Arc};

fn fx6() -> CameraRegistry {
    let mut registry = CameraRegistry::new();
    registry
        .register(Arc::new(qnc_camera_sony_fx6_v6::SonyFx6V6::new()))
        .unwrap();
    registry
}

#[test]
fn card_records_are_completed_once_in_the_background_and_published() {
    let (_dir, config) = qnc_ingest_select::test_support::fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    qnc_ingest_select::test_support::execute_with(&config, &calls, false, false, ".", &fx6());
    assert_eq!(calls.load(Ordering::SeqCst), 0, "Select never probes a card record");
    let target = qnc_ingest_select::test_support::select_target(&config);
    let waiting = camera_clips(&target.content).unwrap();
    assert_eq!(waiting.len(), 2);

    let writer = ProjectDbWriter::start(
        target.records.clone(),
        vec![MediaRecordsModule::factory(), SourceIndexModule::factory()],
    )
    .unwrap();
    let records = ProjectMediaRecords::new(writer.clone());
    let sources = ProjectSourceIndex::new(writer);
    let probes = calls.clone();
    let make_backend = move |_: &str, _: &[SourceReference]| {
        Ok(qnc_ingest_select::test_support::probe_backend(probes.clone()))
    };
    let parts = Parts {
        records: &records,
        sources: &sources,
        make_backend: &make_backend,
    };
    let cancel = AtomicBool::new(false);
    let run = Run {
        workers: 2,
        wanted: &|| None,
        player_works: &|| false,
        cancel: &cancel,
    };
    let done = complete_clips(&parts, waiting, &target.content, &run);
    assert_eq!(done.completed.len(), 2, "{:?}", done.failed);
    assert_eq!(calls.load(Ordering::SeqCst), 4, "original and proxy of two clips, once");
    assert!(camera_clips(&target.content).unwrap().is_empty(), "published as final");

    // A second run finds nothing to do and never probes again.
    let again = camera_clips(&target.content).unwrap();
    let done = complete_clips(&parts, again, &target.content, &run);
    assert!(done.completed.is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 4);
}

#[test]
fn a_working_player_is_waited_for_and_cancel_ends_the_wait() {
    let (_dir, config) = qnc_ingest_select::test_support::fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    qnc_ingest_select::test_support::execute_with(&config, &calls, false, false, ".", &fx6());
    let target = qnc_ingest_select::test_support::select_target(&config);
    let writer = ProjectDbWriter::start(
        target.records.clone(),
        vec![MediaRecordsModule::factory(), SourceIndexModule::factory()],
    )
    .unwrap();
    let records = ProjectMediaRecords::new(writer.clone());
    let sources = ProjectSourceIndex::new(writer);
    let make_backend = |_: &str, _: &[SourceReference]| -> Result<_, String> {
        unreachable!("no probe while a player works")
    };
    let parts = Parts {
        records: &records,
        sources: &sources,
        make_backend: &make_backend,
    };
    let cancel = AtomicBool::new(false);
    let waiting = camera_clips(&target.content).unwrap();
    let player = || {
        cancel.store(true, Ordering::Relaxed);
        true
    };
    let run = Run {
        workers: 2,
        wanted: &|| None,
        player_works: &player,
        cancel: &cancel,
    };
    let done = complete_clips(&parts, waiting, &target.content, &run);
    assert_eq!(done, Completion::default());
    assert_eq!(camera_clips(&target.content).unwrap().len(), 2);
}

#[test]
fn the_wanted_clip_is_completed_first() {
    let (_dir, config) = qnc_ingest_select::test_support::fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    qnc_ingest_select::test_support::execute_with(&config, &calls, false, false, ".", &fx6());
    let target = qnc_ingest_select::test_support::select_target(&config);
    let writer = ProjectDbWriter::start(
        target.records.clone(),
        vec![MediaRecordsModule::factory(), SourceIndexModule::factory()],
    )
    .unwrap();
    let records = ProjectMediaRecords::new(writer.clone());
    let sources = ProjectSourceIndex::new(writer);
    let probes = calls.clone();
    let make_backend = move |_: &str, _: &[SourceReference]| {
        Ok(qnc_ingest_select::test_support::probe_backend(probes.clone()))
    };
    let parts = Parts {
        records: &records,
        sources: &sources,
        make_backend: &make_backend,
    };
    let waiting = camera_clips(&target.content).unwrap();
    let last = waiting.last().unwrap().id().to_string();
    let wanted = || Some(last.clone());
    let cancel = AtomicBool::new(false);
    let run = Run {
        workers: 1,
        wanted: &wanted,
        player_works: &|| false,
        cancel: &cancel,
    };
    let done = complete_clips(&parts, waiting, &target.content, &run);
    assert_eq!(done.completed.first(), Some(&last), "the clip a preview wants goes first");
    assert_eq!(done.completed.len(), 2);
}

fn run_with_first_failure(first: qnc_media_probe::Error) -> (Completion, usize) {
    let (_dir, config) = qnc_ingest_select::test_support::fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    qnc_ingest_select::test_support::execute_with(&config, &calls, false, false, ".", &fx6());
    let target = qnc_ingest_select::test_support::select_target(&config);
    let writer = ProjectDbWriter::start(
        target.records.clone(),
        vec![MediaRecordsModule::factory(), SourceIndexModule::factory()],
    )
    .unwrap();
    let records = ProjectMediaRecords::new(writer.clone());
    let sources = ProjectSourceIndex::new(writer);
    let seen = Arc::new(Mutex::new(std::collections::BTreeSet::new()));
    let probes = calls.clone();
    let make_backend = move |_: &str, _: &[SourceReference]| -> Result<Box<dyn ProbeBackend + Send>, String> {
        Ok(Box::new(Shared {
            seen: seen.clone(),
            first: first.clone(),
            inner: qnc_ingest_select::test_support::probe_backend(probes.clone()),
        }))
    };
    let parts = Parts {
        records: &records,
        sources: &sources,
        make_backend: &make_backend,
    };
    let cancel = AtomicBool::new(false);
    let run = Run {
        workers: 1,
        wanted: &|| None,
        player_works: &|| false,
        cancel: &cancel,
    };
    let waiting = camera_clips(&target.content).unwrap();
    let done = complete_clips(&parts, waiting, &target.content, &run);
    let left = camera_clips(&target.content).unwrap().len();
    (done, left)
}

/// `FirstFails` over a set shared by every backend of a run.
struct Shared {
    seen: Arc<Mutex<std::collections::BTreeSet<String>>>,
    first: qnc_media_probe::Error,
    inner: Box<dyn ProbeBackend + Send>,
}

impl ProbeBackend for Shared {
    fn execute(&self, request: &ProbeRequest) -> qnc_media_probe::Result<Report> {
        if self.seen.lock().unwrap().insert(request.media_uri.clone()) {
            return Err(self.first.clone());
        }
        self.inner.execute(request)
    }
}

#[test]
fn a_probe_that_ran_out_of_time_is_taken_again() {
    let (done, left) = run_with_first_failure(qnc_media_probe::Error::Timeout);
    assert_eq!(done.completed.len(), 2, "{:?}", done.failed);
    assert!(done.failed.is_empty());
    assert_eq!(left, 0);
}

#[test]
fn a_probe_that_failed_on_the_medium_is_never_run_again() {
    let (done, left) = run_with_first_failure(qnc_media_probe::Error::Failed);
    assert!(done.completed.is_empty());
    assert_eq!(done.failed.len(), 2);
    assert_eq!(left, 2, "the records stay camera records");
}
