//! Pure source-index messages. Validation uses supplied facts, never source I/O.
use qnc_contracts::parse_qnc_uri;
pub use qnc_source_contract::SourceReference;
pub use qnc_source_groups::{FileFact, FileState, GroupProposal, SourceGroup};
use serde::{Deserialize, Serialize};

pub const VERSION: &str = "0.1.0";
pub const DATABASE_ID: &str = "qnc.db.source_index";
pub const MAX_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_GROUPS: usize = 256;
pub const MAX_FACTS: usize = 4096;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    InvalidRequest,
    TooLarge,
    Conflict,
    AccessDenied,
    WrongDatabase,
    IncompatibleSchema,
    Unavailable,
    Protocol,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "source index: {self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    pub batch_id: String,
    pub source_uri: String,
    pub proposals: Vec<GroupProposal>,
    pub file_facts: Vec<FileFact>,
}
impl Batch {
    pub fn validate(&self) -> Result<Vec<SourceGroup>> {
        valid_id(&self.batch_id)?;
        if self.proposals.is_empty() {
            return Err(Error::InvalidRequest);
        }
        if self.proposals.len() > MAX_GROUPS || self.file_facts.len() > MAX_FACTS {
            return Err(Error::TooLarge);
        }
        let report =
            qnc_source_groups::assemble(&self.source_uri, self.proposals.clone(), &self.file_facts)
                .map_err(|_| Error::InvalidRequest)?;
        if !report.blocked.is_empty() {
            return Err(Error::InvalidRequest);
        }
        Ok(report.groups)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub record_id: String,
    pub source_uri: String,
    pub group: SourceGroup,
    pub recorded_at_unix_ms: u64,
}
impl Record {
    pub fn validate(&self) -> Result<()> {
        valid_id(&self.record_id)?;
        let p = &self.group.proposal;
        p.validate(&self.source_uri).map_err(|_| Error::Protocol)?;
        let mut facts: Vec<_> = std::iter::once(&p.original)
            .chain(&p.proxies)
            .chain(std::iter::once(&p.evidence.document))
            .map(|reference| FileFact {
                reference: reference.clone(),
                state: FileState::File,
            })
            .collect();
        for fact in &self.group.related_states {
            if let Some(previous) = facts.iter().find(|f| f.reference == fact.reference) {
                if previous != fact {
                    return Err(Error::Protocol);
                }
            } else {
                facts.push(fact.clone());
            }
        }
        let report = qnc_source_groups::assemble(&self.source_uri, vec![p.clone()], &facts)
            .map_err(|_| Error::Protocol)?;
        if report.groups != [self.group.clone()] || !report.blocked.is_empty() {
            return Err(Error::Protocol);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub batch_id: String,
    pub record_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "action",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Operation {
    Write(Batch),
    Read { record_id: String },
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
        match &self.operation {
            Operation::Write(batch) => {
                batch.validate()?;
            }
            Operation::Read { record_id } => valid_id(record_id)?,
        }
        if serde_json::to_vec(self)
            .map_err(|_| Error::InvalidRequest)?
            .len()
            > MAX_BYTES
        {
            return Err(Error::TooLarge);
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
    Record(Option<Box<Record>>),
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
            (Data::Written(receipt), Operation::Write(batch)) => {
                if receipt.batch_id != batch.batch_id
                    || receipt.record_ids.len() != batch.proposals.len()
                {
                    return Err(Error::Protocol);
                }
                let mut ids = std::collections::BTreeSet::new();
                for id in &receipt.record_ids {
                    valid_id(id).map_err(|_| Error::Protocol)?;
                    if !ids.insert(id) {
                        return Err(Error::Protocol);
                    }
                }
            }
            (Data::Record(record), Operation::Read { record_id }) => {
                if let Some(record) = record {
                    record.validate()?;
                    if &record.record_id != record_id {
                        return Err(Error::Protocol);
                    }
                }
            }
            _ => return Err(Error::Protocol),
        }
        Ok(data)
    }
}

pub fn validate_db_uri(uri: &str) -> Result<()> {
    let p = parse_qnc_uri(uri).map_err(|_| Error::InvalidRequest)?;
    if p.resource_kind != "db" || p.resource_id != "source_index" {
        return Err(Error::InvalidRequest);
    }
    let expected = if p.environment == "local" {
        "qnc://local/db/source_index".to_owned()
    } else {
        let authority = p.authority.as_deref().ok_or(Error::InvalidRequest)?;
        valid_id(authority)?;
        format!("qnc://{}/{authority}/db/source_index", p.environment)
    };
    if uri != expected {
        return Err(Error::InvalidRequest);
    }
    Ok(())
}

pub fn valid_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 256
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        Err(Error::InvalidRequest)
    } else {
        Ok(())
    }
}
