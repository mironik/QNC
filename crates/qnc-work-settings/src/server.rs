use crate::{local, ReadReply, ENDPOINT};
use serde::Deserialize;
use std::{io::Read, path::Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    registry_uri: String,
}

// Storage-side read-only endpoint. No SQL, paths, project choices or settings are accepted from clients.
pub fn respond(
    mut request: tiny_http::Request,
    registry_file: &Path,
    registry_uri: &str,
    token: &str,
) {
    let expected = format!("Bearer {token}");
    let authorized = !token.is_empty()
        && request
            .headers()
            .iter()
            .any(|h| h.field.equiv("Authorization") && h.value.as_str() == expected);
    if !authorized {
        let _ = request.respond(tiny_http::Response::empty(401));
        return;
    }
    if request.method() != &tiny_http::Method::Post || request.url() != ENDPOINT {
        let _ = request.respond(tiny_http::Response::empty(404));
        return;
    }
    let mut body = Vec::new();
    if request
        .as_reader()
        .take(4097)
        .read_to_end(&mut body)
        .is_err()
        || body.len() > 4096
    {
        let _ = request.respond(tiny_http::Response::empty(413));
        return;
    }
    let Ok(body) = serde_json::from_slice::<Request>(&body) else {
        let _ = request.respond(tiny_http::Response::empty(400));
        return;
    };
    if body.registry_uri != registry_uri {
        let _ = request.respond(tiny_http::Response::empty(403));
        return;
    }
    let reply = ReadReply {
        result: local::read(registry_file, registry_uri),
    };
    let Ok(bytes) = serde_json::to_vec(&reply) else {
        let _ = request.respond(tiny_http::Response::empty(500));
        return;
    };
    let response = tiny_http::Response::from_data(bytes)
        .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())
        .with_header(tiny_http::Header::from_bytes("Cache-Control", "no-store").unwrap());
    let _ = request.respond(response);
}
