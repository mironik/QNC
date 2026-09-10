//! Read-only media bytes. No format discovery, decoding, database or application state.
mod bridge;
mod local;
mod range;
mod remote;
pub mod server;

pub use bridge::LoopbackBridge;
pub use qnc_source_reader::{LocalSource, SourceReference};
use qnc_transport_resolver::ResolverConfig;
pub use remote::HttpEndpoint;
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Seek, SeekFrom};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const ENDPOINT: &str = "/v1/media/bytes";
pub const MAX_READ_BYTES: usize = 1024 * 1024;
pub const VERSION_HEADER: &str = "X-QNC-Media-Version";
pub const URI_HEADER: &str = "X-QNC-Media-URI";
pub const STAMP_HEADER: &str = "X-QNC-Media-Stamp";
pub const EXPECTED_STAMP_HEADER: &str = "X-QNC-Expected-Stamp";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaInfo {
    pub media_uri: String,
    pub byte_len: u64,
    /// Storage size/mtime observation, not a content hash or media metadata revision.
    pub storage_stamp: String,
}

pub struct MediaStream {
    backend: Backend,
}
enum Backend {
    Local(local::LocalMedia),
    Remote(remote::RemoteMedia),
}
impl std::fmt::Debug for MediaStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaStream")
            .field("info", self.info())
            .finish_non_exhaustive()
    }
}
impl MediaStream {
    pub fn local(source: &LocalSource, media_uri: &str) -> io::Result<Self> {
        Ok(Self {
            backend: Backend::Local(local::LocalMedia::open(source, media_uri)?),
        })
    }
    pub fn remote(resolver: &ResolverConfig, media_uri: &str, token: &str) -> io::Result<Self> {
        Ok(Self {
            backend: Backend::Remote(remote::RemoteMedia::open(HttpEndpoint::resolve(
                resolver, media_uri, token,
            )?)?),
        })
    }
    pub fn info(&self) -> &MediaInfo {
        match &self.backend {
            Backend::Local(s) => &s.info,
            Backend::Remote(s) => &s.info,
        }
    }
}
impl Read for MediaStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match &mut self.backend {
            Backend::Local(s) => s.read(buffer),
            Backend::Remote(s) => s.read(buffer),
        }
    }
}
impl Seek for MediaStream {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        match &mut self.backend {
            Backend::Local(s) => s.seek(position),
            Backend::Remote(s) => s.seek(position),
        }
    }
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Invalid media byte transport response",
    )
}
fn changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Source file changed during reading",
    )
}
fn reference(uri: &str) -> io::Result<SourceReference> {
    let reference = SourceReference::from_uri(uri).map_err(source_error)?;
    if reference.relative_path() == "." {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Expected a file URI",
        ));
    }
    Ok(reference)
}
fn source_error(e: qnc_source_reader::ReadError) -> io::Error {
    use qnc_source_reader::ReadError as E;
    let kind = match e {
        E::NotFound => io::ErrorKind::NotFound,
        E::AccessDenied | E::UnboundSource => io::ErrorKind::PermissionDenied,
        E::InvalidReference | E::NotFile => io::ErrorKind::InvalidInput,
        _ => io::ErrorKind::Other,
    };
    io::Error::new(kind, e)
}

#[cfg(test)]
mod tests;
