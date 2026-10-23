//! Public project content database. This module neither scans nor probes media.
mod database;
mod transport;
pub use database::ContentStore;
pub use qnc_json_transport::{Access, Credentials};
use qnc_media_records::{Phase, Snapshot};
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
            return Err(qnc_ui_text::text("db.invalid_clip_record").into());
        }
        for uri in std::iter::once(&self.snapshot.binding.original_uri)
            .chain(self.snapshot.binding.proxy_uri.iter())
        {
            qnc_contracts::parse_qnc_uri(uri).map_err(|e| e.to_string())?;
            if !uri.starts_with(&format!("{}/", self.source_uri.trim_end_matches('/'))) {
                return Err(qnc_ui_text::text("db.media_does_belong_recorded_source").into());
            }
        }
        if let Some(uri) = &self.thumbnail_uri {
            qnc_contracts::parse_qnc_uri(uri).map_err(|e| e.to_string())?;
            let parsed = qnc_contracts::parse_qnc_uri(uri).map_err(|e| e.to_string())?;
            if parsed.resource_kind != "source" && parsed.resource_kind != "project" {
                return Err(qnc_ui_text::text("db.poster_belongs_neither_source_nor_project").into());
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
    /// What the imported file in the project is a byte copy of; `None` when the media is
    /// linked where it is (the imported URI is then the original or proxy itself).
    #[serde(default)]
    pub imported_copy_of: Option<ImportedCopy>,
    /// The optimized copy the import made in the project (H.264 8-bit 4:2:0 of the
    /// original, same frames, timecode and sound), described by its one probe.
    #[serde(default)]
    pub imported_optimized: Option<qnc_media_metadata::MediaRepresentation>,
}

/// An imported media file in the project that is a byte for byte copy of one representation
/// of the clip, so the saved record of that representation describes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportedCopy {
    Original,
    Proxy,
}

impl ImportedCopy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Proxy => "proxy",
        }
    }
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
            return Err(qnc_ui_text::text("db.invalid_clip_summary").into());
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
    Select {
        clip_ids: Vec<String>,
        selected: bool,
    },
    /// The selected clips that serve what the project uses (`needs`, user 2026-10-23).
    QueueSelected { needs: qnc_import_fit::Needs },
    ClaimNext,
    /// The importer still works on this clip: renews its lease.
    Heartbeat {
        clip_id: String,
    },
    /// What one process or form tells the others (playback, worker lease, result).
    SetRuntime {
        key: String,
        value: String,
    },
    GetRuntime {
        key: String,
    },
    /// A lease one process holds (external audit 2026-10-17): taken, renewed and let go
    /// by its owner only, each in one conditional statement, so two processes never both
    /// hold it and the end of one never lets go of the other's.
    Lease {
        key: String,
        owner: String,
        act: LeaseAct,
        /// A lease not renewed for longer than this may be taken by another owner.
        max_age_seconds: i64,
    },
    /// The poster made in the project folder for a clip whose source has none; kept
    /// only when the clip has no poster yet.
    SetPoster {
        clip_id: String,
        thumbnail_uri: String,
    },
    FinishImport {
        clip_id: String,
        media_uri: Option<String>,
        /// The poster copied into the project together with the media, if any.
        #[serde(default)]
        thumbnail_uri: Option<String>,
        /// What the imported file is a byte copy of, when it was copied into the project.
        #[serde(default)]
        copy_of: Option<ImportedCopy>,
        /// The optimized copy the import made, with its probed description.
        #[serde(default)]
        optimized: Option<qnc_media_metadata::MediaRepresentation>,
        error: Option<String>,
    },
    /// A claimed clip with parts (Ingest SubClip, user 2026-10-23) is not imported whole:
    /// it goes back to detected and unticked; its parts come in from their own card.
    FinishAsParts {
        clip_id: String,
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
                | Self::GetRuntime { .. }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: String,
    pub db_uri: String,
    pub operation: Operation,
    /// The lease a write is made under: checked in the same transaction as the write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease: Option<qnc_write_lease::WriteLease>,
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
    Inventory(Vec<InventoryClip>),
    Removed(Vec<String>),
    Runtime(Option<RuntimeEntry>),
    /// The lease act succeeded for its owner.
    Leased(bool),
    Changed,
}

/// What a lease owner does with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseAct {
    /// Takes it when nobody holds it, its holder let it go, it is too old, or it is
    /// already this owner's.
    Claim,
    /// Renews it while it is still this owner's.
    Renew,
    /// Lets it go while it is still this owner's.
    Release,
}

/// The value a held lease has: `on:` and its owner (read as "on" by everyone).
pub use qnc_write_lease::WriteLease;

pub fn lease_value(owner: &str) -> String {
    qnc_write_lease::held_by(owner)
}

/// A runtime entry and how old it is by the clock of the database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeEntry {
    pub value: String,
    pub age_seconds: i64,
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
        return Err(qnc_ui_text::text("db.project_database_work_settings_missing").into());
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
        .ok_or(qnc_ui_text::text("db.project_identity_missing"))?;
    if id.is_empty() || id.contains(['/', '\\', ':']) || matches!(id, "." | "..") {
        return Err(qnc_ui_text::text("db.invalid_project_identity").into());
    }
    Ok(id.into())
}

/// A clip can be imported when its record serves what the project uses (`qnc-import-fit`):
/// a card record is completed in the background (v5), a gap in a representation the
/// project does not use is not in the way (user 2026-10-23).
pub(crate) fn ready(clip: &StoredClip, needs: qnc_import_fit::Needs) -> bool {
    qnc_import_fit::conflict(&clip.clip.snapshot, needs).is_none()
}

#[cfg(test)]
mod tests;
