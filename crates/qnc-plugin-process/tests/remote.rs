//! A plug-in on a server (docs/102 step 6): the first line names the program with the token, a
//! refusal of the server is the plug-in's error, never a quiet fall back to this computer.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::Duration;

use qnc_plugin_process::{Plugin, PluginSpec, Remote};

#[test]
fn the_server_hears_the_token_and_the_program_and_its_refusal_is_the_error() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut first = String::new();
        BufReader::new(stream.try_clone().unwrap()).read_line(&mut first).unwrap();
        let mut stream = stream;
        stream.write_all(b"{\"error\":\"a wrong token\"}\n").unwrap();
        first
    });
    let spec = PluginSpec { executable: "qnc-shot-content-siglip".into(), contract: "qnc.shot-content".into(), version: 2, timeout: Duration::from_secs(5), remote: Some(Remote { address, token: "t".into() }) };
    let error = Plugin::new(spec).ready().unwrap_err();
    assert!(error.contains("a wrong token"), "{error}");
    let first: serde_json::Value = serde_json::from_str(server.join().unwrap().trim_end()).unwrap();
    assert_eq!(first, serde_json::json!({ "token": "t", "program": "qnc-shot-content-siglip" }));
}
