//! Read-only integration check, not an Ingest workflow or camera detector.
use qnc_media_metadata::{inspect, StreamDetails};
use qnc_sony_metadata::{
    read_index, read_metadata, BoundMedia, ClipBinding, SidecarDocument, XmlDocument,
};
use qnc_source_reader::{EntryKind, LocalSource, SourceReader, MAX_TEXT_BYTES};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

struct Loopback {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Loopback {
    fn start(source: LocalSource) -> Result<(Self, SourceReader), Box<dyn std::error::Error>> {
        let server = tiny_http::Server::http("127.0.0.1:0").map_err(|e| e.to_string())?;
        let endpoint = format!("http://{}", server.server_addr());
        let token = "xml-verification-loopback-only";
        let reader = SourceReader::remote(source.source_uri(), &endpoint, token)?;
        let stop = Arc::new(AtomicBool::new(false));
        let end = stop.clone();
        let thread = thread::spawn(move || {
            while !end.load(Ordering::Relaxed) {
                match server.recv_timeout(Duration::from_millis(20)) {
                    Ok(Some(request)) => {
                        qnc_source_reader::server::respond(request, &source, token)
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
            reader,
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

fn bind(reader: &SourceReader, path: &str) -> Result<BoundMedia, Box<dyn std::error::Error>> {
    let reference = reader.reference(path)?;
    let info = reader.stat(&reference)?;
    if info.kind != EntryKind::File {
        return Err("referenced media is not a regular file".into());
    }
    Ok(BoundMedia {
        relative_path: path.into(),
        media_uri: info.uri,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err(
            "usage: sony_source local|lan-loopback|intranet-loopback <private-recording-root>"
                .into(),
        );
    }
    let mode = args[0].to_str().ok_or("invalid mode")?;
    let root = Path::new(&args[1]);
    let (reader, _server) = match mode {
        "local" => (
            SourceReader::local("qnc://local/source/verification-card", root)?,
            None,
        ),
        "lan-loopback" | "intranet-loopback" => {
            let environment = mode.strip_suffix("-loopback").unwrap();
            let uri = format!("qnc://{environment}/verification/source/card");
            let (server, reader) = Loopback::start(LocalSource::new(&uri, root)?)?;
            (reader, Some(server))
        }
        _ => return Err("unsupported verification mode".into()),
    };
    let start = Instant::now();
    let document = reader.read_text(&reader.reference("MEDIAPRO.XML")?, MAX_TEXT_BYTES)?;
    let index = read_index(&XmlDocument {
        document_uri: document.info.uri,
        text: document.text,
    })?;
    let mut proxy_count = 0;
    let mut dates = 0;
    let mut original_frames = 0;
    let mut proxy_frames = 0;
    let mut complete = 0;
    let mut conflicts = 0;
    let mut xml_reads = 1;
    let mut stat_reads = 0;
    for (i, material) in index.materials.iter().enumerate() {
        if material.proxies.len() > 1 {
            return Err("multiple proxy representations need an explicit policy".into());
        }
        let original = bind(&reader, &material.original.relative_path)?;
        stat_reads += 1;
        let proxy = material
            .proxies
            .first()
            .map(|p| bind(&reader, &p.relative_path))
            .transpose()?;
        if proxy.is_some() {
            stat_reads += 1;
        }
        let xml: Vec<_> = material
            .related
            .iter()
            .filter(|r| r.kind == "XML")
            .collect();
        if xml.len() != 1 {
            return Err("verification expects one explicitly linked sidecar".into());
        }
        let side = reader.read_text(&reader.reference(&xml[0].relative_path)?, MAX_TEXT_BYTES)?;
        xml_reads += 1;
        let sidecar = SidecarDocument {
            relative_path: xml[0].relative_path.clone(),
            document: XmlDocument {
                document_uri: side.info.uri,
                text: side.text,
            },
        };
        let binding = ClipBinding {
            clip_id: format!("verification-{i}"),
            original,
            proxy,
        };
        let result = read_metadata(&index, i, &binding, Some(&sidecar))?;
        let has_frames = |media: &qnc_media_metadata::MediaRepresentation| {
            media.streams.iter().any(|s| matches!(&s.details, StreamDetails::Video(v) if v.exact_frame_count().is_some()))
        };
        original_frames += usize::from(has_frames(&result.metadata.original));
        dates += usize::from(result.metadata.original.tags.contains_key("creation_time"));
        if let Some(proxy) = &result.metadata.proxy {
            proxy_count += 1;
            proxy_frames += usize::from(has_frames(proxy));
        }
        conflicts += result
            .notices
            .iter()
            .filter(|n| n.code == "conflict")
            .count();
        complete += usize::from(inspect(&result.metadata).is_complete());
    }
    println!(
        "{}",
        serde_json::json!({
            "mode": mode, "materials": index.materials.len(), "proxies": proxy_count,
            "xml_reads": xml_reads, "media_stat_reads": stat_reads,
            "originals_with_exact_frames": original_frames, "proxies_with_exact_frames": proxy_frames,
            "creation_dates": dates, "complete_metadata_records": complete, "conflicts": conflicts,
            "transport_and_parser_elapsed_ms": start.elapsed().as_millis()
        })
    );
    Ok(())
}
