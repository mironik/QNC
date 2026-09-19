//! Public project-scoped Ingest output. This module neither scans nor probes media.
mod database;
mod transport;
pub use database::ContentStore;
pub use qnc_json_transport::{Access, Credentials};
use qnc_media_records::{Phase, Snapshot};
pub use qnc_wave::WaveArtifactRecord;
use serde::{Deserialize, Serialize};
pub use transport::{
    respond, ContentClient, ContentTarget, ContentWriteCompletion, ContentWriteData,
    ContentWriteResult, ContentWriteTransport, ENDPOINT,
};

pub const VERSION: &str = "0.2.4";
pub const SCHEMA_VERSION: &str = "0.2.0";
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
pub const PAGE_SIZE: usize = 64;
pub type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogClip {
    pub name: String,
    pub source_uri: String,
    pub source_name: String,
    pub serial_number: String,
    pub volume_name: String,
    pub thumbnail_uri: Option<String>,
    pub media_records_uri: String,
    pub snapshot: Snapshot,
}

impl CatalogClip {
    pub fn id(&self) -> &str {
        &self.snapshot.metadata.clip_id
    }
    pub fn validate(&self) -> Result<()> {
        self.snapshot.validate().map_err(|e| e.to_string())?;
        qnc_media_records::validate_db_uri(&self.media_records_uri).map_err(|e| e.to_string())?;
        let source = qnc_contracts::parse_qnc_uri(&self.source_uri).map_err(|e| e.to_string())?;
        if source.resource_kind != "source" || self.name.trim().is_empty() || self.name.len() > 1024
        {
            return Err("Neispravan zapis klipa.".into());
        }
        for uri in std::iter::once(&self.snapshot.binding.original_uri)
            .chain(self.snapshot.binding.proxy_uri.iter())
            .chain(self.thumbnail_uri.iter())
        {
            qnc_contracts::parse_qnc_uri(uri).map_err(|e| e.to_string())?;
            if !uri.starts_with(&format!("{}/", self.source_uri.trim_end_matches('/'))) {
                return Err("Medij ne pripada zapisanom izvoru.".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportStatus {
    Detected,
    Queued,
    Processing,
    Imported,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredClip {
    pub clip: CatalogClip,
    pub selected: bool,
    pub import_status: ImportStatus,
    pub import_error: Option<String>,
    pub imported_media_uri: Option<String>,
}

/// Lightweight UI/catalog row. This is intentionally not enough for playback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredClipSummary {
    pub clip_id: String,
    pub name: String,
    pub source_uri: String,
    pub source_name: String,
    pub serial_number: String,
    pub volume_name: String,
    pub thumbnail_uri: Option<String>,
    pub duration_seconds: f64,
    pub selected: bool,
    pub import_status: ImportStatus,
    pub import_error: Option<String>,
    pub imported_media_uri: Option<String>,
    pub revision: u32,
    pub final_record: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilmstripFrameRecord {
    pub index: usize,
    pub seek_sec: String,
    pub artifact_uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilmstripArtifactRecord {
    pub clip_id: String,
    pub status: String,
    pub duration_sec: String,
    pub frame_count: usize,
    pub artifact_uri: String,
    pub frames: Vec<FilmstripFrameRecord>,
}

/// Lightweight catalog signature for deciding whether a visible catalog is stale.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogStats {
    pub clip_count: u64,
    pub selected_count: u64,
    pub revision_sum: u64,
    pub max_revision: u32,
    pub fingerprint: u64,
}

impl StoredClipSummary {
    pub fn validate(&self) -> Result<()> {
        qnc_media_records::valid_id(&self.clip_id).map_err(|e| e.to_string())?;
        let source = qnc_contracts::parse_qnc_uri(&self.source_uri).map_err(|e| e.to_string())?;
        if source.resource_kind != "source" || self.name.trim().is_empty() {
            return Err("Neispravan sazetak klipa.".into());
        }
        if let Some(uri) = &self.thumbnail_uri {
            qnc_contracts::parse_qnc_uri(uri).map_err(|e| e.to_string())?;
        }
        if let Some(uri) = &self.imported_media_uri {
            qnc_contracts::parse_qnc_uri(uri).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

/// Lightweight reconciliation facts, without reloading probe JSON or thumbnails.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryClip {
    pub clip_id: String,
    pub source_uri: String,
    pub original_uri: String,
    pub revision: u32,
    pub final_record: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "action",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Operation {
    Publish(Box<CatalogClip>),
    PublishBatch(Vec<CatalogClip>),
    Inventory {
        source_uri: String,
        after: Option<String>,
    },
    RemoveMissing {
        clips: Vec<InventoryClip>,
    },
    List {
        after: Option<String>,
    },
    ListSummary {
        after: Option<String>,
    },
    Stats,
    Read {
        clip_id: String,
    },
    PublishFilmstrip(Box<FilmstripArtifactRecord>),
    ReadFilmstrip {
        clip_id: String,
    },
    PublishWave(Box<WaveArtifactRecord>),
    ReadWave {
        clip_id: String,
    },
    Select {
        clip_ids: Vec<String>,
        selected: bool,
    },
    QueueSelected,
    ClaimNext,
    /// The importer still works on this clip: renews its lease.
    Heartbeat {
        clip_id: String,
    },
    FinishImport {
        clip_id: String,
        media_uri: Option<String>,
        /// The poster copied into the project together with the media, if any.
        #[serde(default)]
        thumbnail_uri: Option<String>,
        error: Option<String>,
    },
}
impl Operation {
    pub fn is_write(&self) -> bool {
        !matches!(
            self,
            Self::List { .. }
                | Self::ListSummary { .. }
                | Self::Stats
                | Self::Inventory { .. }
                | Self::Read { .. }
                | Self::ReadFilmstrip { .. }
                | Self::ReadWave { .. }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: String,
    pub db_uri: String,
    pub operation: Operation,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    Saved(Box<StoredClip>),
    Claimed(Option<Box<StoredClip>>),
    Clips(Vec<StoredClip>),
    ClipSummaries(Vec<StoredClipSummary>),
    CatalogStats(CatalogStats),
    Clip(Option<Box<StoredClip>>),
    Filmstrip(Option<Box<FilmstripArtifactRecord>>),
    Wave(Option<Box<WaveArtifactRecord>>),
    Inventory(Vec<InventoryClip>),
    Removed(Vec<String>),
    Changed,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub version: String,
    pub db_uri: String,
    pub result: Result<Data>,
}

pub fn content_uri(workspace_uri: &str) -> Result<String> {
    let uri = qnc_contracts::parse_qnc_uri(workspace_uri).map_err(|e| e.to_string())?;
    if uri.resource_kind != "db" || !uri.resource_id.starts_with("project_workspace/") {
        return Err("Nedostaje projektna baza iz radnih postavki.".into());
    }
    Ok(workspace_uri.replacen("/db/project_workspace/", "/db/ingest_content/", 1))
}

pub(crate) fn project_id(uri: &str) -> Result<String> {
    let parsed = qnc_contracts::parse_qnc_uri(uri).map_err(|e| e.to_string())?;
    if parsed.resource_kind != "db" {
        return Err("Neispravan DB URI.".into());
    }
    let id = parsed
        .resource_id
        .strip_prefix("ingest_content/")
        .ok_or("Nedostaje projektni identitet.")?;
    if id.is_empty() || id.contains(['/', '\\', ':']) || matches!(id, "." | "..") {
        return Err("Neispravan projektni identitet.".into());
    }
    Ok(id.into())
}

/// A clip can be imported when its metadata is final and either complete or declared
/// by the card: a final record without probe evidence is never probed to fill it, so
/// waiting for more would wait forever. A probed record that is still partial stays
/// blocked.
pub(crate) fn ready(clip: &StoredClip) -> bool {
    let snapshot = &clip.clip.snapshot;
    snapshot.phase == Phase::Final
        && (snapshot.completeness == qnc_media_records::Completeness::Complete
            || !snapshot
                .metadata
                .evidence
                .iter()
                .any(|e| e.kind == qnc_media_records::EvidenceKind::Ffprobe))
}

#[cfg(test)]
mod tests;
