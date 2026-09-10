#![cfg(test)]
use super::*;
use qnc_json_transport::Credentials;
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
use tiny_http::{Header, Response, Server, StatusCode};

const SOURCE: &str = "qnc://local/source/card";
const TOKEN: &str = "test-read-only-token";

struct Host {
    base: String,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Host {
    fn new(handler: impl Fn(tiny_http::Request) + Send + 'static) -> Self {
        let server = Server::http("127.0.0.1:0").unwrap();
        let base = format!("http://{}", server.server_addr());
        let stop = Arc::new(AtomicBool::new(false));
        let shutdown = stop.clone();
        let worker = thread::spawn(move || {
            while !shutdown.load(Ordering::Acquire) {
                if let Some(request) = server.recv_timeout(Duration::from_millis(20)).unwrap() {
                    handler(request);
                }
            }
        });
        Self {
            base,
            stop,
            worker: Some(worker),
        }
    }
    fn source(source: LocalSource) -> Self {
        let credentials = Credentials::new(TOKEN, "test-distinct-write-token").unwrap();
        Self::new(move |request| {
            let _ = server::respond(request, &source, &credentials);
        })
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.worker.take().unwrap().join().unwrap();
    }
}
fn uri(source: &str, path: &str) -> String {
    SourceReference::new(source, path).unwrap().uri()
}
fn fixture(source: &str) -> (tempfile::TempDir, LocalSource, String, Vec<u8>) {
    let dir = tempfile::tempdir().unwrap();
    let bytes: Vec<_> = (0..(MAX_READ_BYTES * 2 + 27))
        .map(|n| (n % 251) as u8)
        .collect();
    fs::write(dir.path().join("clip.bin"), &bytes).unwrap();
    let owner = LocalSource::new(source, dir.path()).unwrap();
    (dir, owner, uri(source, "clip.bin"), bytes)
}
fn exercise(mut reader: impl Read + Seek, bytes: &[u8]) {
    let mut output = vec![0; MAX_READ_BYTES * 2];
    assert_eq!(reader.read(&mut output).unwrap(), MAX_READ_BYTES);
    assert_eq!(&output[..MAX_READ_BYTES], &bytes[..MAX_READ_BYTES]);
    reader.seek(SeekFrom::Start(137)).unwrap();
    let mut part = [0; 73];
    reader.read_exact(&mut part).unwrap();
    assert_eq!(part, bytes[137..210]);
    reader.seek(SeekFrom::Current(-1)).unwrap();
    assert_eq!(reader.stream_position().unwrap(), 209);
    reader.seek(SeekFrom::End(-73)).unwrap();
    reader.read_exact(&mut part).unwrap();
    assert_eq!(part, bytes[bytes.len() - 73..]);
    assert_eq!(reader.read(&mut part).unwrap(), 0);
    assert_eq!(reader.read(&mut []).unwrap(), 0);
    assert!(reader.seek(SeekFrom::Start(u64::MAX)).is_err());
    assert!(reader.seek(SeekFrom::End(i64::MIN)).is_err());
    reader.seek(SeekFrom::End(7)).unwrap();
    assert_eq!(reader.read(&mut part).unwrap(), 0);
    reader.rewind().unwrap();
    let mut all = Vec::new();
    reader.read_to_end(&mut all).unwrap();
    assert_eq!(all, bytes);
}

#[test]
fn local_seek_and_read_are_bounded_and_leave_source_unchanged() {
    let (dir, source, uri, bytes) = fixture(SOURCE);
    exercise(MediaStream::local(&source, &uri).unwrap(), &bytes);
    assert_eq!(fs::read(dir.path().join("clip.bin")).unwrap(), bytes);
}

#[test]
fn lan_and_intranet_use_identical_authenticated_ranges() {
    for environment in ["lan", "intranet"] {
        let source_uri = format!("qnc://{environment}/storage/source/card");
        let (dir, source, uri, bytes) = fixture(&source_uri);
        let host = Host::source(source);
        let resolver = ResolverConfig::new(dir.path())
            .with_lan_authority("storage", &host.base)
            .with_intranet_authority("storage", &host.base);
        exercise(MediaStream::remote(&resolver, &uri, TOKEN).unwrap(), &bytes);
        assert_eq!(fs::read(dir.path().join("clip.bin")).unwrap(), bytes);
    }
}

#[test]
fn local_owner_can_serve_the_same_uri_for_codec_http_input() {
    let (_dir, source, uri, bytes) = fixture(SOURCE);
    let host = Host::source(source);
    let endpoint = HttpEndpoint::for_owner_endpoint(&host.base, &uri, TOKEN).unwrap();
    exercise(remote::RemoteMedia::open(endpoint).unwrap(), &bytes);
}

#[test]
fn source_and_file_boundary_rejects_roots_directories_and_traversal() {
    let (dir, source, file, _) = fixture(SOURCE);
    fs::create_dir(dir.path().join("folder")).unwrap();
    for bad in [
        SOURCE.to_owned(),
        uri(SOURCE, "folder"),
        uri("qnc://local/source/other", "clip.bin"),
        format!("{SOURCE}/file/../clip.bin"),
        format!("{SOURCE}/file/%2E%2E/clip%2Ebin"),
        "C:\\clip.bin".into(),
    ] {
        assert!(MediaStream::local(&source, &bad).is_err(), "{bad}");
    }
    let mut handle = source
        .open_read_only_file(&SourceReference::from_uri(&file).unwrap())
        .unwrap();
    assert!(std::io::Write::write_all(&mut handle, b"no write").is_err());
    assert!(MediaStream::local(&source, &uri(SOURCE, "missing.bin")).is_err());
}

#[test]
fn changed_file_is_rejected_without_silent_reopen() {
    let (dir, source, uri, _) = fixture(SOURCE);
    let mut local = MediaStream::local(&source, &uri).unwrap();
    let host = Host::source(source);
    let mut remote = remote::RemoteMedia::open(
        HttpEndpoint::for_owner_endpoint(&host.base, &uri, TOKEN).unwrap(),
    )
    .unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(dir.path().join("clip.bin"))
        .unwrap()
        .set_len(12)
        .unwrap();
    assert_eq!(
        local.read(&mut [0; 5]).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    assert_eq!(
        remote.read(&mut [0; 5]).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    assert_eq!(remote.stream_position().unwrap(), 0);
}

#[test]
fn endpoint_disallows_insecure_remote_credentials_and_implicit_fallback() {
    let file = uri(SOURCE, "clip.bin");
    for base in [
        "http://10.0.0.1",
        "ftp://localhost",
        "https://user:pass@host",
        "https://host?q=1",
        "https://host#fragment",
    ] {
        assert!(HttpEndpoint::for_owner_endpoint(base, &file, TOKEN).is_err());
    }
    for token in ["", "bad token", "bad\r\nheader"] {
        assert!(HttpEndpoint::for_owner_endpoint("https://host", &file, token).is_err());
    }
    assert!(
        HttpEndpoint::resolve(
            &ResolverConfig::new(std::path::PathBuf::new()),
            &file,
            TOKEN
        )
        .is_err()
    );
    let endpoint = HttpEndpoint::for_owner_endpoint("https://host", &file, TOKEN).unwrap();
    assert!(!format!("{endpoint:?}").contains(TOKEN));
}

#[test]
fn http_head_and_range_statuses_auth_and_methods() {
    let (_dir, source, uri, bytes) = fixture(SOURCE);
    let host = Host::source(source);
    let ep = HttpEndpoint::for_owner_endpoint(&host.base, &uri, TOKEN).unwrap();
    let request =
        |method| ureq::request(method, ep.url()).set("Authorization", ep.authorization_header());
    let head = request("HEAD").call().unwrap();
    assert_eq!(
        head.header("Content-Length").unwrap(),
        bytes.len().to_string()
    );
    let mut body = Vec::new();
    head.into_reader().read_to_end(&mut body).unwrap();
    assert!(body.is_empty());
    for (range, expected) in [
        ("bytes=5-17", &bytes[5..18]),
        ("bytes=-9", &bytes[bytes.len() - 9..]),
        ("bytes=2097170-", &bytes[2097170..]),
    ] {
        let response = request("GET").set("Range", range).call().unwrap();
        assert_eq!(response.status(), 206);
        let mut body = Vec::new();
        response.into_reader().read_to_end(&mut body).unwrap();
        assert_eq!(body, expected);
    }
    let status = |result: Result<ureq::Response, ureq::Error>| match result {
        Err(ureq::Error::Status(s, _)) => s,
        _ => panic!("expected error status"),
    };
    assert_eq!(status(ureq::get(ep.url()).call()), 401);
    assert_eq!(
        status(request("GET").set("Authorization", "Bearer wrong").call()),
        401
    );
    assert_eq!(status(request("POST").call()), 405);
    assert_eq!(
        status(request("GET").set("Range", "bytes=0-1,4-7").call()),
        416
    );
    assert_eq!(
        status(request("GET").set("Range", "bytes=99999999-").call()),
        416
    );
    assert_eq!(
        status(
            request("GET")
                .set(EXPECTED_STAMP_HEADER, "different")
                .call()
        ),
        412
    );
}

#[test]
fn empty_files_return_eof_without_range_request() {
    let (dir, source, uri, _) = fixture(SOURCE);
    fs::write(dir.path().join("clip.bin"), []).unwrap();
    let host = Host::source(source.clone());
    let mut local = MediaStream::local(&source, &uri).unwrap();
    let mut remote = remote::RemoteMedia::open(
        HttpEndpoint::for_owner_endpoint(&host.base, &uri, TOKEN).unwrap(),
    )
    .unwrap();
    assert_eq!(local.read(&mut [0; 1]).unwrap(), 0);
    assert_eq!(remote.read(&mut [0; 1]).unwrap(), 0);
}

#[test]
fn malformed_range_responses_do_not_advance_cursor_or_modify_output() {
    for bad in [
        "version", "uri", "stamp", "range", "length", "status", "encoding", "redirect",
    ] {
        let uri = uri(SOURCE, "clip.bin");
        let identity = uri.clone();
        let host = Host::new(move |request| {
            let head = request.method() == &tiny_http::Method::Head;
            let mut headers = vec![
                (VERSION_HEADER, VERSION.to_owned()),
                (URI_HEADER, identity.clone()),
                (STAMP_HEADER, "8-abc".into()),
                ("Accept-Ranges", "bytes".into()),
            ];
            let mut status = if head { 200 } else { 206 };
            let mut len = 8;
            if !head {
                headers.push(("Content-Range", "bytes 0-7/8".into()));
                match bad {
                    "version" => headers[0].1 = "99".into(),
                    "uri" => headers[1].1 = uri_fn_wrong(),
                    "stamp" => headers[2].1 = "8-def".into(),
                    "range" => headers[4].1 = "bytes 1-8/9".into(),
                    "length" => len = 7,
                    "status" => status = 200,
                    "encoding" => headers.push(("Content-Encoding", "br".into())),
                    "redirect" => {
                        status = 302;
                        headers.push(("Location", "http://127.0.0.1:1/".into()));
                    }
                    _ => unreachable!(),
                }
            }
            let headers = headers
                .into_iter()
                .map(|(n, v)| Header::from_bytes(n, v).unwrap())
                .collect();
            let _ = request.respond(Response::new(
                StatusCode(status),
                headers,
                io::Cursor::new(vec![1; len]),
                Some(len),
                None,
            ));
        });
        let mut remote = remote::RemoteMedia::open(
            HttpEndpoint::for_owner_endpoint(&host.base, &uri, TOKEN).unwrap(),
        )
        .unwrap();
        let mut bytes = [42; 8];
        assert!(remote.read(&mut bytes).is_err(), "{bad}");
        assert_eq!(remote.stream_position().unwrap(), 0);
        assert_eq!(bytes, [42; 8]);
    }
}
fn uri_fn_wrong() -> String {
    uri(SOURCE, "wrong.bin")
}

#[test]
fn codec_bridge_serves_local_and_validated_remote_streams() {
    let (_dir, source, file, bytes) = fixture(SOURCE);
    let bridge = LoopbackBridge::new(MediaStream::local(&source, &file).unwrap()).unwrap();
    exercise(
        remote::RemoteMedia::open(bridge.endpoint().clone()).unwrap(),
        &bytes,
    );
    let (dir, source, file, bytes) = fixture("qnc://lan/storage/source/card");
    let host = Host::source(source);
    let resolver = ResolverConfig::new(dir.path()).with_lan_authority("storage", &host.base);
    let bridge =
        LoopbackBridge::new(MediaStream::remote(&resolver, &file, TOKEN).unwrap()).unwrap();
    exercise(
        remote::RemoteMedia::open(bridge.endpoint().clone()).unwrap(),
        &bytes,
    );
}

#[test]
fn codec_bridge_does_not_hide_changed_remote_source() {
    let (dir, source, file, _) = fixture("qnc://intranet/storage/source/card");
    let host = Host::source(source);
    let resolver = ResolverConfig::new(dir.path()).with_intranet_authority("storage", &host.base);
    let bridge =
        LoopbackBridge::new(MediaStream::remote(&resolver, &file, TOKEN).unwrap()).unwrap();
    let mut reader = remote::RemoteMedia::open(bridge.endpoint().clone()).unwrap();
    fs::write(dir.path().join("clip.bin"), b"changed").unwrap();
    assert!(reader.read(&mut [0; 16]).is_err());
}

#[test]
fn truncated_response_is_not_retried_or_exposed_as_valid_bytes() {
    use std::{io::Write, net::TcpListener};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = HttpEndpoint::for_owner_endpoint(
        &format!("http://{}", listener.local_addr().unwrap()),
        &uri(SOURCE, "clip.bin"),
        TOKEN,
    )
    .unwrap();
    let file_uri = uri(SOURCE, "clip.bin");
    let worker = thread::spawn(move || {
        for method in ["HEAD", "GET"] {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") && request.len() < 8192 {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            assert!(request.starts_with(method.as_bytes()));
            let status = if method == "HEAD" {
                "200 OK"
            } else {
                "206 Partial Content"
            };
            let range = if method == "HEAD" {
                ""
            } else {
                "Content-Range: bytes 0-7/8\r\n"
            };
            write!(socket,"HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: 8\r\nAccept-Ranges: bytes\r\n{VERSION_HEADER}: {VERSION}\r\n{URI_HEADER}: {file_uri}\r\n{STAMP_HEADER}: 8-abc\r\n{range}\r\n").unwrap();
            if method == "GET" {
                socket.write_all(b"short").unwrap();
            }
        }
    });
    let mut remote = remote::RemoteMedia::open(endpoint).unwrap();
    let mut buffer = [42; 8];
    assert!(remote.read(&mut buffer).is_err());
    assert_eq!(remote.stream_position().unwrap(), 0);
    assert_eq!(buffer, [42; 8]);
    worker.join().unwrap();
}

#[test]
fn http_owner_cannot_serve_another_source_or_query_ambiguity() {
    let (_dir, source, file, _) = fixture(SOURCE);
    let host = Host::source(source);
    let status = |url: &str| match ureq::get(url)
        .set("Authorization", &format!("Bearer {TOKEN}"))
        .call()
    {
        Err(ureq::Error::Status(code, _)) => code,
        _ => panic!("expected rejection"),
    };
    let ep = HttpEndpoint::for_owner_endpoint(&host.base, &file, TOKEN).unwrap();
    assert_eq!(status(&format!("{}&uri=second", ep.url())), 400);
    let ep = HttpEndpoint::for_owner_endpoint(
        &host.base,
        &uri("qnc://local/source/other", "clip.bin"),
        TOKEN,
    )
    .unwrap();
    assert_eq!(status(ep.url()), 403);
    let ep =
        HttpEndpoint::for_owner_endpoint(&host.base, &uri(SOURCE, "missing.bin"), TOKEN).unwrap();
    assert_eq!(status(ep.url()), 404);
}

#[test]
fn open_rejects_wrong_identity_version_or_missing_storage_headers() {
    for field in [
        VERSION_HEADER,
        URI_HEADER,
        STAMP_HEADER,
        "Accept-Ranges",
        "Content-Length",
    ] {
        let file = uri(SOURCE, "clip.bin");
        let identity = file.clone();
        let host = Host::new(move |request| {
            let headers = [
                (VERSION_HEADER, VERSION.to_owned()),
                (URI_HEADER, identity.clone()),
                (STAMP_HEADER, "8-abc".into()),
                ("Accept-Ranges", "bytes".into()),
            ]
            .into_iter()
            .filter(|(name, _)| *name != field)
            .map(|(n, v)| Header::from_bytes(n, v).unwrap())
            .collect();
            let size = if field == "Content-Length" {
                None
            } else {
                Some(8)
            };
            let _ = request.respond(Response::new(
                StatusCode(200),
                headers,
                io::empty(),
                size,
                None,
            ));
        });
        assert!(
            remote::RemoteMedia::open(
                HttpEndpoint::for_owner_endpoint(&host.base, &file, TOKEN).unwrap()
            )
            .is_err(),
            "{field}"
        );
    }
}

#[cfg(unix)]
#[test]
fn source_capability_rejects_external_symlink() {
    let (dir, source, _, _) = fixture(SOURCE);
    let outside = tempfile::NamedTempFile::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join("escape")).unwrap();
    assert!(MediaStream::local(&source, &uri(SOURCE, "escape")).is_err());
}
