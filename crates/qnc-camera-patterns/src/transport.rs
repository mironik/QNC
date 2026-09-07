use crate::*;
use qnc_transport_resolver::{ResolvedEndpoint, ResolverConfig};
use std::{io::Read, time::Duration};

pub const ENDPOINT: &str = "/v1/camera-patterns/read";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: String,
    catalog_uri: String,
}

fn valid_token(token: &str) -> bool {
    !token.is_empty() && token.len() <= 4096 && token.bytes().all(|b| (33..=126).contains(&b))
}

/// Call outside the UI thread. Local files are opened read-only; remote endpoints return the same projection.
pub fn read_uri(resolver: &ResolverConfig, uri: &str, token: Option<&str>) -> Result<Catalog> {
    validate_uri(uri)?;
    let catalog = match resolver
        .resolve(uri)
        .map_err(|_| "catalog resolver binding unavailable")?
        .endpoint
    {
        ResolvedEndpoint::LocalPath(path) => database::read(&path, uri)?,
        ResolvedEndpoint::NetworkEndpoint { base_url, .. } => {
            let url = url::Url::parse(&base_url).map_err(|_| "invalid catalog endpoint")?;
            let loopback = match url.host() {
                Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
                Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
                Some(url::Host::Domain(name)) => name == "localhost",
                None => false,
            };
            let token = token
                .filter(|t| valid_token(t))
                .ok_or("catalog token missing/invalid")?;
            if !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
                || url.host().is_none()
                || !(url.scheme() == "https" || (url.scheme() == "http" && loopback))
            {
                return Err("catalog endpoint requires HTTPS except loopback".into());
            }
            let response = ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(10))
                .redirects(0)
                .build()
                .post(&format!("{base_url}{ENDPOINT}"))
                .set("Authorization", &format!("Bearer {token}"))
                .send_json(serde_json::json!({"version": VERSION, "catalog_uri": uri}))
                .map_err(|_| "catalog transport unavailable or access denied")?;
            if response.status() != 200
                || response
                    .header("Content-Type")
                    .and_then(|s| s.split(';').next())
                    .map(str::trim)
                    != Some("application/json")
            {
                return Err("invalid catalog response".into());
            }
            let mut bytes = Vec::new();
            response
                .into_reader()
                .take(MAX_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "catalog transport read failed")?;
            if bytes.len() as u64 > MAX_BYTES {
                return Err("catalog response limit exceeded".into());
            }
            serde_json::from_slice(&bytes).map_err(|_| "invalid catalog response")?
        }
    };
    catalog.validate()?;
    if catalog.catalog_uri != uri {
        return Err("catalog response URI mismatch".into());
    }
    Ok(catalog)
}

/// The storage host owns the immutable catalog snapshot, endpoint lifetime and TLS/proxy configuration.
pub fn respond(mut request: tiny_http::Request, catalog: &Catalog, token: &str) {
    let auth: Vec<_> = request
        .headers()
        .iter()
        .filter(|h| h.field.equiv("Authorization"))
        .collect();
    if !valid_token(token) || auth.len() != 1 || auth[0].value.as_str() != format!("Bearer {token}")
    {
        let _ = request.respond(tiny_http::Response::empty(401));
        return;
    }
    if request.url() != ENDPOINT || request.method() != &tiny_http::Method::Post {
        let _ = request.respond(tiny_http::Response::empty(404));
        return;
    }
    let content_types: Vec<_> = request
        .headers()
        .iter()
        .filter(|h| h.field.equiv("Content-Type"))
        .collect();
    if content_types.len() != 1
        || content_types[0]
            .value
            .as_str()
            .split(';')
            .next()
            .map(str::trim)
            != Some("application/json")
    {
        let _ = request.respond(tiny_http::Response::empty(415));
        return;
    }
    let mut bytes = Vec::new();
    if request
        .as_reader()
        .take(4097)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() > 4096
    {
        let _ = request.respond(tiny_http::Response::empty(413));
        return;
    }
    let Ok(body) = serde_json::from_slice::<Request>(&bytes) else {
        let _ = request.respond(tiny_http::Response::empty(400));
        return;
    };
    if body.version != VERSION {
        let _ = request.respond(tiny_http::Response::empty(400));
        return;
    }
    if body.catalog_uri != catalog.catalog_uri {
        let _ = request.respond(tiny_http::Response::empty(403));
        return;
    }
    if catalog.validate().is_err() {
        let _ = request.respond(tiny_http::Response::empty(500));
        return;
    }
    let Ok(bytes) = serde_json::to_vec(catalog) else {
        let _ = request.respond(tiny_http::Response::empty(500));
        return;
    };
    let _ = request.respond(
        tiny_http::Response::from_data(bytes)
            .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())
            .with_header(tiny_http::Header::from_bytes("Cache-Control", "no-store").unwrap()),
    );
}
