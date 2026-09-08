use super::*;
#[path = "acquisition_tests.rs"]
mod acquisition_tests;
use contract::*;
use qnc_media_metadata as m;
use qnc_source_groups::{GroupEvidence, GroupProposal, RelatedReference};
use qnc_source_index_contract::{FileFact, FileState, SourceReference};
use qnc_transport_resolver::ResolverConfig;
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Barrier,
    },
    thread,
    time::Duration,
};
const URI: &str = "qnc://local/db/media_records";
const READ: &str = "media-test-read";
const WRITE: &str = "media-test-write";

fn fact<T>(id: &str, value: T) -> Option<m::Fact<T>> {
    Some(m::Fact {
        value,
        evidence_id: id.into(),
        locator: "/test/field".into(),
    })
}
fn representation(id: &str, uri: String, codec: &str) -> m::MediaRepresentation {
    m::MediaRepresentation {
        media_uri: uri,
        container: fact(id, "mxf".into()),
        duration_seconds: fact(
            id,
            m::Rational {
                numerator: 10,
                denominator: 1,
            },
        ),
        streams_complete: fact(id, true),
        streams: vec![m::MediaStream {
            index: fact(id, 0),
            codec: fact(id, m::Signal::Known(codec.into())),
            profile: None,
            time_base: fact(
                id,
                m::Rational {
                    numerator: 1,
                    denominator: 48000,
                },
            ),
            start_pts: fact(id, -1024),
            duration_ts: fact(id, 480000),
            details: m::StreamDetails::Audio(Box::new(m::AudioMetadata {
                sample_rate_hz: fact(id, 48000),
                channels: fact(id, 2),
                sample_format: fact(id, m::Signal::Known("s32".into())),
                channel_layout: fact(id, m::Signal::Known("stereo".into())),
                bits_per_sample: fact(id, 24),
            })),
        }],
        tags: BTreeMap::from([(
            "creation_time".into(),
            fact(id, "2026-09-07T10:23:11+02:00".into()).unwrap(),
        )]),
    }
}
fn input(id: &str) -> Write {
    let root = SourceReference::new("qnc://local/source/card", "ROOT").unwrap();
    let p = GroupProposal {
        original: root.descendant(&format!("{id}.MXF")).unwrap(),
        proxies: vec![root.descendant(&format!("{id}.MP4")).unwrap()],
        related: vec![RelatedReference {
            reference: root.descendant(&format!("{id}.XML")).unwrap(),
            kind: "XML".into(),
        }],
        evidence: GroupEvidence {
            reader_id: "test.reader".into(),
            document: root.descendant("INDEX.XML").unwrap(),
            locator: id.into(),
        },
        recording_identity: id.into(),
        root,
    };
    let facts: Vec<_> = p
        .references()
        .map(|r| FileFact {
            reference: r.clone(),
            state: FileState::File,
        })
        .collect();
    let group = qnc_source_groups::assemble(p.root.source_uri(), vec![p.clone()], &facts)
        .unwrap()
        .groups
        .remove(0);
    let doc_uri = p.evidence.document.uri();
    let mut proxy = representation("proxy", p.proxies[0].uri(), "aac");
    proxy.container = fact("proxy", "mp4".into());
    Write {
        request_id: format!("request-{id}"),
        expected_revision: 0,
        phase: Phase::Camera,
        source_index_uri: "qnc://local/db/source_index".into(),
        source_record: SourceRecord {
            record_id: format!("source-{id}"),
            source_uri: p.root.source_uri().into(),
            group,
            recorded_at_unix_ms: 1,
        },
        metadata: m::ClipMetadata {
            contract_id: m::CONTRACT_ID.into(),
            contract_version: m::CONTRACT_VERSION.into(),
            clip_id: format!("clip-{id}"),
            evidence: vec![
                m::Evidence {
                    id: "original".into(),
                    kind: EvidenceKind::CameraMetadata,
                    document_uri: doc_uri.clone(),
                    media_uri: p.original.uri(),
                },
                m::Evidence {
                    id: "proxy".into(),
                    kind: EvidenceKind::CameraMetadata,
                    document_uri: doc_uri.clone(),
                    media_uri: p.proxies[0].uri(),
                },
            ],
            original: representation("original", p.original.uri(), "pcm_s24le"),
            proxy: Some(proxy),
        },
        documents: vec![Document {
            document_uri: doc_uri,
            media_type: DocumentType::Xml,
            text: "<camera/>".into(),
        }],
    }
}
fn local(path: &Path, create: bool, access: Access) -> Client {
    let resolver = ResolverConfig::new(path.parent().unwrap()).with_local_binding(URI, path);
    if create {
        Client::create_local(&resolver, URI).unwrap()
    } else {
        Client::open(&resolver, URI, access, None).unwrap()
    }
}

#[test]
fn changing_camera_index_keeps_both_immutable_versions_and_rejects_forgery() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("media.db");
    let mut db = local(&path, true, Access::ReadWrite);
    let mut first = input("first");
    freeze_camera_documents(&mut first.metadata, &mut first.documents).unwrap();
    db.write(first.clone()).unwrap();
    let mut second = input("second");
    second.documents[0].text = "<camera><new-recording/></camera>".into();
    freeze_camera_documents(&mut second.metadata, &mut second.documents).unwrap();
    assert_ne!(
        first.documents[0].document_uri,
        second.documents[0].document_uri
    );
    db.write(second.clone()).unwrap();
    for write in [&first, &second] {
        assert_eq!(
            db.document(&write.documents[0].document_uri).unwrap(),
            Some(write.documents[0].clone())
        );
    }
    let mut forged = input("forged");
    freeze_camera_documents(&mut forged.metadata, &mut forged.documents).unwrap();
    forged.documents[0].text = "<changed-after-hashing/>".into();
    assert!(db.write(forged).is_err());
    let mut unrelated = input("unrelated");
    unrelated.documents[0].document_uri = "qnc://local/source/another/index.xml".into();
    for evidence in &mut unrelated.metadata.evidence {
        evidence.document_uri = unrelated.documents[0].document_uri.clone();
    }
    freeze_camera_documents(&mut unrelated.metadata, &mut unrelated.documents).unwrap();
    assert!(db.write(unrelated).is_err());
}
fn count(path: &Path, table: &str) -> usize {
    let conn =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}
#[test]
fn metadata_and_documents_survive_writer_shutdown_without_source_or_app() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("media.sqlite");
    let mut client = local(&path, true, Access::ReadWrite);
    let write = input("A");
    let receipt = client.write(write.clone()).unwrap();
    assert_eq!(receipt.completeness, Completeness::Complete);
    assert_eq!(receipt.phase, Phase::Camera);
    let s = client.read(&receipt.clip_id, None).unwrap().unwrap();
    assert_eq!(s.metadata, write.metadata);
    assert_eq!(s.binding.source_record_id, "source-A");
    drop(client);
    let mut reader = local(&path, false, Access::ReadOnly);
    assert_eq!(reader.read(&receipt.clip_id, None).unwrap(), Some(s));
    assert_eq!(
        reader.document(&write.documents[0].document_uri).unwrap(),
        Some(write.documents[0].clone())
    );
    assert_eq!(reader.write(write), Err(Error::AccessDenied));
    assert_eq!(count(&path, "public_media_heads"), 1);
    assert_eq!(count(&path, "public_media_documents"), 1);
}
#[test]
fn final_partial_is_immutable_and_is_not_a_completed_metadata_claim() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("media.sqlite");
    let mut client = local(&path, true, Access::ReadWrite);
    let mut write = input("A");
    write.metadata.original.container = None;
    assert_eq!(
        client.write(write.clone()).unwrap().completeness,
        Completeness::Partial
    );
    write.request_id = "final-A".into();
    write.expected_revision = 1;
    write.phase = Phase::Final;
    let receipt = client.write(write.clone()).unwrap();
    assert_eq!(receipt.revision, 2);
    assert_eq!(receipt.completeness, Completeness::Partial);
    assert_eq!(client.write(write.clone()).unwrap(), receipt);
    assert_eq!(
        client.read("clip-A", Some(1)).unwrap().unwrap().phase,
        Phase::Camera
    );
    let snapshot = client.read("clip-A", None).unwrap().unwrap();
    assert_eq!(snapshot.phase, Phase::Final);
    assert!(!snapshot.report.is_complete());
    write.request_id = "late-repair".into();
    write.metadata.original.container = fact("original", "mxf".into());
    assert_eq!(client.write(write), Err(Error::Finalized));
    assert_eq!(count(&path, "public_media_snapshots"), 2);
}
#[test]
fn complete_final_can_be_committed_directly_without_ffprobe_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let mut client = local(&dir.path().join("media.sqlite"), true, Access::ReadWrite);
    let mut write = input("A");
    write.phase = Phase::Final;
    let receipt = client.write(write.clone()).unwrap();
    assert_eq!(receipt.completeness, Completeness::Complete);
    assert_eq!(receipt.revision, 1);
    assert!(write
        .metadata
        .evidence
        .iter()
        .all(|e| e.kind == EvidenceKind::CameraMetadata));
    assert_eq!(client.write(write).unwrap(), receipt);
}
#[test]
fn binding_rejects_unrelated_original_dropped_proxy_and_multiple_proxies() {
    let mut write = input("A");
    write.metadata.original.media_uri = "qnc://local/media/unrelated".into();
    assert_eq!(write.validate(), Err(Error::InvalidMetadata));
    let mut write = input("A");
    write.metadata.proxy = None;
    assert_eq!(write.validate(), Err(Error::InvalidMetadata));
    let mut write = input("A");
    let extra = write
        .source_record
        .group
        .proposal
        .root
        .descendant("another.MP4")
        .unwrap();
    write.source_record.group.proposal.proxies.push(extra);
    assert_eq!(write.validate(), Err(Error::UnsupportedProxySet));
    let mut write = input("A");
    write.metadata.original.streams[0]
        .codec
        .as_mut()
        .unwrap()
        .evidence_id = "proxy".into();
    assert_eq!(write.validate(), Err(Error::InvalidMetadata));
}
#[test]
fn missing_unrelated_duplicate_or_wrong_document_evidence_is_rejected() {
    let mut write = input("A");
    write.documents.clear();
    assert_eq!(write.validate(), Err(Error::InvalidRequest));
    let mut write = input("A");
    write.documents.push(write.documents[0].clone());
    assert_eq!(write.validate(), Err(Error::InvalidRequest));
    let mut write = input("A");
    write.metadata.evidence[0].document_uri = "qnc://local/artifact/unrelated".into();
    assert!(write.validate().is_err());
    let mut write = input("A");
    write.metadata.evidence[0].kind = EvidenceKind::Ffprobe;
    assert_eq!(write.validate(), Err(Error::InvalidMetadata));
    write.phase = Phase::Final;
    assert_eq!(write.validate(), Err(Error::InvalidMetadata));
    let mut write = input("A");
    write.documents[0].text = "x".repeat(MAX_DOCUMENT_BYTES + 1);
    assert_eq!(write.validate(), Err(Error::TooLarge));
}
#[test]
fn shared_document_is_deduplicated_and_changed_text_rolls_back_new_documents() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("media.sqlite");
    let mut client = local(&path, true, Access::ReadWrite);
    let a = input("A");
    client.write(a.clone()).unwrap();
    client.write(input("B")).unwrap();
    assert_eq!(count(&path, "evidence_documents"), 1);
    let mut c = input("C");
    let side = c.source_record.group.proposal.related[0].reference.uri();
    c.documents.insert(
        0,
        Document {
            document_uri: side.clone(),
            media_type: DocumentType::Xml,
            text: "<side/>".into(),
        },
    );
    c.metadata.evidence.push(m::Evidence {
        id: "side".into(),
        kind: EvidenceKind::CameraMetadata,
        document_uri: side,
        media_uri: c.metadata.original.media_uri.clone(),
    });
    c.documents[1].text = "<changed/>".into();
    assert_eq!(client.write(c), Err(Error::Conflict));
    assert_eq!(count(&path, "media_heads"), 2);
    assert_eq!(count(&path, "evidence_documents"), 1);
    assert_eq!(count(&path, "write_receipts"), 2);
    let mut replay = a;
    replay.documents[0].text = "<changed/>".into();
    assert_eq!(client.write(replay), Err(Error::Conflict));
    let conn = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT sum(instr(descriptor_json, '<camera/>')) FROM write_receipts",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}
#[test]
fn source_record_cannot_be_duplicated_as_a_new_clip_or_rebound() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("media.sqlite");
    let mut client = local(&path, true, Access::ReadWrite);
    let a = input("A");
    client.write(a.clone()).unwrap();
    let mut duplicate = a;
    duplicate.request_id = "duplicate".into();
    duplicate.metadata.clip_id = "different-clip".into();
    assert_eq!(client.write(duplicate), Err(Error::Conflict));
    let mut other = input("B");
    other.metadata.clip_id = "clip-A".into();
    other.phase = Phase::Final;
    other.expected_revision = 1;
    assert_eq!(client.write(other), Err(Error::Conflict));
    assert_eq!(count(&path, "media_heads"), 1);
}
#[test]
fn stale_revisions_conflicting_requests_and_camera_updates_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut client = local(&dir.path().join("media.sqlite"), true, Access::ReadWrite);
    let a = input("A");
    let receipt = client.write(a.clone()).unwrap();
    assert_eq!(client.write(a.clone()).unwrap(), receipt);
    let mut w = a.clone();
    w.phase = Phase::Final;
    assert_eq!(client.write(w), Err(Error::Conflict));
    let mut w = a;
    w.request_id = "another".into();
    w.phase = Phase::Final;
    assert_eq!(client.write(w.clone()), Err(Error::StaleRevision));
    w.expected_revision = 1;
    w.phase = Phase::Camera;
    assert_eq!(client.write(w), Err(Error::Conflict));
}
#[test]
fn independent_connections_serialize_the_same_idempotent_request() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("media.sqlite");
    drop(local(&path, true, Access::ReadWrite));
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let path = path.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                let mut client = local(&path, false, Access::ReadWrite);
                barrier.wait();
                client.write(input("A")).unwrap()
            })
        })
        .collect();
    let receipts: Vec<_> = handles.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(receipts[0], receipts[1]);
    assert_eq!(count(&path, "media_snapshots"), 1);
}
#[test]
fn rejects_old_schema_and_does_not_create_db_on_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("media.sqlite");
    assert!(Store::open_owner_binding(&path, Access::ReadOnly, false).is_err());
    assert!(!path.exists());
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE old_data(value TEXT); INSERT INTO old_data VALUES('keep');")
        .unwrap();
    assert!(matches!(
        Store::open_owner_binding(&path, Access::ReadWrite, true),
        Err(Error::IncompatibleSchema)
    ));
    assert_eq!(count(&path, "old_data"), 1);
    assert_eq!(count(&path, "sqlite_schema"), 1);
}

struct Host {
    url: String,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Host {
    fn start(path: &Path, uri: &str) -> Self {
        let mut store = Store::open_owner_binding(path, Access::ReadWrite, true).unwrap();
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr());
        let stop = Arc::new(AtomicBool::new(false));
        let end = stop.clone();
        let uri = uri.to_string();
        let thread = thread::spawn(move || {
            let credentials = Credentials::new(READ, WRITE).unwrap();
            while !end.load(Ordering::Relaxed) {
                if let Some(request) = server.recv_timeout(Duration::from_millis(20)).unwrap() {
                    respond(request, &mut store, &uri, &credentials);
                }
            }
        });
        Self {
            url,
            stop,
            thread: Some(thread),
        }
    }
    fn client(&self, uri: &str, token: &str) -> Client {
        let resolver = ResolverConfig::new("")
            .with_lan_authority("test", &self.url)
            .with_intranet_authority("test", &self.url);
        Client::open(&resolver, uri, Access::ReadWrite, Some(token)).unwrap()
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}
#[test]
fn lan_intranet_readonly_grants_receipts_and_documents_match_local_contract() {
    for env in ["lan", "intranet"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("media.sqlite");
        let uri = format!("qnc://{env}/test/db/media_records");
        let host = Host::start(&path, &uri);
        let write = input("A");
        let receipt = host.client(&uri, WRITE).write(write.clone()).unwrap();
        assert_eq!(
            host.client(&uri, WRITE).write(write.clone()).unwrap(),
            receipt
        );
        let mut reader = host.client(&uri, READ);
        assert_eq!(
            reader.read("clip-A", None).unwrap().unwrap().metadata,
            write.metadata
        );
        assert_eq!(
            reader.document(&write.documents[0].document_uri).unwrap(),
            Some(write.documents[0].clone())
        );
        assert_eq!(reader.write(write.clone()), Err(Error::AccessDenied));
        assert_eq!(
            host.client(&uri, "bad-token").read("clip-A", None),
            Err(Error::AccessDenied)
        );
        let other = if env == "lan" { "intranet" } else { "lan" };
        assert_eq!(
            host.client(&format!("qnc://{other}/test/db/media_records"), WRITE)
                .write(write),
            Err(Error::WrongDatabase)
        );
        drop(host);
        assert!(local(&path, false, Access::ReadOnly)
            .read("clip-A", None)
            .unwrap()
            .is_some());
    }
}
#[test]
fn wire_cannot_supply_complete_flag_raw_path_version_or_forged_response() {
    let request = Request {
        version: VERSION.into(),
        db_uri: URI.into(),
        operation: Operation::Write(Box::new(input("A"))),
    };
    let mut json = serde_json::to_value(&request).unwrap();
    json["operation"]["payload"]["complete"] = true.into();
    assert!(serde_json::from_value::<Request>(json).is_err());
    let mut wrong = request.clone();
    wrong.version = "bad".into();
    assert_eq!(wrong.validate(), Err(Error::InvalidRequest));
    for uri in [
        "C:/file.sqlite",
        "qnc://local/db/../media_records",
        " qnc://local/db/media_records",
    ] {
        assert!(validate_db_uri(uri).is_err());
    }
    let reply = Reply {
        version: VERSION.into(),
        db_uri: URI.into(),
        result: Ok(Data::Written(Receipt {
            request_id: "request-A".into(),
            clip_id: "clip-A".into(),
            revision: 1,
            phase: Phase::Camera,
            completeness: Completeness::Partial,
        })),
    };
    assert_eq!(reply.validate(&request), Err(Error::Protocol));
}
#[test]
fn production_modules_do_not_depend_on_apps_parsers_or_scanner() {
    let production = include_str!("../Cargo.toml")
        .split("[dev-dependencies]")
        .next()
        .unwrap();
    for name in [
        "qnc-ingest",
        "qnc-project",
        "qnc-source-reader",
        "qnc-scanner",
        "qnc-sony-metadata",
        "eframe",
    ] {
        assert!(!production.contains(name), "{name}");
    }
    let contract = include_str!("../../qnc-media-records/Cargo.toml");
    for name in [
        "rusqlite",
        "ureq",
        "qnc-json-transport",
        "qnc-source-index-db",
    ] {
        assert!(!contract.contains(name), "{name}");
    }
}

#[test]
fn media_db_contract_and_immutable_json_probe_evidence_round_trip() {
    let contract = qnc_db_contract::DatabaseContract::from_json_str(
        "media-records",
        include_str!("../../../contracts/databases/media-records.database.json"),
    )
    .unwrap();
    assert_eq!(contract.database_id, "qnc.db.media_records");
    assert_eq!(contract.schema_version, VERSION);
    let dir = tempfile::tempdir().unwrap();
    let mut client = local(&dir.path().join("media.sqlite"), true, Access::ReadWrite);
    let mut write = input("A");
    write.phase = Phase::Final;
    let json_uri = "qnc://local/artifact/probe-json-A";
    for e in &mut write.metadata.evidence {
        e.kind = EvidenceKind::Ffprobe;
        e.document_uri = json_uri.into();
    }
    // Supplied producer fixture, not a subprocess invocation or JSON parser test.
    write.documents = vec![Document {
        document_uri: json_uri.into(),
        media_type: DocumentType::Json,
        text: "{\"producer_fixture\":true}".into(),
    }];
    let receipt = client.write(write.clone()).unwrap();
    assert_eq!(receipt.phase, Phase::Final);
    assert_eq!(
        client.document(json_uri).unwrap(),
        Some(write.documents[0].clone())
    );
    assert_eq!(client.write(write).unwrap(), receipt);
}
