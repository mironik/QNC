use super::*;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum IngestPayload {
    #[default]
    None,
    SourceKind(SourceKind),
    LocationUri(String),
    ClipId(String),
    Bool(bool),
    Frame(i64),
    AudioLane(String),
    ClipFilter(ClipFilter),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IngestIntent {
    pub action_id: String,
    pub payload: IngestPayload,
}

impl IngestIntent {
    pub fn new(action_id: impl Into<String>, payload: IngestPayload) -> Self {
        Self {
            action_id: action_id.into(),
            payload,
        }
    }

    pub fn empty(action_id: impl Into<String>) -> Self {
        Self::new(action_id, IngestPayload::None)
    }
}
