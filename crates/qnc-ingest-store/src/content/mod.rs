//! Public project-scoped Ingest output. This module neither scans nor probes media.
mod database;
mod transport;
pub use database::ContentStore;
pub use qnc_json_transport::{Access, Credentials};
use qnc_media_records::{Phase, Snapshot};
use serde::{Deserialize, Serialize};
pub use transport::{respond, ContentClient, ContentTarget, ENDPOINT};

pub const VERSION: &str = "0.2.0";
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
    Select {
        clip_ids: Vec<String>,
        selected: bool,
    },
    QueueSelected,
    ClaimNext,
    FinishImport {
        clip_id: String,
        media_uri: Option<String>,
        error: Option<String>,
    },
}
impl Operation {
    pub fn is_write(&self) -> bool {
        !matches!(self, Self::List { .. } | Self::Inventory { .. })
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

pub(crate) fn ready(clip: &StoredClip) -> bool {
    clip.clip.snapshot.phase == Phase::Final
        && clip.clip.snapshot.completeness == qnc_media_records::Completeness::Complete
}

#[cfg(test)]
mod tests;
