//! Opens source media from the Select configuration: a local binding reads the
//! source directly, a remote binding (LAN or intranet) reads through the media
//! stream endpoint. The caller never sees a path.

use crate::{MediaOpener, MediaRead};
use qnc_ingest_select::selection_config::{ProbeConfig, SourceConfig};
use qnc_media_probe::{Binding, Executor, OwnerConfig, Request};
use qnc_media_stream::{LocalSource, MediaStream, SourceReference};
use std::io::Read;

struct Stream(MediaStream);

impl Read for Stream {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buffer)
    }
}

impl MediaRead for Stream {
    fn byte_len(&self) -> u64 {
        self.0.info().byte_len
    }
}

pub struct ConfigMediaOpener {
    pause: std::sync::Arc<dyn Fn() -> bool + Send + Sync>,
    sources: Vec<SourceConfig>,
}

impl ConfigMediaOpener {
    pub fn new(
        sources: Vec<SourceConfig>,
        pause: std::sync::Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Self {
        Self { sources, pause }
    }
}

impl MediaOpener for ConfigMediaOpener {
    fn paused(&self) -> bool {
        (self.pause)()
    }

    fn describe(
        &self,
        media_uri: &str,
        file: &std::path::Path,
    ) -> Result<qnc_media_metadata::MediaRepresentation, String> {
        self.probe_new_file(media_uri, file)
    }

    fn local_path(&self, media_uri: &str) -> Option<std::path::PathBuf> {
        let reference = SourceReference::from_uri(media_uri).ok()?;
        let source = self
            .sources
            .iter()
            .find(|s| s.location.uri == reference.source_uri())?;
        source.local_media_path(media_uri).ok().flatten()
    }

    fn open(&self, media_uri: &str) -> Result<Box<dyn MediaRead>, String> {
        let reference = SourceReference::from_uri(media_uri).map_err(|e| e.to_string())?;
        let source = self
            .sources
            .iter()
            .find(|s| s.location.uri == reference.source_uri())
            .ok_or_else(|| "Izvor medija nije vezan u konfiguraciji.".to_string())?;
        let stream = if let Some(root) = &source.location.file {
            qnc_dir_browser::verify_local_volume_serial(root, &source.serial_number)
                .map_err(|e| e.to_string())?;
            let local = LocalSource::new(&source.location.uri, root).map_err(|e| e.to_string())?;
            MediaStream::local(&local, media_uri).map_err(|e| e.to_string())?
        } else {
            let resolver = source.location.resolver().map_err(|e| e.to_string())?;
            let token = source
                .location
                .token()
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "Vjerodajnica izvora nedostaje.".to_string())?;
            MediaStream::remote(&resolver, media_uri, &token).map_err(|e| e.to_string())?
        };
        Ok(Box::new(Stream(stream)))
    }
}

impl ConfigMediaOpener {
    /// One probe of a file the import made in the project, with the probe this computer
    /// configures for its sources (the same ffprobe and limits as Select).
    pub(crate) fn probe_new_file(
        &self,
        media_uri: &str,
        file: &std::path::Path,
    ) -> Result<qnc_media_metadata::MediaRepresentation, String> {
        let (executable, probe_size_bytes, analyze_duration_us) = self
            .sources
            .iter()
            .find_map(|source| match &source.probe {
                ProbeConfig::Local { executable, probe_size_bytes, analyze_duration_us } => {
                    Some((executable.clone(), *probe_size_bytes, *analyze_duration_us))
                }
                ProbeConfig::Remote { .. } => None,
            })
            .ok_or("Na ovom racunalu nije zadan lokalni probe za opis nove datoteke.")?;
        let executor = Executor::new(OwnerConfig {
            executable,
            bindings: vec![Binding {
                media_uri: media_uri.into(),
                private_file: file.canonicalize().map_err(|e| e.to_string())?,
            }],
            timeout_ms: 30_000,
            probe_size_bytes,
            analyze_duration_us,
            demuxers: vec!["mov".into()],
        })
        .map_err(|e| e.to_string())?;
        let request_id = uuid_like(media_uri);
        let document_uri = format!("qnc://local/artifact/probe-{request_id}");
        let report = executor
            .execute(&Request {
                version: qnc_media_probe::VERSION.into(),
                request_id,
                media_uri: media_uri.into(),
                document_uri: document_uri.clone(),
            })
            .map_err(|e| e.to_string())?;
        qnc_ffprobe_metadata::read(&report.json, media_uri, &document_uri, "ffprobe-optimized")
            .map(|parsed| parsed.media)
            .map_err(|e| e.to_string())
    }
}

/// A request id from the medium (letters, digits and `-` only).
fn uuid_like(media_uri: &str) -> String {
    let id: String = media_uri
        .chars()
        .rev()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(48)
        .collect();
    format!("optimized-{id}")
}
