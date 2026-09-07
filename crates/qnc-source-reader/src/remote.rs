use crate::*;
use qnc_transport_resolver::{ResolvedEndpoint, ResolverConfig};
use std::{io::Read, net::IpAddr, path::PathBuf, time::Duration};

#[derive(Clone)]
pub(crate) struct RemoteSource {
    pub source_uri: String,
    endpoint: String,
    token: String,
    agent: ureq::Agent,
}

impl RemoteSource {
    pub fn new(source_uri: &str, base_url: &str, token: &str) -> Result<Self, ReadError> {
        let parsed = validate_source_uri(source_uri)?;
        let authority = parsed
            .authority
            .as_deref()
            .ok_or(ReadError::TransportConfig)?;
        let url = url::Url::parse(base_url).map_err(|_| ReadError::TransportConfig)?;
        let loopback = match url.host() {
            Some(url::Host::Ipv4(ip)) => IpAddr::V4(ip).is_loopback(),
            Some(url::Host::Ipv6(ip)) => IpAddr::V6(ip).is_loopback(),
            Some(url::Host::Domain(host)) => host == "localhost",
            None => false,
        };
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.host().is_none()
            || !(url.scheme() == "https" || url.scheme() == "http" && loopback)
            || !valid_token(token)
        {
            return Err(ReadError::TransportConfig);
        }
        let resolver = ResolverConfig::new(PathBuf::new());
        let resolver = if parsed.environment == "lan" {
            resolver.with_lan_authority(authority, base_url)
        } else {
            resolver.with_intranet_authority(authority, base_url)
        };
        let ResolvedEndpoint::NetworkEndpoint { base_url, .. } = resolver
            .resolve(source_uri)
            .map_err(|_| ReadError::TransportConfig)?
            .endpoint
        else {
            return Err(ReadError::TransportConfig);
        };
        Ok(Self {
            source_uri: source_uri.into(),
            endpoint: format!("{base_url}{ENDPOINT}"),
            token: token.into(),
            agent: ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(10))
                .redirects(0)
                .build(),
        })
    }

    pub fn execute(
        &self,
        reference: &SourceReference,
        operation: Operation,
    ) -> Result<SourceData, ReadError> {
        let request = Request {
            version: VERSION.into(),
            reference: reference.clone(),
            operation,
        };
        let response = self
            .agent
            .post(&self.endpoint)
            .set("Authorization", &format!("Bearer {}", self.token))
            .send_json(serde_json::to_value(&request).map_err(|_| ReadError::Protocol)?)
            .map_err(|error| match error {
                ureq::Error::Status(401 | 403, _) => ReadError::AccessDenied,
                _ => ReadError::Unavailable,
            })?;
        if response.status() != 200
            || response
                .header("Content-Type")
                .and_then(|s| s.split(';').next())
                .map(str::trim)
                != Some("application/json")
        {
            return Err(ReadError::Protocol);
        }
        let limit = request.operation.response_limit();
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ReadError::Unavailable)?;
        if bytes.len() as u64 > limit {
            return Err(ReadError::TooLarge);
        }
        let reply: Reply = serde_json::from_slice(&bytes).map_err(|_| ReadError::Protocol)?;
        validate_reply(reply, &request)
    }
}

pub(crate) fn valid_token(token: &str) -> bool {
    !token.is_empty() && token.len() <= 4096 && token.bytes().all(|b| (33..=126).contains(&b))
}

fn validate_reply(reply: Reply, request: &Request) -> Result<SourceData, ReadError> {
    if reply.version != VERSION || reply.reference != request.reference {
        return Err(ReadError::Protocol);
    }
    let data = reply.result?;
    if let (SourceData::Listing(listing), Operation::List { max_entries }) =
        (&data, &request.operation)
    {
        listing.validate(&request.reference, *max_entries)?;
        return Ok(data);
    }
    let info = match (&data, &request.operation) {
        (SourceData::Bytes(document), Operation::ReadBytes { max_bytes }) => {
            if document.info.kind != EntryKind::File
                || document.info.byte_len != Some(document.bytes.len() as u64)
                || document.bytes.len() as u64 > *max_bytes
            {
                return Err(ReadError::Protocol);
            }
            &document.info
        }
        (SourceData::Stat(info), Operation::Stat) => info,
        (SourceData::Text(document), Operation::ReadText { max_bytes }) => {
            if document.info.kind != EntryKind::File
                || document.info.byte_len != Some(document.text.len() as u64)
                || document.text.len() as u64 > *max_bytes
            {
                return Err(ReadError::Protocol);
            }
            &document.info
        }
        _ => return Err(ReadError::Protocol),
    };
    if info.uri != request.reference.uri()
        || !matches!(info.kind, EntryKind::File | EntryKind::Directory)
        || (info.kind == EntryKind::File) != info.byte_len.is_some()
    {
        return Err(ReadError::Protocol);
    }
    Ok(data)
}
