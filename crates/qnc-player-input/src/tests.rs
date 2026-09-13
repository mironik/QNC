#![cfg(test)]
use super::*;
use qnc_ingest_store::content::{
    self, Access, CatalogClip, ContentClient, ContentStore, ContentTarget, Credentials,
    ImportStatus, StoredClip, content_uri,
};
use qnc_media_metadata as m;
use qnc_media_records::{Binding, Completeness};
use qnc_transport_resolver::ResolverConfig;
use qnc_work_settings::{ReaderConfig, StoragePolicy};
use rusqlite::Connection;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

impl PlayerClipSource for StoredClip {
    fn snapshot(&self) -> &Snapshot {
        &self.clip.snapshot
    }

    fn imported_media_uri(&self) -> Option<&String> {
        self.imported_media_uri.as_ref()
    }

    fn validate_clip(&self) -> Result<()> {
        self.clip.validate().map_err(InputError::InvalidRecord)
    }
}

#[derive(Clone)]
struct StorePlayerContentReader {
    target: ContentTarget,
}

impl PlayerContentRead for StorePlayerContentReader {
    fn read_clip(&self, clip_id: &str) -> std::result::Result<Option<PlayerClipRecord>, String> {
        let stored = self.target.open(Access::ReadOnly)?.read(clip_id)?;
        Ok(stored.map(|stored| PlayerClipRecord {
            name: stored.clip.name,
            snapshot: stored.clip.snapshot,
            imported_media_uri: stored.imported_media_uri,
        }))
    }
}

fn fact<T>(value: T, id: &str) -> Option<m::Fact<T>> {
    Some(m::Fact {
        value,
        evidence_id: id.into(),
        locator: "/saved".into(),
    })
}
fn media(context: &str, proxy: bool) -> m::MediaRepresentation {
    let id = if proxy { "proxy" } else { "original" };
    let f = |v| fact(m::Signal::Known(String::from(v)), id);
    let mut streams = vec![m::MediaStream {
        index: fact(0, id),
        codec: f("h264"),
        profile: None,
        time_base: fact(
            m::Rational {
                numerator: 1,
                denominator: if proxy { 30_000 } else { 60_000 },
            },
            id,
        ),
        start_pts: fact(0, id),
        duration_ts: fact(if proxy { 30_030 } else { 60_060 }, id),
        details: m::StreamDetails::Video(Box::new(m::VideoMetadata {
            width: fact(if proxy { 960 } else { 1920 }, id),
            height: fact(if proxy { 540 } else { 1080 }, id),
            frame_rate: fact(FrameTimebase::new(30_000, 1001).unwrap(), id),
            frame_rate_mode: fact(m::FrameRateMode::Constant, id),
            frame_count: fact(m::FrameCount::Exact(30), id),
            scan_mode: fact(m::ScanMode::Progressive, id),
            pixel_format: fact(if proxy { "yuv420p" } else { "yuv422p10le" }.into(), id),
            sample_aspect_ratio: fact(
                m::Rational {
                    numerator: 1,
                    denominator: 1,
                },
                id,
            ),
            rotation_degrees: fact(m::Signal::Unspecified, id),
            color: m::ColorMetadata {
                primaries: f("bt709"),
                transfer: f("bt709"),
                matrix: f("bt709"),
                range: f("tv"),
            },
        })),
    }];
    for index in if proxy { 1..=1 } else { 1..=4 } {
        streams.push(m::MediaStream {
            index: fact(index, id),
            codec: f(if proxy { "aac" } else { "pcm_s24le" }),
            profile: None,
            time_base: fact(
                m::Rational {
                    numerator: 1,
                    denominator: 48_000,
                },
                id,
            ),
            start_pts: fact(0, id),
            duration_ts: fact(48_048, id),
            details: m::StreamDetails::Audio(Box::new(m::AudioMetadata {
                sample_rate_hz: fact(48_000, id),
                channels: fact(if proxy { 2 } else { 1 }, id),
                sample_format: f(if proxy { "fltp" } else { "s32" }),
                channel_layout: if proxy {
                    f("stereo")
                } else {
                    fact(m::Signal::Unspecified, id)
                },
                bits_per_sample: if proxy { None } else { fact(24, id) },
            })),
        });
    }
    m::MediaRepresentation {
        media_uri: format!("{context}/source/card/file/{id}"),
        container: fact(if proxy { "mp4" } else { "mxf" }.into(), id),
        duration_seconds: fact(
            m::Rational {
                numerator: 1001,
                denominator: 1000,
            },
            id,
        ),
        streams_complete: fact(true, id),
        streams,
        tags: BTreeMap::new(),
    }
}
fn stored(context: &str) -> StoredClip {
    let original = media(context, false);
    let proxy = media(context, true);
    let metadata = m::ClipMetadata {
        contract_id: m::CONTRACT_ID.into(),
        contract_version: m::CONTRACT_VERSION.into(),
        clip_id: "c1".into(),
        evidence: [("original", &original), ("proxy", &proxy)]
            .map(|(id, media)| m::Evidence {
                id: id.into(),
                kind: m::EvidenceKind::CameraMetadata,
                document_uri: format!("{context}/source/card/index.xml"),
                media_uri: media.media_uri.clone(),
            })
            .into(),
        original,
        proxy: Some(proxy),
    };
    let mut result = StoredClip {
        clip: CatalogClip {
            name: "Example clip".into(),
            source_uri: format!("{context}/source/card"),
            source_name: "Camera".into(),
            serial_number: "card-123".into(),
            volume_name: "CARD".into(),
            thumbnail_uri: None,
            media_records_uri: format!("{context}/db/media_records"),
            snapshot: Snapshot {
                binding: Binding {
                    source_index_uri: format!("{context}/db/source_index"),
                    source_record_id: "r1".into(),
                    original_uri: metadata.original.media_uri.clone(),
                    proxy_uri: metadata.proxy.as_ref().map(|m| m.media_uri.clone()),
                },
                revision: 1,
                phase: Phase::Final,
                completeness: Completeness::Complete,
                report: m::inspect(&metadata),
                metadata,
                recorded_at_unix_ms: 1,
            },
        },
        selected: false,
        import_status: ImportStatus::Detected,
        import_error: None,
        imported_media_uri: None,
    };
    refresh(&mut result);
    assert!(
        result.clip.snapshot.report.is_complete(),
        "{:?}",
        result.clip.snapshot.report
    );
    result
}
fn refresh(stored: &mut StoredClip) {
    let snapshot = &mut stored.clip.snapshot;
    snapshot.report = m::inspect(&snapshot.metadata);
    snapshot.completeness = qnc_media_records::completeness(&snapshot.report);
}
fn without_proxy(stored: &mut StoredClip) {
    let snapshot = &mut stored.clip.snapshot;
    snapshot.metadata.proxy = None;
    snapshot.metadata.evidence.retain(|e| e.id == "original");
    snapshot.binding.proxy_uri = None;
    refresh(stored);
}
fn settings(context: &str, mode: &str) -> WorkSettings {
    WorkSettings {
        contract_version: qnc_work_settings::VERSION.into(),
        project_id: "p1".into(),
        project_name: "Test".into(),
        workspace_db_uri: format!("{context}/db/project_workspace/p1"),
        output_root_uri: format!("{context}/project/p1"),
        storage: StoragePolicy {
            ingest_profile: "field".into(),
            ingest_media: "link".into(),
            proxy_policy: "link_when_available".into(),
            original_policy: "ignore_for_fast_news".into(),
        },
        input: serde_json::json!({"mode":"auto"}),
        playback: serde_json::json!({"input":mode}),
        video: serde_json::json!({"fps":50}),
        audio: serde_json::json!({"sample_rate":44100,"channels":2}),
        ai: serde_json::json!({"enabled":false}),
        keyboard_shortcuts: serde_json::json!({"active_preset":"qnc"}),
    }
}

#[test]
fn policy_uses_saved_playback_input_not_storage_or_project_format() {
    let clip = stored("qnc://local");
    for (mode, variant, channels) in [
        ("original", Representation::Original, 4),
        ("proxy", Representation::Proxy, 4),
        ("proxy_if_available", Representation::Proxy, 4),
    ] {
        let settings = settings("qnc://local", mode);
        let input = prepare(&settings, &clip).unwrap();
        assert_eq!(input.representation, variant);
        assert_eq!(input.layout.audio_channels.len(), channels);
        assert_eq!(
            input.project_audio,
            ProjectAudio {
                channels: 2,
                sample_rate_hz: 44100
            }
        );
        assert_eq!(input.audio_media(), &clip.clip.snapshot.metadata.original);
        assert_eq!(
            input.layout.video.as_ref().unwrap().timebase,
            FrameTimebase::new(30_000, 1001).unwrap()
        );
        input
            .validate_for(&settings.workspace_db_uri, "c1")
            .unwrap();
        let expected = if variant == Representation::Proxy {
            clip.clip.snapshot.metadata.proxy.as_ref().unwrap()
        } else {
            &clip.clip.snapshot.metadata.original
        };
        assert_eq!(
            input.media().unwrap(),
            expected,
            "full saved metadata is preserved exactly"
        );
    }
}
#[test]
fn missing_proxy_is_not_a_second_clip_or_implicit_fallback() {
    let mut clip = stored("qnc://local");
    without_proxy(&mut clip);
    assert_eq!(
        prepare(&settings("qnc://local", "proxy"), &clip),
        Err(InputError::MissingProxy)
    );
    assert_eq!(
        prepare(&settings("qnc://local", "proxy_if_available"), &clip)
            .unwrap()
            .representation,
        Representation::Original
    );
}
#[test]
fn every_native_audio_channel_is_preserved_in_stream_order() {
    let mut clip = stored("qnc://local");
    clip.clip.snapshot.metadata.original.streams.reverse();
    refresh(&mut clip);
    let input = prepare(&settings("qnc://local", "original"), &clip).unwrap();
    assert_eq!(
        input.layout.audio_channels,
        (1..=4)
            .map(|i| AudioChannel {
                stream_index: i,
                channel_index: 0
            })
            .collect::<Vec<_>>()
    );
    let input = prepare(&settings("qnc://local", "proxy"), &clip).unwrap();
    assert_eq!(
        input.layout.audio_channels,
        (1..=4)
            .map(|stream_index| AudioChannel {
                stream_index,
                channel_index: 0
            })
            .collect::<Vec<_>>()
    );
    let m::StreamDetails::Audio(audio) = &input.media().unwrap().streams[1].details else {
        panic!()
    };
    assert_eq!(
        audio.bits_per_sample, None,
        "AAC is not PCM24 from the original"
    );
    assert!(
        input
            .audio_media()
            .streams
            .iter()
            .filter_map(|s| match &s.details {
                m::StreamDetails::Audio(a) => Some(a),
                _ => None,
            })
            .all(|a| a.channels.as_ref().unwrap().value == 1
                && a.bits_per_sample.as_ref().unwrap().value == 24)
    );
}

#[test]
fn two_original_mono_channels_stay_two_in_every_transport_scope() {
    for scope in ["qnc://local", "qnc://lan/server", "qnc://intranet/server"] {
        let mut clip = stored(scope);
        clip.clip.snapshot.metadata.original.streams.truncate(3);
        refresh(&mut clip);
        let input = prepare(&settings(scope, "proxy"), &clip).unwrap();
        assert_eq!(
            input.layout.audio_channels,
            vec![
                AudioChannel {
                    stream_index: 1,
                    channel_index: 0
                },
                AudioChannel {
                    stream_index: 2,
                    channel_index: 0
                },
            ]
        );
        assert_eq!(input.audio_media(), &clip.clip.snapshot.metadata.original);
        input.validate_for(&input.workspace_db_uri, "c1").unwrap();
    }
}

#[test]
fn proxy_picture_cannot_hide_incomplete_original_audio_or_mismatched_timing() {
    let mut clip = stored("qnc://local");
    let m::StreamDetails::Audio(a) = &mut clip.clip.snapshot.metadata.original.streams[1].details
    else {
        panic!()
    };
    a.sample_rate_hz = None;
    refresh(&mut clip);
    assert!(matches!(
        prepare(&settings("qnc://local", "proxy"), &clip),
        Err(InputError::IncompleteMedia(_))
    ));

    let mut clip = stored("qnc://local");
    let m::StreamDetails::Video(v) =
        &mut clip.clip.snapshot.metadata.proxy.as_mut().unwrap().streams[0].details
    else {
        panic!()
    };
    v.frame_count = fact(m::FrameCount::Exact(29), "proxy");
    refresh(&mut clip);
    assert!(prepare(&settings("qnc://local", "proxy"), &clip).is_err());
}

#[test]
fn original_mono_and_proxy_picture_keep_their_own_pts_and_saved_rate_evidence() {
    for scope in ["qnc://local", "qnc://lan/server", "qnc://intranet/server"] {
        let mut clip = stored(scope);
        clip.clip.snapshot.metadata.original.streams[0].start_pts = fact(60000, "original");
        let proxy = &mut clip.clip.snapshot.metadata.proxy.as_mut().unwrap().streams[0];
        proxy.start_pts = fact(90000, "proxy");
        let m::StreamDetails::Video(v) = &mut proxy.details else {
            panic!()
        };
        v.frame_rate_mode = fact(m::FrameRateMode::Unknown, "proxy");
        refresh(&mut clip);
        let input = prepare(&settings(scope, "proxy"), &clip).unwrap();
        assert_eq!(input.layout.audio_channels.len(), 4);
        assert_eq!(
            input.audio_media().streams[0]
                .start_pts
                .as_ref()
                .unwrap()
                .value,
            60000
        );
        assert_eq!(
            input.media().unwrap().streams[0]
                .start_pts
                .as_ref()
                .unwrap()
                .value,
            90000
        );
        assert_eq!(
            input.layout.video.as_ref().unwrap().frame_rate_mode,
            m::FrameRateMode::Unknown
        );
        input.validate_for(&input.workspace_db_uri, "c1").unwrap();
    }
}
#[test]
fn missing_or_invalid_settings_have_no_default() {
    for mode in ["", "automatic", "ORIGINAL"] {
        assert!(matches!(
            prepare(&settings("qnc://local", mode), &stored("qnc://local")),
            Err(InputError::Settings(_))
        ));
    }
}

#[test]
fn project_audio_is_required_and_does_not_rewrite_native_inventory() {
    let clip = stored("qnc://local");
    for channels in [1, 2, 4] {
        let mut s = settings("qnc://local", "proxy");
        s.audio["channels"] = serde_json::json!(channels);
        let input = prepare(&s, &clip).unwrap();
        assert_eq!(input.project_audio.channels, channels);
        assert_eq!(input.layout.audio_channels.len(), 4);
        assert_eq!(input.audio_media(), &clip.clip.snapshot.metadata.original);
    }
    for bad in [
        serde_json::Value::Null,
        serde_json::json!("2"),
        serde_json::json!(0),
        serde_json::json!(-1),
        serde_json::json!(65),
    ] {
        let mut s = settings("qnc://local", "proxy");
        s.audio["channels"] = bad;
        assert!(matches!(prepare(&s, &clip), Err(InputError::Settings(_))));
    }
    let input = prepare(&settings("qnc://local", "proxy"), &clip).unwrap();
    let mut wire = serde_json::to_value(&input).unwrap();
    wire.as_object_mut().unwrap().remove("project_audio");
    assert!(serde_json::from_value::<PreparedInput>(wire).is_err());
    let mut invalid = input;
    invalid.project_audio.sample_rate_hz = 0;
    assert!(
        invalid
            .validate_for(&invalid.workspace_db_uri, "c1")
            .is_err()
    );
}

#[test]
fn reader_applies_audio_changes_from_database_without_project_process_or_json() {
    let f = Fixture::new("qnc://local");
    let reader = f.local_reader();
    let uri = "qnc://local/db/project_workspace/p1";
    assert_eq!(reader.load(uri, "c1").unwrap().project_audio.channels, 2);
    // Emulate an owner's saved settings change, not a consumer write.
    let db = Connection::open(&f.db).unwrap();
    let raw: String = db
        .query_row("SELECT settings_json FROM project_settings", [], |r| {
            r.get(0)
        })
        .unwrap();
    let mut saved: serde_json::Value = serde_json::from_str(&raw).unwrap();
    saved["audio"]["channels"] = serde_json::json!(4);
    saved["audio"]["sample_rate"] = serde_json::json!(48000);
    db.execute(
        "UPDATE project_settings SET settings_json=?1",
        [saved.to_string()],
    )
    .unwrap();
    drop(db);
    let before = std::fs::read(&f.db).unwrap();
    let input = reader.load(uri, "c1").unwrap();
    assert_eq!(
        input.project_audio,
        ProjectAudio {
            channels: 4,
            sample_rate_hz: 48000
        }
    );
    assert_eq!(std::fs::read(&f.db).unwrap(), before);
}

#[test]
fn relocated_import_without_representation_binding_is_not_guessed() {
    let mut clip = stored("qnc://local");
    clip.import_status = ImportStatus::Imported;
    clip.imported_media_uri = Some("qnc://local/project/p1/proxy/copied.mp4".into());
    assert!(matches!(
        prepare(&settings("qnc://local", "proxy"), &clip),
        Err(InputError::UnsupportedMedia(_))
    ));
    clip.imported_media_uri = clip.clip.snapshot.binding.proxy_uri.clone();
    assert!(prepare(&settings("qnc://local", "proxy"), &clip).is_ok());
}
#[test]
fn unfinished_missing_and_estimated_metadata_never_trigger_repair() {
    let settings = settings("qnc://local", "proxy");
    let mut clip = stored("qnc://local");
    clip.clip.snapshot.phase = Phase::Camera;
    assert_eq!(prepare(&settings, &clip), Err(InputError::NotFinal));
    clip.clip.snapshot.phase = Phase::Final;
    clip.clip
        .snapshot
        .metadata
        .proxy
        .as_mut()
        .unwrap()
        .container = None;
    refresh(&mut clip);
    assert!(matches!(
        prepare(&settings, &clip),
        Err(InputError::IncompleteMedia(_))
    ));
    assert!(
        prepare(&super::tests::settings("qnc://local", "original"), &clip).is_ok(),
        "unused incomplete proxy does not invalidate complete original"
    );
    let mut clip = stored("qnc://local");
    let m::StreamDetails::Video(v) =
        &mut clip.clip.snapshot.metadata.proxy.as_mut().unwrap().streams[0].details
    else {
        panic!()
    };
    v.frame_count = fact(m::FrameCount::Estimated(30), "proxy");
    refresh(&mut clip);
    assert!(matches!(
        prepare(&settings, &clip),
        Err(InputError::UnsupportedMedia(_))
    ));
}
#[test]
fn unknown_frame_rate_mode_remains_unknown_and_audio_only_gets_no_fps() {
    let mut clip = stored("qnc://local");
    without_proxy(&mut clip);
    let m::StreamDetails::Video(v) = &mut clip.clip.snapshot.metadata.original.streams[0].details
    else {
        panic!()
    };
    v.frame_rate_mode = fact(m::FrameRateMode::Unknown, "original");
    refresh(&mut clip);
    let input = prepare(&settings("qnc://local", "original"), &clip).unwrap();
    assert_eq!(
        input.layout.video.unwrap().frame_rate_mode,
        m::FrameRateMode::Unknown
    );
    clip.clip.snapshot.metadata.original.streams.remove(0);
    refresh(&mut clip);
    let input = prepare(&settings("qnc://local", "original"), &clip).unwrap();
    assert_eq!(input.layout.video, None);
    assert_eq!(input.layout.audio_channels.len(), 4);
}
#[test]
fn ambiguous_video_and_oversized_audio_require_explicit_support() {
    let mut clip = stored("qnc://local");
    without_proxy(&mut clip);
    let mut video = clip.clip.snapshot.metadata.original.streams[0].clone();
    video.index = fact(9, "original");
    clip.clip.snapshot.metadata.original.streams.push(video);
    refresh(&mut clip);
    assert!(matches!(
        prepare(&settings("qnc://local", "original"), &clip),
        Err(InputError::UnsupportedMedia(_))
    ));
    clip.clip.snapshot.metadata.original.streams.pop();
    let m::StreamDetails::Audio(a) = &mut clip.clip.snapshot.metadata.original.streams[1].details
    else {
        panic!()
    };
    a.channels = fact(u32::MAX, "original");
    refresh(&mut clip);
    assert!(matches!(
        prepare(&settings("qnc://local", "original"), &clip),
        Err(InputError::UnsupportedMedia(_))
    ));
}
#[test]
fn wire_descriptor_rejects_wrong_context_and_tampered_projection() {
    let input = prepare(&settings("qnc://local", "proxy"), &stored("qnc://local")).unwrap();
    let encoded = serde_json::to_string(&input).unwrap();
    let roundtrip: PreparedInput = serde_json::from_str(&encoded).unwrap();
    assert_eq!(input, roundtrip);
    assert!(
        input
            .validate_for("qnc://local/db/project_workspace/p2", "c1")
            .is_err()
    );
    assert!(input.validate_for(&input.workspace_db_uri, "c2").is_err());
    for change in 0..4 {
        let mut bad = input.clone();
        match change {
            0 => bad.contract_version = "9".into(),
            1 => bad.layout.audio_channels.clear(),
            2 => bad.layout.video.as_mut().unwrap().timebase.fps_num = 25,
            _ => bad.representation = Representation::Original,
        }
        assert!(bad.validate_for(&input.workspace_db_uri, "c1").is_err());
    }
}

struct Fixture {
    _dir: tempfile::TempDir,
    registry: PathBuf,
    db: PathBuf,
    content_uri: String,
}
impl Fixture {
    fn new(context: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let registry = dir.path().join("registry.db");
        let db = dir.path().join("qnc_project.db");
        let s = settings(context, "proxy_if_available");
        let content_uri = content_uri(&s.workspace_db_uri).unwrap();
        let saved = serde_json::json!({"storage":s.storage,"input":s.input,"playback":s.playback,"video":s.video,"audio":s.audio,"ai":s.ai,"keyboard_shortcuts":s.keyboard_shortcuts});
        let c = Connection::open(&db).unwrap();
        c.execute_batch("CREATE TABLE project_settings(project_id TEXT,settings_json TEXT); CREATE VIEW public_project_settings AS SELECT * FROM project_settings;").unwrap();
        c.execute(
            "INSERT INTO project_settings VALUES('p1',?1)",
            [saved.to_string()],
        )
        .unwrap();
        drop(c);
        let c = Connection::open(&registry).unwrap();
        c.execute_batch("CREATE TABLE projects(project_id TEXT,name TEXT,project_uri TEXT); CREATE VIEW public_projects AS SELECT * FROM projects;
            CREATE TABLE app_settings(key TEXT,value TEXT); CREATE VIEW public_app_settings AS SELECT * FROM app_settings;
            INSERT INTO app_settings VALUES('active_project_id','p1'); CREATE TABLE project_storage_locations(project_id TEXT,local_path TEXT);").unwrap();
        c.execute(
            "INSERT INTO projects VALUES('p1','Test',?1)",
            [&s.output_root_uri],
        )
        .unwrap();
        c.execute(
            "INSERT INTO project_storage_locations VALUES('p1',?1)",
            [dir.path().to_str().unwrap()],
        )
        .unwrap();
        drop(c);
        let mut writer =
            ContentClient::from_owner_binding(&db, &content_uri, Access::ReadWrite).unwrap();
        writer.publish(stored(context).clip).unwrap();
        drop(writer);
        Self {
            _dir: dir,
            registry,
            db,
            content_uri,
        }
    }

    fn local_reader(&self) -> InputReader {
        let target = ContentTarget::from_owner_binding(&self.db, &self.content_uri).unwrap();
        InputReader::with_content_reader(
            SettingsReader::local(&self.registry),
            Arc::new(StorePlayerContentReader { target }),
        )
    }
}
#[test]
fn real_public_local_reader_does_not_change_database_or_selection() {
    let f = Fixture::new("qnc://local");
    let before = (
        std::fs::read(&f.registry).unwrap(),
        std::fs::read(&f.db).unwrap(),
    );
    let reader = f.local_reader();
    let uri = "qnc://local/db/project_workspace/p1";
    assert_eq!(
        reader.load(uri, "c1").unwrap().representation,
        Representation::Proxy
    );
    assert_eq!(reader.load(uri, "absent"), Err(InputError::MissingClip));
    assert_eq!(
        reader.load("qnc://local/db/project_workspace/p2", "c1"),
        Err(InputError::WrongWorkspace)
    );
    assert!(reader.load("C:\\raw\\db", "c1").is_err());
    assert_eq!(
        before,
        (
            std::fs::read(&f.registry).unwrap(),
            std::fs::read(&f.db).unwrap()
        )
    );
    Connection::open(&f.registry)
        .unwrap()
        .execute("DELETE FROM app_settings", [])
        .unwrap();
    assert!(matches!(
        reader.load(uri, "c1"),
        Err(InputError::Settings(_))
    ));
}

fn network(environment: &str, changed: bool) {
    let context = format!("qnc://{environment}/test-storage");
    let f = Fixture::new(&context);
    let before = std::fs::read(&f.db).unwrap();
    let (key, token) = ["USERNAME", "USER"]
        .into_iter()
        .find_map(|k| std::env::var(k).ok().map(|v| (k, v)))
        .expect("test login name as non-secret loopback credential");
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", server.server_addr());
    let registry_uri = format!("{context}/db/project_registry");
    let config = ReaderConfig {
        registry_uri: registry_uri.clone(),
        registry_file: None,
        endpoint: Some(endpoint),
        token_env: Some(key.into()),
    };
    let db_uri = format!("{context}/db/ingest_content/p1");
    let authority = "test-storage";
    let resolver = if environment == "lan" {
        ResolverConfig::new(PathBuf::new())
            .with_lan_authority(authority, config.endpoint.clone().unwrap())
    } else {
        ResolverConfig::new(PathBuf::new())
            .with_intranet_authority(authority, config.endpoint.clone().unwrap())
    };
    let content_reader = Arc::new(StorePlayerContentReader {
        target: ContentTarget::from_remote_binding(resolver, &db_uri, token.clone()).unwrap(),
    });
    let db_file = f.db.clone();
    let registry_file = f.registry.clone();
    let handle = std::thread::spawn(move || {
        let mut db = ContentStore::open_owner_binding(&db_file, &db_uri, Access::ReadOnly).unwrap();
        let credentials = Credentials::new(&token, "test-writer-not-used").unwrap();
        for expected in [
            qnc_work_settings::ENDPOINT,
            content::ENDPOINT,
            qnc_work_settings::ENDPOINT,
        ] {
            let request = server
                .recv_timeout(Duration::from_secs(10))
                .unwrap()
                .expect("reader request");
            assert_eq!(request.url(), expected);
            if expected == content::ENDPOINT {
                content::respond(request, &mut db, &db_uri, &credentials);
                if changed {
                    Connection::open(&registry_file)
                        .unwrap()
                        .execute("UPDATE projects SET name='Changed'", [])
                        .unwrap();
                }
            } else {
                qnc_work_settings::server::respond(request, &registry_file, &registry_uri, &token);
            }
        }
    });
    let result = InputReader::with_content_reader(
        SettingsReader::from_config(config).unwrap(),
        content_reader,
    )
    .load(&format!("{context}/db/project_workspace/p1"), "c1");
    handle.join().unwrap();
    if changed {
        assert_eq!(result, Err(InputError::ChangedSettings));
    } else {
        let input = result.unwrap();
        assert_eq!(input.representation, Representation::Proxy);
        assert_eq!(
            input.project_audio,
            ProjectAudio {
                channels: 2,
                sample_rate_hz: 44100
            }
        );
        assert!(input.media().unwrap().media_uri.starts_with(&context));
    }
    assert_eq!(before, std::fs::read(&f.db).unwrap());
}
#[test]
fn lan_reads_saved_settings_and_one_clip_through_public_endpoints() {
    network("lan", false);
}
#[test]
fn intranet_uses_the_same_input_contract() {
    network("intranet", false);
}
#[test]
fn settings_change_during_read_is_not_accepted() {
    network("lan", true);
}
