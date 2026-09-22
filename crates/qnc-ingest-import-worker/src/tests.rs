use super::*;
use qnc_camera_sony_fx6_v6::SonyFx6V6;
use qnc_ingest_select::{test_support, Event};
use qnc_ingest_store::content::{Access, ImportStatus};
use qnc_ingest_work_plan::IngestWorkPlan;
use std::{
    collections::BTreeMap,
    io::Cursor,
    sync::{atomic::AtomicUsize, Arc},
};

const DB_URI: &str = "qnc://local/db/ingest_content/p1";

struct Bytes {
    data: Cursor<Vec<u8>>,
    declared: u64,
}

impl Read for Bytes {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.data.read(buffer)
    }
}

impl MediaRead for Bytes {
    fn byte_len(&self) -> u64 {
        self.declared
    }
}

/// Media by URI held in memory: stands in for local disk, LAN or intranet.
struct Memory {
    media: BTreeMap<String, Vec<u8>>,
    /// Claims more bytes than the stream delivers (a cut connection).
    lie_about_length: bool,
}

impl MediaOpener for Memory {
    fn open(&self, media_uri: &str) -> Result<Box<dyn MediaRead>, String> {
        let data = self
            .media
            .get(media_uri)
            .ok_or_else(|| format!("nema medija {media_uri}"))?
            .clone();
        let declared = data.len() as u64 + u64::from(self.lie_about_length) * 10;
        Ok(Box::new(Bytes {
            data: Cursor::new(data),
            declared,
        }))
    }
}

fn plan(media: &str, playback: &str) -> IngestWorkPlan {
    let settings = qnc_work_settings_fixture(media, playback);
    IngestWorkPlan::from_settings(settings).unwrap()
}

fn qnc_work_settings_fixture(media: &str, playback: &str) -> qnc_work_settings::WorkSettings {
    serde_json::from_value(serde_json::json!({
        "contract_version": "0.1.0",
        "project_id": "p1",
        "project_name": "Projekt",
        "workspace_db_uri": "qnc://local/db/project_workspace/p1",
        "output_root_uri": "qnc://local/project/p1",
        "storage": {
            "ingest_profile": "field",
            "ingest_media": media,
            "proxy_policy": "link_when_available",
            "original_policy": "link_when_available"
        },
        "input": {"mode": "auto"},
        "playback": {"input": playback},
        "video": {"fps": 25.0},
        "audio": {"sample_rate": 48000, "channels": 2},
        "ai": {"enabled": false},
        "keyboard_shortcuts": {"active_preset": "qnc"}
    }))
    .unwrap()
}

struct Fixture {
    _card: tempfile::TempDir,
    project: tempfile::TempDir,
    client: ContentClient,
    target: qnc_ingest_store::content::ContentTarget,
    media: BTreeMap<String, Vec<u8>>,
    original: String,
    proxy: String,
}

/// Two real clips from the Sony card fixture, selected and queued.
fn fixture() -> Fixture {
    fixture_with(true)
}

fn fixture_with(queued: bool) -> Fixture {
    let (card, config) = test_support::fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    let registry = qnc_camera_adapter_registry();
    let events = test_support::execute_with(&config, &calls, false, false, ".", &registry);
    let ids: Vec<String> = events
        .iter()
        .filter_map(|e| {
            if let Event::Clip(c) = e {
                Some(c.clip_id.clone())
            } else {
                None
            }
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let content = config
        .source_index
        .file
        .as_ref()
        .unwrap()
        .with_file_name("content.db");
    let mut client =
        ContentClient::from_owner_binding(&content, DB_URI, Access::ReadWrite).unwrap();
    let target =
        qnc_ingest_store::content::ContentTarget::from_owner_binding(&content, DB_URI).unwrap();
    client.select(ids.clone(), true).unwrap();
    if queued {
        client.queue_selected().unwrap();
    }
    let first = client.read(&ids[0]).unwrap().unwrap();
    let original = first.clip.snapshot.binding.original_uri.clone();
    let proxy = first.clip.snapshot.binding.proxy_uri.clone().unwrap();
    let mut media = BTreeMap::new();
    for id in &ids {
        let clip = client.read(id).unwrap().unwrap();
        let binding = &clip.clip.snapshot.binding;
        media.insert(
            binding.original_uri.clone(),
            format!("original of {id}").into_bytes(),
        );
        if let Some(poster) = &clip.clip.thumbnail_uri {
            media.insert(poster.clone(), format!("poster of {id}").into_bytes());
        }
        if let Some(proxy) = &binding.proxy_uri {
            media.insert(proxy.clone(), format!("proxy of {id}").into_bytes());
        }
    }
    Fixture {
        _card: card,
        project: tempfile::tempdir().unwrap(),
        client,
        target,
        media,
        original,
        proxy,
    }
}

fn qnc_camera_adapter_registry() -> qnc_camera_adapter::CameraRegistry {
    let mut registry = qnc_camera_adapter::CameraRegistry::new();
    registry.register(Arc::new(SonyFx6V6::new())).unwrap();
    registry
}

fn opener(media: &BTreeMap<String, Vec<u8>>, lie: bool) -> Memory {
    Memory {
        media: media.clone(),
        lie_about_length: lie,
    }
}

fn nothing() -> AtomicBool {
    AtomicBool::new(false)
}

#[test]
fn settings_decide_the_action_for_every_media_mode() {
    let mut f = fixture();
    let clip = f.client.claim_next().unwrap().unwrap();
    assert_eq!(
        action_for(&clip, &plan("original", "original")).unwrap(),
        Action::Copy {
            source_uri: f.original.clone(),
            folder: Folder::Original
        }
    );
    assert_eq!(
        action_for(&clip, &plan("proxy", "original")).unwrap(),
        Action::Copy {
            source_uri: f.proxy.clone(),
            folder: Folder::Proxy
        }
    );
    assert_eq!(
        action_for(&clip, &plan("link", "original")).unwrap(),
        Action::Link {
            media_uri: f.original.clone()
        }
    );
    assert_eq!(
        action_for(&clip, &plan("link", "proxy")).unwrap(),
        Action::Link {
            media_uri: f.proxy.clone()
        }
    );
    assert_eq!(
        action_for(&clip, &plan("link", "proxy_if_available")).unwrap(),
        Action::Link {
            media_uri: f.proxy.clone()
        }
    );
}

#[test]
fn original_mode_copies_the_original_into_the_project_and_records_the_outcome() {
    let mut f = fixture();
    let project = f.project.path().to_path_buf();
    let outcome = run_next(
        &mut f.client,
        &plan("original", "original"),
        &project,
        &opener(&f.media, false),
        &nothing(),
    )
    .unwrap()
    .unwrap();
    let uri = outcome.result.unwrap();
    assert!(uri.starts_with("qnc://local/project/p1/original/"));
    let file = project
        .join("original")
        .join(uri.rsplit('/').next().unwrap());
    assert!(std::fs::read(&file).unwrap().starts_with(b"original of"));
    let stored = f.client.read(&outcome.clip_id).unwrap().unwrap();
    assert_eq!(stored.import_status, ImportStatus::Imported);
    assert_eq!(stored.imported_media_uri.as_deref(), Some(uri.as_str()));
}

#[test]
fn proxy_mode_copies_the_proxy_and_link_mode_copies_nothing() {
    let mut f = fixture();
    let project = f.project.path().to_path_buf();
    let opener = opener(&f.media, false);
    let proxy = run_next(
        &mut f.client,
        &plan("proxy", "original"),
        &project,
        &opener,
        &nothing(),
    )
    .unwrap()
    .unwrap()
    .result
    .unwrap();
    assert!(proxy.starts_with("qnc://local/project/p1/proxy/"));
    assert!(project.join("proxy").is_dir());
    let linked = run_next(
        &mut f.client,
        &plan("link", "original"),
        &project,
        &opener,
        &nothing(),
    )
    .unwrap()
    .unwrap()
    .result
    .unwrap();
    assert!(linked.starts_with("qnc://local/source/"));
    assert!(!project.join("original").exists(), "link never copies");
}

#[test]
fn a_medium_that_cannot_be_opened_is_recorded_as_a_failed_import() {
    let mut f = fixture();
    let project = f.project.path().to_path_buf();
    let empty = Memory {
        media: BTreeMap::new(),
        lie_about_length: false,
    };
    let outcome = run_next(
        &mut f.client,
        &plan("original", "original"),
        &project,
        &empty,
        &nothing(),
    )
    .unwrap()
    .unwrap();
    assert!(outcome.result.is_err());
    let stored = f.client.read(&outcome.clip_id).unwrap().unwrap();
    assert_eq!(stored.import_status, ImportStatus::Failed);
    assert!(stored.import_error.is_some());
}

#[test]
fn a_cancelled_import_copies_nothing() {
    let mut f = fixture();
    let project = f.project.path().to_path_buf();
    let cancel = AtomicBool::new(true);
    let outcome = run_next(
        &mut f.client,
        &plan("original", "original"),
        &project,
        &opener(&f.media, false),
        &cancel,
    )
    .unwrap()
    .unwrap();
    assert!(outcome.result.unwrap_err().contains("prekinut"));
}

#[test]
fn drain_imports_every_queued_clip_once() {
    let mut f = fixture();
    let project = f.project.path().to_path_buf();
    let outcomes = drain(
        &mut f.client,
        &plan("original", "original"),
        &project,
        &opener(&f.media, false),
        &nothing(),
    )
    .unwrap();
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.iter().all(|o| o.result.is_ok()));
    let again = drain(
        &mut f.client,
        &plan("original", "original"),
        &project,
        &opener(&f.media, false),
        &nothing(),
    )
    .unwrap();
    assert!(again.is_empty(), "an imported clip is never claimed again");
}

#[test]
fn file_names_are_safe_on_every_operating_system() {
    let name = safe_name("clip-1", "qnc://local/source/card/Clip/TEST%20A:*?.MXF");
    assert!(name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')));
    assert!(name.starts_with("clip-1_"));
    assert!(name.len() <= 120);
}

#[test]
fn draining_the_queue_reports_every_clip() {
    let mut f = fixture();
    let outcomes = drain(
        &mut f.client,
        &plan("original", "original"),
        f.project.path(),
        &opener(&f.media, false),
        &nothing(),
    )
    .unwrap();
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.iter().all(|o| o.result.is_ok()));
    assert!(f.client.claim_next().unwrap().is_none());
}

#[test]
fn the_write_transport_queues_the_selected_clips_and_hands_them_out_once() {
    let mut f = fixture_with(false);
    assert!(
        f.client.claim_next().unwrap().is_none(),
        "nothing queued yet"
    );
    queue_selected(f.target.clone()).unwrap();
    let mut queue = TransportQueue::start(f.target.clone()).unwrap();
    let first = queue.claim_next().unwrap().unwrap();
    let second = queue.claim_next().unwrap().unwrap();
    assert_ne!(first.clip.id(), second.clip.id());
    assert!(
        queue.claim_next().unwrap().is_none(),
        "each clip is handed out once"
    );
    queue
        .finish_import(first.clip.id().into(), Some(f.original.clone()), None, None)
        .unwrap();
    let stored = f.client.read(first.clip.id()).unwrap().unwrap();
    assert_eq!(stored.import_status, ImportStatus::Imported);
}

#[test]
fn copying_the_media_copies_the_poster_and_records_it() {
    let mut f = fixture();
    let project = f.project.path().to_path_buf();
    let plan = plan("original", "original");
    let opener = opener(&f.media, false);
    let mut with_poster = None;
    while let Some(outcome) = run_next(&mut f.client, &plan, &project, &opener, &nothing()).unwrap()
    {
        if outcome.thumbnail_uri.is_some() {
            with_poster = Some(outcome);
        }
    }
    let outcome = with_poster.expect("a Sony clip has a poster");
    let poster = outcome.thumbnail_uri.clone().unwrap();
    assert_eq!(
        poster,
        format!(
            "qnc://local/project/p1/ingest/thumbnails/{}/poster.jpg",
            outcome.clip_id
        )
    );
    let file = project
        .join("ingest")
        .join("thumbnails")
        .join(&outcome.clip_id)
        .join("poster.jpg");
    assert!(std::fs::read(&file).unwrap().starts_with(b"poster of"));
    let stored = f.client.read(&outcome.clip_id).unwrap().unwrap();
    assert!(stored.clip.thumbnail_uri.is_some());
}

#[test]
fn link_mode_keeps_media_link_but_copies_poster_to_project() {
    let mut f = fixture();
    let project = f.project.path().to_path_buf();
    let plan = plan("link", "original");
    let opener = opener(&f.media, false);
    let mut with_poster = None;
    while let Some(outcome) = run_next(&mut f.client, &plan, &project, &opener, &nothing()).unwrap()
    {
        if outcome.thumbnail_uri.is_some() {
            with_poster = Some(outcome);
            break;
        }
    }
    let outcome = with_poster.expect("a Sony clip has a poster");
    let stored = f.client.read(&outcome.clip_id).unwrap().unwrap();
    assert_eq!(
        Some(outcome.result.as_ref().unwrap().as_str()),
        stored.imported_media_uri.as_deref()
    );
    let poster = outcome.thumbnail_uri.unwrap();
    assert_eq!(
        poster,
        format!(
            "qnc://local/project/p1/ingest/thumbnails/{}/poster.jpg",
            outcome.clip_id
        )
    );
    assert!(project
        .join("ingest")
        .join("thumbnails")
        .join(&outcome.clip_id)
        .join("poster.jpg")
        .is_file());
    assert_eq!(stored.clip.thumbnail_uri.as_deref(), Some(poster.as_str()));
}

#[test]
fn a_poster_that_cannot_be_read_does_not_fail_the_import() {
    let mut f = fixture();
    let project = f.project.path().to_path_buf();
    f.media.retain(|_, data| !data.starts_with(b"poster of"));
    let outcome = run_next(
        &mut f.client,
        &plan("original", "original"),
        &project,
        &opener(&f.media, false),
        &nothing(),
    )
    .unwrap()
    .unwrap();
    assert!(outcome.result.is_ok());
    assert!(outcome.thumbnail_uri.is_none());
}

struct Holding {
    inner: Memory,
    hold: Arc<std::sync::atomic::AtomicBool>,
}

impl MediaOpener for Holding {
    fn open(&self, media_uri: &str) -> Result<Box<dyn MediaRead>, String> {
        self.inner.open(media_uri)
    }
    fn paused(&self) -> bool {
        self.hold.load(Ordering::Relaxed)
    }
}

#[test]
fn the_copy_waits_while_the_opener_is_paused_and_finishes_when_released() {
    let mut f = fixture();
    let project = f.project.path().to_path_buf();
    let hold = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let opener = Holding {
        inner: opener(&f.media, false),
        hold: hold.clone(),
    };
    let plan = plan("original", "original");
    let cancel = nothing();
    let original = project.join("original");
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| run_next(&mut f.client, &plan, &project, &opener, &cancel));
        std::thread::sleep(std::time::Duration::from_millis(400));
        let written: u64 = std::fs::read_dir(&original)
            .map(|d| d.map(|e| e.unwrap().metadata().unwrap().len()).sum())
            .unwrap_or(0);
        assert_eq!(written, 0, "nothing is copied while paused");
        hold.store(false, Ordering::Relaxed);
        let outcome = worker.join().unwrap().unwrap().unwrap();
        assert!(outcome.result.is_ok());
    });
}

#[test]
fn a_later_uvezi_imports_only_the_clips_that_are_not_imported_yet() {
    let mut f = fixture();
    let plan = plan("original", "original");
    let opener = opener(&f.media, false);
    let project = f.project.path().to_path_buf();
    // The first Uvezi imports one of the two selected clips.
    let first = run_next(&mut f.client, &plan, &project, &opener, &nothing())
        .unwrap()
        .unwrap();
    assert!(first.result.is_ok());
    // A later Uvezi selects both again: only the difference is queued and imported.
    queue_selected(f.target.clone()).unwrap();
    let outcomes = drain(&mut f.client, &plan, &project, &opener, &nothing()).unwrap();
    assert_eq!(outcomes.len(), 1);
    assert_ne!(outcomes[0].clip_id, first.clip_id);
}

#[test]
fn launching_while_the_lease_in_the_database_is_alive_starts_nothing() {
    let f = fixture();
    let root = std::path::Path::new("unused");
    // No worker executable beside the test binary: this fails if it tries to start one.
    let _lease =
        qnc_ingest_runtime::Beat::start(f.target.clone(), qnc_ingest_runtime::WORKER).unwrap();
    assert!(launch_worker(root, &f.target).is_ok());
}

#[test]
fn without_a_lease_and_without_its_executable_launching_says_so() {
    let f = fixture();
    let error = launch_worker(std::path::Path::new("unused"), &f.target).unwrap_err();
    assert!(error.contains("Nedostaje"), "{error}");
}

#[test]
fn the_copy_waits_while_the_database_says_a_player_works() {
    let f = fixture();
    let pause = qnc_ingest_runtime::playback_pause(f.target.clone());
    assert!(!pause());
    let mut writer = qnc_ingest_runtime::Writer::start(f.target.clone()).unwrap();
    writer.set(qnc_ingest_runtime::PLAYBACK, "on").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(600));
    assert!(pause());
}

#[test]
fn a_clip_without_a_card_poster_and_without_a_local_file_gets_no_poster_and_no_error() {
    let mut f = fixture();
    let id = f
        .client
        .claim_next()
        .unwrap()
        .unwrap()
        .clip
        .id()
        .to_string();
    let mut clip = f.client.read(&id).unwrap().unwrap();
    clip.clip.thumbnail_uri = None;
    let project = f.project.path().to_path_buf();
    let poster = import_poster(
        &clip,
        &plan("link", "original"),
        &project,
        &opener(&f.media, false),
        &nothing(),
    );
    assert!(poster.is_none());
    assert!(
        !project.join("ingest").exists(),
        "nothing is written when no poster can be made"
    );
}

#[test]
fn a_linked_clip_with_a_card_poster_gets_a_project_poster() {
    let mut f = fixture();
    let mut clip = f.client.claim_next().unwrap().unwrap();
    if clip.clip.thumbnail_uri.is_none() {
        clip = f.client.claim_next().unwrap().unwrap();
    }
    assert!(
        clip.clip.thumbnail_uri.is_some(),
        "the fixture has a clip with a card poster"
    );
    let project = f.project.path().to_path_buf();
    let poster = import_poster(
        &clip,
        &plan("link", "original"),
        &project,
        &opener(&f.media, false),
        &nothing(),
    );
    assert_eq!(
        poster.as_deref(),
        Some(
            format!(
                "qnc://local/project/p1/ingest/thumbnails/{}/poster.jpg",
                clip.clip.id()
            )
            .as_str()
        )
    );
}

#[test]
fn writing_is_allowed_only_below_the_project_folder() {
    let project = std::path::Path::new("proj");
    assert!(inside_project(project, &project.join("original").join("a.mxf")).is_ok());
    assert!(inside_project(project, std::path::Path::new("card/Clip/a.mxf")).is_err());
    assert!(inside_project(project, &project.join("..").join("card").join("a")).is_err());
}
