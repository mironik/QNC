//! Sony-specific live test composition, not production discovery or an application workflow.
use qnc_media_record_db::{contract::*, Access, Client, Credentials, Store};
use qnc_sony_metadata::{
    read_index, read_metadata, BoundMedia, ClipBinding, SidecarDocument, SonyIndexReader,
    XmlDocument,
};
use qnc_source_groups::IndexReader;
use qnc_source_index_contract::{Batch, FileFact, FileState, SourceReference};
use qnc_source_index_db::{Client as IndexClient, Store as IndexStore};
use qnc_source_reader::{EntryKind, LocalSource, SourceReader, MAX_TEXT_BYTES};
use qnc_transport_resolver::ResolverConfig;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
type TestResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const READ: &str = "live-media-records-read";
const WRITE: &str = "live-media-records-write";
const LOCAL: &str = "qnc://local/db/media_records";
const LOCAL_INDEX: &str = "qnc://local/db/source_index";

struct Host {
    url: String,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Host {
    fn start(path: &Path, index_path: &Path, env: &str, source: LocalSource) -> TestResult<Self> {
        let mut store = Store::open_owner_binding(path, Access::ReadWrite, true)?;
        let mut index = IndexStore::open_owner_binding(index_path, Access::ReadWrite, true)?;
        let uri = format!("qnc://{env}/verification/db/media_records");
        let index_uri = format!("qnc://{env}/verification/db/source_index");
        let server = tiny_http::Server::http("127.0.0.1:0").map_err(|e| e.to_string())?;
        let url = format!("http://{}", server.server_addr());
        let stop = Arc::new(AtomicBool::new(false));
        let end = stop.clone();
        let credentials = Credentials::new(READ, WRITE)?;
        let thread = thread::spawn(move || {
            while !end.load(Ordering::Relaxed) {
                match server.recv_timeout(Duration::from_millis(20)) {
                    Ok(Some(request)) => match request.url() {
                        qnc_source_reader::ENDPOINT => {
                            qnc_source_reader::server::respond(request, &source, READ)
                        }
                        qnc_source_index_db::ENDPOINT => qnc_source_index_db::respond(
                            request,
                            &mut index,
                            &index_uri,
                            &credentials,
                        ),
                        _ => qnc_media_record_db::respond(request, &mut store, &uri, &credentials),
                    },
                    Ok(None) => {}
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            url,
            stop,
            thread: Some(thread),
        })
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn main() -> TestResult<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err(
            "usage: sony_records local|lan-loopback|intranet-loopback <private-recording-root>"
                .into(),
        );
    }
    let mode = args[0].to_str().ok_or("invalid mode")?;
    let root = Path::new(&args[1]);
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("media.sqlite");
    let index_path = temp.path().join("source-index.sqlite");
    let start = Instant::now();
    let (source, mut client, mut source_db, index_uri, host) = match mode {
        "local" => {
            let resolver = ResolverConfig::new(temp.path())
                .with_local_binding(LOCAL, &path)
                .with_local_binding(LOCAL_INDEX, &index_path);
            (
                SourceReader::local("qnc://local/source/live-card", root)?,
                Client::create_local(&resolver, LOCAL)?,
                IndexClient::create_local(&resolver, LOCAL_INDEX)?,
                LOCAL_INDEX.to_string(),
                None,
            )
        }
        "lan-loopback" | "intranet-loopback" => {
            let env = mode.strip_suffix("-loopback").unwrap();
            let source_uri = format!("qnc://{env}/verification/source/card");
            let host = Host::start(
                &path,
                &index_path,
                env,
                LocalSource::new(&source_uri, root)?,
            )?;
            let resolver = ResolverConfig::new(PathBuf::new())
                .with_lan_authority("verification", &host.url)
                .with_intranet_authority("verification", &host.url);
            let uri = format!("qnc://{env}/verification/db/media_records");
            let index_uri = format!("qnc://{env}/verification/db/source_index");
            let source = SourceReader::remote(&source_uri, &host.url, READ)?;
            let client = Client::open(&resolver, &uri, Access::ReadWrite, Some(WRITE))?;
            let index_client =
                IndexClient::open(&resolver, &index_uri, Access::ReadWrite, Some(WRITE))?;
            (source, client, index_client, index_uri, Some(host))
        }
        _ => return Err("unsupported mode".into()),
    };
    let document = source.read_text(&source.reference("MEDIAPRO.XML")?, MAX_TEXT_BYTES)?;
    let index_document = XmlDocument {
        document_uri: document.info.uri,
        text: document.text,
    };
    let index = read_index(&index_document)?;
    let proposals = SonyIndexReader.read(
        &SourceReference::new(source.source_uri(), ".")?,
        &qnc_source_groups::IndexDocument {
            reference: source.reference("MEDIAPRO.XML")?,
            text: index_document.text.clone(),
        },
    )?;
    let unique: BTreeMap<_, _> = proposals
        .iter()
        .flat_map(|p| p.references())
        .map(|r| (r.uri(), r.clone()))
        .collect();
    let mut facts = BTreeMap::new();
    for (uri, reference) in unique {
        let state = match source.stat(&reference) {
            Ok(info) if info.kind == EntryKind::File => FileState::File,
            Err(qnc_source_reader::ReadError::NotFound) => FileState::Missing,
            _ => FileState::Unavailable,
        };
        facts.insert(uri, FileFact { reference, state });
    }
    let mut source_ids = Vec::new();
    for (i, group) in proposals.chunks(64).enumerate() {
        let refs: BTreeSet<_> = group
            .iter()
            .flat_map(|p| p.references())
            .map(|r| r.uri())
            .collect();
        let receipt = source_db.write(Batch {
            batch_id: format!("live-source-{i}"),
            source_uri: source.source_uri().into(),
            proposals: group.to_vec(),
            file_facts: facts
                .iter()
                .filter(|(uri, _)| refs.contains(*uri))
                .map(|(_, f)| f.clone())
                .collect(),
        })?;
        source_ids.extend(receipt.record_ids);
    }
    if source_ids.len() != index.materials.len() {
        return Err("source DB count does not match explicit Sony index".into());
    }
    let mut snapshots = Vec::new();
    let mut documents = BTreeMap::new();
    let index_proof = Document {
        document_uri: index_document.document_uri.clone(),
        media_type: DocumentType::Xml,
        text: index_document.text.clone(),
    };
    documents.insert(index_proof.document_uri.clone(), index_proof.clone());
    for (i, (material, source_id)) in index.materials.iter().zip(&source_ids).enumerate() {
        let source_record = source_db
            .read(source_id)?
            .ok_or("missing persisted source record")?;
        if material.proxies.len() > 1 {
            return Err("multiple proxies require a separate explicit metadata contract".into());
        }
        let bind = |path: &str| -> TestResult<BoundMedia> {
            Ok(BoundMedia {
                relative_path: path.into(),
                media_uri: source.reference(path)?.uri(),
            })
        };
        let binding = ClipBinding {
            clip_id: format!("clip-{source_id}"),
            original: bind(&material.original.relative_path)?,
            proxy: material
                .proxies
                .first()
                .map(|p| bind(&p.relative_path))
                .transpose()?,
        };
        let linked: Vec<_> = material
            .related
            .iter()
            .filter(|r| r.kind == "XML")
            .collect();
        if linked.len() != 1 {
            return Err("test requires one explicitly linked Sony sidecar".into());
        }
        let side =
            source.read_text(&source.reference(&linked[0].relative_path)?, MAX_TEXT_BYTES)?;
        let side_proof = Document {
            document_uri: side.info.uri.clone(),
            media_type: DocumentType::Xml,
            text: side.text.clone(),
        };
        let parsed = read_metadata(
            &index,
            i,
            &binding,
            Some(&SidecarDocument {
                relative_path: linked[0].relative_path.clone(),
                document: XmlDocument {
                    document_uri: side.info.uri,
                    text: side.text,
                },
            }),
        )?;
        if parsed.notices.iter().any(|n| n.code == "conflict") {
            return Err("camera metadata conflict".into());
        }
        documents.insert(side_proof.document_uri.clone(), side_proof.clone());
        let write = Write {
            request_id: format!("camera-{i}"),
            expected_revision: 0,
            phase: Phase::Camera,
            source_index_uri: index_uri.clone(),
            source_record,
            metadata: parsed.metadata,
            documents: vec![index_proof.clone(), side_proof],
        };
        write.validate().map_err(|error| {
            format!(
                "camera record {i}: {error}; invalid metadata: {:?}",
                qnc_media_metadata::inspect(&write.metadata)
                    .issues
                    .into_iter()
                    .filter(|issue| issue.code == qnc_media_metadata::IssueCode::Invalid)
                    .collect::<Vec<_>>()
            )
        })?;
        let receipt = client.write(write.clone())?;
        if client.write(write.clone())? != receipt {
            return Err("replay changed receipt".into());
        }
        let snapshot = client
            .read(&receipt.clip_id, None)?
            .ok_or("missing metadata snapshot")?;
        if snapshot.metadata != write.metadata || snapshot.phase != Phase::Camera {
            return Err("DB altered camera data".into());
        }
        snapshots.push(snapshot);
    }
    drop(client);
    drop(source_db);
    drop(source);
    drop(host);
    let resolver = ResolverConfig::new(temp.path()).with_local_binding(LOCAL, &path);
    let mut reader = Client::open(&resolver, LOCAL, Access::ReadOnly, None)?;
    for snapshot in &snapshots {
        if reader.read(&snapshot.metadata.clip_id, None)?.as_ref() != Some(snapshot) {
            return Err("readback after producer shutdown differs".into());
        }
    }
    for (uri, expected) in &documents {
        if reader.document(uri)?.as_ref() != Some(expected) {
            return Err("archived evidence differs".into());
        }
    }
    let conn =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let doc_count: usize =
        conn.query_row("SELECT count(*) FROM public_media_documents", [], |r| {
            r.get(0)
        })?;
    if doc_count != documents.len() {
        return Err("documents were not deduplicated".into());
    }
    println!(
        "{}",
        serde_json::json!({"mode":mode, "clips":snapshots.len(), "proxies":snapshots.iter().filter(|s| s.metadata.proxy.is_some()).count(), "creation_dates":snapshots.iter().filter(|s| s.metadata.original.tags.contains_key("creation_time")).count(), "complete":snapshots.iter().filter(|s| s.completeness == Completeness::Complete).count(), "partial":snapshots.iter().filter(|s| s.completeness == Completeness::Partial).count(), "archived_documents":doc_count, "replay_identical":true, "read_after_producer_shutdown":true, "elapsed_ms":start.elapsed().as_millis()})
    );
    Ok(())
}
