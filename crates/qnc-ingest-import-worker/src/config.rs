//! Opens source media from the Select configuration: a local binding reads the
//! source directly, a remote binding (LAN or intranet) reads through the media
//! stream endpoint. The caller never sees a path.

use crate::{MediaOpener, MediaRead};
use qnc_ingest_select::selection_config::SourceConfig;
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
    sources: Vec<SourceConfig>,
}

impl ConfigMediaOpener {
    pub fn new(sources: Vec<SourceConfig>) -> Self {
        Self { sources }
    }
}

impl MediaOpener for ConfigMediaOpener {
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
