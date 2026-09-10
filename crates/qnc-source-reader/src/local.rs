use crate::*;
use cap_std::{
    ambient_authority,
    fs::{Dir, Metadata},
};
use qnc_transport_resolver::{ResolvedEndpoint, ResolverConfig};
use std::{
    io::{self, Read},
    path::PathBuf,
    sync::Arc,
};

#[derive(Clone)]
pub struct LocalSource {
    source_uri: String,
    directory: Arc<Dir>,
    match_case: MatchCase,
}

impl fmt::Debug for LocalSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalSource")
            .field("source_uri", &self.source_uri)
            .finish_non_exhaustive()
    }
}

impl LocalSource {
    /// Owner-only binding. A storage endpoint can publish this directory under a network URI.
    pub fn new(source_uri: &str, private_directory: impl AsRef<Path>) -> Result<Self, ReadError> {
        let parsed = validate_source_uri(source_uri)?;
        let private_uri = format!("qnc://local/source/{}", parsed.resource_id);
        let resolver = ResolverConfig::new(PathBuf::new())
            .with_local_binding(&private_uri, private_directory.as_ref());
        let ResolvedEndpoint::LocalPath(path) = resolver
            .resolve(&private_uri)
            .map_err(|_| ReadError::TransportConfig)?
            .endpoint
        else {
            return Err(ReadError::TransportConfig);
        };
        let directory = Dir::open_ambient_dir(path, ambient_authority()).map_err(io_error)?;
        Ok(Self {
            source_uri: source_uri.into(),
            directory: Arc::new(directory),
            match_case: MatchCase::Exact,
        })
    }

    pub fn source_uri(&self) -> &str {
        &self.source_uri
    }

    /// Owner-side byte-stream adapter. The handle is read-only and reveals no private path.
    pub fn open_read_only_file(
        &self,
        reference: &SourceReference,
    ) -> Result<std::fs::File, ReadError> {
        reference.validate()?;
        if reference.source_uri() != self.source_uri {
            return Err(ReadError::UnboundSource);
        }
        if !self
            .directory
            .metadata(reference.relative_path())
            .map_err(io_error)?
            .is_file()
        {
            return Err(ReadError::NotFile);
        }
        let file = self
            .directory
            .open(reference.relative_path())
            .map_err(io_error)?;
        if !file.metadata().map_err(io_error)?.is_file() {
            return Err(ReadError::NotFile);
        }
        Ok(file.into_std())
    }

    pub fn with_match_case(mut self, match_case: MatchCase) -> Self {
        self.match_case = match_case;
        self
    }

    pub(crate) fn execute(
        &self,
        reference: &SourceReference,
        operation: &Operation,
    ) -> Result<SourceData, ReadError> {
        reference.validate()?;
        operation.validate()?;
        if reference.source_uri() != self.source_uri {
            return Err(ReadError::UnboundSource);
        }
        if let Operation::List { max_entries } = operation {
            return self.list(reference, *max_entries).map(SourceData::Listing);
        }
        let metadata = self
            .directory
            .metadata(reference.relative_path())
            .map_err(io_error)?;
        let info = file_info(reference, &metadata)?;
        let (Operation::ReadText { max_bytes } | Operation::ReadBytes { max_bytes }) = operation
        else {
            return Ok(SourceData::Stat(info));
        };
        if info.kind != EntryKind::File {
            return Err(ReadError::NotFile);
        }
        if metadata.len() > *max_bytes {
            return Err(ReadError::TooLarge);
        }
        let mut file = self
            .directory
            .open(reference.relative_path())
            .map_err(io_error)?;
        let before = file.metadata().map_err(io_error)?;
        if !before.is_file() {
            return Err(ReadError::NotFile);
        }
        if before.len() > *max_bytes {
            return Err(ReadError::TooLarge);
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take(max_bytes + 1)
            .read_to_end(&mut bytes)
            .map_err(io_error)?;
        if bytes.len() as u64 > *max_bytes {
            return Err(ReadError::TooLarge);
        }
        let after = file.metadata().map_err(io_error)?;
        if bytes.len() as u64 != before.len()
            || after.len() != before.len()
            || after.modified().ok() != before.modified().ok()
        {
            return Err(ReadError::Changed);
        }
        if matches!(operation, Operation::ReadBytes { .. }) {
            return Ok(SourceData::Bytes(BinaryDocument {
                info: file_info(reference, &before)?,
                bytes,
            }));
        }
        let text = String::from_utf8(bytes).map_err(|_| ReadError::InvalidUtf8)?;
        Ok(SourceData::Text(TextDocument {
            info: file_info(reference, &before)?,
            text,
        }))
    }

    fn list(
        &self,
        reference: &SourceReference,
        max_entries: usize,
    ) -> Result<DirectoryListing, ReadError> {
        let dir = self
            .directory
            .open_dir(reference.relative_path())
            .map_err(io_error)?;
        let mut entries = Vec::new();
        for entry in dir.entries().map_err(io_error)? {
            if entries.len() == max_entries {
                return Err(ReadError::TooLarge);
            }
            let entry = entry.map_err(io_error)?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| ReadError::InvalidUtf8)?;
            let path = if reference.relative_path() == "." {
                name.clone()
            } else {
                format!("{}/{}", reference.relative_path(), name)
            };
            let child = SourceReference::new(&self.source_uri, &path)?;
            let kind = entry.file_type().map_err(io_error)?;
            let kind = if kind.is_symlink() {
                EntryKind::Link
            } else if kind.is_dir() {
                EntryKind::Directory
            } else if kind.is_file() {
                EntryKind::File
            } else {
                EntryKind::Other
            };
            entries.push(DirectoryEntry {
                name,
                reference: child,
                kind,
            });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        let listing = DirectoryListing {
            directory_uri: reference.uri(),
            match_case: self.match_case,
            entries,
        };
        listing.validate(reference, max_entries)?;
        if serde_json::to_vec(&listing)
            .map_err(|_| ReadError::Protocol)?
            .len() as u64
            > MAX_LIST_BYTES - 65536
        {
            return Err(ReadError::TooLarge);
        }
        Ok(listing)
    }
}

fn file_info(reference: &SourceReference, metadata: &Metadata) -> Result<FileInfo, ReadError> {
    let (kind, byte_len) = if metadata.is_file() {
        (EntryKind::File, Some(metadata.len()))
    } else if metadata.is_dir() {
        (EntryKind::Directory, None)
    } else {
        return Err(ReadError::UnsupportedType);
    };
    Ok(FileInfo {
        uri: reference.uri(),
        kind,
        byte_len,
    })
}

fn io_error(error: io::Error) -> ReadError {
    match error.kind() {
        io::ErrorKind::NotFound => ReadError::NotFound,
        io::ErrorKind::PermissionDenied => ReadError::AccessDenied,
        _ => ReadError::Unavailable,
    }
}
