use super::*;
use crate::selection_config::{Binding, ProbeConfig, SourceConfig};
use qnc_ingest_store::content::ContentClient;
use serde_json::json;
use std::{path::Path, sync::mpsc};

const SOURCE: &str = "qnc://local/source/card-test";
const INDEX: &str = include_str!("../../qnc-sony-metadata/tests/fixtures/MEDIAPRO.XML");
const SIDE: &str = include_str!("../../qnc-sony-metadata/tests/fixtures/TEST-AM01.XML");

fn local(uri: &str, file: &Path) -> Binding {
    Binding {
        uri: uri.into(),
        file: Some(file.into()),
        endpoint: None,
        token_env: None,
    }
}

pub(crate) fn fixture() -> (tempfile::TempDir, SelectionConfig) {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("card");
    let recording = card.join("PRIVATE/XDROOT");
    for name in ["Clip", "Sub", "Thmbnl"] {
        std::fs::create_dir_all(recording.join(name)).unwrap();
    }
    std::fs::write(recording.join("MEDIAPRO.XML"), INDEX).unwrap();
    for name in [
        "Clip/TEST A.MXF",
        "Clip/TEST B.MXF",
        "Sub/TEST AS03.MP4",
        "Sub/TEST BS03.MP4",
        "Thmbnl/TEST AT01.JPG",
    ] {
        std::fs::write(recording.join(name), "not media; fixture backend only").unwrap();
    }
    std::fs::write(recording.join("Clip/TEST AM01.XML"), SIDE).unwrap();
    let mut jpg = std::io::Cursor::new(Vec::new());
    image::RgbImage::from_pixel(32, 18, image::Rgb([200, 40, 80]))
        .write_to(&mut jpg, image::ImageFormat::Jpeg)
        .unwrap();
    std::fs::write(recording.join("Thmbnl/TEST AT01.JPG"), jpg.into_inner()).unwrap();
    std::fs::write(
        recording.join("Clip/TEST BM01.XML"),
        SIDE.replace("ORIGINAL-A", "ORIGINAL-B")
            .replace("500", "100"),
    )
    .unwrap();
    let config = SelectionConfig {
        version: "0.1.0".into(),
        parallelism: 2,
        catalog: local(
            "qnc://local/catalog/camera-patterns",
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../catalogs/camera-patterns/camera-patterns-2026.09.07.1.sqlite"),
        ),
        source_index: local("qnc://local/db/source_index", &dir.path().join("index.db")),
        media_records: local("qnc://local/db/media_records", &dir.path().join("media.db")),
        sources: vec![SourceConfig {
            location: local(SOURCE, &card),
            name: "Card".into(),
            serial_number: String::new(),
            volume_name: String::new(),
            scope: qnc_camera_detector::SourceScope::CardRelative,
            probe: ProbeConfig::Local {
                executable: dir.path().join("never-executed"),
                probe_size_bytes: 8388608,
                analyze_duration_us: 1000000,
            },
        }],
    };
    drop(
        ContentClient::from_owner_binding(
            &dir.path().join("content.db"),
            "qnc://local/db/ingest_content/p1",
            Access::ReadWrite,
        )
        .unwrap(),
    );
    (dir, config)
}

#[test]
fn select_session_cancel_clears_pending_without_joining() {
    let mut session = SelectSession::default();
    let cancel = Arc::new(AtomicBool::new(false));
    let (_send, receive) = mpsc::sync_channel(1);
    session.cancel = Some(cancel.clone());
    session.result = Some(receive);

    session.cancel();

    assert!(cancel.load(Ordering::Relaxed));
    assert!(!session.has_pending_work());
}

struct Backend {
    calls: Arc<AtomicUsize>,
    fail: bool,
    partial: bool,
}
impl ProbeBackend for Backend {
    fn execute(&self, request: &ProbeRequest) -> qnc_media_probe::Result<qnc_media_probe::Report> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(qnc_media_probe::Error::Timeout);
        }
        let a = request.media_uri.contains("TEST%20A");
        let frames = if a { 500 } else { 100 };
        let mut video = json!({"index":0,"codec_type":"video","codec_name":"h264","width":1920,"height":1080,
            "time_base":"1/50","start_pts":0,"duration_ts":frames,"avg_frame_rate":"50/1","r_frame_rate":"50/1","nb_frames":frames.to_string(),
            "field_order":"progressive","pix_fmt":"yuv422p","sample_aspect_ratio":"1:1",
            "color_primaries":"bt709","color_transfer":"bt709","color_space":"bt709","color_range":"tv"});
        if self.partial {
            video.as_object_mut().unwrap().remove("sample_aspect_ratio");
        }
        let json = json!({"streams":[video],"format":{"filename":request.media_uri,
            "format_name": if request.media_uri.contains("/Sub/") { "mov,mp4,m4a,3gp,3g2,mj2" } else {"mxf"},
            "duration":(frames/50).to_string()},"programs":[],"chapters":[]}).to_string();
        Ok(qnc_media_probe::Report {
            request_id: request.request_id.clone(),
            media_uri: request.media_uri.clone(),
            document_uri: request.document_uri.clone(),
            json,
            elapsed_ms: 1,
        })
    }
}
pub(crate) fn execute(
    config: &SelectionConfig,
    calls: &Arc<AtomicUsize>,
    fail: bool,
    partial: bool,
    path: &str,
) -> Vec<Event> {
    let (send, receive) = mpsc::sync_channel(128);
    let result = run_inner(
        config,
        &SourceReference::new(SOURCE, path).unwrap(),
        &send,
        &AtomicBool::new(false),
        |_, _| {
            Ok(Box::new(Backend {
                calls: calls.clone(),
                fail,
                partial,
            }))
        },
        ContentTarget::from_owner_binding(
            &config
                .source_index
                .file
                .as_ref()
                .unwrap()
                .with_file_name("content.db"),
            "qnc://local/db/ingest_content/p1",
        )
        .unwrap(),
    );
    drop(send);
    let events: Vec<_> = receive.into_iter().collect();
    if let Err(error) = result {
        let summary: Vec<_> = events.iter().map(event_summary).collect();
        panic!("select failed: {error}; events: {summary:?}");
    }
    events
}

fn event_summary(event: &Event) -> String {
    match event {
        Event::Status(message) => format!("status:{message}"),
        Event::Clip(clip) => format!(
            "clip:{} rev={} state={:?}",
            clip.clip_id, clip.metadata_revision, clip.save_state
        ),
        Event::Existing(ids) => format!("existing:{}", ids.len()),
        Event::Saved { revisions, error } => {
            format!("saved:{} error={error:?}", revisions.len())
        }
        Event::Removed(ids) => format!("removed:{}", ids.len()),
        Event::Warning(message) => format!("warning:{message}"),
        Event::Finished(result) => format!("finished:{}", result.is_ok()),
    }
}

#[test]
fn select_persists_two_original_proxy_groups_and_reselect_reads_db() {
    let (_dir, config) = fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    let events = execute(&config, &calls, false, false, ".");
    let warnings: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let Event::Warning(w) = e {
                Some(w)
            } else {
                None
            }
        })
        .collect();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    let clips: BTreeSet<_> = events
        .iter()
        .filter_map(|e| {
            if let Event::Clip(c) = e {
                Some(&c.clip_id)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(clips.len(), 2);
    assert!(events.iter().any(|e| matches!(e, Event::Clip(c) if c.thumb_status == crate::SelectThumbStatus::Ready && c.thumb_image.is_some())));
    let mut db = config.media_records.media_db().unwrap();
    for id in clips {
        let saved = db.read(id, None).unwrap().unwrap();
        assert_eq!(saved.phase, Phase::Final);
        assert_eq!(saved.completeness, Completeness::Complete);
        assert!(saved.metadata.proxy.is_some());
        for evidence in saved.metadata.evidence {
            assert!(db.document(&evidence.document_uri).unwrap().is_some());
        }
    }
    let second = execute(&config, &calls, false, false, "PRIVATE/XDROOT/Clip");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        4,
        "subfolder navigation must not change media identity or re-probe"
    );
    assert_eq!(
        second
            .iter()
            .filter(|e| matches!(e, Event::Clip(_)))
            .count(),
        0
    );
    assert!(
        second
            .iter()
            .any(|e| matches!(e, Event::Existing(ids) if ids.len() == 2)),
        "reselect preserves existing thumbnails without decoding them again"
    );
}

#[test]
fn reselect_adds_only_new_clips_removes_confirmed_missing_and_keeps_selection() {
    let (dir, config) = fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    execute(&config, &calls, false, false, ".");
    let mut content = ContentClient::from_owner_binding(
        &dir.path().join("content.db"),
        "qnc://local/db/ingest_content/p1",
        Access::ReadWrite,
    )
    .unwrap();
    let clips = content.list(None).unwrap();
    let a = clips.iter().find(|c| c.clip.name == "TEST A.MXF").unwrap();
    let a_id = a.clip.id().to_string();
    let b_id = clips
        .iter()
        .find(|c| c.clip.name == "TEST B.MXF")
        .unwrap()
        .clip
        .id()
        .to_string();
    content.select(vec![a_id.clone()], true).unwrap();
    let recording = dir.path().join("card/PRIVATE/XDROOT");
    // A fixture camera adds one recording and removes B; the real card is never changed.
    let xml = INDEX
        .replace("TEST B", "TEST C")
        .replace("ORIGINAL-B", "ORIGINAL-C")
        .replace("PROXY-B", "PROXY-C");
    std::fs::write(recording.join("MEDIAPRO.XML"), xml).unwrap();
    std::fs::write(recording.join("Clip/TEST C.MXF"), "fixture only").unwrap();
    std::fs::write(recording.join("Sub/TEST CS03.MP4"), "fixture only").unwrap();
    std::fs::write(
        recording.join("Clip/TEST CM01.XML"),
        SIDE.replace("ORIGINAL-A", "ORIGINAL-C")
            .replace("500", "100"),
    )
    .unwrap();
    std::fs::remove_file(recording.join("Clip/TEST B.MXF")).unwrap();
    let events = execute(&config, &calls, false, false, ".");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        6,
        "only C original/proxy acquired: {events:?}"
    );
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Removed(ids) if ids.contains(&b_id))));
    assert!(events
        .iter()
        .filter_map(|e| if let Event::Clip(c) = e {
            Some(c)
        } else {
            None
        })
        .all(|c| c.name == "TEST C.MXF" && !c.previously_seen));
    let clips = content.list(None).unwrap();
    assert_eq!(clips.len(), 2);
    assert!(clips.iter().find(|c| c.clip.id() == a_id).unwrap().selected);
    assert!(
        recording.join("Sub/TEST BS03.MP4").exists(),
        "no media deletion"
    );
    assert!(
        config
            .media_records
            .media_db()
            .unwrap()
            .read(&b_id, None)
            .unwrap()
            .is_some(),
        "acquisition evidence retained"
    );
    let next = execute(&config, &calls, false, false, ".");
    assert!(!next
        .iter()
        .any(|e| matches!(e, Event::Clip(_) | Event::Saved { .. })));
    assert_eq!(calls.load(Ordering::SeqCst), 6);
}

#[test]
fn preview_arrives_while_publication_is_blocked() {
    let (dir, config) = fixture();
    let path = dir.path().join("content.db");
    drop(
        ContentClient::from_owner_binding(
            &path,
            "qnc://local/db/ingest_content/p1",
            Access::ReadWrite,
        )
        .unwrap(),
    );
    let calls = Arc::new(AtomicUsize::new(0));
    let (send, receive) = mpsc::sync_channel(128);
    std::thread::scope(|scope| {
        let worker = scope.spawn(move || {
            run_inner(
                &config,
                &SourceReference::new(SOURCE, ".").unwrap(),
                &send,
                &AtomicBool::new(false),
                |_, _| {
                    Ok(Box::new(Backend {
                        calls: calls.clone(),
                        fail: false,
                        partial: false,
                    }))
                },
                ContentTarget::from_owner_binding(&path, "qnc://local/db/ingest_content/p1")
                    .unwrap(),
            )
        });
        worker.join().unwrap().unwrap();
        let events = receive.into_iter().collect::<Vec<_>>();
        let preview_index = events.iter().position(|event| {
            matches!(event, Event::Clip(c) if c.save_state == crate::SelectSaveState::Pending)
        });
        let saved_index = events
            .iter()
            .position(|event| matches!(event, Event::Saved { .. }));
        assert!(
            preview_index.is_some() && saved_index.is_some() && preview_index < saved_index,
            "UI sees the clip before its catalog write: {events:?}"
        );
    });
}

#[test]
fn incomplete_scan_and_outside_scope_never_remove_existing_clips() {
    let (dir, config) = fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    execute(&config, &calls, false, false, ".");
    let recording = dir.path().join("card/PRIVATE/XDROOT");
    std::fs::remove_file(recording.join("Clip/TEST B.MXF")).unwrap();
    let outside = execute(&config, &calls, false, false, "PRIVATE/XDROOT/Thmbnl");
    assert!(!outside
        .iter()
        .any(|e| matches!(e, Event::Removed(ids) if !ids.is_empty())));
    std::fs::write(recording.join("MEDIAPRO.XML"), "<broken").unwrap();
    let incomplete = execute(&config, &calls, false, false, ".");
    assert!(incomplete.iter().any(|e| matches!(e, Event::Warning(_))));
    assert!(!incomplete
        .iter()
        .any(|e| matches!(e, Event::Removed(ids) if !ids.is_empty())));
    let mut content = ContentClient::from_owner_binding(
        &dir.path().join("content.db"),
        "qnc://local/db/ingest_content/p1",
        Access::ReadOnly,
    )
    .unwrap();
    assert_eq!(content.list(None).unwrap().len(), 2);
    assert_eq!(calls.load(Ordering::SeqCst), 4);
}

#[test]
fn failed_probe_is_durable_and_never_retried() {
    let (_dir, config) = fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    let first = execute(&config, &calls, true, false, ".");
    assert_eq!(
        first
            .iter()
            .filter(|e| matches!(e, Event::Warning(_)))
            .count(),
        2
    );
    let count = calls.load(Ordering::SeqCst);
    assert_eq!(count, 2);
    execute(&config, &calls, false, false, ".");
    assert_eq!(calls.load(Ordering::SeqCst), count);
}

#[test]
fn partial_final_metadata_is_not_a_reason_for_another_probe() {
    let (_dir, config) = fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    let first = execute(&config, &calls, false, true, ".");
    assert_eq!(
        first
            .iter()
            .filter(|e| matches!(e, Event::Warning(_)))
            .count(),
        2
    );
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    execute(&config, &calls, false, false, ".");
    assert_eq!(calls.load(Ordering::SeqCst), 4);
}

#[test]
fn concurrent_select_jobs_do_not_duplicate_probe_calls() {
    let (_dir, config) = fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    std::thread::scope(|scope| {
        let one = scope.spawn(|| execute(&config, &calls, false, false, "."));
        let two = scope.spawn(|| execute(&config, &calls, false, false, "."));
        one.join().unwrap();
        two.join().unwrap();
    });
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    execute(&config, &calls, false, false, ".");
    assert_eq!(calls.load(Ordering::SeqCst), 4);
}

#[test]
fn select_uses_same_source_and_database_contracts_over_lan_and_intranet() {
    for environment in ["lan", "intranet"] {
        let (dir, mut config) = fixture();
        let content_uri = format!("qnc://{environment}/fixture/db/ingest_content/p1");
        let mut content = qnc_ingest_store::content::ContentStore::open_owner_binding(
            &dir.path().join("content.db"),
            &content_uri,
            Access::ReadWrite,
        )
        .unwrap();
        let published_content = content_uri.clone();
        let source_uri = format!("qnc://{environment}/fixture/source/card-test");
        let index_uri = format!("qnc://{environment}/fixture/db/source_index");
        let media_uri = format!("qnc://{environment}/fixture/db/media_records");
        let local_source =
            qnc_source_reader::LocalSource::new(&source_uri, dir.path().join("card")).unwrap();
        let mut index = qnc_source_index_db::Store::open_owner_binding(
            &dir.path().join("index.db"),
            qnc_source_index_db::Access::ReadWrite,
            true,
        )
        .unwrap();
        let mut media = qnc_media_record_db::Store::open_owner_binding(
            &dir.path().join("media.db"),
            qnc_media_record_db::Access::ReadWrite,
            true,
        )
        .unwrap();
        let credentials =
            qnc_media_record_db::Credentials::new("fixture-read", "fixture-write").unwrap();
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", server.server_addr());
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let (published_index, published_media) = (index_uri.clone(), media_uri.clone());
        let server_thread = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                let Some(request) = server
                    .recv_timeout(std::time::Duration::from_millis(20))
                    .unwrap()
                else {
                    continue;
                };
                match request.url() {
                    qnc_ingest_store::content::ENDPOINT => qnc_ingest_store::content::respond(
                        request,
                        &mut content,
                        &published_content,
                        &credentials,
                    ),
                    qnc_source_reader::ENDPOINT => {
                        qnc_source_reader::server::respond(request, &local_source, "fixture-write")
                    }
                    qnc_source_index_db::ENDPOINT => qnc_source_index_db::respond(
                        request,
                        &mut index,
                        &published_index,
                        &credentials,
                    ),
                    qnc_media_record_db::ENDPOINT => qnc_media_record_db::respond(
                        request,
                        &mut media,
                        &published_media,
                        &credentials,
                    ),
                    _ => {
                        let _ = request.respond(tiny_http::Response::empty(404));
                    }
                }
            }
        });
        let variable = format!("QNC_SELECT_TEST_TOKEN_{environment}");
        std::env::set_var(&variable, "fixture-write");
        let remote = |uri: String| Binding {
            uri,
            file: None,
            endpoint: Some(endpoint.clone()),
            token_env: Some(variable.clone()),
        };
        config.sources[0].location = remote(source_uri.clone());
        config.source_index = remote(index_uri);
        config.media_records = remote(media_uri);
        let result = std::panic::catch_unwind(|| {
            let mut browser = config.browser().unwrap();
            assert_eq!(browser.roots(environment).unwrap().entries.len(), 1);
            assert!(!browser.open(&source_uri).unwrap().entries.is_empty());
            let calls = Arc::new(AtomicUsize::new(0));
            for _ in 0..2 {
                let (send, receive) = mpsc::sync_channel(128);
                run_inner(
                    &config,
                    &SourceReference::new(&source_uri, ".").unwrap(),
                    &send,
                    &AtomicBool::new(false),
                    |_, _| {
                        Ok(Box::new(Backend {
                            calls: calls.clone(),
                            fail: false,
                            partial: false,
                        }))
                    },
                    ContentTarget::from_remote_binding(
                        if environment == "lan" {
                            qnc_transport_resolver::ResolverConfig::new(dir.path())
                                .with_lan_authority("fixture", &endpoint)
                        } else {
                            qnc_transport_resolver::ResolverConfig::new(dir.path())
                                .with_intranet_authority("fixture", &endpoint)
                        },
                        &content_uri,
                        "fixture-write".to_string(),
                    )
                    .unwrap(),
                )
                .unwrap();
                drop(send);
                let warnings: Vec<_> = receive
                    .into_iter()
                    .filter_map(|e| {
                        if let Event::Warning(w) = e {
                            Some(w)
                        } else {
                            None
                        }
                    })
                    .collect();
                assert!(warnings.is_empty(), "{environment}: {warnings:?}");
            }
            assert_eq!(calls.load(Ordering::SeqCst), 4);
        });
        stop.store(true, Ordering::Relaxed);
        server_thread.join().unwrap();
        std::env::remove_var(variable);
        result.unwrap();
    }
}

#[test]
fn mounted_lan_and_intranet_sources_use_qnc_uri_with_private_host_binding() {
    for environment in ["lan", "intranet"] {
        let (dir, mut config) = fixture();
        let source_uri = format!("qnc://{environment}/fixture/source/card-test");
        config.sources[0].location = Binding {
            uri: source_uri.clone(),
            file: Some(dir.path().join("card")),
            endpoint: None,
            token_env: None,
        };

        let resolved = config.sources[0]
            .location
            .resolver()
            .unwrap()
            .resolve(&source_uri)
            .unwrap();

        assert_eq!(resolved.environment, environment);
        assert_eq!(resolved.authority.as_deref(), Some("fixture"));
        assert!(matches!(
            resolved.endpoint,
            qnc_transport_resolver::ResolvedEndpoint::LocalPath(_)
        ));
        assert!(!config
            .browser()
            .unwrap()
            .roots(environment)
            .unwrap()
            .entries
            .is_empty());
    }
}
