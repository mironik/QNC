//! Bounded JSON transport only. Callers own wire schemas, authorization per action and lifecycle.
use qnc_transport_resolver::{ResolvedEndpoint, ResolverConfig};
use serde::{de::DeserializeOwned, Serialize};
use std::{io::Read, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Configuration,
    AccessDenied,
    TooLarge,
    Unavailable,
    Protocol,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON transport: {self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    ReadOnly,
    ReadWrite,
}

pub struct Credentials {
    read: String,
    write: String,
}
impl Credentials {
    pub fn new(read_token: &str, write_token: &str) -> Result<Self> {
        if !valid_token(read_token) || !valid_token(write_token) || read_token == write_token {
            return Err(Error::Configuration);
        }
        Ok(Self {
            read: format!("Bearer {read_token}"),
            write: format!("Bearer {write_token}"),
        })
    }
    fn access(&self, header: &str) -> Option<Access> {
        if equal_token(header, &self.write) {
            Some(Access::ReadWrite)
        } else if equal_token(header, &self.read) {
            Some(Access::ReadOnly)
        } else {
            None
        }
    }
}
fn equal_token(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            == 0
}
fn valid_token(token: &str) -> bool {
    !token.is_empty() && token.len() <= 4096 && token.bytes().all(|b| (33..=126).contains(&b))
}
fn valid_endpoint(path: &str) -> bool {
    path.starts_with("/v1/")
        && path.len() < 256
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_-".contains(&b))
}
fn valid_limit(bytes: usize) -> bool {
    bytes > 0 && bytes <= 32 * 1024 * 1024
}

pub struct JsonClient {
    endpoint: String,
    token: String,
    max_bytes: usize,
    agent: ureq::Agent,
}
impl JsonClient {
    /// The caller sets its operation deadline; this never enables retries.
    pub fn with_timeout(mut self, timeout: Duration) -> Result<Self> {
        if timeout.is_zero() || timeout > Duration::from_secs(600) {
            return Err(Error::Configuration);
        }
        self.agent = ureq::AgentBuilder::new()
            .timeout(timeout)
            .redirects(0)
            .build();
        Ok(self)
    }

    pub fn connect(
        resolver: &ResolverConfig,
        uri: &str,
        path: &str,
        token: &str,
        max_bytes: usize,
    ) -> Result<Self> {
        if !valid_token(token) {
            return Err(Error::AccessDenied);
        }
        if !valid_endpoint(path) || !valid_limit(max_bytes) {
            return Err(Error::Configuration);
        }
        let ResolvedEndpoint::NetworkEndpoint { base_url, .. } = resolver
            .resolve(uri)
            .map_err(|_| Error::Configuration)?
            .endpoint
        else {
            return Err(Error::Configuration);
        };
        let url = url::Url::parse(&base_url).map_err(|_| Error::Configuration)?;
        let loopback = match url.host() {
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            Some(url::Host::Domain(host)) => host == "localhost",
            None => false,
        };
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.host().is_none()
            || !(url.scheme() == "https" || url.scheme() == "http" && loopback)
        {
            return Err(Error::Configuration);
        }
        Ok(Self {
            endpoint: format!("{}{path}", base_url.trim_end_matches('/')),
            token: token.into(),
            max_bytes,
            agent: ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(10))
                .redirects(0)
                .build(),
        })
    }
    pub fn post<Q: Serialize, R: DeserializeOwned>(&self, request: &Q) -> Result<R> {
        let bytes = serde_json::to_vec(request).map_err(|_| Error::Protocol)?;
        if bytes.len() > self.max_bytes {
            return Err(Error::TooLarge);
        }
        let response = self
            .agent
            .post(&self.endpoint)
            .set("Authorization", &format!("Bearer {}", self.token))
            .set("Content-Type", "application/json")
            .send_bytes(&bytes)
            .map_err(|e| match e {
                ureq::Error::Status(401 | 403, _) => Error::AccessDenied,
                _ => Error::Unavailable,
            })?;
        if response.status() != 200
            || response
                .header("Content-Type")
                .and_then(|s| s.split(';').next())
                .map(str::trim)
                != Some("application/json")
        {
            return Err(Error::Protocol);
        }
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take((self.max_bytes + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Unavailable)?;
        if bytes.len() > self.max_bytes {
            return Err(Error::TooLarge);
        }
        serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)
    }
}

/// Handler scope is one owner binding. The caller MUST check resource identity and action access.
pub fn respond_json<Q: DeserializeOwned, R: Serialize>(
    mut request: tiny_http::Request,
    path: &str,
    credentials: &Credentials,
    max_bytes: usize,
    handler: impl FnOnce(Q, Access) -> R,
) {
    let auth: Vec<_> = request
        .headers()
        .iter()
        .filter(|h| h.field.equiv("Authorization"))
        .collect();
    let access = if auth.len() == 1 {
        credentials.access(auth[0].value.as_str())
    } else {
        None
    };
    let Some(access) = access else {
        let _ = request.respond(tiny_http::Response::empty(401));
        return;
    };
    if !valid_endpoint(path) || !valid_limit(max_bytes) {
        let _ = request.respond(tiny_http::Response::empty(503));
        return;
    }
    if request.method() != &tiny_http::Method::Post || request.url() != path {
        let _ = request.respond(tiny_http::Response::empty(404));
        return;
    }
    if !request.headers().iter().any(|h| {
        h.field.equiv("Content-Type")
            && h.value.as_str().split(';').next().map(str::trim) == Some("application/json")
    }) {
        let _ = request.respond(tiny_http::Response::empty(415));
        return;
    }
    let mut bytes = Vec::new();
    if request.body_length().is_some_and(|n| n > max_bytes)
        || request
            .as_reader()
            .take((max_bytes + 1) as u64)
            .read_to_end(&mut bytes)
            .is_err()
        || bytes.len() > max_bytes
    {
        let _ = request.respond(tiny_http::Response::empty(413));
        return;
    }
    let Ok(body) = serde_json::from_slice::<Q>(&bytes) else {
        let _ = request.respond(tiny_http::Response::empty(400));
        return;
    };
    let Ok(bytes) = serde_json::to_vec(&handler(body, access)) else {
        let _ = request.respond(tiny_http::Response::empty(500));
        return;
    };
    if bytes.len() > max_bytes {
        let _ = request.respond(tiny_http::Response::empty(413));
        return;
    }
    let _ = request.respond(
        tiny_http::Response::from_data(bytes)
            .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())
            .with_header(tiny_http::Header::from_bytes("Cache-Control", "no-store").unwrap()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_insecure_authorities_bad_routes_and_unbounded_sizes() {
        for base in [
            "http://192.0.2.1",
            "https://user:secret@example.test",
            "https://example.test/?q=x",
            "https://example.test/#fragment",
        ] {
            let resolver = ResolverConfig::new("").with_lan_authority("test", base);
            assert!(JsonClient::connect(
                &resolver,
                "qnc://lan/test/db/example",
                "/v1/example",
                "token",
                1024
            )
            .is_err());
        }
        let resolver = ResolverConfig::new("").with_lan_authority("test", "http://127.0.0.1:9");
        for (path, limit) in [
            ("/v1/example?x=y", 1024),
            ("/v1/../example", 1024),
            ("/v1/example", 0),
            ("/v1/example", 33 * 1024 * 1024),
        ] {
            assert!(JsonClient::connect(
                &resolver,
                "qnc://lan/test/db/example",
                path,
                "token",
                limit
            )
            .is_err());
        }
        assert!(Credentials::new("same", "same").is_err());
        assert!(Credentials::new("bad\nheader", "other").is_err());
    }
    #[test]
    fn hostile_responses_are_bounded_typed_and_not_redirected() {
        for case in [0, 1, 2] {
            let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
            let url = format!("http://{}", server.server_addr());
            let resolver = ResolverConfig::new("").with_lan_authority("test", &url);
            let client = JsonClient::connect(
                &resolver,
                "qnc://lan/test/db/example",
                "/v1/example",
                "token",
                1024,
            )
            .unwrap();
            let thread = std::thread::spawn(move || {
                let request = server
                    .recv_timeout(Duration::from_secs(2))
                    .unwrap()
                    .unwrap();
                let response = match case {
                    0 => tiny_http::Response::from_string("{}").with_header(
                        tiny_http::Header::from_bytes("Content-Type", "text/plain").unwrap(),
                    ),
                    1 => tiny_http::Response::from_string("x".repeat(1025)).with_header(
                        tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
                    ),
                    _ => tiny_http::Response::from_string("")
                        .with_status_code(302)
                        .with_header(
                            tiny_http::Header::from_bytes("Location", "/v1/redirect").unwrap(),
                        ),
                };
                request.respond(response).unwrap();
                assert!(server
                    .recv_timeout(Duration::from_millis(30))
                    .unwrap()
                    .is_none());
            });
            let result = client.post::<_, serde_json::Value>(&serde_json::json!({}));
            if case == 1 {
                assert_eq!(result, Err(Error::TooLarge));
            } else {
                assert!(result.is_err());
            }
            thread.join().unwrap();
        }
    }
    #[test]
    fn request_limit_is_checked_before_network_access() {
        let resolver = ResolverConfig::new("").with_lan_authority("test", "http://127.0.0.1:9");
        let client = JsonClient::connect(
            &resolver,
            "qnc://lan/test/db/example",
            "/v1/example",
            "token",
            16,
        )
        .unwrap();
        assert_eq!(
            client.post::<_, serde_json::Value>(&"x".repeat(17)),
            Err(Error::TooLarge)
        );
    }
}
