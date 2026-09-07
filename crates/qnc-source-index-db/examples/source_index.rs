//! Live module composition only. Card/catalog read-only, DB is a disposable test artifact.
use qnc_camera_patterns::Catalog;
use qnc_source_index_db::{contract::Batch, Access, Client, Credentials, Store};
use qnc_source_reader::{LocalSource, SourceReader};
use qnc_transport_resolver::ResolverConfig;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const LOCAL: &str = "qnc://local/db/source_index";
const READ: &str = "live-source-index-read-only";
const WRITE: &str = "live-source-index-write-only";

struct Host {
    url: String,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Host {
    fn start(path: &Path, uri: String, catalog: Catalog, source: LocalSource) -> Result<Self> {
        let mut store = Store::open_owner_binding(path, Access::ReadWrite, true)?;
        let server = tiny_http::Server::http("127.0.0.1:0").map_err(|e| e.to_string())?;
        let url = format!("http://{}", server.server_addr());
        let stop = Arc::new(AtomicBool::new(false));
        let end = stop.clone();
        let credentials = Credentials::new(READ, WRITE)?;
        let thread = thread::spawn(move || {
            while !end.load(Ordering::Relaxed) {
                match server.recv_timeout(Duration::from_millis(20)) {
                    Ok(Some(request)) => match request.url() {
                        qnc_camera_patterns::ENDPOINT => {
                            qnc_camera_patterns::respond(request, &catalog, READ)
                        }
                        qnc_source_reader::ENDPOINT => {
                            qnc_source_reader::server::respond(request, &source, READ)
                        }
                        _ => qnc_source_index_db::respond(request, &mut store, &uri, &credentials),
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

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: source_index local|lan-loopback|intranet-loopback <private-catalog-db> <private-card-root>".into());
    }
    let mode = args[0].to_str().ok_or("invalid mode")?;
    let catalog_uri = "qnc://local/catalog/camera-patterns";
    let resolver =
        ResolverConfig::new(PathBuf::new()).with_local_binding(catalog_uri, Path::new(&args[1]));
    let catalog = qnc_camera_patterns::read_uri(&resolver, catalog_uri, None)?;
    let card = Path::new(&args[2]);
    let output = tempfile::tempdir()?;
    let path = output.path().join("source-index.sqlite");
    let start = Instant::now();
    let (catalog, source, mut client, host) = match mode {
        "local" => {
            let resolver = ResolverConfig::new(output.path()).with_local_binding(LOCAL, &path);
            (
                catalog,
                SourceReader::local("qnc://local/source/live-card", card)?,
                Client::create_local(&resolver, LOCAL)?,
                None,
            )
        }
        "lan-loopback" | "intranet-loopback" => {
            let env = mode.strip_suffix("-loopback").unwrap();
            let uri = format!("qnc://{env}/verification/db/source_index");
            let source_uri = format!("qnc://{env}/verification/source/card");
            let mut served = catalog;
            served.catalog_uri = format!("qnc://{env}/verification/catalog/camera-patterns");
            let catalog_uri = served.catalog_uri.clone();
            let host = Host::start(
                &path,
                uri.clone(),
                served,
                LocalSource::new(&source_uri, card)?,
            )?;
            let resolver = ResolverConfig::new(PathBuf::new())
                .with_lan_authority("verification", &host.url)
                .with_intranet_authority("verification", &host.url);
            let catalog = qnc_camera_patterns::read_uri(&resolver, &catalog_uri, Some(READ))?;
            let source = SourceReader::remote(&source_uri, &host.url, READ)?;
            let client = Client::open(&resolver, &uri, Access::ReadWrite, Some(WRITE))?;
            (catalog, source, client, Some(host))
        }
        _ => return Err("unsupported mode".into()),
    };
    let report = qnc_scanner::scan_roles(
        &catalog,
        &source,
        qnc_camera_detector::SourceScope::CardRelative,
        &[&qnc_sony_metadata::SonyIndexReader],
        qnc_scanner::ScanLimits::default(),
    )?;
    if !report.relationships_resolved() {
        return Err("unresolved relationships: no DB write".into());
    }
    let mut ids = vec![];
    let write_start = Instant::now();
    for (i, groups) in report.grouping.groups.chunks(64).enumerate() {
        let references: std::collections::BTreeSet<_> = groups
            .iter()
            .flat_map(|g| g.proposal.references())
            .map(|r| r.uri())
            .collect();
        let batch = Batch {
            batch_id: format!("live-{i}"),
            source_uri: source.source_uri().into(),
            proposals: groups.iter().map(|g| g.proposal.clone()).collect(),
            file_facts: report
                .file_facts
                .iter()
                .filter(|f| references.contains(&f.reference.uri()))
                .cloned()
                .collect(),
        };
        let receipt = client.write(batch.clone())?;
        if client.write(batch)? != receipt {
            return Err("retry changed receipt".into());
        }
        ids.extend(receipt.record_ids);
    }
    let write_ms = write_start.elapsed().as_millis();
    for (id, expected) in ids.iter().zip(&report.grouping.groups) {
        let record = client.read(id)?.ok_or("record missing over transport")?;
        if &record.group != expected {
            return Err("transport readback changed group".into());
        }
    }
    drop(client);
    drop(host);
    drop(source);
    let resolver = ResolverConfig::new(output.path()).with_local_binding(LOCAL, &path);
    let mut reader = Client::open(&resolver, LOCAL, Access::ReadOnly, None)?;
    for (id, expected) in ids.iter().zip(&report.grouping.groups) {
        let record = reader
            .read(id)?
            .ok_or("record missing after writer shutdown")?;
        if &record.group != expected {
            return Err("persisted group changed".into());
        }
    }
    let conn =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let count: usize = conn.query_row("SELECT count(*) FROM public_source_records", [], |r| {
        r.get(0)
    })?;
    if count != ids.len() {
        return Err("duplicate records".into());
    }
    println!(
        "{}",
        serde_json::json!({"mode":mode, "groups":count, "proxies":report.grouping.groups.iter().map(|g| g.proposal.proxies.len()).sum::<usize>(), "related_files":report.grouping.groups.iter().map(|g| g.proposal.related.len()).sum::<usize>(), "replay_identical":true, "read_after_writer_shutdown":true, "write_and_replay_ms":write_ms, "elapsed_ms":start.elapsed().as_millis()})
    );
    Ok(())
}
