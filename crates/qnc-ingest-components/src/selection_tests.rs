use super::*;
use crate::selection_config::{Binding, ProbeConfig, SourceConfig};
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

fn fixture() -> (tempfile::TempDir, SelectionConfig) {
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
    (dir, config)
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
fn execute(
    config: &SelectionConfig,
    calls: &Arc<AtomicUsize>,
    fail: bool,
    partial: bool,
    path: &str,
) -> Vec<Event> {
    let (send, receive) = mpsc::sync_channel(128);
    run_inner(
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
    )
    .unwrap();
    drop(send);
    receive.into_iter().collect()
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
    assert!(events.iter().any(|e| matches!(e, Event::Clip(c) if c.thumb_status == crate::ThumbStatus::Ready && c.thumb_image.is_some())));
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
        2
    );
    assert!(
        second
            .iter()
            .any(|e| matches!(e, Event::Clip(c) if c.thumb_image.is_some())),
        "thumbnail reload from persisted source links"
    );
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
