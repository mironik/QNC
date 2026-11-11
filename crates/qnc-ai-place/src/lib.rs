//! Where the AI programs of this computer run (docs/102 step 6, user 2026-11-11: an AI server on
//! the LAN, the intranet, or a laptop). A setting of this computer, never of a project:
//! `ai-place.json` beside the other settings of this computer. Without it every AI program runs
//! beside the application; with `{ "server": "host:port", "fingerprint": "...", "token": "..." }` every AI program runs
//! on that server (`qnc-ai-server`), which speaks the same plug-in contracts.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use serde::Deserialize;

pub const MODULE_ID: &str = "qnc.module.ai-place";
/// The setting of this computer, beside its other settings.
pub const FILE: &str = "ai-place.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    server: String,
    /// The SHA-256 of the server's certificate, as its server says it (TLS: only it is trusted).
    fingerprint: String,
    token: String,
}

/// The AI server of this computer: its address, the fingerprint of its certificate (TLS: only that
/// server is trusted) and its token. Its first line, over TLS, is `{ token, program }`; its answer
/// `{ ok: true }` or `{ error }`; then the plug-in's lines go both ways unchanged.
#[derive(Clone, PartialEq, Eq)]
pub struct AiServer {
    pub address: String,
    pub fingerprint: String,
    pub token: String,
}

impl std::fmt::Debug for AiServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiServer").field("address", &self.address).finish_non_exhaustive()
    }
}

#[derive(Debug, Deserialize)]
struct ServerAnswer {
    #[serde(default)]
    ok: Option<bool>,
    #[serde(default)]
    error: Option<String>,
}

impl qnc_plugin_process::Connect for AiServer {
    fn address(&self) -> String {
        self.address.clone()
    }

    fn connect(&self, program: &str, timeout: Duration, lines: mpsc::Sender<Result<String, String>>) -> Result<Box<dyn Write + Send>, String> {
        use std::net::ToSocketAddrs;
        let at = |e: String| format!("AI server {}: {e}", self.address);
        let address = self.address.to_socket_addrs().map_err(|e| at(e.to_string()))?.next().ok_or_else(|| at("no address".into()))?;
        let stream = std::net::TcpStream::connect_timeout(&address, Duration::from_secs(5)).map_err(|e| at(e.to_string()))?;
        let _ = stream.set_nodelay(true);
        stream.set_read_timeout(Some(timeout)).map_err(|e| e.to_string())?;
        // TLS with the one certificate the setting names (nothing goes over the network readable).
        let mut tls = qnc_ai_tls::connect(stream, qnc_ai_tls::client_config(&self.fingerprint).map_err(at)?).map_err(at)?;
        let mut first = serde_json::to_vec(&serde_json::json!({ "token": self.token, "program": program })).map_err(|e| e.to_string())?;
        first.push(b'\n');
        tls.write_all(&first).and_then(|()| tls.flush()).map_err(|e| at(e.to_string()))?;
        let answer = qnc_ai_tls::read_line(&mut tls, 4096).map_err(at)?;
        let answer: ServerAnswer = serde_json::from_slice(&answer).map_err(|e| at(format!("an answer outside its contract: {e}")))?;
        if let Some(error) = answer.error {
            return Err(at(error));
        }
        if answer.ok != Some(true) {
            return Err(at("no answer".into()));
        }
        // One thread runs the connection: the program's lines go to `lines` as a process's do.
        let (outgoing, to_send) = mpsc::channel::<Vec<u8>>();
        std::thread::Builder::new()
            .name("qnc-ai-server-connection".into())
            .spawn(move || {
                qnc_ai_tls::pump(tls, qnc_plugin_process::line_sink(lines.clone()), to_send);
                let _ = lines.send(Err("the AI server ended the connection".into()));
            })
            .map_err(|e| e.to_string())?;
        Ok(Box::new(qnc_plugin_process::ChannelWriter::new(outgoing)))
    }
}

/// The AI server of this computer from the settings of the QNC `root`; none when AI runs here.
/// A setting that cannot be read is an error, never a quiet fall back to this computer.
pub fn server_of(root: &Path) -> Result<Option<AiServer>, String> {
    let path = qnc_dev_diagnostics::config_path(root).with_file_name(FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let file: File = serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    if file.server.trim().is_empty() || file.token.trim().is_empty() || file.fingerprint.trim().is_empty() {
        return Err(format!("{}: an AI server needs its address, its fingerprint and its token", path.display()));
    }
    Ok(Some(AiServer { address: file.server, fingerprint: file.fingerprint, token: file.token }))
}

/// The AI server of this computer (the QNC root it runs in); none when AI runs here.
pub fn server() -> Result<Option<AiServer>, String> {
    let root = qnc_dev_diagnostics::resolve_qnc_root().ok_or("no QNC root")?;
    server_of(&root)
}

/// The plug-in spec of AI program `program`: beside the application, or on the AI server of this
/// computer (there the program is named, the server starts its own).
pub fn spec(program: &str, contract: &str, version: u32, timeout: std::time::Duration) -> Result<qnc_plugin_process::PluginSpec, String> {
    let remote = server()?;
    let executable = match &remote {
        Some(_) => PathBuf::from(program),
        None => qnc_plugin_process::beside_application(program)?,
    };
    let remote = remote.map(|server| Arc::new(server) as qnc_plugin_process::Remote);
    Ok(qnc_plugin_process::PluginSpec { executable, contract: contract.into(), version, timeout, remote })
}

#[cfg(test)]
mod tests {
    #[test]
    fn without_the_setting_ai_runs_here_and_a_half_setting_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(super::server_of(root.path()).unwrap(), None);
        let file = qnc_dev_diagnostics::config_path(root.path()).with_file_name(super::FILE);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, r#"{"server": "127.0.0.1:7843", "fingerprint": "ab", "token": ""}"#).unwrap();
        assert!(super::server_of(root.path()).is_err());
        std::fs::write(&file, r#"{"server": "127.0.0.1:7843", "fingerprint": "ab", "token": "t"}"#).unwrap();
        assert_eq!(super::server_of(root.path()).unwrap().unwrap().address, "127.0.0.1:7843");
    }
}
