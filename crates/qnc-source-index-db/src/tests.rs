use super::*;
use contract::*;
use qnc_source_groups::{GroupEvidence, RelatedReference};
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

const URI: &str = "qnc://local/db/source_index";
const SOURCE: &str = "qnc://local/source/test-card";
const READ: &str = "source-index-test-read";
const WRITE: &str = "source-index-test-write";

fn proposal(id: &str) -> GroupProposal {
    let root = SourceReference::new(SOURCE, "RECORDINGS").unwrap();
    GroupProposal {
        original: root.descendant(&format!("{id}.MXF")).unwrap(),
        proxies: vec![root.descendant(&format!("{id}-small.MP4")).unwrap()],
        related: vec![RelatedReference {
            reference: root.descendant(&format!("{id}.XML")).unwrap(),
            kind: "metadata".into(),
        }],
        evidence: GroupEvidence {
            reader_id: "test.index.read".into(),
            document: root.descendant("INDEX.XML").unwrap(),
            locator: id.into(),
        },
        root,
        recording_identity: id.into(),
    }
}
fn batch(id: &str, proposals: Vec<GroupProposal>) -> Batch {
    let facts: BTreeMap<_, _> = proposals
        .iter()
        .flat_map(GroupProposal::references)
        .map(|r| {
            (
                r.uri(),
                FileFact {
                    reference: r.clone(),
                    state: FileState::File,
                },
            )
        })
        .collect();
    Batch {
        batch_id: id.into(),
        source_uri: SOURCE.into(),
        proposals,
        file_facts: facts.into_values().collect(),
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
fn row_count(path: &Path, table: &str) -> usize {
    let conn =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

#[test]
fn restart_and_public_read_without_writer_or_application() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let mut client = local(&path, true, Access::ReadWrite);
    let mut p = proposal("A");
    p.proxies.push(p.root.descendant("A-tiny.MP4").unwrap());
    let input = batch("one", vec![p]);
    let receipt = client.write(input.clone()).unwrap();
    assert_eq!(receipt.record_ids.len(), 1);
    assert_eq!(client.write(input.clone()).unwrap(), receipt);
    let record = client.read(&receipt.record_ids[0]).unwrap().unwrap();
    assert_eq!(record.group.proposal, input.proposals[0]);
    assert!(record.recorded_at_unix_ms > 0);
    drop(client);
    assert_eq!(row_count(&path, "public_source_records"), 1);
    assert_eq!(row_count(&path, "public_source_media"), 3);
    assert_eq!(row_count(&path, "public_source_support"), 2);
    let mut reader = local(&path, false, Access::ReadOnly);
    assert_eq!(reader.read(&record.record_id).unwrap(), Some(record));
    assert_eq!(reader.write(input.clone()), Err(Error::AccessDenied));
    assert_eq!(reader.read("absent").unwrap(), None);
    drop(reader);
    let mut writer = local(&path, false, Access::ReadWrite);
    assert_eq!(writer.write(input.clone()).unwrap(), receipt);
    let mut again = input;
    again.batch_id = "new-request-same-content".into();
    assert_eq!(writer.write(again).unwrap().record_ids, receipt.record_ids);
    assert_eq!(row_count(&path, "public_source_records"), 1);
}

#[test]
fn changed_request_id_or_group_never_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let mut client = local(&path, true, Access::ReadWrite);
    let original = batch("one", vec![proposal("A")]);
    let receipt = client.write(original.clone()).unwrap();
    assert_eq!(
        client.write(batch("one", vec![proposal("B")])),
        Err(Error::Conflict)
    );
    let mut changed = original.proposals[0].clone();
    changed.proxies.clear();
    assert_eq!(
        client.write(batch("two", vec![changed])),
        Err(Error::Conflict)
    );
    assert_eq!(
        client
            .read(&receipt.record_ids[0])
            .unwrap()
            .unwrap()
            .group
            .proposal,
        original.proposals[0]
    );
    assert_eq!(row_count(&path, "write_receipts"), 1);
}

#[test]
fn later_storage_conflict_rolls_back_entire_batch() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let mut client = local(&path, true, Access::ReadWrite);
    let first = proposal("A");
    client.write(batch("one", vec![first.clone()])).unwrap();
    let mut bad = proposal("B");
    bad.original = first.original;
    assert_eq!(
        client.write(batch("two", vec![proposal("C"), bad])),
        Err(Error::Conflict)
    );
    assert_eq!(row_count(&path, "public_source_records"), 1);
    assert_eq!(row_count(&path, "public_source_media"), 2);
    assert_eq!(row_count(&path, "write_receipts"), 1);
    assert!(client.write(batch("two", vec![proposal("C")])).is_ok());
}

#[test]
fn cross_batch_media_support_role_conflicts_are_rejected_both_ways() {
    for media_first in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.sqlite");
        let mut client = local(&path, true, Access::ReadWrite);
        let a = proposal("A");
        let mut b = proposal("B");
        b.related[0].reference = a.original.clone();
        let (first, second) = if media_first { (a, b) } else { (b, a) };
        client.write(batch("one", vec![first])).unwrap();
        assert_eq!(
            client.write(batch("two", vec![second])),
            Err(Error::Conflict)
        );
        assert_eq!(row_count(&path, "public_source_records"), 1);
    }
}

#[test]
fn invalid_required_facts_rejected_but_optional_missing_is_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let mut client = local(&path, true, Access::ReadWrite);
    let mut input = batch("one", vec![proposal("A")]);
    let original = input.proposals[0].original.clone();
    input
        .file_facts
        .iter_mut()
        .find(|f| f.reference == original)
        .unwrap()
        .state = FileState::Missing;
    assert_eq!(client.write(input.clone()), Err(Error::InvalidRequest));
    assert_eq!(row_count(&path, "public_source_records"), 0);
    input
        .file_facts
        .iter_mut()
        .find(|f| f.reference == original)
        .unwrap()
        .state = FileState::File;
    let related = input.proposals[0].related[0].reference.clone();
    input
        .file_facts
        .iter_mut()
        .find(|f| f.reference == related)
        .unwrap()
        .state = FileState::Missing;
    let receipt = client.write(input).unwrap();
    let record = client.read(&receipt.record_ids[0]).unwrap().unwrap();
    assert_eq!(record.group.related_states[0].state, FileState::Missing);
}

#[test]
fn parallel_connections_return_one_committed_identity_and_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    drop(local(&path, true, Access::ReadWrite));
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let path = path.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                let mut client = local(&path, false, Access::ReadWrite);
                barrier.wait();
                client
                    .write(batch("same-request", vec![proposal("A")]))
                    .unwrap()
            })
        })
        .collect();
    let receipts: Vec<_> = handles.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(receipts[0], receipts[1]);
    assert_eq!(row_count(&path, "public_source_records"), 1);
    assert_eq!(row_count(&path, "write_receipts"), 1);
}

#[test]
fn incompatible_schema_is_not_repaired_or_migrated() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TABLE user_data (value TEXT); INSERT INTO user_data VALUES ('keep');",
    )
    .unwrap();
    for initialize in [false, true] {
        assert!(matches!(
            Store::open_owner_binding(&path, Access::ReadWrite, initialize),
            Err(Error::IncompatibleSchema)
        ));
    }
    assert_eq!(row_count(&path, "user_data"), 1);
    assert_eq!(row_count(&path, "sqlite_schema"), 1);
    drop(conn);
    let absent = dir.path().join("missing.sqlite");
    assert!(Store::open_owner_binding(&absent, Access::ReadOnly, false).is_err());
    assert!(!absent.exists());
}

#[test]
fn replaced_public_view_is_rejected_before_execution() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    drop(local(&path, true, Access::ReadWrite));
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(
        "DROP VIEW public_source_records; CREATE VIEW public_source_records AS SELECT 1;",
    )
    .unwrap();
    assert!(matches!(
        Store::open_owner_binding(&path, Access::ReadOnly, false),
        Err(Error::IncompatibleSchema)
    ));
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
fn lan_and_intranet_same_wire_contract_readonly_grant_and_replay() {
    for environment in ["lan", "intranet"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.sqlite");
        let uri = format!("qnc://{environment}/test/db/source_index");
        let host = Host::start(&path, &uri);
        let input = batch("one", vec![proposal("A")]);
        let receipt = host.client(&uri, WRITE).write(input.clone()).unwrap();
        assert_eq!(
            host.client(&uri, WRITE).write(input.clone()).unwrap(),
            receipt
        );
        let mut reader = host.client(&uri, READ);
        assert_eq!(reader.write(input.clone()), Err(Error::AccessDenied));
        assert_eq!(
            reader
                .read(&receipt.record_ids[0])
                .unwrap()
                .unwrap()
                .group
                .proposal,
            input.proposals[0]
        );
        assert_eq!(
            host.client(&uri, "invalid-token")
                .read(&receipt.record_ids[0]),
            Err(Error::AccessDenied)
        );
        let other_env = if environment == "lan" {
            "intranet"
        } else {
            "lan"
        };
        assert_eq!(
            host.client(&format!("qnc://{other_env}/test/db/source_index"), WRITE)
                .write(input),
            Err(Error::WrongDatabase)
        );
        drop(host);
        assert_eq!(row_count(&path, "public_source_records"), 1);
    }
}

#[test]
fn wire_rejects_version_unknown_fields_oversize_and_sql_payload() {
    let dir = tempfile::tempdir().unwrap();
    let uri = "qnc://lan/test/db/source_index";
    let host = Host::start(&dir.path().join("index.sqlite"), uri);
    let post = |bytes: &[u8]| {
        ureq::post(&format!("{}{ENDPOINT}", host.url))
            .set("Authorization", &format!("Bearer {WRITE}"))
            .set("Content-Type", "application/json")
            .send_bytes(bytes)
            .map_err(|error| match error {
                ureq::Error::Status(status, _) => status,
                _ => 0,
            })
    };
    let mut request = Request {
        version: "999".into(),
        db_uri: uri.into(),
        operation: Operation::Write(batch("one", vec![proposal("A")])),
    };
    let reply: Reply = post(&serde_json::to_vec(&request).unwrap())
        .unwrap()
        .into_json()
        .unwrap();
    assert_eq!(reply.result, Err(Error::InvalidRequest));
    request.version = VERSION.into();
    let mut json = serde_json::to_value(&request).unwrap();
    json["sql"] = "DROP TABLE source_records".into();
    assert!(matches!(
        post(&serde_json::to_vec(&json).unwrap()),
        Err(400)
    ));
    assert!(matches!(post(&vec![b' '; MAX_BYTES + 1]), Err(413)));
}

#[test]
fn transport_rejects_raw_paths_insecure_hosts_and_credentials() {
    for base in [
        "http://192.0.2.1",
        "https://user:secret@example.test",
        "https://example.test/?token=x",
        "https://example.test/#x",
    ] {
        let resolver = ResolverConfig::new("").with_lan_authority("test", base);
        assert!(Client::open(
            &resolver,
            "qnc://lan/test/db/source_index",
            Access::ReadOnly,
            Some(READ)
        )
        .is_err());
    }
    assert!(Credentials::new(READ, READ).is_err());
    assert!(Credentials::new("bad\nheader", WRITE).is_err());
    for uri in [
        "C:/data/index.sqlite",
        "qnc://local/db/../source_index",
        " qnc://local/db/source_index",
        "qnc://local/db/source_index?path=x",
    ] {
        assert!(validate_db_uri(uri).is_err());
    }
}

#[test]
fn manifests_and_dependency_boundaries_are_explicit() {
    let db = qnc_db_contract::DatabaseContract::from_json_str(
        "source-index",
        include_str!("../../../contracts/databases/source-index.database.json"),
    )
    .unwrap();
    assert_eq!(db.database_id, DATABASE_ID);
    assert_eq!(db.schema_version, VERSION);
    let production = include_str!("../Cargo.toml")
        .split("[dev-dependencies]")
        .next()
        .unwrap();
    for forbidden in [
        "qnc-scanner",
        "qnc-sony-metadata",
        "qnc-ingest",
        "qnc-project",
        "eframe",
        "qnc-media-metadata",
        "qnc-source-reader",
    ] {
        assert!(
            !production.contains(forbidden),
            "forbidden production dependency {forbidden}"
        );
    }
    for manifest in [
        include_str!("../../../contracts/modules/source-index-contract.module.json"),
        include_str!("../../../contracts/modules/source-index-db.module.json"),
    ] {
        let json: serde_json::Value = serde_json::from_str(manifest).unwrap();
        assert_eq!(json["module_version"], VERSION);
        assert!(json.get("allowed_applications").is_none());
    }
}
