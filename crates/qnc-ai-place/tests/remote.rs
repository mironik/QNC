//! A plug-in on a server (docs/102 step 6): over TLS with the server of the fingerprint named, the
//! first line names the program with the token, a refusal of the server is the plug-in's error,
//! never a quiet fall back to this computer; another server's certificate is refused.

use std::io::Write;
use std::net::TcpListener;
use std::time::Duration;

use std::sync::Arc;

use qnc_ai_place::AiServer;
use qnc_plugin_process::{Plugin, PluginSpec};

fn spec(address: String, fingerprint: String) -> PluginSpec {
    PluginSpec {
        executable: "qnc-shot-content-siglip".into(),
        contract: "qnc.shot-content".into(),
        version: 2,
        timeout: Duration::from_secs(5),
        remote: Some(Arc::new(AiServer { address, fingerprint, token: "t".into() })),
    }
}

#[test]
fn the_server_hears_the_token_and_the_program_over_tls_and_its_refusal_is_the_error() {
    let folder = tempfile::tempdir().unwrap();
    let (config, fingerprint) = qnc_ai_tls::server_config(folder.path()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut tls = qnc_ai_tls::accept(stream, config.clone()).unwrap();
        let first = qnc_ai_tls::read_line(&mut tls, 4096).unwrap();
        tls.write_all(b"{\"error\":\"a wrong token\"}\n").unwrap();
        tls.flush().unwrap();
        // A second client, with another fingerprint, does not get through the handshake.
        let (stream, _) = listener.accept().unwrap();
        assert!(qnc_ai_tls::accept(stream, config).is_err());
        String::from_utf8(first).unwrap()
    });
    let error = Plugin::new(spec(address.clone(), fingerprint)).ready().unwrap_err();
    assert!(error.contains("a wrong token"), "{error}");
    let other = Plugin::new(spec(address, "0".repeat(64))).ready().unwrap_err();
    assert!(other.contains("certificate"), "{other}");
    let first: serde_json::Value = serde_json::from_str(&server.join().unwrap()).unwrap();
    assert_eq!(first, serde_json::json!({ "token": "t", "program": "qnc-shot-content-siglip" }));
}
