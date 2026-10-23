use crate::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestDbTransport {
    pub registry_uri: String,
}

impl IngestDbTransport {
    pub fn local_default() -> Self {
        Self {
            registry_uri: INGEST_REGISTRY_DB_LOCAL_URI.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSelectionRecord {
    pub source_uri: String,
    pub source_kind: String,
    pub display_name: String,
    pub serial_number: String,
    pub volume_name: String,
    pub private_local_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSessionRecord {
    pub session_id: String,
    pub source_uri: String,
    pub card_id: Option<String>,
    pub selected_at_utc: String,
}
