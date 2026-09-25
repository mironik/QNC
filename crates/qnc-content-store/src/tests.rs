use super::*;
use qnc_media_metadata as m;
use qnc_media_records::{Binding, Completeness};
use rusqlite::Connection;
use std::collections::BTreeMap;

const URI: &str = "qnc://local/db/ingest_content/p1";

#[test]
fn read_one_clip_is_read_only_and_schema_version_is_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("content.db");
    let expected = {
        let mut writer = ContentClient::from_owner_binding(&file, URI, Access::ReadWrite).unwrap();
        writer.publish(clip("c1")).unwrap()
    };
    let before = std::fs::read(&file).unwrap();
    let mut reader = ContentClient::from_owner_binding(&file, URI, Access::ReadOnly).unwrap();
    assert_eq!(reader.read("c1").unwrap(), Some(expected));
    assert_eq!(reader.read("missing").unwrap(), None);
    assert!(reader.read("c1' OR 1=1").is_err());
    assert!(reader.read("../c1").is_err());
    assert!(reader.select(vec!["c1".into()], true).is_err());
    drop(reader);
    assert_eq!(before, std::fs::read(&file).unwrap());
    let db = Connection::open_with_flags(file, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let schema: String = db
        .query_row("SELECT version FROM ingest_content_schema", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(schema, SCHEMA_VERSION);
    assert_ne!(
        schema, VERSION,
        "transport extension does not change stored schema"
    );
}

#[test]
fn list_summary_reads_catalog_columns_without_loading_snapshot_json() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("content.db");
    let mut source = clip("c1");
    source.thumbnail_uri = Some("qnc://local/source/card/thumbs/c1.jpg".into());
    {
        let mut writer = ContentClient::from_owner_binding(&file, URI, Access::ReadWrite).unwrap();
        writer.publish(source).unwrap();
    }
    let db = Connection::open(&file).unwrap();
    db.execute(
        "UPDATE clips SET catalog_json='not valid json' WHERE clip_id='c1'",
        [],
    )
    .unwrap();
    drop(db);

    let before = std::fs::read(&file).unwrap();
    let mut reader = ContentClient::from_owner_binding(&file, URI, Access::ReadOnly).unwrap();
    let rows = reader.list_summary(None).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].clip_id, "c1");
    assert_eq!(rows[0].name, "c1");
    assert_eq!(rows[0].source_name, "Card");
    assert_eq!(rows[0].serial_number, "serial-123");
    assert_eq!(rows[0].duration_seconds, 10.0);
    assert_eq!(
        rows[0].thumbnail_uri.as_deref(),
        Some("qnc://local/source/card/thumbs/c1.jpg")
    );
    assert!(reader.list(None).is_err());
    drop(reader);
    assert_eq!(before, std::fs::read(&file).unwrap());
}

#[test]
fn stats_detects_catalog_changes_without_loading_snapshot_json() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("content.db");
    {
        let mut writer = ContentClient::from_owner_binding(&file, URI, Access::ReadWrite).unwrap();
        writer.publish(clip("c1")).unwrap();
    }
    let db = Connection::open(&file).unwrap();
    db.execute(
        "UPDATE clips SET catalog_json='not valid json' WHERE clip_id='c1'",
        [],
    )
    .unwrap();
    drop(db);

    let before = std::fs::read(&file).unwrap();
    let mut reader = ContentClient::from_owner_binding(&file, URI, Access::ReadOnly).unwrap();
    let initial = reader.stats().unwrap();
    assert_eq!(initial.clip_count, 1);
    assert_eq!(initial.selected_count, 0);
    assert_eq!(initial.max_revision, 1);
    assert!(reader.list(None).is_err());
    drop(reader);
    assert_eq!(before, std::fs::read(&file).unwrap());

    let mut writer = ContentClient::from_owner_binding(&file, URI, Access::ReadWrite).unwrap();
    writer.select(vec!["c1".into()], true).unwrap();
    let selected = writer.stats().unwrap();
    assert_eq!(selected.clip_count, 1);
    assert_eq!(selected.selected_count, 1);
    assert_ne!(selected.fingerprint, initial.fingerprint);
    writer.publish(clip("c2")).unwrap();
    let added = writer.stats().unwrap();
    assert_eq!(added.clip_count, 2);
    assert_ne!(added.fingerprint, selected.fingerprint);
}

#[test]
fn read_rejects_wrong_remote_clip_version_and_database_without_fallback() {
    for case in 0..4 {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", server.server_addr());
        let uri = "qnc://lan/test/db/ingest_content/p1";
        let thread = std::thread::spawn(move || {
            let mut request = server
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
                .unwrap();
            let body: Request = serde_json::from_reader(request.as_reader()).unwrap();
            assert!(matches!(body.operation, Operation::Read { .. }));
            let reply = Reply {
                version: if case == 0 { "old" } else { VERSION }.into(),
                db_uri: if case == 1 {
                    "qnc://lan/test/db/ingest_content/p2"
                } else {
                    uri
                }
                .into(),
                result: Ok(if case == 3 {
                    Data::Clips(vec![])
                } else {
                    Data::Clip(Some(Box::new(StoredClip {
                        clip: clip(if case == 2 { "wrong-clip" } else { "c1" }),
                        selected: false,
                        import_status: ImportStatus::Detected,
                        import_error: None,
                        imported_media_uri: None,
                    })))
                }),
            };
            request
                .respond(tiny_http::Response::from_string(
                    serde_json::to_string(&reply).unwrap(),
                ))
                .unwrap();
        });
        let resolver = qnc_transport_resolver::ResolverConfig::new(std::path::PathBuf::new())
            .with_lan_authority("test", &endpoint);
        let mut client =
            ContentClient::from_remote(&resolver, uri, Access::ReadOnly, "test-read").unwrap();
        assert!(client.read("c1").is_err());
        thread.join().unwrap();
    }
}

#[test]
fn batch_publication_is_atomic_and_inventory_removal_is_guarded() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("content.db");
    database(&path);
    let mut writer = ContentClient::from_owner_binding(&path, URI, Access::ReadWrite).unwrap();
    let mut invalid = clip("bad");
    invalid.name.clear();
    assert!(writer.publish_batch(vec![clip("first"), invalid]).is_err());
    assert!(
        writer.list(None).unwrap().is_empty(),
        "whole batch rolls back"
    );
    writer
        .publish_batch(vec![clip("first"), clip("second")])
        .unwrap();
    let source = clip("first").source_uri;
    let inventory = writer.inventory(&source, None).unwrap();
    assert_eq!(inventory.len(), 2);
    assert!(writer
        .inventory("qnc://local/source/other", None)
        .unwrap()
        .is_empty());
    let mut reader = ContentClient::from_owner_binding(&path, URI, Access::ReadOnly).unwrap();
    assert_eq!(reader.inventory(&source, None).unwrap(), inventory);
    assert!(reader.remove_missing(inventory.clone()).is_err());
    let mut stale = inventory[0].clone();
    stale.revision += 1;
    assert!(writer.remove_missing(vec![stale]).unwrap().is_empty());
    writer.select(vec!["second".into()], true).unwrap();
    writer.queue_selected().unwrap();
    assert_eq!(writer.remove_missing(inventory).unwrap(), vec!["first"]);
    let rows = writer.list(None).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].import_status, ImportStatus::Queued);
    assert!(rows[0].selected);
}

#[test]
fn filmstrip_publication_writes_public_frame_uris_without_touching_media() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("content.db");
    database(&path);
    let mut writer = ContentClient::from_owner_binding(&path, URI, Access::ReadWrite).unwrap();
    writer.publish(clip("c1")).unwrap();
    let artifact = FilmstripArtifactRecord {
        clip_id: "c1".into(),
        status: "ready".into(),
        duration_sec: "10.00".into(),
        frame_count: 2,
        artifact_uri: "qnc://local/project/p1/products/filmstrip/c1".into(),
        frames: vec![
            FilmstripFrameRecord {
                index: 0,
                seek_sec: "0.00".into(),
                artifact_uri: "qnc://local/project/p1/products/filmstrip/c1/000_0_00.jpg".into(),
            },
            FilmstripFrameRecord {
                index: 1,
                seek_sec: "5.00".into(),
                artifact_uri: "qnc://local/project/p1/products/filmstrip/c1/001_5_00.jpg".into(),
            },
        ],
    };
    writer.publish_filmstrip(artifact.clone()).unwrap();

    let mut reader = ContentClient::from_owner_binding(&path, URI, Access::ReadOnly).unwrap();
    assert_eq!(reader.read_filmstrip("c1").unwrap(), Some(artifact));
    assert!(reader.read_filmstrip("missing").unwrap().is_none());
    drop(reader);

    let db = Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row::<u32, _, _>("SELECT count(*) FROM public_filmstrip_frames", [], |r| r
            .get(0))
            .unwrap(),
        2
    );
    assert_eq!(
        db.query_row::<String, _, _>(
            "SELECT artifact_uri FROM public_filmstrip_frames WHERE frame_index=1",
            [],
            |r| r.get(0)
        )
        .unwrap(),
        "qnc://local/project/p1/products/filmstrip/c1/001_5_00.jpg"
    );
}

#[test]
fn content_write_transport_serializes_filmstrip_publication() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("content.db");
    database(&path);
    {
        let mut writer = ContentClient::from_owner_binding(&path, URI, Access::ReadWrite).unwrap();
        writer.publish(clip("c1")).unwrap();
        writer.publish(clip("c2")).unwrap();
    }
    let mut transport =
        ContentWriteTransport::start(ContentTarget::for_test_owner_binding(&path, URI)).unwrap();
    transport
        .publish_filmstrip("p1::c1".into(), filmstrip("c1"))
        .unwrap();
    transport
        .publish_filmstrip("p1::c2".into(), filmstrip("c2"))
        .unwrap();
    let mut completions = Vec::new();
    for _ in 0..100 {
        completions.extend(transport.poll());
        if completions.len() == 2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(completions.len(), 2, "{completions:?}");
    assert!(completions
        .iter()
        .all(|completion| completion.result.is_ok()));
    assert!(!transport.has_pending());

    let mut reader = ContentClient::from_owner_binding(&path, URI, Access::ReadOnly).unwrap();
    assert_eq!(reader.read_filmstrip("c1").unwrap().unwrap().frame_count, 2);
    assert_eq!(reader.read_filmstrip("c2").unwrap().unwrap().frame_count, 2);
}

#[test]
fn wave_publication_writes_public_peak_json_without_sidecar_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("content.db");
    database(&path);
    let mut writer = ContentClient::from_owner_binding(&path, URI, Access::ReadWrite).unwrap();
    writer.publish(clip("c1")).unwrap();
    let artifact = wave("c1");
    writer.publish_wave(artifact.clone()).unwrap();

    let mut reader = ContentClient::from_owner_binding(&path, URI, Access::ReadOnly).unwrap();
    assert_eq!(reader.read_wave("c1").unwrap(), Some(artifact));
    assert!(reader.read_wave("missing").unwrap().is_none());
    assert!(reader.publish_wave(wave("c1")).is_err());
    drop(reader);

    let db = Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row::<u32, _, _>("SELECT count(*) FROM public_wave_artifacts", [], |r| r
            .get(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row::<String, _, _>(
            "SELECT artifact_uri FROM public_wave_artifacts WHERE clip_id='c1'",
            [],
            |r| r.get(0)
        )
        .unwrap(),
        "qnc://local/db/ingest_content/p1/wave/c1"
    );
}

#[test]
fn content_write_transport_serializes_wave_publication() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("content.db");
    database(&path);
    {
        let mut writer = ContentClient::from_owner_binding(&path, URI, Access::ReadWrite).unwrap();
        writer.publish(clip("c1")).unwrap();
        writer.publish(clip("c2")).unwrap();
    }
    let mut transport =
        ContentWriteTransport::start(ContentTarget::for_test_owner_binding(&path, URI)).unwrap();
    transport.publish_wave("p1::c1".into(), wave("c1")).unwrap();
    transport.publish_wave("p1::c2".into(), wave("c2")).unwrap();
    let mut completions = Vec::new();
    for _ in 0..100 {
        completions.extend(transport.poll());
        if completions.len() == 2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(completions.len(), 2, "{completions:?}");
    assert!(completions
        .iter()
        .all(|completion| completion.result.is_ok()));
    assert!(!transport.has_pending());

    let mut reader = ContentClient::from_owner_binding(&path, URI, Access::ReadOnly).unwrap();
    assert_eq!(reader.read_wave("c1").unwrap().unwrap().a1_peaks.len(), 3);
    assert_eq!(reader.read_wave("c2").unwrap().unwrap().a2_peaks.len(), 3);
}

#[cfg(windows)]
#[test]
fn windows_delete_denied_directory_supports_durable_content_writes() {
    use std::process::Command;
    struct LockedFixture {
        directory: tempfile::TempDir,
        user: String,
    }
    impl Drop for LockedFixture {
        fn drop(&mut self) {
            let path = self.directory.path().canonicalize().unwrap();
            assert!(path.starts_with(std::env::temp_dir().canonicalize().unwrap()));
            let status = Command::new("icacls")
                .arg(&path)
                .arg("/remove:d")
                .arg(&self.user)
                .output()
                .unwrap();
            assert!(status.status.success());
        }
    }
    for journal_mode in ["PERSIST", "WAL"] {
        let fixture = LockedFixture {
            directory: tempfile::tempdir().unwrap(),
            user: format!(
                "{}\\{}",
                std::env::var("USERDOMAIN").unwrap(),
                std::env::var("USERNAME").unwrap()
            ),
        };
        let path = fixture.directory.path().canonicalize().unwrap();
        assert!(path.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        let file = path.join("content.db");
        database(&file);
        let project = Connection::open(&file).unwrap();
        project
            .pragma_update(None, "journal_mode", journal_mode)
            .unwrap();
        project
            .execute_batch("CREATE TABLE unrelated(value TEXT);")
            .unwrap();
        for entry in std::fs::read_dir(&path).unwrap() {
            let entry = entry.unwrap();
            let mut permissions = entry.metadata().unwrap().permissions();
            permissions.set_readonly(true);
            std::fs::set_permissions(entry.path(), permissions).unwrap();
        }
        assert!(Command::new("icacls")
            .arg(&path)
            .arg("/deny")
            .arg(format!("{}:(OI)(CI)(DE,DC)", fixture.user))
            .output()
            .unwrap()
            .status
            .success());
        let mut store = ContentStore::open_owner_binding(&file, URI, Access::ReadWrite).unwrap();
        run(&mut store, Operation::Publish(Box::new(clip("c1")))).unwrap();
        drop(store);
        assert!(std::fs::remove_file(&file).is_err());
        let mut store = ContentStore::open_owner_binding(&file, URI, Access::ReadWrite).unwrap();
        run(&mut store, Operation::Publish(Box::new(clip("c2")))).unwrap();
        let Data::Clips(rows) = run(&mut store, Operation::List { after: None }).unwrap() else {
            panic!()
        };
        assert_eq!(rows.len(), 2);
    }
}

fn database(path: &std::path::Path) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        "CREATE TABLE project_settings(project_id TEXT,settings_json TEXT);
        INSERT INTO project_settings VALUES('p1','{\"storage\":{\"ingest_media\":\"link\"}}');
        CREATE VIEW public_project_settings AS SELECT * FROM project_settings;",
    )
    .unwrap();
}
fn fact<T>(value: T) -> Option<m::Fact<T>> {
    Some(m::Fact {
        value,
        evidence_id: "camera".into(),
        locator: "/clip".into(),
    })
}
fn clip(id: &str) -> CatalogClip {
    let original_uri = format!("qnc://local/source/card/media/{id}.wav");
    let metadata = m::ClipMetadata {
        contract_id: m::CONTRACT_ID.into(),
        contract_version: m::CONTRACT_VERSION.into(),
        clip_id: id.into(),
        evidence: vec![m::Evidence {
            id: "camera".into(),
            kind: m::EvidenceKind::CameraMetadata,
            document_uri: "qnc://local/source/card/index.xml".into(),
            media_uri: original_uri.clone(),
        }],
        original: m::MediaRepresentation {
            media_uri: original_uri.clone(),
            container: fact("wav".into()),
            duration_seconds: fact(m::Rational {
                numerator: 10,
                denominator: 1,
            }),
            streams_complete: fact(true),
            streams: vec![m::MediaStream {
                index: fact(0),
                codec: fact(m::Signal::Known("pcm_s24le".into())),
                profile: None,
                time_base: fact(m::Rational {
                    numerator: 1,
                    denominator: 48000,
                }),
                start_pts: fact(0),
                duration_ts: fact(480000),
                details: m::StreamDetails::Audio(Box::new(m::AudioMetadata {
                    sample_rate_hz: fact(48000),
                    channels: fact(2),
                    sample_format: fact(m::Signal::Known("s32".into())),
                    channel_layout: fact(m::Signal::Known("stereo".into())),
                    bits_per_sample: fact(24),
                })),
            }],
            tags: BTreeMap::from([(
                "creation_time".into(),
                fact("2026-09-08T10:00:00Z".into()).unwrap(),
            )]),
        },
        proxy: None,
    };
    let report = m::inspect(&metadata);
    assert!(report.is_complete(), "{report:?}");
    CatalogClip {
        name: id.into(),
        source_uri: "qnc://local/source/card".into(),
        source_name: "Card".into(),
        serial_number: "serial-123".into(),
        volume_name: "CAMERA".into(),
        thumbnail_uri: None,
        media_records_uri: "qnc://local/db/media_records".into(),
        snapshot: Snapshot {
            binding: Binding {
                source_index_uri: "qnc://local/db/source_index".into(),
                source_record_id: format!("record-{id}"),
                original_uri,
                proxy_uri: None,
            },
            revision: 1,
            phase: Phase::Final,
            completeness: Completeness::Complete,
            metadata,
            report,
            recorded_at_unix_ms: 1234,
        },
    }
}
fn filmstrip(id: &str) -> FilmstripArtifactRecord {
    FilmstripArtifactRecord {
        clip_id: id.into(),
        status: "ready".into(),
        duration_sec: "10.00".into(),
        frame_count: 2,
        artifact_uri: format!("qnc://local/project/p1/products/filmstrip/{id}"),
        frames: vec![
            FilmstripFrameRecord {
                index: 0,
                seek_sec: "0.00".into(),
                artifact_uri: format!(
                    "qnc://local/project/p1/products/filmstrip/{id}/000_0_00.jpg"
                ),
            },
            FilmstripFrameRecord {
                index: 1,
                seek_sec: "5.00".into(),
                artifact_uri: format!(
                    "qnc://local/project/p1/products/filmstrip/{id}/001_5_00.jpg"
                ),
            },
        ],
    }
}
fn wave(id: &str) -> WaveArtifactRecord {
    WaveArtifactRecord {
        clip_id: id.into(),
        status: "ready".into(),
        artifact_uri: format!("qnc://local/db/ingest_content/p1/wave/{id}"),
        source_uri: format!("qnc://local/source/card/media/{id}.wav"),
        source_sample_rate_hz: 48_000,
        peak_count: 3,
        a1_peaks: vec![0.0, 0.5, 1.0],
        a2_peaks: vec![0.1, 0.4, 0.8],
        a3_peaks: Vec::new(),
        a4_peaks: Vec::new(),
        warning: None,
        render_version: qnc_wave::WAVE_RENDER_VERSION,
    }
}
fn run(store: &mut ContentStore, op: Operation) -> Result<Data> {
    store.execute(&Request {
        version: VERSION.into(),
        db_uri: URI.into(),
        operation: op,
    })
}
#[test]
fn public_output_survives_restart_without_touching_project_settings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("content.db");
    database(&path);
    let mut store = ContentStore::open_owner_binding(&path, URI, Access::ReadWrite).unwrap();
    run(&mut store, Operation::Publish(Box::new(clip("c1")))).unwrap();
    run(
        &mut store,
        Operation::Select {
            clip_ids: vec!["c1".into()],
            selected: true,
        },
    )
    .unwrap();
    drop(store);
    assert!(path.with_file_name("content.db-journal").is_file());
    let mut store = ContentStore::open_owner_binding(&path, URI, Access::ReadOnly).unwrap();
    let Data::Clips(clips) = run(&mut store, Operation::List { after: None }).unwrap() else {
        panic!()
    };
    assert_eq!(clips.len(), 1);
    assert!(clips[0].selected);
    assert_eq!(clips[0].clip.serial_number, "serial-123");
    assert!(run(&mut store, Operation::QueueSelected).is_err());
    let db = Connection::open(&path).unwrap();
    let settings_db = Connection::open(&path).unwrap();
    assert_eq!(
        settings_db
            .query_row::<String, _, _>("SELECT settings_json FROM project_settings", [], |r| r
                .get(0))
            .unwrap(),
        "{\"storage\":{\"ingest_media\":\"link\"}}"
    );
    assert_eq!(
        db.query_row::<u32, _, _>("SELECT count(*) FROM public_probe_records", [], |r| r
            .get(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row::<String, _, _>("SELECT created_at_utc FROM public_clips", [], |r| r.get(0))
            .unwrap(),
        "2026-09-08T10:00:00Z"
    );
}

#[test]
fn concurrent_writers_keep_journal_and_claim_a_clip_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut first = ContentStore::open_owner_binding(&path, URI, Access::ReadWrite).unwrap();
    let mut second = ContentStore::open_owner_binding(&path, URI, Access::ReadWrite).unwrap();
    run(&mut first, Operation::Publish(Box::new(clip("c1")))).unwrap();
    run(
        &mut first,
        Operation::Select {
            clip_ids: vec!["c1".into()],
            selected: true,
        },
    )
    .unwrap();
    run(&mut first, Operation::QueueSelected).unwrap();
    let worker = std::thread::spawn(move || run(&mut second, Operation::ClaimNext).unwrap());
    let local = run(&mut first, Operation::ClaimNext).unwrap();
    let remote = worker.join().unwrap();
    let count = [local, remote]
        .iter()
        .filter(|r| matches!(r, Data::Claimed(Some(_))))
        .count();
    assert_eq!(count, 1);
    drop(first);
    assert!(path.with_file_name("db-journal").is_file());
    ContentStore::open_owner_binding(&path, URI, Access::ReadOnly).unwrap();
}
#[test]
fn reselection_preserves_import_and_never_replaces_final_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    database(&path);
    let mut store = ContentStore::open_owner_binding(&path, URI, Access::ReadWrite).unwrap();
    run(&mut store, Operation::Publish(Box::new(clip("c1")))).unwrap();
    run(
        &mut store,
        Operation::Select {
            clip_ids: vec!["c1".into()],
            selected: true,
        },
    )
    .unwrap();
    run(&mut store, Operation::QueueSelected).unwrap();
    assert!(matches!(
        run(&mut store, Operation::ClaimNext).unwrap(),
        Data::Claimed(Some(_))
    ));
    assert!(matches!(
        run(&mut store, Operation::ClaimNext).unwrap(),
        Data::Claimed(None)
    ));
    run(
        &mut store,
        Operation::FinishImport {
            clip_id: "c1".into(),
            media_uri: Some(clip("c1").snapshot.binding.original_uri),
            thumbnail_uri: None,
            error: None,
        },
    )
    .unwrap();
    let Data::Saved(saved) = run(&mut store, Operation::Publish(Box::new(clip("c1")))).unwrap()
    else {
        panic!()
    };
    assert_eq!(saved.import_status, ImportStatus::Imported);
    assert!(saved.selected);
    let mut changed = clip("c1");
    changed.snapshot.metadata.original.duration_seconds = fact(m::Rational {
        numerator: 11,
        denominator: 1,
    });
    changed.snapshot.report = m::inspect(&changed.snapshot.metadata);
    assert!(run(&mut store, Operation::Publish(Box::new(changed))).is_err());
    let mut relabeled = clip("c1");
    relabeled.name = "renamed label".into();
    relabeled.volume_name = "new volume label".into();
    run(&mut store, Operation::Publish(Box::new(relabeled))).unwrap();
    let db = Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row::<String, _, _>("SELECT name FROM public_clips", [], |r| r.get(0))
            .unwrap(),
        "renamed label"
    );
    assert_eq!(
        db.query_row::<String, _, _>("SELECT volume_name FROM public_clip_sources", [], |r| r
            .get(0))
            .unwrap(),
        "new volume label"
    );
}
#[test]
fn different_project_and_project_settings_file_are_never_repaired() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    database(&path);
    let before = std::fs::read(&path).unwrap();
    assert!(ContentStore::open_owner_binding(
        &path,
        "qnc://local/db/ingest_content/p2",
        Access::ReadWrite
    )
    .is_err());
    assert_eq!(before, std::fs::read(&path).unwrap());
    drop(ContentStore::open_owner_binding(&path, URI, Access::ReadWrite).unwrap());
    assert!(ContentStore::open_owner_binding(
        &path,
        "qnc://local/db/ingest_content/p2",
        Access::ReadWrite
    )
    .is_err());
}

#[test]
fn trigger_cannot_write_project_settings_through_ingest_connection() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("project.db");
    database(&path);
    drop(ContentStore::open_owner_binding(&path, URI, Access::ReadWrite).unwrap());
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER foreign_write AFTER INSERT ON clips BEGIN
         UPDATE project_settings SET settings_json='changed'; END;",
        )
        .unwrap();
    let mut store = ContentStore::open_owner_binding(&path, URI, Access::ReadWrite).unwrap();
    assert!(run(&mut store, Operation::Publish(Box::new(clip("c1")))).is_err());
    let db = Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row::<i64, _, _>("SELECT count(*) FROM clips", [], |r| r.get(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row::<String, _, _>("SELECT settings_json FROM project_settings", [], |r| r
            .get(0))
            .unwrap(),
        "{\"storage\":{\"ingest_media\":\"link\"}}"
    );
}

#[test]
fn existing_project_wal_mode_survives_ingest_writer_and_read_only_settings_reader() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("project.db");
    database(&path);
    let project = Connection::open(&path).unwrap();
    project.pragma_update(None, "journal_mode", "WAL").unwrap();
    let mut store = ContentStore::open_owner_binding(&path, URI, Access::ReadWrite).unwrap();
    run(&mut store, Operation::Publish(Box::new(clip("c1")))).unwrap();
    assert_eq!(
        project
            .pragma_query_value::<String, _>(None, "journal_mode", |r| r.get(0))
            .unwrap(),
        "wal"
    );
    let mut reader = ContentStore::open_owner_binding(&path, URI, Access::ReadOnly).unwrap();
    assert!(
        matches!(run(&mut reader, Operation::List { after: None }).unwrap(), Data::Clips(rows) if rows.len()==1)
    );
}

#[cfg(windows)]
#[test]
fn readonly_wal_companions_do_not_block_owned_schema_writes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("project.db");
    database(&path);
    let project = Connection::open(&path).unwrap();
    project.pragma_update(None, "journal_mode", "WAL").unwrap();
    project
        .execute_batch("CREATE TABLE unrelated(value TEXT);")
        .unwrap();
    let files = ["project.db", "project.db-wal", "project.db-shm"];
    for name in files {
        let file = dir.path().join(name);
        let mut permissions = std::fs::metadata(&file).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(file, permissions).unwrap();
    }
    let result = ContentStore::open_owner_binding(&path, URI, Access::ReadWrite)
        .and_then(|mut store| run(&mut store, Operation::Publish(Box::new(clip("c1")))));
    let restored = files.map(|name| {
        !std::fs::metadata(dir.path().join(name))
            .unwrap()
            .permissions()
            .readonly()
    });
    // Always release the fixture, including on a regression failure.
    for name in files {
        let file = dir.path().join(name);
        let mut permissions = std::fs::metadata(&file).unwrap().permissions();
        permissions.set_readonly(false);
        std::fs::set_permissions(file, permissions).unwrap();
    }
    assert!(result.is_ok(), "{result:?}");
    assert!(restored.into_iter().all(|writable| writable));
    assert_eq!(
        project
            .pragma_query_value::<String, _>(None, "journal_mode", |r| r.get(0))
            .unwrap(),
        "wal"
    );
    assert_eq!(
        project
            .query_row::<String, _, _>("SELECT settings_json FROM project_settings", [], |r| r
                .get(0))
            .unwrap(),
        "{\"storage\":{\"ingest_media\":\"link\"}}"
    );
}

#[test]
fn colliding_table_rolls_back_schema_creation_without_migration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("project.db");
    database(&path);
    let db = Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE clips(value TEXT); INSERT INTO clips VALUES('keep');")
        .unwrap();
    assert!(ContentStore::open_owner_binding(&path, URI, Access::ReadWrite).is_err());
    assert_eq!(
        db.query_row::<i64, _, _>(
            "SELECT count(*) FROM sqlite_master WHERE name='ingest_content_schema'",
            [],
            |r| r.get(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row::<String, _, _>("SELECT value FROM clips", [], |r| r.get(0))
            .unwrap(),
        "keep"
    );
}

#[test]
fn foreign_schema_is_never_adopted_as_an_output() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("foreign.db");
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TABLE other_application(value TEXT); INSERT INTO other_application VALUES('keep');").unwrap();
    drop(connection);
    let before = std::fs::read(&path).unwrap();
    assert!(ContentStore::open_owner_binding(&path, URI, Access::ReadWrite).is_err());
    assert_eq!(before, std::fs::read(&path).unwrap());
}

#[test]
fn lan_and_intranet_use_same_authenticated_db_contract() {
    for environment in ["lan", "intranet"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        database(&path);
        let uri = format!("qnc://{environment}/test/db/ingest_content/p1");
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", server.server_addr());
        let published = uri.clone();
        let mut store = ContentStore::open_owner_binding(&path, &uri, Access::ReadWrite).unwrap();
        let thread = std::thread::spawn(move || {
            let credentials = Credentials::new("test-read", "test-write").unwrap();
            for _ in 0..2 {
                respond(server.recv().unwrap(), &mut store, &published, &credentials);
            }
        });
        let resolver = if environment == "lan" {
            qnc_transport_resolver::ResolverConfig::new(dir.path())
                .with_lan_authority("test", &endpoint)
        } else {
            qnc_transport_resolver::ResolverConfig::new(dir.path())
                .with_intranet_authority("test", &endpoint)
        };
        let mut client =
            ContentClient::from_remote(&resolver, &uri, Access::ReadWrite, "test-write").unwrap();
        client.publish(clip("c1")).unwrap();
        let mut reader =
            ContentClient::from_remote(&resolver, &uri, Access::ReadOnly, "test-read").unwrap();
        assert_eq!(reader.list(None).unwrap().len(), 1);
        assert!(reader.queue_selected().is_err());
        thread.join().unwrap();
    }
}

fn partial(id: &str, probed: bool) -> CatalogClip {
    let mut clip = clip(id);
    let uri = clip.snapshot.metadata.original.media_uri.clone();
    clip.snapshot.metadata.original.container = None;
    if probed {
        clip.snapshot.metadata.evidence.push(m::Evidence {
            id: "probe".into(),
            kind: m::EvidenceKind::Ffprobe,
            document_uri: "qnc://local/artifact/probe-a".into(),
            media_uri: uri,
        });
    }
    clip.snapshot.report = m::inspect(&clip.snapshot.metadata);
    clip.snapshot.completeness = Completeness::Partial;
    clip
}

fn queue(clip: CatalogClip) -> Result<Data> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    database(&path);
    let mut store = ContentStore::open_owner_binding(&path, URI, Access::ReadWrite).unwrap();
    let id = clip.id().to_string();
    run(&mut store, Operation::Publish(Box::new(clip))).unwrap();
    run(
        &mut store,
        Operation::Select {
            clip_ids: vec![id],
            selected: true,
        },
    )
    .unwrap();
    run(&mut store, Operation::QueueSelected)
}

#[test]
fn a_final_record_declared_by_the_card_can_be_queued_for_import() {
    assert!(queue(partial("c1", false)).is_ok());
}

#[test]
fn a_probed_record_that_is_still_partial_cannot_be_queued() {
    assert!(queue(partial("c1", true)).is_err());
}

#[test]
fn a_camera_phase_record_cannot_be_queued() {
    let mut camera = partial("c1", false);
    camera.snapshot.phase = Phase::Camera;
    assert!(queue(camera).is_err());
}

fn claimed_store(path: &std::path::Path) -> ContentStore {
    database(path);
    let mut store = ContentStore::open_owner_binding(path, URI, Access::ReadWrite).unwrap();
    run(&mut store, Operation::Publish(Box::new(clip("c1")))).unwrap();
    run(
        &mut store,
        Operation::Select {
            clip_ids: vec!["c1".into()],
            selected: true,
        },
    )
    .unwrap();
    run(&mut store, Operation::QueueSelected).unwrap();
    assert!(matches!(
        run(&mut store, Operation::ClaimNext).unwrap(),
        Data::Claimed(Some(_))
    ));
    store
}

fn age_lease(path: &std::path::Path, seconds: i64) {
    let db = Connection::open(path).unwrap();
    db.pragma_update(None, "journal_mode", "PERSIST").unwrap();
    db.execute(
        "UPDATE clips SET import_claimed_at = import_claimed_at - ?1",
        [seconds],
    )
    .unwrap();
}

#[test]
fn an_import_with_a_fresh_lease_is_not_claimed_twice() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = claimed_store(&path);
    age_lease(&path, 60);
    assert!(matches!(
        run(&mut store, Operation::ClaimNext).unwrap(),
        Data::Claimed(None)
    ));
}

#[test]
fn an_import_whose_importer_stopped_reporting_is_offered_again() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = claimed_store(&path);
    age_lease(&path, 600);
    assert!(matches!(
        run(&mut store, Operation::ClaimNext).unwrap(),
        Data::Claimed(Some(_))
    ));
}

#[test]
fn a_heartbeat_keeps_the_lease_alive_and_needs_a_running_import() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = claimed_store(&path);
    age_lease(&path, 600);
    run(
        &mut store,
        Operation::Heartbeat {
            clip_id: "c1".into(),
        },
    )
    .unwrap();
    assert!(matches!(
        run(&mut store, Operation::ClaimNext).unwrap(),
        Data::Claimed(None)
    ));
    run(
        &mut store,
        Operation::FinishImport {
            clip_id: "c1".into(),
            media_uri: Some(clip("c1").snapshot.binding.original_uri),
            thumbnail_uri: None,
            error: None,
        },
    )
    .unwrap();
    assert!(run(
        &mut store,
        Operation::Heartbeat {
            clip_id: "c1".into()
        }
    )
    .is_err());
}

fn poster_uri() -> String {
    "qnc://local/project/p1/products/thumbnails/c1_poster.jpg".into()
}

#[test]
fn an_imported_poster_replaces_the_card_poster_and_survives_a_new_select() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = claimed_store(&path);
    run(
        &mut store,
        Operation::FinishImport {
            clip_id: "c1".into(),
            media_uri: Some(clip("c1").snapshot.binding.original_uri),
            thumbnail_uri: Some(poster_uri()),
            error: None,
        },
    )
    .unwrap();
    let poster = |path: &std::path::Path| {
        Connection::open(path)
            .unwrap()
            .query_row::<Option<String>, _, _>("SELECT thumbnail_uri FROM public_clips", [], |r| {
                r.get(0)
            })
            .unwrap()
    };
    assert_eq!(poster(&path), Some(poster_uri()));
    run(&mut store, Operation::Publish(Box::new(clip("c1")))).unwrap();
    assert_eq!(poster(&path), Some(poster_uri()));
}

#[test]
fn what_a_process_tells_the_others_is_kept_in_the_database_with_its_age() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    database(&path);
    let mut store = ContentStore::open_owner_binding(&path, URI, Access::ReadWrite).unwrap();
    let get = |store: &mut ContentStore, key: &str| match run(
        store,
        Operation::GetRuntime { key: key.into() },
    )
    .unwrap()
    {
        Data::Runtime(entry) => entry,
        _ => panic!(),
    };
    assert!(get(&mut store, "playback_active").is_none());
    run(
        &mut store,
        Operation::SetRuntime {
            key: "playback_active".into(),
            value: "1".into(),
        },
    )
    .unwrap();
    let entry = get(&mut store, "playback_active").unwrap();
    assert_eq!(entry.value, "1");
    assert!(
        (0..=2).contains(&entry.age_seconds),
        "{}",
        entry.age_seconds
    );
    // A newer entry replaces the old one.
    run(
        &mut store,
        Operation::SetRuntime {
            key: "playback_active".into(),
            value: "0".into(),
        },
    )
    .unwrap();
    assert_eq!(get(&mut store, "playback_active").unwrap().value, "0");
}

#[test]
fn a_runtime_key_must_be_plain() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    database(&path);
    let mut store = ContentStore::open_owner_binding(&path, URI, Access::ReadWrite).unwrap();
    for key in ["", "Upper", "with space", "a;b"] {
        assert!(run(
            &mut store,
            Operation::SetRuntime {
                key: key.into(),
                value: "x".into()
            }
        )
        .is_err());
    }
}

fn imported_store(path: &std::path::Path) -> ContentStore {
    let mut store = claimed_store(path);
    run(
        &mut store,
        Operation::FinishImport {
            clip_id: "c1".into(),
            media_uri: Some(clip("c1").snapshot.binding.original_uri),
            thumbnail_uri: None,
            error: None,
        },
    )
    .unwrap();
    store
}

fn create_segment(
    store: &mut ContentStore,
    kind: &str,
    range: (u64, u64),
    fps: (u32, u32),
) -> Result<Data> {
    run(
        store,
        Operation::CreateSegment {
            project_id: "p1".into(),
            kind: kind.into(),
            clip_id: "c1".into(),
            in_frame: range.0,
            out_frame: range.1,
            fps_num: fps.0,
            fps_den: fps.1,
        },
    )
}

/// The program: active segments in order.
fn segments(store: &mut ContentStore) -> Vec<ProgramSegment> {
    let Data::Segments(rows) = run(store, Operation::ListSegments).unwrap() else {
        panic!()
    };
    rows.into_iter().filter(|row| row.active).collect()
}

#[test]
fn segments_are_appended_in_order_and_keep_their_source_range() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = imported_store(&dir.path().join("db"));
    let Data::Created(ton) = create_segment(&mut store, "tonovi", (10, 60), (50, 1)).unwrap()
    else {
        panic!()
    };
    create_segment(&mut store, "offovi", (100, 125), (100, 2)).unwrap();
    let rows = segments(&mut store);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].segment_id, ton);
    assert_eq!(
        (rows[0].kind.as_str(), rows[0].in_frame, rows[0].out_frame),
        ("tonovi", 10, 60)
    );
    assert_eq!((rows[1].kind.as_str(), rows[1].sort_index), ("offovi", 1));
}

#[test]
fn a_segment_needs_a_real_range_a_known_kind_an_imported_clip_and_the_story_rate() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut claimed = claimed_store(&path);
    assert!(create_segment(&mut claimed, "tonovi", (0, 10), (50, 1))
        .unwrap_err()
        .contains("nije uvezen"));
    drop(claimed);
    let dir = tempfile::tempdir().unwrap();
    let mut store = imported_store(&dir.path().join("db"));
    assert!(create_segment(&mut store, "tonovi", (10, 10), (50, 1)).is_err());
    assert!(create_segment(&mut store, "voice", (0, 10), (50, 1)).is_err());
    assert!(create_segment(&mut store, "tonovi", (0, 10), (0, 1)).is_err());
    create_segment(&mut store, "tonovi", (0, 10), (50, 1)).unwrap();
    let mixed = create_segment(&mut store, "offovi", (0, 10), (25, 1)).unwrap_err();
    assert!(mixed.contains("mijesani fps"), "{mixed}");
    assert_eq!(segments(&mut store).len(), 1);
}

#[test]
fn deleting_closes_the_gap_and_moving_swaps_neighbours_only() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = imported_store(&dir.path().join("db"));
    for start in [0, 100, 200] {
        create_segment(&mut store, "tonovi", (start, start + 10), (50, 1)).unwrap();
    }
    let ids = segments(&mut store)
        .into_iter()
        .map(|row| row.segment_id)
        .collect::<Vec<_>>();
    let move_segment = |store: &mut ContentStore, id: &str, up: bool| {
        run(
            store,
            Operation::MoveSegment {
                segment_id: id.into(),
                up,
            },
        )
        .unwrap()
    };
    move_segment(&mut store, &ids[0], true);
    move_segment(&mut store, &ids[2], true);
    let order = |store: &mut ContentStore| {
        segments(store)
            .into_iter()
            .map(|row| (row.segment_id, row.sort_index))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        order(&mut store),
        vec![
            (ids[0].clone(), 0),
            (ids[2].clone(), 1),
            (ids[1].clone(), 2)
        ]
    );
    run(
        &mut store,
        Operation::DeleteSegment {
            segment_id: ids[2].clone(),
        },
    )
    .unwrap();
    assert_eq!(
        order(&mut store),
        vec![(ids[0].clone(), 0), (ids[1].clone(), 1)]
    );
    assert!(run(
        &mut store,
        Operation::DeleteSegment {
            segment_id: ids[2].clone(),
        },
    )
    .is_err());
}

/// M on the Wrap segment that holds a program frame (the end belongs to the last one).
fn marker(store: &mut ContentStore, program_frame: u64) -> Result<Data> {
    let mut start = 0;
    let mut target = (String::new(), 0);
    for segment in segments(store) {
        let frames = segment.out_frame - segment.in_frame;
        target = (segment.segment_id, program_frame.saturating_sub(start));
        if program_frame < start + frames {
            break;
        }
        start += frames;
    }
    run(
        store,
        Operation::CreateMarker {
            part_id: target.0,
            local_frame: target.1,
        },
    )
}

/// User markers (not the locked program start and end).
fn markers(store: &mut ContentStore) -> Vec<u64> {
    let Data::Markers(rows) = run(store, Operation::ListMarkers).unwrap() else {
        panic!()
    };
    rows.into_iter()
        .filter(|row| row.system_role.is_empty())
        .map(|row| row.program_frame)
        .collect()
}

/// Locked boundary markers as (role, frame, origin segment).
fn boundaries(path: &std::path::Path) -> Vec<(String, i64, String)> {
    let conn = Connection::open(path).unwrap();
    let mut statement = conn
        .prepare(
            "SELECT system_role, timeline_frame, origin_part_id FROM story_markers
             WHERE system_role != '' ORDER BY timeline_frame",
        )
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

/// Three ten-frame segments: program frames 0..10, 10..20, 20..30.
fn three_segments(store: &mut ContentStore) -> Vec<String> {
    for start in [0, 100, 200] {
        create_segment(store, "tonovi", (start, start + 10), (50, 1)).unwrap();
    }
    segments(store)
        .into_iter()
        .map(|row| row.segment_id)
        .collect()
}

#[test]
fn start_and_end_are_stored_locked_markers_that_follow_the_program() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    let ids = three_segments(&mut store);
    assert_eq!(
        boundaries(&path),
        vec![
            ("program_start".into(), 0, ids[0].clone()),
            ("program_end".into(), 30, ids[2].clone())
        ]
    );
    let delete = |store: &mut ContentStore, id: &str| {
        run(
            store,
            Operation::DeleteSegment {
                segment_id: id.into(),
            },
        )
    };
    delete(&mut store, &ids[2]).unwrap();
    assert_eq!(
        boundaries(&path)[1],
        ("program_end".into(), 20, ids[1].clone())
    );
    delete(&mut store, &ids[0]).unwrap();
    delete(&mut store, &ids[1]).unwrap();
    assert!(boundaries(&path).is_empty(), "no segments, no markers");
}

#[test]
fn m_keeps_its_segment_and_a_marker_on_the_same_frame_is_refreshed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    assert!(marker(&mut store, 5).is_err(), "no story fps, no marker");
    let ids = three_segments(&mut store);
    let Data::Created(id) = run(
        &mut store,
        Operation::CreateMarker {
            part_id: ids[1].clone(),
            local_frame: 4,
        },
    )
    .unwrap() else {
        panic!()
    };
    let Data::Created(again) = marker(&mut store, 14).unwrap() else {
        panic!()
    };
    assert_eq!(again, id, "v5 refreshes the marker already on that frame");
    let Data::Created(start) = marker(&mut store, 0).unwrap() else {
        panic!()
    };
    assert_ne!(start, id);
    assert_eq!(markers(&mut store), vec![14]);
    let conn = Connection::open(&path).unwrap();
    let (origin, tc, local): (String, String, i64) = conn
        .query_row(
            "SELECT origin_part_id, tc, origin_local_frame FROM story_markers WHERE marker_id = ?1",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(local, 4, "placed on the segment at its own frame 4");
    assert_eq!(
        (origin.as_str(), tc.as_str()),
        (ids[1].as_str(), "00:00:00:14")
    );
}

#[test]
fn moving_and_deleting_markers_keep_the_start_and_the_end_locked() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    let Data::Created(id) = marker(&mut store, 14).unwrap() else {
        panic!()
    };
    marker(&mut store, 25).unwrap();
    let move_to = |store: &mut ContentStore, marker_id: &str, frame| {
        run(
            store,
            Operation::MoveMarker {
                marker_id: marker_id.into(),
                program_frame: frame,
            },
        )
    };
    assert!(move_to(&mut store, &id, 25)
        .unwrap_err()
        .contains("already exists"));
    assert!(move_to(&mut store, &id, 31)
        .unwrap_err()
        .contains("trajanja"));
    move_to(&mut store, &id, 21).unwrap();
    assert_eq!(markers(&mut store), vec![21, 25]);
    let Data::Created(start) = marker(&mut store, 0).unwrap() else {
        panic!()
    };
    assert!(move_to(&mut store, &start, 5)
        .unwrap_err()
        .contains("Početni"));
    let delete = |store: &mut ContentStore, marker_id: &str| {
        run(
            store,
            Operation::DeleteMarker {
                marker_id: marker_id.into(),
            },
        )
    };
    assert!(delete(&mut store, &start).unwrap_err().contains("Početni"));
    delete(&mut store, &id).unwrap();
    assert_eq!(markers(&mut store), vec![25]);
    assert!(delete(&mut store, &id).unwrap_err().contains("not found"));
}

#[test]
fn markers_cross_segment_borders_and_stay_on_their_frame_when_segments_move() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = imported_store(&dir.path().join("db"));
    let ids = three_segments(&mut store);
    marker(&mut store, 10).unwrap();
    marker(&mut store, 16).unwrap();
    run(
        &mut store,
        Operation::MoveSegment {
            segment_id: ids[2].clone(),
            up: true,
        },
    )
    .unwrap();
    assert_eq!(markers(&mut store), vec![10, 16]);
}

#[test]
fn deleting_a_segment_deactivates_it_shifts_markers_and_moves_the_selection() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    let ids = three_segments(&mut store);
    for frame in [5, 13, 24] {
        marker(&mut store, frame).unwrap();
    }
    drop(store);
    Connection::open(&path)
        .unwrap()
        .execute(
            "UPDATE story_state SET selected_part_id = ?1 WHERE id = 1",
            [&ids[1]],
        )
        .unwrap();
    let mut store = ContentStore::open_owner_binding(&path, URI, Access::ReadWrite).unwrap();
    run(
        &mut store,
        Operation::DeleteSegment {
            segment_id: ids[1].clone(),
        },
    )
    .unwrap();
    // v5: 13 was inside the window 10..20; 24 moves left by ten frames.
    assert_eq!(markers(&mut store), vec![5, 14]);
    let conn = Connection::open(&path).unwrap();
    let (active, selected): (i64, String) = conn
        .query_row(
            "SELECT p.active, s.selected_part_id FROM story_parts p, story_state s
             WHERE p.part_id = ?1",
            [&ids[1]],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(active, 0, "the row stays, inactive");
    assert_ne!(selected, ids[1]);
    assert!(!selected.is_empty());
    assert_eq!(segments(&mut store).len(), 2);
}

#[test]
fn development_program_tables_are_removed_on_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    drop(store);
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TABLE program_segments (segment_id TEXT PRIMARY KEY, marker_mode TEXT);
             CREATE TABLE program_markers (marker_id TEXT PRIMARY KEY, segment_id TEXT);",
        )
        .unwrap();
    let mut store = ContentStore::open_owner_binding(&path, URI, Access::ReadWrite).unwrap();
    assert_eq!(segments(&mut store).len(), 3);
    drop(store);
    let legacy: i64 = Connection::open(&path)
        .unwrap()
        .query_row(
            "SELECT count(*) FROM sqlite_master
             WHERE name IN ('program_segments', 'program_markers')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(legacy, 0);
}

/// Slots as (start marker frame, end marker frame, has a cover).
fn slot_rows(path: &std::path::Path) -> Vec<(i64, i64, bool)> {
    let conn = Connection::open(path).unwrap();
    let mut statement = conn
        .prepare(
            "SELECT start_frame, end_frame, has_cover FROM public_story_marker_slots
             ORDER BY slot_index",
        )
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

/// The slot id (marker pair) covering a program frame.
fn slot_at_frame(path: &std::path::Path, frame: i64) -> String {
    Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT slot_id FROM story_marker_slots WHERE start_frame <= ?1 AND ?1 < end_frame",
            [frame],
            |row| row.get(0),
        )
        .unwrap()
}

/// Puts a cover into a slot the way the cover owner will (store closed first).
fn put_cover(path: &std::path::Path, cover_id: &str, slot_id: &str) {
    let conn = Connection::open(path).unwrap();
    let (start, end): (i64, i64) = conn
        .query_row(
            "SELECT start_frame, end_frame FROM story_marker_slots WHERE slot_id = ?1",
            [slot_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    conn.execute(
        "INSERT INTO story_covers (cover_id, slot_id, timeline_start_frame, timeline_end_frame,
            clip_id, source_in_frame, source_out_frame, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'c1', 0, 100, ?1, ?1)",
        rusqlite::params![cover_id, slot_id, start, end],
    )
    .unwrap();
}

/// (slot id, program start, program end) of a cover.
fn cover_row(path: &std::path::Path, cover_id: &str) -> (String, i64, i64) {
    Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT slot_id, timeline_start_frame, timeline_end_frame FROM story_covers
             WHERE cover_id = ?1",
            [cover_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
}

fn reopen(path: &std::path::Path) -> ContentStore {
    ContentStore::open_owner_binding(path, URI, Access::ReadWrite).unwrap()
}

#[test]
fn slots_are_stored_between_adjacent_markers_and_named_by_their_pair() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    assert_eq!(slot_rows(&path), vec![(0, 30, false)]);
    let Data::Created(m14) = marker(&mut store, 14).unwrap() else {
        panic!()
    };
    marker(&mut store, 25).unwrap();
    assert_eq!(
        slot_rows(&path),
        vec![(0, 14, false), (14, 25, false), (25, 30, false)]
    );
    let middle = slot_at_frame(&path, 20);
    assert!(middle.starts_with(&format!("{m14}|")), "{middle}");
    // Segment borders at 10 and 20 are no markers, so no slot ends there.
    assert!(slot_rows(&path)
        .iter()
        .all(|(start, end, _)| ![10, 20].contains(start) && ![10, 20].contains(end)));
}

#[test]
fn a_cover_follows_its_slot_when_markers_move_or_split_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    let Data::Created(m14) = marker(&mut store, 14).unwrap() else {
        panic!()
    };
    let Data::Created(m25) = marker(&mut store, 25).unwrap() else {
        panic!()
    };
    let slot = slot_at_frame(&path, 20);
    drop(store);
    put_cover(&path, "cover_a", &slot);
    let mut store = reopen(&path);
    run(
        &mut store,
        Operation::MoveMarker {
            marker_id: m25.clone(),
            program_frame: 27,
        },
    )
    .unwrap();
    // The same pair moves: the cover takes the new frames.
    assert_eq!(cover_row(&path, "cover_a"), (slot.clone(), 14, 27));
    // A new marker splits the slot: the cover is trimmed to the part that keeps its start.
    let Data::Created(m20) = marker(&mut store, 20).unwrap() else {
        panic!()
    };
    assert_eq!(
        cover_row(&path, "cover_a"),
        (format!("{m14}|{m20}"), 14, 20)
    );
    assert!(slot_rows(&path)[1].2, "the slot shows its cover");
}

fn cover_count(path: &std::path::Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT count(*) FROM story_covers", [], |row| row.get(0))
        .unwrap()
}

#[test]
fn deleting_a_marker_deletes_its_slots_with_their_covers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    let Data::Created(m14) = marker(&mut store, 14).unwrap() else {
        panic!()
    };
    marker(&mut store, 25).unwrap();
    let (left, middle, right) = (
        slot_at_frame(&path, 5),
        slot_at_frame(&path, 20),
        slot_at_frame(&path, 27),
    );
    drop(store);
    put_cover(&path, "cover_left", &left);
    put_cover(&path, "cover_middle", &middle);
    put_cover(&path, "cover_right", &right);
    Connection::open(&path)
        .unwrap()
        .execute(
            "UPDATE story_state SET selected_slot_id = ?1 WHERE id = 1",
            [&middle],
        )
        .unwrap();
    let mut store = reopen(&path);
    run(
        &mut store,
        Operation::DeleteMarker {
            marker_id: m14.clone(),
        },
    )
    .unwrap();
    // M14 closed the left slot and opened the middle one: both go with their covers.
    assert_eq!(cover_count(&path), 1);
    assert_eq!(cover_row(&path, "cover_right").1, 25);
    assert_eq!(slot_rows(&path), vec![(0, 25, false), (25, 30, true)]);
    let selected: String = Connection::open(&path)
        .unwrap()
        .query_row("SELECT selected_slot_id FROM story_state", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(selected, "", "a slot that no longer exists is not selected");
}

#[test]
fn deleting_a_segment_deletes_the_slots_of_the_markers_inside_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    let ids = three_segments(&mut store);
    marker(&mut store, 5).unwrap();
    marker(&mut store, 14).unwrap();
    let (first, inside) = (slot_at_frame(&path, 2), slot_at_frame(&path, 20));
    drop(store);
    put_cover(&path, "cover_first", &first);
    put_cover(&path, "cover_inside", &inside);
    let mut store = reopen(&path);
    run(
        &mut store,
        Operation::DeleteSegment {
            segment_id: ids[1].clone(),
        },
    )
    .unwrap();
    // M14 was inside the deleted segment: its slots and their covers go.
    assert_eq!(cover_count(&path), 1);
    assert_eq!(cover_row(&path, "cover_first"), (first, 0, 5));
    run(
        &mut store,
        Operation::DeleteSegment {
            segment_id: ids[0].clone(),
        },
    )
    .unwrap();
    run(
        &mut store,
        Operation::DeleteSegment {
            segment_id: ids[2].clone(),
        },
    )
    .unwrap();
    assert_eq!(cover_count(&path), 0, "no program, no markers, no covers");
}

#[test]
fn reads_list_deleted_segments_all_markers_slots_and_the_selection() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    let ids = three_segments(&mut store);
    marker(&mut store, 14).unwrap();
    run(
        &mut store,
        Operation::DeleteSegment {
            segment_id: ids[2].clone(),
        },
    )
    .unwrap();
    let Data::Segments(all) = run(&mut store, Operation::ListSegments).unwrap() else {
        panic!()
    };
    assert_eq!(all.len(), 3, "a deleted segment stays listed");
    assert_eq!(all.iter().filter(|row| !row.active).count(), 1);
    let Data::Markers(markers) = run(&mut store, Operation::ListMarkers).unwrap() else {
        panic!()
    };
    let roles: Vec<(&str, u64)> = markers
        .iter()
        .map(|row| (row.system_role.as_str(), row.program_frame))
        .collect();
    assert_eq!(
        roles,
        vec![("program_start", 0), ("", 14), ("program_end", 20)]
    );
    let Data::Slots(slots) = run(&mut store, Operation::ListSlots).unwrap() else {
        panic!()
    };
    let spans: Vec<(u64, u64)> = slots
        .iter()
        .map(|slot| (slot.start_frame, slot.end_frame))
        .collect();
    assert_eq!(spans, vec![(0, 14), (14, 20)]);
    assert_eq!(
        slots[1].slot_id,
        format!("{}|{}", markers[1].marker_id, markers[2].marker_id)
    );
    run(
        &mut store,
        Operation::SelectSlot {
            slot_id: slots[1].slot_id.clone(),
        },
    )
    .unwrap();
    run(
        &mut store,
        Operation::SelectPart {
            part_id: ids[0].clone(),
        },
    )
    .unwrap();
    assert!(run(
        &mut store,
        Operation::SelectSlot {
            slot_id: "missing|slot".into(),
        },
    )
    .is_err());
    let Data::StorySelection(selection) = run(&mut store, Operation::ReadStorySelection).unwrap()
    else {
        panic!()
    };
    assert_eq!(selection.selected_part_id, ids[0]);
    assert_eq!(selection.selected_slot_id, slots[1].slot_id);
}

#[test]
fn a_segment_hears_source_channel_one_on_a1_and_covers_are_read_with_their_a2_channel() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    marker(&mut store, 14).unwrap();
    assert!(
        segments(&mut store)
            .iter()
            .all(|row| row.a1_source_channel == 0),
        "v5: channel 1 of the source until the user picks another one"
    );
    drop(store);
    put_cover(&path, "cover-1", &slot_at_frame(&path, 20));
    Connection::open(&path)
        .unwrap()
        .execute(
            "UPDATE story_covers SET a2_source_channel = 1, source_fps_num = 50,
                virtual_shot_id = 'shot' WHERE cover_id = 'cover-1'",
            [],
        )
        .unwrap();
    let mut store = reopen(&path);
    let Data::Covers(covers) = run(&mut store, Operation::ListCovers).unwrap() else {
        panic!()
    };
    assert_eq!(covers.len(), 1);
    let cover = &covers[0];
    assert_eq!(
        (
            cover.cover_id.as_str(),
            cover.clip_id.as_str(),
            cover.virtual_shot_id.as_str()
        ),
        ("cover-1", "c1", "shot")
    );
    assert_eq!(
        (cover.program_start_frame, cover.program_end_frame),
        (14, 30)
    );
    assert_eq!((cover.source_in_frame, cover.source_out_frame), (0, 100));
    assert_eq!(
        (cover.fps_num, cover.fps_den, cover.a2_source_channel),
        (50, 1, 1)
    );
}

#[test]
fn a_development_story_without_the_audio_channel_columns_is_removed_on_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    marker(&mut store, 14).unwrap();
    drop(store);
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "DROP VIEW public_story_parts; ALTER TABLE story_parts DROP COLUMN a1_source_channel;",
        )
        .unwrap();
    let mut store = reopen(&path);
    assert!(segments(&mut store).is_empty(), "removed, not converted");
    assert!(markers(&mut store).is_empty());
    three_segments(&mut store);
    assert_eq!(segments(&mut store).len(), 3, "a new story starts cleanly");
}

fn create_cover(store: &mut ContentStore, slot_id: &str, range: (u64, u64)) -> Result<Data> {
    run(
        store,
        Operation::CreateCover {
            project_id: "p1".into(),
            slot_id: slot_id.into(),
            clip_id: "c1".into(),
            clip_name: "Clip".into(),
            in_frame: range.0,
            out_frame: range.1,
            fps_num: 50,
            fps_den: 1,
        },
    )
}

fn covers(store: &mut ContentStore) -> Vec<ProgramCover> {
    let Data::Covers(rows) = run(store, Operation::ListCovers).unwrap() else {
        panic!()
    };
    rows
}

#[test]
fn a_cover_is_a_b_roll_virtual_shot_in_its_slot_and_replaces_the_one_there() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    marker(&mut store, 14).unwrap();
    let slot = slot_at_frame(&path, 20);
    let Data::Created(first) = create_cover(&mut store, &slot, (40, 90)).unwrap() else {
        panic!()
    };
    let Data::Created(second) = create_cover(&mut store, &slot, (5, 25)).unwrap() else {
        panic!()
    };
    let rows = covers(&mut store);
    assert_eq!(
        rows.len(),
        1,
        "v5: a new cover replaces the one in its slot"
    );
    let cover = &rows[0];
    assert_eq!(cover.cover_id, second);
    assert_ne!(first, second);
    assert_eq!(
        (cover.program_start_frame, cover.program_end_frame),
        (14, 30)
    );
    assert_eq!(
        (cover.source_in_frame, cover.source_out_frame),
        (5, 25),
        "the source is not cut to the slot"
    );
    assert_eq!(
        (cover.fps_num, cover.fps_den, cover.a2_source_channel),
        (50, 1, 0)
    );
    let classes: Vec<(String, String, i64, i64)> = Connection::open(&path)
        .unwrap()
        .prepare("SELECT shot_id, class, in_frame, out_frame FROM virtual_shots ORDER BY shot_id")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let b_roll: Vec<_> = classes.iter().filter(|row| row.1 == "b_roll").collect();
    assert_eq!(b_roll.len(), 2, "each cover writes its B-roll virtual shot");
    assert_eq!(cover.virtual_shot_id, b_roll[1].0);
    assert!(classes.iter().all(|row| row.1 != "short"), "never a short");
    let Data::ShortClips(shorts) = run(&mut store, Operation::ListShorts).unwrap() else {
        panic!()
    };
    assert!(shorts.is_empty());
    let Data::StorySelection(selection) = run(&mut store, Operation::ReadStorySelection).unwrap()
    else {
        panic!()
    };
    assert_eq!(
        selection.selected_cover_id, second,
        "v5: the new cover is selected"
    );
}

#[test]
fn a_cover_needs_a_slot_the_story_rate_and_one_frame() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    assert!(
        create_cover(&mut store, "a|b", (0, 10)).is_err(),
        "no story"
    );
    three_segments(&mut store);
    let slot = slot_at_frame(&path, 5);
    assert!(create_cover(&mut store, "missing|slot", (0, 10)).is_err());
    assert!(create_cover(&mut store, &slot, (10, 10)).is_err());
    let mixed = run(
        &mut store,
        Operation::CreateCover {
            project_id: "p1".into(),
            slot_id: slot.clone(),
            clip_id: "c1".into(),
            clip_name: "Clip".into(),
            in_frame: 0,
            out_frame: 10,
            fps_num: 25,
            fps_den: 1,
        },
    );
    assert!(mixed.is_err(), "mixed fps is refused");
    assert!(covers(&mut store).is_empty());
}

#[test]
fn a_cover_is_selected_and_deleted_and_its_selection_goes_with_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    marker(&mut store, 14).unwrap();
    let Data::Created(early) = create_cover(&mut store, &slot_at_frame(&path, 5), (0, 10)).unwrap()
    else {
        panic!()
    };
    let Data::Created(late) = create_cover(&mut store, &slot_at_frame(&path, 20), (0, 10)).unwrap()
    else {
        panic!()
    };
    run(
        &mut store,
        Operation::SelectCover {
            cover_id: early.clone(),
        },
    )
    .unwrap();
    assert!(run(
        &mut store,
        Operation::SelectCover {
            cover_id: "none".into()
        }
    )
    .is_err());
    run(
        &mut store,
        Operation::DeleteCover {
            cover_id: late.clone(),
        },
    )
    .unwrap();
    let Data::StorySelection(selection) = run(&mut store, Operation::ReadStorySelection).unwrap()
    else {
        panic!()
    };
    assert_eq!(
        selection.selected_cover_id, early,
        "another cover's selection stays"
    );
    run(&mut store, Operation::DeleteCover { cover_id: early }).unwrap();
    let Data::StorySelection(selection) = run(&mut store, Operation::ReadStorySelection).unwrap()
    else {
        panic!()
    };
    assert!(selection.selected_cover_id.is_empty());
    assert!(covers(&mut store).is_empty());
    assert!(run(&mut store, Operation::DeleteCover { cover_id: late }).is_err());
    let b_roll: i64 = Connection::open(&path)
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM virtual_shots WHERE class = 'b_roll'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(b_roll, 2, "the B-roll shots stay in the B-roll tab");
}
