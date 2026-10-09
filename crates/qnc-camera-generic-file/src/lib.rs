//! Camera adapter for single media files that no card index describes: a clip sent by
//! FTP, an export from another program, a file copied into the project's `incoming/ftp`
//! (v5: the "Kamera / FTP incoming" source, `scan_media_files`). Each video file is one
//! clip, the original only, with no proxy. Nothing about it is declared, so the record is
//! completed by one probe in Ingest ([`MetadataSufficiency::NeedsProbe`]); no fact is
//! taken from the file name. Used only where the selected folder is no card of a known
//! camera.

use qnc_camera_adapter::{CameraAdapter, MetadataSufficiency};
use qnc_media_metadata::{ClipMetadata, MediaRepresentation};
use qnc_source_contract::SourceReference;
use qnc_source_groups::{FileReader, GroupProposal, IndexDocument, IndexReader};

pub const ADAPTER_ID: &str = "camera.generic.single-file";
pub const PATTERN_ID: &str = "generic-single-file";
pub const READER_ID: &str = "camera.generic.single-file.read";

const PATTERNS: &[&str] = &[PATTERN_ID];

/// The video files v5 takes from an incoming folder (`MEDIA_EXTENSIONS` without the
/// audio-only ones, which v5 removes after discovery).
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mxf", "mov", "mp4", "mts", "m2ts", "avi", "mkv", "m4v", "r3d", "wmv", "mpg", "mpeg", "lrv",
];

#[derive(Debug, Default, Clone, Copy)]
pub struct GenericFile;

impl GenericFile {
    pub fn new() -> Self {
        Self
    }
}

/// There is no index: the reader exists only to name the groups of this adapter.
struct NoIndex;

impl IndexReader for NoIndex {
    fn reader_id(&self) -> &str {
        READER_ID
    }
    fn namespace(&self) -> &str {
        "urn:qnc:generic-single-file"
    }
    fn read(&self, _: &SourceReference, _: &IndexDocument) -> Result<Vec<GroupProposal>, String> {
        Err("single files have no index".into())
    }
}

impl FileReader for GenericFile {
    fn reader_id(&self) -> &str {
        READER_ID
    }
    fn accepts(&self, file: &SourceReference) -> bool {
        let name = file.relative_path().rsplit('/').next().unwrap_or_default();
        !name.starts_with('.')
            && name
                .rsplit_once('.')
                .is_some_and(|(_, ext)| VIDEO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
    }
}

impl CameraAdapter for GenericFile {
    fn adapter_id(&self) -> &str {
        ADAPTER_ID
    }

    fn pattern_ids(&self) -> &[&'static str] {
        PATTERNS
    }

    fn index(&self) -> &dyn IndexReader {
        &NoIndex
    }

    fn documents(&self, _: &GroupProposal) -> Vec<SourceReference> {
        Vec::new()
    }

    fn thumbnail(&self, _: &GroupProposal) -> Option<SourceReference> {
        None
    }

    /// The record knows only which file the clip is.
    fn metadata(
        &self,
        clip_id: &str,
        group: &GroupProposal,
        _: &[IndexDocument],
    ) -> Result<ClipMetadata, String> {
        if !group.is_single_file() || group.evidence.reader_id != READER_ID {
            return Err("not a single-file group of this adapter".into());
        }
        Ok(ClipMetadata {
            contract_id: qnc_media_metadata::CONTRACT_ID.into(),
            contract_version: qnc_media_metadata::CONTRACT_VERSION.into(),
            clip_id: clip_id.into(),
            evidence: vec![],
            original: MediaRepresentation {
                media_uri: group.original.uri(),
                container: None,
                duration_seconds: None,
                streams_complete: None,
                streams: vec![],
                tags: Default::default(),
            },
            proxy: None,
        })
    }

    fn sufficiency(&self, _: &ClipMetadata) -> MetadataSufficiency {
        MetadataSufficiency::NeedsProbe
    }

    fn files(&self) -> Option<&dyn FileReader> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_camera_adapter::CameraRegistry;
    use std::sync::Arc;

    fn file(path: &str) -> SourceReference {
        SourceReference::new("qnc://local/source/disk", path).unwrap()
    }

    #[test]
    fn takes_video_files_only_and_always_probes_once() {
        let adapter = GenericFile::new();
        assert!(adapter.accepts(&file("incoming/ftp/Mironik 2002.MXF")));
        assert!(adapter.accepts(&file("incoming/ftp/a/b.mp4")));
        assert!(!adapter.accepts(&file("incoming/ftp/notes.txt")));
        assert!(!adapter.accepts(&file("incoming/ftp/sound.wav")));
        assert!(!adapter.accepts(&file("incoming/ftp/._hidden.mov")));
        let root = file("incoming/ftp");
        let group = qnc_source_groups::single_file(READER_ID, &root, &file("incoming/ftp/A.MXF"));
        let record = adapter.metadata("clip-1", &group, &[]).unwrap();
        assert_eq!(record.original.media_uri, group.original.uri());
        assert!(record.original.streams.is_empty() && record.proxy.is_none());
        assert_eq!(adapter.sufficiency(&record), MetadataSufficiency::NeedsProbe);
    }

    #[test]
    fn registers_beside_card_cameras_with_its_file_reader() {
        let mut registry = CameraRegistry::new();
        registry.register(Arc::new(GenericFile::new())).unwrap();
        assert_eq!(registry.file_readers().len(), 1);
        assert_eq!(registry.for_reader(READER_ID).unwrap().adapter_id(), ADAPTER_ID);
    }
}
