//! The QNC AI server (docs/102 step 6, user 2026-11-11: an AI server on the LAN, the intranet or a
//! laptop). It listens where `ai-server.json` (beside the settings of this computer) says, with
//! that token. A QNC that names this server in its `ai-place.json` sends its first line
//! `{ "token", "program" }`; the server answers `{ "ok": true }` and starts that program of its
//! own AI catalog (only a program the catalog names, beside the server), then passes the plug-in's
//! JSON lines both ways unchanged. The programs find their models in this computer's AI catalog
//! (plug-in contracts version 2), so nothing of the other computer is read here.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Stdio;

use serde::{Deserialize, Serialize};

/// The setting of this computer as a server, beside its other settings.
const FILE: &str = "ai-server.json";
/// The longest first line a client may send.
const LONGEST_FIRST_LINE: usize = 4096;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Setting {
    /// Where it listens: `127.0.0.1:7843` for this computer only, an address of the LAN for others.
    listen: String,
    token: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Asked {
    token: String,
    program: String,
}

#[derive(Debug, Serialize)]
struct Answer {
    #[serde(skip_serializing_if = "Option::is_none")]
    ok: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn main() {
    qnc_dev_diagnostics::log_panics("qnc-ai-server");
    if let Err(error) = run() {
        eprintln!("qnc-ai-server: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let root = qnc_dev_diagnostics::resolve_qnc_root().ok_or("no QNC root")?;
    let path = qnc_dev_diagnostics::config_path(&root).with_file_name(FILE);
    let text = std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let setting: Setting = serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    if setting.token.len() < 16 {
        return Err(format!("{}: the token must be at least 16 characters", path.display()));
    }
    let programs = qnc_ai_catalog::installed_programs()?;
    // TLS: the server's own certificate, made the first time beside the settings; a QNC names
    // its fingerprint in its ai-place.json and trusts no other server.
    let folder = path.parent().ok_or("no settings folder")?;
    let (tls, fingerprint) = qnc_ai_tls::server_config(folder)?;
    println!("QNC AI server fingerprint (for ai-place.json): {fingerprint}");
    log(format!("ai-server fingerprint {fingerprint}"));
    let listener = TcpListener::bind(&setting.listen).map_err(|error| format!("listen {}: {error}", setting.listen))?;
    log(format!("ai-server listens on {} for {}", setting.listen, programs.join(", ")));
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let (token, programs, tls) = (setting.token.clone(), programs.clone(), tls.clone());
        let _ = std::thread::Builder::new().name("qnc-ai-server-client".into()).spawn(move || {
            let peer = stream.peer_addr().map(|addr| addr.to_string()).unwrap_or_default();
            match qnc_ai_tls::accept(stream, tls).and_then(|stream| serve(stream, &token, &programs)) {
                Ok(program) => log(format!("ai-server {peer} {program} ended")),
                Err(error) => log(format!("ai-server {peer} refused: {error}")),
            }
        });
    }
    Ok(())
}

type Tls = qnc_ai_tls::StreamOwned<qnc_ai_tls::ServerConnection, TcpStream>;

/// One client over TLS: its first line, then its program's lines both ways until either ends.
fn serve(mut writer: Tls, token: &str, programs: &[String]) -> Result<String, String> {
    let first = String::from_utf8(qnc_ai_tls::read_line(&mut writer, LONGEST_FIRST_LINE)?).map_err(|_| "a first line that is not UTF-8".to_string())?;
    let refuse = |writer: &mut Tls, why: String| {
        let _ = say(writer, &Answer { ok: None, error: Some(why.clone()) });
        Err(why)
    };
    let asked: Asked = match serde_json::from_str(first.trim_end()) {
        Ok(asked) => asked,
        Err(error) => return refuse(&mut writer, format!("a first line outside the server's contract: {error}")),
    };
    if !same(asked.token.as_bytes(), token.as_bytes()) {
        return refuse(&mut writer, "a wrong token".into());
    }
    if !programs.contains(&asked.program) {
        return refuse(&mut writer, format!("the program {} is not in this server's AI catalog", asked.program));
    }
    let executable = match qnc_plugin_process::beside_application(&asked.program) {
        Ok(executable) => executable,
        Err(error) => return refuse(&mut writer, error),
    };
    let mut child = match qnc_plugin_process::background_command(&executable).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() {
        Ok(child) => child,
        Err(error) => return refuse(&mut writer, format!("{} does not start: {error}", asked.program)),
    };
    say(&mut writer, &Answer { ok: Some(true), error: None })?;
    log(format!("ai-server started {}", asked.program));
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let mut stdout = child.stdout.take().ok_or("no stdout")?;
    // The program's answers go to the thread that runs the connection; its end ends the connection.
    let (answers, to_send) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buffer = vec![0u8; 64 << 10];
        while let Ok(read) = stdout.read(&mut buffer) {
            if read == 0 || answers.send(buffer[..read].to_vec()).is_err() {
                break;
            }
        }
    });
    // What the client sends goes to the program; the client gone ends it.
    qnc_ai_tls::pump(writer, |bytes| stdin.write_all(bytes).and_then(|()| stdin.flush()).is_ok(), to_send);
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
    Ok(asked.program)
}

fn say(writer: &mut Tls, answer: &Answer) -> Result<(), String> {
    let mut line = serde_json::to_vec(answer).map_err(|error| error.to_string())?;
    line.push(b'\n');
    writer.write_all(&line).and_then(|()| writer.flush()).map_err(|error| error.to_string())
}

/// The token compared in the same time whatever its first difference.
fn same(given: &[u8], token: &[u8]) -> bool {
    given.len() == token.len() && given.iter().zip(token).fold(0u8, |differs, (a, b)| differs | (a ^ b)) == 0
}

fn log(line: String) {
    eprintln!("{line}");
    qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line);
}
