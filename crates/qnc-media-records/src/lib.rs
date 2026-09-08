//! Pure persisted-media contract. No I/O, field merging or process execution.
use qnc_contracts::parse_qnc_uri;
pub use qnc_media_metadata::{ClipMetadata, EvidenceKind, IssueCode, MetadataReport};
pub use qnc_source_index_contract::{valid_id, Record as SourceRecord};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

mod acquisition;
pub use acquisition::*;
mod camera_documents;
pub use camera_documents::{camera_document_uri, freeze_camera_documents};

pub const VERSION: &str = "0.2.0";
pub const CONTRACT_ID: &str = "qnc.media.records";
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    InvalidRequest,
    InvalidMetadata,
    UnsupportedProxySet,
    TooLarge,
    Conflict,
    StaleRevision,
    Finalized,
    AccessDenied,
    WrongDatabase,
    IncompatibleSchema,
    Unavailable,
    Protocol,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "media record: {self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Camera,
    Final,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Completeness {
    Partial,
    Complete,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    Xml,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub document_uri: String,
    pub media_type: DocumentType,
    pub text: String,
}
impl Document {
    pub fn validate(&self) -> Result<()> {
        validate_resource_uri(&self.document_uri)?;
        if self.text.is_empty() {
            return Err(Error::InvalidRequest);
        }
        if self.text.len() > MAX_DOCUMENT_BYTES {
            return Err(Error::TooLarge);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub source_index_uri: String,
    pub source_record_id: String,
    pub original_uri: String,
    pub proxy_uri: Option<String>,
}
impl Binding {
    pub fn validate(&self, metadata: &ClipMetadata) -> Result<()> {
        qnc_source_index_contract::validate_db_uri(&self.source_index_uri)
            .map_err(|_| Error::InvalidRequest)?;
        valid_id(&self.source_record_id).map_err(|_| Error::InvalidRequest)?;
        if self.original_uri != metadata.original.media_uri
            || self.proxy_uri.as_deref() != metadata.proxy.as_ref().map(|p| p.media_uri.as_str())
        {
            return Err(Error::InvalidMetadata);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Write {
    pub request_id: String,
    pub expected_revision: u32,
    pub phase: Phase,
    pub source_index_uri: String,
    pub source_record: SourceRecord,
    pub metadata: ClipMetadata,
    pub documents: Vec<Document>,
}
impl Write {
    pub fn validate(&self) -> Result<(Binding, MetadataReport)> {
        valid_id(&self.request_id).map_err(|_| Error::InvalidRequest)?;
        if self.expected_revision > 1 {
            return Err(Error::StaleRevision);
        }
        if self.documents.len() > 16 {
            return Err(Error::TooLarge);
        }
        bounded(self)?;
        self.source_record
            .validate()
            .map_err(|_| Error::InvalidRequest)?;
        let p = &self.source_record.group.proposal;
        if p.proxies.len() > 1 {
            return Err(Error::UnsupportedProxySet);
        }
        let binding = Binding {
            source_index_uri: self.source_index_uri.clone(),
            source_record_id: self.source_record.record_id.clone(),
            original_uri: p.original.uri(),
            proxy_uri: p.proxies.first().map(|p| p.uri()),
        };
        binding.validate(&self.metadata)?;
        let report = inspect(&self.metadata, self.phase)?;
        let mut documents = BTreeMap::new();
        for doc in &self.documents {
            doc.validate()?;
            if documents.insert(doc.document_uri.as_str(), doc).is_some() {
                return Err(Error::InvalidRequest);
            }
        }
        let camera_documents: BTreeSet<_> = std::iter::once(p.evidence.document.uri())
            .chain(
                self.source_record
                    .group
                    .related_states
                    .iter()
                    .filter(|f| f.state == qnc_source_index_contract::FileState::File)
                    .map(|f| f.reference.uri()),
            )
            .collect();
        let mut used = BTreeSet::new();
        for evidence in &self.metadata.evidence {
            let doc = documents
                .get(evidence.document_uri.as_str())
                .ok_or(Error::InvalidRequest)?;
            used.insert(evidence.document_uri.as_str());
            match evidence.kind {
                EvidenceKind::CameraMetadata
                    if !camera_documents.contains(&evidence.document_uri)
                        && !camera_documents.iter().any(|uri| {
                            camera_document_uri(uri, &doc.text).as_ref()
                                == Ok(&evidence.document_uri)
                        }) =>
                {
                    return Err(Error::InvalidMetadata)
                }
                EvidenceKind::Ffprobe if doc.media_type != DocumentType::Json => {
                    return Err(Error::InvalidMetadata)
                }
                _ => {}
            }
        }
        if used.len() != documents.len() {
            return Err(Error::InvalidRequest);
        }
        Ok((binding, report))
    }
}

pub fn inspect(metadata: &ClipMetadata, phase: Phase) -> Result<MetadataReport> {
    valid_id(&metadata.clip_id).map_err(|_| Error::InvalidMetadata)?;
    if metadata.evidence.len() > 64
        || std::iter::once(&metadata.original)
            .chain(&metadata.proxy)
            .any(|r| r.streams.len() > 128 || r.tags.len() > 512)
    {
        return Err(Error::TooLarge);
    }
    bounded(metadata)?;
    if phase == Phase::Camera
        && metadata
            .evidence
            .iter()
            .any(|e| e.kind == EvidenceKind::Ffprobe)
    {
        return Err(Error::InvalidMetadata);
    }
    let report = qnc_media_metadata::inspect(metadata);
    if report.issues.iter().any(|i| i.code == IssueCode::Invalid) {
        return Err(Error::InvalidMetadata);
    }
    Ok(report)
}
pub fn completeness(report: &MetadataReport) -> Completeness {
    if report.is_complete() {
        Completeness::Complete
    } else {
        Completeness::Partial
    }
}
pub fn next_revision(previous: Option<(u32, Phase)>, expected: u32, next: Phase) -> Result<u32> {
    match previous {
        None if expected == 0 => Ok(1),
        Some((_, Phase::Final)) => Err(Error::Finalized),
        Some((revision, Phase::Camera)) if revision == expected && next == Phase::Final => {
            Ok(revision + 1)
        }
        Some((revision, _)) if revision == expected => Err(Error::Conflict),
        _ => Err(Error::StaleRevision),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub binding: Binding,
    pub revision: u32,
    pub phase: Phase,
    pub completeness: Completeness,
    pub metadata: ClipMetadata,
    pub report: MetadataReport,
    pub recorded_at_unix_ms: u64,
}
impl Snapshot {
    pub fn validate(&self) -> Result<()> {
        self.binding.validate(&self.metadata)?;
        if !matches!(self.revision, 1 | 2) || (self.revision == 2 && self.phase != Phase::Final) {
            return Err(Error::Protocol);
        }
        let report = inspect(&self.metadata, self.phase)?;
        if self.report != report || self.completeness != completeness(&report) {
            return Err(Error::Protocol);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub request_id: String,
    pub clip_id: String,
    pub revision: u32,
    pub phase: Phase,
    pub completeness: Completeness,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "action",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Operation {
    Write(Box<Write>),
    BeginAcquisition(BeginAcquisition),
    FinishAcquisition(FinishAcquisition),
    Acquisition {
        media_uri: String,
    },
    Read {
        clip_id: String,
        revision: Option<u32>,
    },
    Document {
        document_uri: String,
    },
}
impl Operation {
    pub fn is_write(&self) -> bool {
        matches!(
            self,
            Self::Write(_) | Self::BeginAcquisition(_) | Self::FinishAcquisition(_)
        )
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: String,
    pub db_uri: String,
    pub operation: Operation,
}
impl Request {
    pub fn validate(&self) -> Result<()> {
        if self.version != VERSION {
            return Err(Error::InvalidRequest);
        }
        validate_db_uri(&self.db_uri)?;
        bounded(self)?;
        match &self.operation {
            Operation::Write(w) => {
                w.validate()?;
            }
            Operation::Read { clip_id, revision } => {
                valid_id(clip_id).map_err(|_| Error::InvalidRequest)?;
                if revision.is_some_and(|r| !matches!(r, 1 | 2)) {
                    return Err(Error::InvalidRequest);
                }
            }
            Operation::Document { document_uri } => validate_resource_uri(document_uri)?,
            Operation::BeginAcquisition(begin) => begin.validate()?,
            Operation::FinishAcquisition(finish) => finish.validate()?,
            Operation::Acquisition { media_uri } => validate_resource_uri(media_uri)?,
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Data {
    Written(Receipt),
    AcquisitionClaim(Box<AcquisitionClaim>),
    Acquisition(Option<Box<Acquisition>>),
    Snapshot(Option<Box<Snapshot>>),
    Document(Option<Document>),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub version: String,
    pub db_uri: String,
    pub result: Result<Data>,
}
impl Reply {
    pub fn validate(self, request: &Request) -> Result<Data> {
        if self.version != VERSION || self.db_uri != request.db_uri {
            return Err(Error::Protocol);
        }
        let data = self.result?;
        match (&data, &request.operation) {
            (Data::AcquisitionClaim(claim), Operation::BeginAcquisition(begin)) => {
                claim.acquisition.validate()?;
                if claim.acquisition.request.media_uri != begin.media_uri
                    || (claim.granted
                        && (claim.acquisition.request != *begin
                            || claim.acquisition.outcome.is_some()))
                {
                    return Err(Error::Protocol);
                }
            }
            (Data::Acquisition(Some(attempt)), Operation::FinishAcquisition(finish)) => {
                attempt.validate()?;
                finish.validate_for(&attempt.request)?;
                if attempt.outcome.as_ref() != Some(&finish.outcome) {
                    return Err(Error::Protocol);
                }
            }
            (Data::Acquisition(attempt), Operation::Acquisition { media_uri }) => {
                if let Some(attempt) = attempt {
                    attempt.validate()?;
                    if &attempt.request.media_uri != media_uri {
                        return Err(Error::Protocol);
                    }
                }
            }
            (Data::Written(r), Operation::Write(w)) => {
                if r.request_id != w.request_id
                    || r.clip_id != w.metadata.clip_id
                    || r.phase != w.phase
                    || r.revision != w.expected_revision + 1
                    || r.completeness != completeness(&inspect(&w.metadata, w.phase)?)
                {
                    return Err(Error::Protocol);
                }
            }
            (Data::Snapshot(Some(s)), Operation::Read { clip_id, revision }) => {
                s.validate()?;
                if &s.metadata.clip_id != clip_id || revision.is_some_and(|r| r != s.revision) {
                    return Err(Error::Protocol);
                }
            }
            (Data::Document(Some(d)), Operation::Document { document_uri }) => {
                d.validate()?;
                if &d.document_uri != document_uri {
                    return Err(Error::Protocol);
                }
            }
            (Data::Snapshot(None), Operation::Read { .. })
            | (Data::Document(None), Operation::Document { .. }) => {}
            _ => return Err(Error::Protocol),
        }
        Ok(data)
    }
}

pub fn validate_db_uri(uri: &str) -> Result<()> {
    let p = parse_qnc_uri(uri).map_err(|_| Error::InvalidRequest)?;
    if p.resource_kind != "db" || p.resource_id != "media_records" {
        return Err(Error::InvalidRequest);
    }
    let expected = if p.environment == "local" {
        "qnc://local/db/media_records".into()
    } else {
        let authority = p.authority.ok_or(Error::InvalidRequest)?;
        valid_id(&authority).map_err(|_| Error::InvalidRequest)?;
        format!("qnc://{}/{authority}/db/media_records", p.environment)
    };
    if uri != expected {
        return Err(Error::InvalidRequest);
    }
    Ok(())
}
pub fn validate_resource_uri(uri: &str) -> Result<()> {
    let p = parse_qnc_uri(uri).map_err(|_| Error::InvalidRequest)?;
    if uri.len() > 8192
        || uri != uri.trim()
        || p.resource_kind.contains(':')
        || p.resource_id.contains(':')
        || uri.split('/').any(|s| {
            s == "."
                || s == ".."
                || s.chars()
                    .any(|c| c.is_control() || matches!(c, '\\' | '?' | '#'))
        })
    {
        return Err(Error::InvalidRequest);
    }
    Ok(())
}
fn bounded(value: &impl Serialize) -> Result<()> {
    if serde_json::to_vec(value)
        .map_err(|_| Error::InvalidRequest)?
        .len()
        > MAX_BYTES
    {
        Err(Error::TooLarge)
    } else {
        Ok(())
    }
}
