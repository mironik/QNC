use super::*;
use std::{
    fs,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

const LOCAL: &str = "qnc://local/source/card-a";
const TOKEN: &str = "test-source-access-token";

#[test]
fn bounded_binary_reads_match_local_lan_and_intranet_without_utf8_conversion() {
    let root = fixture();
    let expected = [0xff, 0xfe, 0];
    let local = SourceReader::local(LOCAL, root.path()).unwrap();
    assert_eq!(
        local
            .read_bytes(&local.reference("bad.xml").unwrap(), 3)
            .unwrap()
            .bytes,
        expected
    );
    for environment in ["lan", "intranet"] {
        let uri = format!("qnc://{environment}/fixture/source/card-a");
        let server = TestServer::source(&uri, root.path());
        let remote = SourceReader::remote(&uri, &server.url, TOKEN).unwrap();
        let reference = remote.reference("bad.xml").unwrap();
        let result = remote.read_bytes(&reference, 3).unwrap();
        assert_eq!(result.bytes, expected);
        assert_eq!(result.info.uri, reference.uri());
        assert_eq!(remote.read_bytes(&reference, 2), Err(ReadError::TooLarge));
        assert_eq!(
            remote.read_bytes(&reference, MAX_BINARY_BYTES + 1),
            Err(ReadError::TooLarge)
        );
        assert_eq!(
            remote.read_bytes(&remote.reference("Clip").unwrap(), 100),
            Err(ReadError::NotFile)
        );
    }
    assert_eq!(fs::read(root.path().join("bad.xml")).unwrap(), expected);
}

struct TestServer {
    url: String,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl TestServer {
    fn start(handler: impl Fn(tiny_http::Request) + Send + 'static) -> Self {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr());
        let stop = Arc::new(AtomicBool::new(false));
        let end = stop.clone();
        let thread = thread::spawn(move || {
            while !end.load(Ordering::Relaxed) {
                if let Some(request) = server.recv_timeout(Duration::from_millis(20)).unwrap() {
                    handler(request);
                }
            }
        });
        Self {
            url,
            stop,
            thread: Some(thread),
        }
    }

    fn source(uri: &str, root: &Path) -> Self {
        let source = LocalSource::new(uri, root).unwrap();
        Self::start(move |request| server::respond(request, &source, TOKEN))
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("Clip")).unwrap();
    fs::write(
        root.path().join("Clip/TEST A.XML"),
        "<root value=\"camera\"/>\n",
    )
    .unwrap();
    fs::write(root.path().join("bad.xml"), [0xff, 0xfe, 0]).unwrap();
    root
}

#[test]
fn local_file_stat_and_text_have_qnc_identity() {
    let root = fixture();
    let reader = SourceReader::local(LOCAL, root.path()).unwrap();
    let reference = reader.reference("Clip/TEST A.XML").unwrap();
    let before = fs::read(root.path().join(reference.relative_path())).unwrap();
    assert_eq!(
        reference.uri(),
        "qnc://local/source/card-a/file/Clip/TEST%20A%2EXML"
    );
    assert!(qnc_contracts::parse_qnc_uri(&reference.uri()).is_ok());
    let info = reader.stat(&reference).unwrap();
    let document = reader.read_text(&reference, 1024).unwrap();
    assert_eq!(document.info, info);
    assert_eq!(info.byte_len, Some(before.len() as u64));
    assert_eq!(document.text.as_bytes(), before);
    assert_eq!(
        fs::read(root.path().join(reference.relative_path())).unwrap(),
        before
    );
    assert!(!format!("{reader:?}").contains(&root.path().display().to_string()));
}

#[test]
fn controlled_missing_directory_binary_and_limits() {
    let root = fixture();
    let reader = SourceReader::local(LOCAL, root.path()).unwrap();
    assert_eq!(
        reader.stat(&reader.reference("missing.xml").unwrap()),
        Err(ReadError::NotFound)
    );
    let directory = reader.reference("Clip").unwrap();
    assert_eq!(reader.stat(&directory).unwrap().kind, EntryKind::Directory);
    assert_eq!(reader.read_text(&directory, 100), Err(ReadError::NotFile));
    assert_eq!(
        reader.read_text(&reader.reference("bad.xml").unwrap(), 100),
        Err(ReadError::InvalidUtf8)
    );
    let file = reader.reference("Clip/TEST A.XML").unwrap();
    for limit in [0, 1, MAX_TEXT_BYTES + 1, u64::MAX] {
        assert_eq!(reader.read_text(&file, limit), Err(ReadError::TooLarge));
    }
    let len = reader.stat(&file).unwrap().byte_len.unwrap();
    assert!(reader.read_text(&file, len).is_ok());
    fs::write(root.path().join("empty.xml"), []).unwrap();
    assert_eq!(
        reader
            .read_text(&reader.reference("empty.xml").unwrap(), 1)
            .unwrap()
            .text,
        ""
    );
}

#[test]
fn rejects_unsafe_references_before_io_on_every_environment() {
    for source in [
        LOCAL,
        "qnc://lan/storage/source/card-a",
        "qnc://intranet/storage/source/card-a",
    ] {
        for path in [
            "",
            "/file",
            "../file",
            "Clip/../file",
            "./Clip",
            "C:/file",
            "\\\\host\\file",
            "Clip\\file",
            "Clip//file",
            "Clip/a:stream",
            "Clip/%2e%2e/file",
            "Clip/%252e",
            "Clip/a?x",
            "Clip/a#x",
            "Clip/a\0",
            "Clip/a.",
            "Clip/ a",
            "Clip/a ",
            "Clip/CON.xml",
            "Clip/NUL",
            "Clip/COM1",
        ] {
            assert!(
                SourceReference::new(source, path).is_err(),
                "accepted {path:?}"
            );
        }
        assert!(SourceReference::new(source, "Clip/caf\u{e9} A.XML").is_ok());
    }
    for uri in [
        "/tmp/card",
        "C:\\card",
        "qnc://local/source/../x",
        "qnc://local/media/card",
        "qnc://lan/host:8000/source/card",
        " qnc://local/source/card",
        "qnc://local/source/card?x",
    ] {
        assert!(SourceReference::new(uri, "a.xml").is_err());
    }
}

#[test]
fn deserialized_reference_is_validated_again_at_execution() {
    let root = fixture();
    let reader = SourceReader::local(LOCAL, root.path()).unwrap();
    let malicious: SourceReference = serde_json::from_value(serde_json::json!({
        "source_uri": LOCAL, "relative_path": "../outside.xml"
    }))
    .unwrap();
    assert_eq!(reader.stat(&malicious), Err(ReadError::InvalidReference));
    let foreign = SourceReference::new("qnc://local/source/other", "Clip/TEST A.XML").unwrap();
    assert_eq!(reader.stat(&foreign), Err(ReadError::UnboundSource));
}

#[test]
fn actual_http_transport_matches_local_for_lan_and_intranet() {
    let root = fixture();
    let local = SourceReader::local(LOCAL, root.path()).unwrap();
    let expected = local
        .read_text(&local.reference("Clip/TEST A.XML").unwrap(), 1024)
        .unwrap();
    for environment in ["lan", "intranet"] {
        let uri = format!("qnc://{environment}/storage/source/card-a");
        let server = TestServer::source(&uri, root.path());
        let reader = SourceReader::remote(&uri, &server.url, TOKEN).unwrap();
        let reference = reader.reference("Clip/TEST A.XML").unwrap();
        let document = reader.read_text(&reference, 1024).unwrap();
        assert_eq!(document.text, expected.text);
        assert_eq!(document.info.uri, reference.uri());
        assert_eq!(reader.stat(&reference).unwrap(), document.info);
        assert_eq!(
            reader.stat(&reader.reference("missing.xml").unwrap()),
            Err(ReadError::NotFound)
        );
        assert_eq!(reader.read_text(&reference, 1), Err(ReadError::TooLarge));
        assert_eq!(
            reader.read_text(&reader.reference("bad.xml").unwrap(), 100),
            Err(ReadError::InvalidUtf8)
        );
        assert!(!format!("{reader:?}").contains(TOKEN));
    }
}

#[test]
fn remote_requires_encrypted_transport_except_loopback() {
    let uri = "qnc://lan/storage/source/card-a";
    for url in [
        "http://192.0.2.1",
        "http://storage.example",
        "file:///etc",
        "https://user:pass@host",
        "https://host?x=1",
        "https://host/#x",
    ] {
        assert!(
            SourceReader::remote(uri, url, TOKEN).is_err(),
            "accepted {url}"
        );
    }
    for token in ["", "with space", "header\r\ninjection"] {
        assert!(SourceReader::remote(uri, "https://storage.example", token).is_err());
    }
    assert!(SourceReader::remote(uri, "https://storage.example", TOKEN).is_ok());
    assert!(SourceReader::remote(uri, "http://[::1]:8000", TOKEN).is_ok());
    assert!(SourceReader::remote(LOCAL, "https://storage.example", TOKEN).is_err());
}

#[test]
fn endpoint_rejects_wrong_token_and_wrong_source() {
    let root = fixture();
    let uri = "qnc://lan/storage/source/card-a";
    let server = TestServer::source(uri, root.path());
    let reader = SourceReader::remote(uri, &server.url, "wrong-token").unwrap();
    assert_eq!(
        reader.stat(&reader.reference("Clip/TEST A.XML").unwrap()),
        Err(ReadError::AccessDenied)
    );
    let reader =
        SourceReader::remote("qnc://lan/storage/source/card-b", &server.url, TOKEN).unwrap();
    assert_eq!(
        reader.stat(&reader.reference("Clip/TEST A.XML").unwrap()),
        Err(ReadError::UnboundSource)
    );
}

#[test]
fn endpoint_limits_and_validates_wire_requests() {
    let root = fixture();
    let uri = "qnc://lan/storage/source/card-a";
    let server = TestServer::source(uri, root.path());
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(2))
        .build();
    let request = || {
        agent
            .post(&format!("{}{ENDPOINT}", server.url))
            .set("Authorization", &format!("Bearer {TOKEN}"))
    };
    let mut body = serde_json::json!({"version": VERSION, "reference": {"source_uri": uri, "relative_path": "Clip/TEST A.XML"}, "operation": {"operation": "stat"}});
    body["local_path"] = serde_json::json!("C:/private");
    assert!(matches!(
        request().send_json(&body),
        Err(ureq::Error::Status(400, _))
    ));
    assert!(matches!(
        request()
            .set("Content-Type", "application/json")
            .send_string(&" ".repeat(MAX_REQUEST_BYTES as usize + 1)),
        Err(ureq::Error::Status(413, _))
    ));
    assert!(matches!(
        request().send_string("{}"),
        Err(ureq::Error::Status(415, _))
    ));
    body.as_object_mut().unwrap().remove("local_path");
    body["version"] = serde_json::json!("99");
    assert!(matches!(
        request().send_json(&body),
        Err(ureq::Error::Status(400, _))
    ));
    body["version"] = serde_json::json!(VERSION);
    body["reference"]["relative_path"] = serde_json::json!("../secret");
    let reply: Reply = request().send_json(&body).unwrap().into_json().unwrap();
    assert!(matches!(reply.result, Err(ReadError::InvalidReference)));
}

#[test]
fn remote_rejects_wrong_identity_version_operation_and_lengths() {
    let uri = "qnc://intranet/storage/source/card-a";
    let reference = SourceReference::new(uri, "doc.xml").unwrap();
    let valid = serde_json::to_value(Reply {
        version: VERSION.into(),
        reference: reference.clone(),
        result: Ok(SourceData::Text(TextDocument {
            info: FileInfo {
                uri: reference.uri(),
                kind: EntryKind::File,
                byte_len: Some(4),
            },
            text: "test".into(),
        })),
    })
    .unwrap();
    for case in 0..7 {
        let mut reply = valid.clone();
        match case {
            0 => reply["version"] = serde_json::json!("99"),
            1 => {
                reply["reference"]["source_uri"] =
                    serde_json::json!("qnc://intranet/storage/source/other")
            }
            2 => {
                reply["result"]["Ok"]["value"]["info"]["uri"] =
                    serde_json::json!("qnc://local/source/other/file/doc")
            }
            3 => reply["result"]["Ok"]["kind"] = serde_json::json!("stat"),
            4 => reply["result"]["Ok"]["value"]["info"]["byte_len"] = serde_json::json!(3),
            5 => reply["result"]["Ok"]["value"]["info"]["kind"] = serde_json::json!("directory"),
            _ => {
                reply["result"]["Ok"]["value"]["text"] = serde_json::json!("123456789");
                reply["result"]["Ok"]["value"]["info"]["byte_len"] = serde_json::json!(9);
            }
        }
        let response = serde_json::to_string(&reply).unwrap();
        let server = TestServer::start(move |request| {
            let _ = request.respond(tiny_http::Response::from_string(&response).with_header(
                tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
            ));
        });
        let reader = SourceReader::remote(uri, &server.url, TOKEN).unwrap();
        assert_eq!(
            reader.read_text(&reference, 8),
            Err(ReadError::Protocol),
            "case {case}"
        );
    }
}

#[test]
fn remote_does_not_follow_redirects_or_accept_oversized_responses() {
    let uri = "qnc://lan/storage/source/card-a";
    let server = TestServer::start(|request| {
        let _ = request.respond(tiny_http::Response::empty(302).with_header(
            tiny_http::Header::from_bytes("Location", "http://127.0.0.1:1/secret").unwrap(),
        ));
    });
    let reader = SourceReader::remote(uri, &server.url, TOKEN).unwrap();
    assert!(reader.stat(&reader.reference("doc.xml").unwrap()).is_err());
    let server = TestServer::start(|request| {
        let _ = request.respond(
            tiny_http::Response::from_string(" ".repeat(70 * 1024)).with_header(
                tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
            ),
        );
    });
    let reader = SourceReader::remote(uri, &server.url, TOKEN).unwrap();
    assert_eq!(
        reader.stat(&reader.reference("doc.xml").unwrap()),
        Err(ReadError::TooLarge)
    );
}

#[test]
fn sony_reader_consumes_actual_transport_bindings() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("Clip")).unwrap();
    fs::create_dir(root.path().join("Sub")).unwrap();
    fs::write(
        root.path().join("MEDIAPRO.XML"),
        include_str!("../../qnc-sony-metadata/tests/fixtures/MEDIAPRO.XML"),
    )
    .unwrap();
    fs::write(
        root.path().join("Clip/TEST AM01.XML"),
        include_str!("../../qnc-sony-metadata/tests/fixtures/TEST-AM01.XML"),
    )
    .unwrap();
    fs::write(
        root.path().join("Clip/TEST A.MXF"),
        b"fixture placeholder, never decoded",
    )
    .unwrap();
    fs::write(
        root.path().join("Sub/TEST AS03.MP4"),
        b"fixture placeholder, never decoded",
    )
    .unwrap();
    let uri = "qnc://lan/storage/source/card-a";
    let server = TestServer::source(uri, root.path());
    let reader = SourceReader::remote(uri, &server.url, TOKEN).unwrap();
    let document = reader
        .read_text(&reader.reference("MEDIAPRO.XML").unwrap(), MAX_TEXT_BYTES)
        .unwrap();
    let index = qnc_sony_metadata::read_index(&qnc_sony_metadata::XmlDocument {
        document_uri: document.info.uri,
        text: document.text,
    })
    .unwrap();
    let material = &index.materials[0];
    let bind = |relative: &str| {
        let info = reader.stat(&reader.reference(relative).unwrap()).unwrap();
        assert_eq!(info.kind, EntryKind::File);
        qnc_sony_metadata::BoundMedia {
            relative_path: relative.into(),
            media_uri: info.uri,
        }
    };
    let binding = qnc_sony_metadata::ClipBinding {
        clip_id: "fixture-clip".into(),
        original: bind(&material.original.relative_path),
        proxy: Some(bind(&material.proxies[0].relative_path)),
    };
    let xml = reader
        .read_text(
            &reader.reference("Clip/TEST AM01.XML").unwrap(),
            MAX_TEXT_BYTES,
        )
        .unwrap();
    let sidecar = qnc_sony_metadata::SidecarDocument {
        relative_path: "Clip/TEST AM01.XML".into(),
        document: qnc_sony_metadata::XmlDocument {
            document_uri: xml.info.uri,
            text: xml.text,
        },
    };
    let result = qnc_sony_metadata::read_metadata(&index, 0, &binding, Some(&sidecar)).unwrap();
    assert_eq!(
        result.metadata.original.media_uri,
        binding.original.media_uri
    );
    assert_eq!(
        result.metadata.proxy.unwrap().media_uri,
        binding.proxy.unwrap().media_uri
    );
    assert_eq!(result.metadata.evidence.len(), 3);
}

#[test]
fn manifest_has_no_app_consumers_and_no_write_capabilities() {
    let manifest = include_str!("../../../contracts/modules/source-reader.module.json");
    assert!(qnc_contracts::validate_module_manifest_json("source-reader", manifest).is_ok());
    let manifest: serde_json::Value = serde_json::from_str(manifest).unwrap();
    assert_eq!(manifest["database_write_policy"], "no_db_writes");
    assert_eq!(
        manifest["capabilities"],
        serde_json::json!([
            "source.file.stat",
            "source.text.read",
            "source.bytes.read",
            "source.directory.list",
            "source.file.open_read_only"
        ])
    );
}

#[test]
fn directory_list_is_bounded_and_keeps_actual_names() {
    let root = fixture();
    let local = LocalSource::new(LOCAL, root.path())
        .unwrap()
        .with_match_case(MatchCase::Insensitive);
    let reader = SourceReader::from_local(local);
    let reference = reader.reference(".").unwrap();
    assert_eq!(reference.uri(), LOCAL);
    let listing = reader.list(&reference, 2).unwrap();
    assert_eq!(listing.match_case, MatchCase::Insensitive);
    assert_eq!(listing.entries[0].name, "Clip");
    assert_eq!(listing.entries[0].kind, EntryKind::Directory);
    assert_eq!(listing.entries[1].kind, EntryKind::File);
    let child = reader.list(&listing.entries[0].reference, 1).unwrap();
    assert_eq!(child.entries[0].name, "TEST A.XML");
    assert_eq!(
        child.entries[0].reference.relative_path(),
        "Clip/TEST A.XML"
    );
    for limit in [0, 1, MAX_DIRECTORY_ENTRIES + 1] {
        assert_eq!(reader.list(&reference, limit), Err(ReadError::TooLarge));
    }
    assert!(reader
        .list(&reader.reference("missing").unwrap(), 2)
        .is_err());
    assert!(reader
        .list(&reader.reference("bad.xml").unwrap(), 2)
        .is_err());
}

#[test]
fn directory_list_uses_same_contract_over_lan_and_intranet() {
    let root = fixture();
    for environment in ["lan", "intranet"] {
        let uri = format!("qnc://{environment}/storage/source/card-a");
        let server = TestServer::source(&uri, root.path());
        let remote = SourceReader::remote(&uri, &server.url, TOKEN).unwrap();
        let local = SourceReader::from_local(LocalSource::new(&uri, root.path()).unwrap());
        let reference = remote.reference(".").unwrap();
        assert_eq!(remote.list(&reference, 2), local.list(&reference, 2));
        assert_eq!(remote.list(&reference, 1), Err(ReadError::TooLarge));
        assert_eq!(
            remote.list(&remote.reference("Clip").unwrap(), 1),
            local.list(&local.reference("Clip").unwrap(), 1)
        );
    }
}

#[test]
fn remote_rejects_unbound_duplicate_or_non_child_listing_entries() {
    let root = fixture();
    let uri = "qnc://lan/storage/source/card-a";
    let local = SourceReader::from_local(LocalSource::new(uri, root.path()).unwrap());
    let reference = local.reference(".").unwrap();
    let listing = local.list(&reference, 2).unwrap();
    for case in 0..6 {
        let mut invalid = listing.clone();
        match case {
            0 => invalid.directory_uri = "qnc://local/source/other".into(),
            1 => invalid.entries.push(invalid.entries[0].clone()),
            2 => {
                invalid.entries[0].reference =
                    SourceReference::new("qnc://lan/storage/source/other", "Clip").unwrap()
            }
            3 => invalid.entries[0].reference = local.reference("Clip/nested").unwrap(),
            4 => invalid.entries[0].name = "../Clip".into(),
            _ => invalid.entries[0].name = ".".into(),
        }
        let response = serde_json::to_string(&Reply {
            version: VERSION.into(),
            reference: reference.clone(),
            result: Ok(SourceData::Listing(invalid)),
        })
        .unwrap();
        let server = TestServer::start(move |request| {
            let _ = request.respond(tiny_http::Response::from_string(&response).with_header(
                tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
            ));
        });
        let reader = SourceReader::remote(uri, &server.url, TOKEN).unwrap();
        assert_eq!(
            reader.list(&reference, 3),
            Err(ReadError::Protocol),
            "case {case}"
        );
    }
}

#[cfg(unix)]
#[test]
fn symlink_cannot_escape_capability_root() {
    let root = fixture();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.xml"), "secret").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).unwrap();
    let reader = SourceReader::local(LOCAL, root.path()).unwrap();
    let listing = reader.list(&reader.reference(".").unwrap(), 8).unwrap();
    assert_eq!(
        listing
            .entries
            .iter()
            .find(|e| e.name == "escape")
            .unwrap()
            .kind,
        EntryKind::Link
    );
    let reference = reader.reference("escape/secret.xml").unwrap();
    assert!(reader.stat(&reference).is_err());
    assert!(reader.read_text(&reference, 1024).is_err());
}

#[cfg(windows)]
#[test]
fn junction_cannot_escape_capability_root() {
    let root = fixture();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.xml"), "secret").unwrap();
    let link = root.path().join("escape");
    let output = std::process::Command::new("cmd.exe")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(outside.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "test junction could not be created"
    );
    let reader = SourceReader::local(LOCAL, root.path()).unwrap();
    let listing = reader.list(&reader.reference(".").unwrap(), 8).unwrap();
    assert_eq!(
        listing
            .entries
            .iter()
            .find(|e| e.name == "escape")
            .unwrap()
            .kind,
        EntryKind::Link
    );
    let reference = reader.reference("escape/secret.xml").unwrap();
    assert!(reader.stat(&reference).is_err());
    assert!(reader.read_text(&reference, 1024).is_err());
    assert_eq!(
        fs::read(outside.path().join("secret.xml")).unwrap(),
        b"secret"
    );
    assert!(LocalSource::new(LOCAL, root.path())
        .unwrap()
        .open_read_only_file(&reference)
        .is_err());
}
