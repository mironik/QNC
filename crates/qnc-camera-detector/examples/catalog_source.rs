//! Read-only verification host. Not an application workflow or a resident service.
use qnc_camera_detector::{detect, DetectionReport, Limits, RootFinding, SourceScope};
use qnc_camera_patterns::Catalog;
use qnc_media_metadata::{inspect, StreamDetails};
use qnc_sony_metadata::{
    read_index, read_metadata, BoundMedia, ClipBinding, SidecarDocument, XmlDocument,
};
use qnc_source_reader::{EntryKind, LocalSource, SourceReader, MAX_TEXT_BYTES};
use qnc_transport_resolver::ResolverConfig;
use std::{
    collections::BTreeMap,
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
const TOKEN: &str = "read-only-loopback-verification";

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

fn beneath(root: &RootFinding, relative: &str) -> String {
    if root.root.relative_path() == "." {
        relative.into()
    } else {
        format!("{}/{relative}", root.root.relative_path())
    }
}

fn bind(reader: &SourceReader, root: &RootFinding, relative: &str) -> Result<BoundMedia> {
    let info = reader.stat(&reader.reference(&beneath(root, relative))?)?;
    if info.kind != EntryKind::File {
        return Err("index reference is not a regular file".into());
    }
    Ok(BoundMedia {
        relative_path: relative.into(),
        media_uri: info.uri,
    })
}

fn verify_sony(
    catalog: &Catalog,
    report: &DetectionReport,
    reader: &SourceReader,
) -> Result<Vec<serde_json::Value>> {
    let mut summaries = vec![];
    for root in report.roots.iter().filter(|r| r.has_original_candidates()) {
        let pattern = catalog
            .patterns
            .iter()
            .find(|p| p.id == root.pattern_id)
            .ok_or("missing pattern")?;
        // This verification reader supports Sony's documented index namespace only.
        if !pattern
            .metadata
            .iter()
            .any(|m| m.namespace == "http://xmlns.sony.net/pro/metadata/mediaprofile")
        {
            continue;
        }
        for candidate in root.files.iter().filter(|f| f.role == "index") {
            let document = reader.read_text(&candidate.reference, MAX_TEXT_BYTES)?;
            let index = read_index(&XmlDocument {
                document_uri: document.info.uri,
                text: document.text,
            })?;
            let (
                mut proxies,
                mut xml_reads,
                mut media_stats,
                mut dates,
                mut original_frames,
                mut proxy_frames,
                mut complete,
                mut conflicts,
            ) = (0, 1, 0, 0, 0, 0, 0, 0);
            for (i, material) in index.materials.iter().enumerate() {
                if material.proxies.len() > 1 {
                    return Err("verification requires explicit multi-proxy policy".into());
                }
                let original = bind(reader, root, &material.original.relative_path)?;
                media_stats += 1;
                let proxy = material
                    .proxies
                    .first()
                    .map(|p| bind(reader, root, &p.relative_path))
                    .transpose()?;
                media_stats += usize::from(proxy.is_some());
                let xml: Vec<_> = material
                    .related
                    .iter()
                    .filter(|r| r.kind == "XML")
                    .collect();
                if xml.len() != 1 {
                    return Err("verification expects one explicitly linked XML sidecar".into());
                }
                let document = reader.read_text(
                    &reader.reference(&beneath(root, &xml[0].relative_path))?,
                    MAX_TEXT_BYTES,
                )?;
                xml_reads += 1;
                let sidecar = SidecarDocument {
                    relative_path: xml[0].relative_path.clone(),
                    document: XmlDocument {
                        document_uri: document.info.uri,
                        text: document.text,
                    },
                };
                let result = read_metadata(
                    &index,
                    i,
                    &ClipBinding {
                        clip_id: format!("verification-{i}"),
                        original,
                        proxy,
                    },
                    Some(&sidecar),
                )?;
                let has_frames = |media: &qnc_media_metadata::MediaRepresentation| {
                    media.streams.iter().any(|s| matches!(&s.details, StreamDetails::Video(v) if v.exact_frame_count().is_some()))
                };
                original_frames += usize::from(has_frames(&result.metadata.original));
                dates += usize::from(result.metadata.original.tags.contains_key("creation_time"));
                if let Some(proxy) = &result.metadata.proxy {
                    proxies += 1;
                    proxy_frames += usize::from(has_frames(proxy));
                }
                conflicts += result
                    .notices
                    .iter()
                    .filter(|n| n.code == "conflict")
                    .count();
                complete += usize::from(inspect(&result.metadata).is_complete());
            }
            summaries.push(serde_json::json!({"recording_root": root.root.relative_path(), "materials": index.materials.len(), "proxies": proxies, "xml_reads": xml_reads, "media_stats": media_stats, "creation_dates": dates, "originals_with_exact_frames": original_frames, "proxies_with_exact_frames": proxy_frames, "complete_metadata_records": complete, "conflicts": conflicts}));
        }
    }
    Ok(summaries)
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: catalog_source local|lan-loopback|intranet-loopback <private-catalog-db> <private-card-root>".into());
    }
    let mode = args[0].to_str().ok_or("invalid mode")?;
    let path = Path::new(&args[1]);
    let root = Path::new(&args[2]);
    let start = Instant::now();
    let resolver = ResolverConfig::new(PathBuf::new()).with_local_binding(LOCAL_CATALOG, path);
    let snapshot = qnc_camera_patterns::read_uri(&resolver, LOCAL_CATALOG, None)?;
    let (catalog, reader, _server) = match mode {
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
            let (server, url) = Loopback::start(served, LocalSource::new(&source_uri, root)?)?;
            let resolver = ResolverConfig::new(PathBuf::new())
                .with_lan_authority("verification", &url)
                .with_intranet_authority("verification", &url);
            (
                qnc_camera_patterns::read_uri(&resolver, &catalog_uri, Some(TOKEN))?,
                SourceReader::remote(&source_uri, &url, TOKEN)?,
                Some(server),
            )
        }
        _ => return Err("unknown verification mode".into()),
    };
    let report = detect(
        &catalog,
        &reader,
        SourceScope::CardRelative,
        Limits::default(),
    )?;
    let detection_ms = start.elapsed().as_millis();
    let roots: Vec<_> = report.roots.iter().map(|root| {
        let mut roles = BTreeMap::<&str, usize>::new();
        for file in &root.files { *roles.entry(&file.role).or_default() += 1; }
        serde_json::json!({"pattern": root.pattern_id, "root": root.root.relative_path(), "has_original_candidates": root.has_original_candidates(), "roles": roles})
    }).collect();
    let sony = if report.traversal_complete {
        verify_sony(&catalog, &report, &reader)?
    } else {
        vec![]
    };
    println!(
        "{}",
        serde_json::json!({"mode": mode, "dataset": catalog.dataset_version, "traversal_complete": report.traversal_complete, "directories_listed": report.directories_listed, "issues": report.issues, "ambiguities": report.ambiguities.len(), "coverage_gaps": report.coverage_gap_ids.len(), "roots": roots, "sony_xml": sony, "detection_ms": detection_ms, "total_ms": start.elapsed().as_millis()})
    );
    if !report.traversal_complete {
        return Err("incomplete directory analysis; see issues".into());
    }
    Ok(())
}
