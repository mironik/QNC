//! Storage-side handler. Hosting/TLS/lifecycle belong to the caller, not an app workflow.

use crate::*;
use std::io::Read;

pub fn respond(mut request: tiny_http::Request, source: &LocalSource, bearer_token: &str) {
    let expected = format!("Bearer {bearer_token}");
    let auth: Vec<_> = request
        .headers()
        .iter()
        .filter(|h| h.field.equiv("Authorization"))
        .collect();
    if !remote::valid_token(bearer_token) || auth.len() != 1 || auth[0].value.as_str() != expected {
        let _ = request.respond(tiny_http::Response::empty(401));
        return;
    }
    if request.method() != &tiny_http::Method::Post || request.url() != ENDPOINT {
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
    if request
        .as_reader()
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() as u64 > MAX_REQUEST_BYTES
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
    let result = source.execute(&body.reference, &body.operation);
    let reply = Reply {
        version: VERSION.into(),
        reference: body.reference,
        result,
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
