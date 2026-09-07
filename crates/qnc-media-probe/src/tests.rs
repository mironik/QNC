use super::*;
use std::path::PathBuf;
fn request() -> Request {
    Request {
        version: VERSION.into(),
        request_id: "one".into(),
        media_uri: "qnc://local/source/card/a.MXF".into(),
        document_uri: "qnc://local/artifact/probe-a".into(),
    }
}
fn config() -> OwnerConfig {
    OwnerConfig {
        executable: std::env::current_exe().unwrap(),
        bindings: vec![],
        timeout_ms: 30000,
        probe_size_bytes: 1048576,
        analyze_duration_us: 100000,
        demuxers: vec!["mxf".into(), "mov".into()],
    }
}
#[test]
fn single_full_call_not_per_field_or_frame_scan() {
    let args = process::arguments(&config());
    for required in [
        "-show_format",
        "-show_streams",
        "-show_programs",
        "-show_chapters",
        "1048576",
        "100000",
    ] {
        assert!(args.iter().any(|a| a == required));
    }
    for forbidden in [
        "-count_frames",
        "-show_frames",
        "-show_packets",
        "-read_intervals",
    ] {
        assert!(!args.iter().any(|a| a == forbidden));
    }
    assert_eq!(
        args.iter()
            .filter(|a| a.as_str() == "-show_streams")
            .count(),
        1
    );
}
#[test]
fn exact_owner_bindings_no_implicit_filesystem_fallback() {
    let e = Executor::new(config()).unwrap();
    assert_eq!(e.execute(&request()), Err(Error::UnboundMedia));
    let mut c = config();
    c.bindings.push(Binding {
        media_uri: request().media_uri,
        private_file: PathBuf::from("relative"),
    });
    assert!(matches!(Executor::new(c), Err(Error::Configuration)));
}
#[test]
fn request_validation_rejects_os_paths_extra_fields_and_versions() {
    let mut r = request();
    r.media_uri = "C:\\a.MXF".into();
    assert!(r.validate().is_err());
    let mut r = request();
    r.version = "9".into();
    assert!(r.validate().is_err());
    let mut json = serde_json::to_value(request()).unwrap();
    json["arguments"] = serde_json::json!(["-count_frames"]);
    assert!(serde_json::from_value::<Request>(json).is_err());
}
#[test]
fn bounded_owner_config_and_protocol_reply() {
    let mut c = config();
    c.timeout_ms = 0;
    assert!(matches!(Executor::new(c), Err(Error::Configuration)));
    let mut c = config();
    c.demuxers = vec!["hls".into()];
    assert!(matches!(Executor::new(c), Err(Error::Configuration)));
    let r = request();
    let reply = Reply {
        version: VERSION.into(),
        request_id: "wrong".into(),
        result: Err(Error::Failed),
    };
    assert_eq!(reply.validate(&r), Err(Error::Protocol));
}
#[test]
fn lan_and_intranet_enforce_execute_grant_and_binding() {
    for env in ["lan", "intranet"] {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr());
        let owner = Executor::new(config()).unwrap();
        let host = std::thread::spawn(move || {
            let credentials = Credentials::new("read", "execute").unwrap();
            for _ in 0..2 {
                respond(server.recv().unwrap(), &owner, &credentials);
            }
        });
        let resolver = ResolverConfig::new("")
            .with_lan_authority("test", &url)
            .with_intranet_authority("test", &url);
        let uri = format!("qnc://{env}/test/module/media-probe");
        assert_eq!(
            Client::connect(&resolver, &uri, "read")
                .unwrap()
                .execute(&request()),
            Err(Error::AccessDenied)
        );
        assert_eq!(
            Client::connect(&resolver, &uri, "execute")
                .unwrap()
                .execute(&request()),
            Err(Error::UnboundMedia)
        );
        host.join().unwrap();
    }
}

#[test]
fn batch_rejects_duplicates_before_execution_and_never_retries_failures() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Fail(AtomicUsize);
    impl ProbeBackend for Fail {
        fn execute(&self, _: &Request) -> Result<Report> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(Error::Timeout)
        }
    }
    let backend = Fail(AtomicUsize::new(0));
    assert_eq!(
        execute_batch(&backend, &[request(), request()], 2),
        Err(Error::InvalidRequest)
    );
    assert_eq!(backend.0.load(Ordering::SeqCst), 0);
    assert_eq!(
        execute_batch(&backend, &[request()], 2).unwrap(),
        vec![Err(Error::Timeout)]
    );
    assert_eq!(backend.0.load(Ordering::SeqCst), 1);
}

#[test]
fn child_fixture() {
    use std::io::Write;
    match std::env::var("QNC_PROBE_PROCESS_TEST").as_deref() {
        Ok("sleep") => std::thread::sleep(Duration::from_secs(60)),
        Ok("overflow") => {
            let _ = std::io::stdout().write_all(&vec![b'x'; MAX_DOCUMENT_BYTES + 10000]);
        }
        Ok("fail") => std::process::exit(12),
        Ok("success") => {
            let _ = std::io::stdout().write_all(b"bounded-child-result");
        }
        _ => (),
    }
}

#[test]
fn process_deadline_output_bound_failure_and_success_reap_children() {
    use std::process::{Command, Stdio};
    for (mode, expected) in [
        ("sleep", Some(Error::Timeout)),
        ("overflow", Some(Error::OutputLimit)),
        ("fail", Some(Error::Failed)),
        ("success", None),
    ] {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "tests::child_fixture", "--nocapture"])
            .env("QNC_PROBE_PROCESS_TEST", mode)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let timeout = if mode == "sleep" {
            Duration::from_millis(200)
        } else {
            Duration::from_secs(5)
        };
        let result = process::run_command(&mut command, timeout);
        match expected {
            Some(error) => assert_eq!(result, Err(error)),
            None => assert!(String::from_utf8(result.unwrap())
                .unwrap()
                .contains("bounded-child-result")),
        }
    }
}
