//! Storage-side HTTP handler. Host owns TLS, bounded worker count and connection lifecycle.
use crate::*;
use qnc_json_transport::Credentials;
use tiny_http::{Header, Method, Request, Response, StatusCode};

fn header(name: &str, value: impl AsRef<str>) -> Header {
    Header::from_bytes(name, value.as_ref()).expect("validated ASCII header")
}
fn one<'a>(request: &'a Request, name: &str) -> Result<Option<&'a str>, ()> {
    let mut headers = request
        .headers()
        .iter()
        .filter(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name));
    let value = headers.next().map(|h| h.value.as_str());
    if headers.next().is_some() {
        return Err(());
    }
    Ok(value)
}

fn authorized_uri(request: &Request, credentials: &Credentials) -> Result<String, u16> {
    let authenticated = one(request, "Authorization")
        .ok()
        .flatten()
        .and_then(|h| credentials.access(h))
        .is_some();
    if !authenticated {
        return Err(401);
    }
    if !matches!(request.method(), Method::Get | Method::Head) {
        return Err(405);
    }
    if request.url().len() > 16 * 1024 || !request.url().starts_with(&format!("{ENDPOINT}?")) {
        return Err(404);
    }
    let Some(query) = request.url().strip_prefix(&format!("{ENDPOINT}?")) else {
        unreachable!()
    };
    let pairs: Vec<_> = url::form_urlencoded::parse(query.as_bytes()).collect();
    if pairs.len() != 1 || pairs[0].0 != "uri" {
        return Err(400);
    }
    Ok(pairs[0].1.to_string())
}

fn rejected(request: Request, status: u16) -> io::Result<()> {
    request.respond(Response::empty(status).with_header(header("Allow", "GET, HEAD")))
}

pub fn respond(
    request: Request,
    source: &LocalSource,
    credentials: &Credentials,
) -> io::Result<()> {
    let uri = match authorized_uri(&request, credentials) {
        Ok(uri) => uri,
        Err(status) => return rejected(request, status),
    };
    let mut media = match MediaStream::local(source, &uri) {
        Ok(media) => media,
        Err(e) => {
            let status = match e.kind() {
                io::ErrorKind::NotFound => 404,
                io::ErrorKind::PermissionDenied => 403,
                io::ErrorKind::InvalidInput => 400,
                _ => 503,
            };
            return request.respond(Response::empty(status));
        }
    };
    serve(request, media.info().clone(), &mut media)
}

/// Reuses an already validated stream, including its original storage stamp.
pub fn respond_opened(
    request: Request,
    media: &mut MediaStream,
    credentials: &Credentials,
) -> io::Result<()> {
    let uri = match authorized_uri(&request, credentials) {
        Ok(uri) => uri,
        Err(status) => return rejected(request, status),
    };
    if uri != media.info().media_uri {
        return rejected(request, 403);
    }
    serve(request, media.info().clone(), media)
}

fn serve(
    request: Request,
    info: MediaInfo,
    media: &mut (impl Read + Seek + Send),
) -> io::Result<()> {
    match one(&request, EXPECTED_STAMP_HEADER) {
        Ok(Some(stamp)) if stamp != info.storage_stamp => {
            return request.respond(Response::empty(412));
        }
        Err(()) => {
            return request.respond(Response::empty(400));
        }
        _ => (),
    }
    let range = match one(&request, "Range") {
        Ok(range) => range,
        Err(()) => {
            return request.respond(Response::empty(400));
        }
    };
    let is_head = request.method() == &Method::Head;
    let partial = range.is_some() && !is_head;
    let span = match range::parse(if is_head { None } else { range }, info.byte_len) {
        Ok(span) => span,
        Err(()) => {
            return request.respond(Response::empty(416).with_header(header(
                "Content-Range",
                format!("bytes */{}", info.byte_len),
            )));
        }
    };
    let mut headers = vec![
        header("Accept-Ranges", "bytes"),
        header("Content-Type", "application/octet-stream"),
        header("Cache-Control", "no-store"),
        header(VERSION_HEADER, VERSION),
        header(URI_HEADER, &info.media_uri),
        header(STAMP_HEADER, &info.storage_stamp),
    ];
    if partial {
        headers.push(header(
            "Content-Range",
            format!("bytes {}-{}/{}", span.start, span.end - 1, info.byte_len),
        ));
    }
    if media.seek(SeekFrom::Start(span.start)).is_err() {
        return request.respond(Response::empty(503));
    }
    let count = span.end - span.start;
    let Ok(len) = usize::try_from(count) else {
        return request.respond(Response::empty(413));
    };
    let body: Box<dyn Read + Send + '_> = if is_head {
        Box::new(io::empty())
    } else {
        Box::new(std::io::BufReader::with_capacity(MAX_READ_BYTES, media).take(count))
    };
    // The range contract requires a declared length even for large streaming responses.
    let response = Response::new(
        StatusCode(if partial { 206 } else { 200 }),
        headers,
        body,
        Some(len),
        None,
    )
    .with_chunked_threshold(usize::MAX);
    request.respond(response)
}
