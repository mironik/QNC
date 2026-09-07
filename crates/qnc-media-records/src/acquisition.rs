//! Durable acquisition messages, without invoking or identifying a consumer application.
use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginAcquisition {
    pub attempt_id: String,
    pub clip_id: String,
    pub expected_revision: u32,
    pub media_uri: String,
    pub document_uri: String,
}
impl BeginAcquisition {
    pub fn validate(&self) -> Result<()> {
        valid_id(&self.attempt_id).map_err(|_| Error::InvalidRequest)?;
        valid_id(&self.clip_id).map_err(|_| Error::InvalidRequest)?;
        validate_resource_uri(&self.media_uri)?;
        validate_resource_uri(&self.document_uri)?;
        if self.expected_revision != 1 || self.media_uri == self.document_uri {
            return Err(Error::InvalidRequest);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum AcquisitionOutcome {
    Stored { document_uri: String },
    Failed { code: String },
    Uncertain { code: String },
}
impl AcquisitionOutcome {
    fn validate(&self, begin: &BeginAcquisition) -> Result<()> {
        match self {
            Self::Stored { document_uri } if document_uri == &begin.document_uri => Ok(()),
            Self::Failed { code } | Self::Uncertain { code } => {
                valid_id(code).map_err(|_| Error::InvalidRequest)
            }
            _ => Err(Error::InvalidRequest),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinishAcquisition {
    pub attempt_id: String,
    pub outcome: AcquisitionOutcome,
    pub document: Option<Document>,
}
impl FinishAcquisition {
    pub fn validate(&self) -> Result<()> {
        valid_id(&self.attempt_id).map_err(|_| Error::InvalidRequest)?;
        bounded(self)?;
        match (&self.outcome, &self.document) {
            (AcquisitionOutcome::Stored { document_uri }, Some(document)) => {
                document.validate()?;
                if document_uri != &document.document_uri
                    || document.media_type != DocumentType::Json
                {
                    return Err(Error::InvalidRequest);
                }
            }
            (
                AcquisitionOutcome::Failed { code } | AcquisitionOutcome::Uncertain { code },
                None,
            ) => {
                valid_id(code).map_err(|_| Error::InvalidRequest)?;
            }
            _ => return Err(Error::InvalidRequest),
        }
        Ok(())
    }

    pub fn validate_for(&self, begin: &BeginAcquisition) -> Result<()> {
        self.validate()?;
        if self.attempt_id != begin.attempt_id {
            return Err(Error::Conflict);
        }
        self.outcome.validate(begin)?;
        if let Some(document) = &self.document {
            // Envelope integrity only. Field interpretation remains in the metadata parser.
            let json: serde_json::Value =
                serde_json::from_str(&document.text).map_err(|_| Error::InvalidMetadata)?;
            if json.pointer("/format/filename").and_then(|v| v.as_str()) != Some(&begin.media_uri)
                || !json.get("streams").is_some_and(|v| v.is_array())
                || json.get("error").is_some()
            {
                return Err(Error::InvalidMetadata);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Acquisition {
    pub request: BeginAcquisition,
    pub started_at_unix_ms: u64,
    pub finished_at_unix_ms: Option<u64>,
    pub outcome: Option<AcquisitionOutcome>,
}
impl Acquisition {
    pub fn validate(&self) -> Result<()> {
        self.request.validate()?;
        if self.started_at_unix_ms == 0
            || self.finished_at_unix_ms.is_some() != self.outcome.is_some()
            || self
                .finished_at_unix_ms
                .is_some_and(|t| t < self.started_at_unix_ms)
        {
            return Err(Error::Protocol);
        }
        if let Some(outcome) = &self.outcome {
            outcome.validate(&self.request)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcquisitionClaim {
    /// Only a newly inserted row grants execution. A replay ALWAYS returns false.
    pub granted: bool,
    pub acquisition: Acquisition,
}
