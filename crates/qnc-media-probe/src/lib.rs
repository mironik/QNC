//! Stateless single-execution helper. No source discovery, metadata merge or DB ownership.
mod process;
pub use process::{Binding, Executor, OwnerConfig};
use qnc_json_transport::JsonClient;
pub use qnc_json_transport::{Access, Credentials};
use qnc_media_records::{valid_id, validate_resource_uri, MAX_DOCUMENT_BYTES};
use qnc_transport_resolver::ResolverConfig;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const VERSION: &str = "0.1.0";
pub const ENDPOINT: &str = "/v1/media-probe";
pub const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    InvalidRequest,
    UnboundMedia,
    AccessDenied,
    Configuration,
    Spawn,
    Timeout,
    OutputLimit,
    Failed,
    InvalidOutput,
    TransportUncertain,
    Protocol,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "media probe: {self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: String,
    pub request_id: String,
    pub media_uri: String,
    pub document_uri: String,
}
impl Request {
    pub fn validate(&self) -> Result<()> {
        if self.version != VERSION
            || valid_id(&self.request_id).is_err()
            || self.media_uri == self.document_uri
        {
            return Err(Error::InvalidRequest);
        }
        validate_resource_uri(&self.media_uri).map_err(|_| Error::InvalidRequest)?;
        validate_resource_uri(&self.document_uri).map_err(|_| Error::InvalidRequest)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub request_id: String,
    pub media_uri: String,
    pub document_uri: String,
    pub json: String,
    pub elapsed_ms: u64,
}
impl Report {
    pub fn validate(&self, request: &Request) -> Result<()> {
        if self.request_id != request.request_id
            || self.media_uri != request.media_uri
            || self.document_uri != request.document_uri
            || self.json.len() > MAX_DOCUMENT_BYTES
        {
            return Err(Error::Protocol);
        }
        let json: serde_json::Value =
            serde_json::from_str(&self.json).map_err(|_| Error::InvalidOutput)?;
        if json.pointer("/format/filename").and_then(|s| s.as_str()) != Some(&self.media_uri)
            || !json.get("streams").is_some_and(|v| v.is_array())
            || json.get("error").is_some()
        {
            return Err(Error::InvalidOutput);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub version: String,
    pub request_id: String,
    pub result: Result<Report>,
}
impl Reply {
    pub fn validate(self, request: &Request) -> Result<Report> {
        if self.version != VERSION || self.request_id != request.request_id {
            return Err(Error::Protocol);
        }
        let report = self.result?;
        report.validate(request)?;
        Ok(report)
    }
}

pub struct Client {
    transport: JsonClient,
}
impl Client {
    pub fn connect(resolver: &ResolverConfig, module_uri: &str, token: &str) -> Result<Self> {
        validate_resource_uri(module_uri).map_err(|_| Error::Configuration)?;
        let parsed = qnc_contracts::parse_qnc_uri(module_uri).map_err(|_| Error::Configuration)?;
        if parsed.resource_kind != "module" || parsed.resource_id != "media-probe" {
            return Err(Error::Configuration);
        }
        Ok(Self {
            transport: JsonClient::connect(resolver, module_uri, ENDPOINT, token, MAX_BYTES)
                .and_then(|c| c.with_timeout(Duration::from_secs(40)))
                .map_err(|_| Error::Configuration)?,
        })
    }
    pub fn execute(&self, request: &Request) -> Result<Report> {
        request.validate()?;
        let reply: Reply = self.transport.post(request).map_err(|e| match e {
            qnc_json_transport::Error::AccessDenied => Error::AccessDenied,
            _ => Error::TransportUncertain,
        })?;
        reply.validate(request)
    }
}

/// Storage/module host owns lifecycle. A lost reply MUST NOT be retried automatically.
pub fn respond(request: tiny_http::Request, executor: &Executor, credentials: &Credentials) {
    qnc_json_transport::respond_json(
        request,
        ENDPOINT,
        credentials,
        MAX_BYTES,
        |request: Request, access| {
            let result = if access == Access::ReadWrite {
                executor.execute(&request)
            } else {
                Err(Error::AccessDenied)
            };
            Reply {
                version: VERSION.into(),
                request_id: request.request_id,
                result,
            }
        },
    );
}

pub trait ProbeBackend: Sync {
    fn execute(&self, request: &Request) -> Result<Report>;
}
impl ProbeBackend for Executor {
    fn execute(&self, request: &Request) -> Result<Report> {
        Executor::execute(self, request)
    }
}
impl ProbeBackend for Client {
    fn execute(&self, request: &Request) -> Result<Report> {
        Client::execute(self, request)
    }
}

/// Bounded independent requests, each exactly once in this call. Failures are returned, not retried.
/// This does not replace the owner's durable Select ledger after a crash or uncertain reply.
pub fn execute_batch(
    backend: &impl ProbeBackend,
    requests: &[Request],
    parallelism: usize,
) -> Result<Vec<Result<Report>>> {
    use std::{
        collections::BTreeSet,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Mutex,
        },
    };
    if !(1..=16).contains(&parallelism) || requests.len() > 256 {
        return Err(Error::InvalidRequest);
    }
    let mut media = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut docs = BTreeSet::new();
    for request in requests {
        request.validate()?;
        if !media.insert(&request.media_uri)
            || !ids.insert(&request.request_id)
            || !docs.insert(&request.document_uri)
        {
            return Err(Error::InvalidRequest);
        }
    }
    let next = AtomicUsize::new(0);
    let output = Mutex::new(vec![Err(Error::Failed); requests.len()]);
    std::thread::scope(|scope| {
        for _ in 0..parallelism.min(requests.len()) {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(request) = requests.get(i) else {
                    break;
                };
                let result = backend.execute(request).and_then(|report| {
                    report.validate(request)?;
                    Ok(report)
                });
                output.lock().expect("batch result lock")[i] = result;
            });
        }
    });
    output.into_inner().map_err(|_| Error::Failed)
}

#[cfg(test)]
mod tests;
