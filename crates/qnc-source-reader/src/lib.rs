//! Read-only source access. Private filesystem bindings never cross the wire.

mod local;
mod remote;
pub mod server;

pub use local::LocalSource;
pub use qnc_source_contract::*;
use serde::{Deserialize, Serialize};
use std::{fmt, path::Path};

pub const VERSION: &str = "0.3.0";
pub const ENDPOINT: &str = "/v1/source/read";
pub const MAX_TEXT_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_BINARY_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_REQUEST_BYTES: u64 = 16 * 1024;
pub const MAX_DIRECTORY_ENTRIES: usize = 4096;
pub const MAX_LIST_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Operation {
    Stat,
    ReadText { max_bytes: u64 },
    ReadBytes { max_bytes: u64 },
    List { max_entries: usize },
}

impl Operation {
    fn validate(&self) -> Result<(), ReadError> {
        if matches!(self, Self::ReadBytes { max_bytes } if *max_bytes == 0 || *max_bytes > MAX_BINARY_BYTES)
        {
            return Err(ReadError::TooLarge);
        }
        if matches!(self, Self::List { max_entries } if *max_entries == 0 || *max_entries > MAX_DIRECTORY_ENTRIES)
        {
            return Err(ReadError::TooLarge);
        }
        if matches!(self, Self::ReadText { max_bytes } if *max_bytes == 0 || *max_bytes > MAX_TEXT_BYTES)
        {
            return Err(ReadError::TooLarge);
        }
        Ok(())
    }

    fn response_limit(&self) -> u64 {
        match self {
            Self::Stat => 64 * 1024,
            Self::ReadText { max_bytes } => 6 * max_bytes + 64 * 1024,
            Self::ReadBytes { max_bytes } => 6 * max_bytes + 64 * 1024,
            Self::List { .. } => MAX_LIST_BYTES,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum SourceData {
    Stat(FileInfo),
    Text(TextDocument),
    Bytes(BinaryDocument),
    Listing(DirectoryListing),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: String,
    reference: SourceReference,
    operation: Operation,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    version: String,
    reference: SourceReference,
    result: Result<SourceData, ReadError>,
}

#[derive(Clone)]
pub struct SourceReader {
    backend: Backend,
}

#[derive(Clone)]
enum Backend {
    Local(LocalSource),
    Remote(remote::RemoteSource),
}

impl fmt::Debug for SourceReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceReader")
            .field("source_uri", &self.source_uri())
            .finish_non_exhaustive()
    }
}

impl SourceReader {
    pub fn read_bytes(
        &self,
        reference: &SourceReference,
        max_bytes: u64,
    ) -> Result<BinaryDocument, ReadError> {
        match self.execute(reference, Operation::ReadBytes { max_bytes })? {
            SourceData::Bytes(bytes) => Ok(bytes),
            _ => Err(ReadError::Protocol),
        }
    }
    pub fn local(source_uri: &str, private_directory: impl AsRef<Path>) -> Result<Self, ReadError> {
        Ok(Self::from_local(LocalSource::new(
            source_uri,
            private_directory,
        )?))
    }

    pub fn from_local(source: LocalSource) -> Self {
        Self {
            backend: Backend::Local(source),
        }
    }

    pub fn remote(source_uri: &str, base_url: &str, bearer_token: &str) -> Result<Self, ReadError> {
        Ok(Self {
            backend: Backend::Remote(remote::RemoteSource::new(
                source_uri,
                base_url,
                bearer_token,
            )?),
        })
    }

    pub fn source_uri(&self) -> &str {
        match &self.backend {
            Backend::Local(source) => source.source_uri(),
            Backend::Remote(source) => &source.source_uri,
        }
    }

    pub fn reference(&self, relative_path: &str) -> Result<SourceReference, ReadError> {
        SourceReference::new(self.source_uri(), relative_path)
    }

    pub fn stat(&self, reference: &SourceReference) -> Result<FileInfo, ReadError> {
        match self.execute(reference, Operation::Stat)? {
            SourceData::Stat(info) => Ok(info),
            _ => Err(ReadError::Protocol),
        }
    }

    pub fn list(
        &self,
        reference: &SourceReference,
        max_entries: usize,
    ) -> Result<DirectoryListing, ReadError> {
        match self.execute(reference, Operation::List { max_entries })? {
            SourceData::Listing(listing) => Ok(listing),
            _ => Err(ReadError::Protocol),
        }
    }

    pub fn read_text(
        &self,
        reference: &SourceReference,
        max_bytes: u64,
    ) -> Result<TextDocument, ReadError> {
        match self.execute(reference, Operation::ReadText { max_bytes })? {
            SourceData::Text(document) => Ok(document),
            _ => Err(ReadError::Protocol),
        }
    }

    fn execute(
        &self,
        reference: &SourceReference,
        operation: Operation,
    ) -> Result<SourceData, ReadError> {
        reference.validate()?;
        operation.validate()?;
        if reference.source_uri() != self.source_uri() {
            return Err(ReadError::UnboundSource);
        }
        match &self.backend {
            Backend::Local(source) => source.execute(reference, &operation),
            Backend::Remote(source) => source.execute(reference, operation),
        }
    }
}

#[cfg(test)]
mod tests;
