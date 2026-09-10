use crate::*;
use qnc_transport_resolver::ResolvedEndpoint;
use std::time::Duration;

/// Private transport address for codec adapters, never a persisted media identity.
/// Do not log the authorization header or pass it to another authority.
#[derive(Clone)]
pub struct HttpEndpoint {
    uri: String,
    url: String,
    authorization: String,
}
impl std::fmt::Debug for HttpEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpEndpoint")
            .field("media_uri", &self.uri)
            .finish_non_exhaustive()
    }
}
impl HttpEndpoint {
    pub fn resolve(resolver: &ResolverConfig, uri: &str, token: &str) -> io::Result<Self> {
        reference(uri)?;
        let ResolvedEndpoint::NetworkEndpoint { base_url, .. } =
            resolver.resolve(uri).map_err(|_| invalid())?.endpoint
        else {
            return Err(invalid());
        };
        Self::for_owner_endpoint(&base_url, uri, token)
    }
    /// For an explicit owner-hosted loopback/TLS endpoint, including a local QNC source.
    pub fn for_owner_endpoint(base: &str, uri: &str, token: &str) -> io::Result<Self> {
        reference(uri)?;
        if token.is_empty() || token.len() > 4096 || !token.bytes().all(|b| (33..=126).contains(&b))
        {
            return Err(invalid());
        }
        let mut url = url::Url::parse(base).map_err(|_| invalid())?;
        let loopback = match url.host() {
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            Some(url::Host::Domain("localhost")) => true,
            _ => false,
        };
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.host().is_none()
            || !(url.scheme() == "https" || url.scheme() == "http" && loopback)
        {
            return Err(invalid());
        }
        url.set_path(&format!("{}{ENDPOINT}", url.path().trim_end_matches('/')));
        url.query_pairs_mut().append_pair("uri", uri);
        Ok(Self {
            uri: uri.into(),
            url: url.into(),
            authorization: format!("Bearer {token}"),
        })
    }
    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn authorization_header(&self) -> &str {
        &self.authorization
    }
}

pub(crate) struct RemoteMedia {
    endpoint: HttpEndpoint,
    agent: ureq::Agent,
    pub info: MediaInfo,
    position: u64,
}
impl RemoteMedia {
    pub fn open(endpoint: HttpEndpoint) -> io::Result<Self> {
        // Disable pooling: ureq otherwise retries requests on stale pooled connections.
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(10))
            .redirects(0)
            .max_idle_connections(0)
            .build();
        let response = agent
            .head(&endpoint.url)
            .set("Authorization", &endpoint.authorization)
            .set("Accept-Encoding", "identity")
            .call()
            .map_err(http_error)?;
        if response.status() != 200 {
            return Err(invalid());
        }
        validate_headers(&response, &endpoint.uri)?;
        if response.all("Content-Length").len() != 1 || response.all(STAMP_HEADER).len() != 1 {
            return Err(invalid());
        }
        let byte_len = response
            .header("Content-Length")
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|n| *n <= i64::MAX as u64)
            .ok_or_else(invalid)?;
        let storage_stamp = response
            .header(STAMP_HEADER)
            .filter(|s| {
                !s.is_empty()
                    && s.len() < 128
                    && s.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
            })
            .ok_or_else(invalid)?
            .to_owned();
        let info = MediaInfo {
            media_uri: endpoint.uri.clone(),
            byte_len,
            storage_stamp,
        };
        Ok(Self {
            endpoint,
            agent,
            info,
            position: 0,
        })
    }
}
fn validate_headers(response: &ureq::Response, uri: &str) -> io::Result<()> {
    if response.header("Transfer-Encoding").is_some() {
        return Err(invalid());
    }
    for (name, expected) in [
        (VERSION_HEADER, VERSION),
        (URI_HEADER, uri),
        ("Accept-Ranges", "bytes"),
    ] {
        if response.all(name) != vec![expected] {
            return Err(invalid());
        }
    }
    let encoding = response.all("Content-Encoding");
    if !encoding.is_empty() && encoding != vec!["identity"] {
        return Err(invalid());
    }
    Ok(())
}
impl Read for RemoteMedia {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() || self.position >= self.info.byte_len {
            return Ok(0);
        }
        let count =
            (buffer.len().min(MAX_READ_BYTES) as u64).min(self.info.byte_len - self.position);
        let end = self.position + count - 1;
        let response = self
            .agent
            .get(&self.endpoint.url)
            .set("Authorization", &self.endpoint.authorization)
            .set("Accept-Encoding", "identity")
            .set("Range", &format!("bytes={}-{end}", self.position))
            .set(EXPECTED_STAMP_HEADER, &self.info.storage_stamp)
            .call()
            .map_err(http_error)?;
        if response.status() != 206 {
            return Err(invalid());
        }
        validate_headers(&response, &self.info.media_uri)?;
        let expected_range = format!("bytes {}-{end}/{}", self.position, self.info.byte_len);
        if response.all("Content-Range") != vec![expected_range.as_str()]
            || response.all("Content-Length") != vec![count.to_string().as_str()]
            || response.all(STAMP_HEADER) != vec![self.info.storage_stamp.as_str()]
        {
            return Err(invalid());
        }
        let mut bytes = Vec::with_capacity(count as usize);
        response
            .into_reader()
            .take(count + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 != count {
            return Err(invalid());
        }
        buffer[..bytes.len()].copy_from_slice(&bytes);
        self.position += count;
        Ok(bytes.len())
    }
}
impl Seek for RemoteMedia {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let value = match position {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::Current(n) => i128::from(self.position) + i128::from(n),
            SeekFrom::End(n) => i128::from(self.info.byte_len) + i128::from(n),
        };
        if !(0..=i128::from(i64::MAX)).contains(&value) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid byte seek",
            ));
        }
        self.position = value as u64;
        Ok(self.position)
    }
}
fn http_error(error: ureq::Error) -> io::Error {
    let kind = match error {
        ureq::Error::Status(401 | 403, _) => io::ErrorKind::PermissionDenied,
        ureq::Error::Status(404, _) => io::ErrorKind::NotFound,
        ureq::Error::Status(412, _) => return changed(),
        _ => io::ErrorKind::ConnectionAborted,
    };
    io::Error::new(kind, "Media endpoint request failed; no retry or fallback")
}
