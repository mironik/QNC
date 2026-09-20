use super::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    #[default]
    Local,
    Lan,
    Internet,
}

impl SourceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "Računalo",
            Self::Lan => "LAN",
            Self::Internet => "Internet",
        }
    }

    pub fn action_id(self) -> &'static str {
        match self {
            Self::Local => action_ids::INGEST_SOURCE_KIND_LOCAL,
            Self::Lan => action_ids::INGEST_SOURCE_KIND_LAN,
            Self::Internet => action_ids::INGEST_SOURCE_KIND_INTERNET,
        }
    }
}

pub(crate) fn source_kind_id(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::Local => "local",
        SourceKind::Lan => "lan",
        SourceKind::Internet => "internet",
    }
}
