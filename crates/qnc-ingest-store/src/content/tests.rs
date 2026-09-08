use super::*;
use qnc_media_metadata as m;
use qnc_media_records::{Binding, Completeness};
use rusqlite::Connection;
use std::collections::BTreeMap;

const URI: &str = "qnc://local/db/ingest_content/p1";

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
    let mut permissions = std::fs::metadata(&file).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&file, permissions).unwrap();
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
    changed.snapshot.recorded_at_unix_ms += 1;
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
    let path = dir.path().join("qnc_project.db");
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
    let path = dir.path().join("qnc_project.db");
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

#[test]
fn colliding_table_rolls_back_schema_creation_without_migration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("qnc_project.db");
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
