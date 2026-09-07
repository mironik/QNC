//! Bounded read-only live verification of public modules, without any QNC app or DB writer.
use qnc_camera_detector::SourceScope;
use qnc_camera_patterns::Catalog;
use qnc_scanner::{scan_roles, ScanLimits};
use qnc_sony_metadata::SonyIndexReader;
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
const LOCAL_CATALOG: &str = "qnc://local/catalog/camera-patterns";
const TOKEN: &str = "group-verification-loopback-only";

struct Loopback {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Loopback {
    fn start(catalog: Catalog, source: LocalSource) -> Result<(Self, String)> {
        let server = tiny_http::Server::http("127.0.0.1:0").map_err(|e| e.to_string())?;
        let url = format!("http://{}", server.server_addr());
        let stop = Arc::new(AtomicBool::new(false));
        let end = stop.clone();
        let thread = thread::spawn(move || {
            while !end.load(Ordering::Relaxed) {
                match server.recv_timeout(Duration::from_millis(20)) {
                    Ok(Some(request)) => {
                        if request.url() == qnc_camera_patterns::ENDPOINT {
                            qnc_camera_patterns::respond(request, &catalog, TOKEN);
                        } else {
                            qnc_source_reader::server::respond(request, &source, TOKEN);
                        }
                    }
                    Ok(None) => {}
                    Err(_) => break,
                }
            }
        });
        Ok((
            Self {
                stop,
                thread: Some(thread),
            },
            url,
        ))
    }
}
impl Drop for Loopback {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: source_groups local|lan-loopback|intranet-loopback <private-catalog-db> <private-card-root>".into());
    }
    let mode = args[0].to_str().ok_or("invalid mode")?;
    let start = Instant::now();
    let resolver =
        ResolverConfig::new(PathBuf::new()).with_local_binding(LOCAL_CATALOG, Path::new(&args[1]));
    let snapshot = qnc_camera_patterns::read_uri(&resolver, LOCAL_CATALOG, None)?;
    let root = Path::new(&args[2]);
    let (catalog, source, _host) = match mode {
        "local" => (
            snapshot,
            SourceReader::local("qnc://local/source/verification-card", root)?,
            None,
        ),
        "lan-loopback" | "intranet-loopback" => {
            let environment = mode.strip_suffix("-loopback").unwrap();
            let catalog_uri = format!("qnc://{environment}/verification/catalog/camera-patterns");
            let source_uri = format!("qnc://{environment}/verification/source/card");
            let mut served = snapshot;
            served.catalog_uri = catalog_uri.clone();
            let (host, url) = Loopback::start(served, LocalSource::new(&source_uri, root)?)?;
            let resolver = ResolverConfig::new(PathBuf::new())
                .with_lan_authority("verification", &url)
                .with_intranet_authority("verification", &url);
            (
                qnc_camera_patterns::read_uri(&resolver, &catalog_uri, Some(TOKEN))?,
                SourceReader::remote(&source_uri, &url, TOKEN)?,
                Some(host),
            )
        }
        _ => return Err("unsupported verification mode".into()),
    };
    // Reader registration belongs to this verification caller, not the public scanner.
    let report = scan_roles(
        &catalog,
        &source,
        SourceScope::CardRelative,
        &[&SonyIndexReader],
        ScanLimits::default(),
    )?;
    let groups = &report.grouping.groups;
    let proxy_count: usize = groups.iter().map(|g| g.proposal.proxies.len()).sum();
    let support_count: usize = groups.iter().map(|g| g.proposal.related.len()).sum();
    println!(
        "{}",
        serde_json::json!({
            "mode": mode, "dataset": catalog.dataset_version, "relationships_resolved": report.relationships_resolved(),
            "directories_listed": report.detection.directories_listed, "indexes_read": report.indexes_read,
            "file_facts": report.file_facts.len(), "groups": groups.len(), "proxies": proxy_count, "related_files": support_count,
            "blocked_groups": report.grouping.blocked.len(), "unresolved_files": report.unresolved_files,
            "issues": report.issues, "elapsed_ms": start.elapsed().as_millis()
        })
    );
    if !report.relationships_resolved() {
        return Err("source relationships are unresolved; no DB write performed".into());
    }
    Ok(())
}
