//! The QNC AI server (docs/102 step 6, user 2026-11-11: an AI server on the LAN, the intranet or a
//! laptop). It listens where `ai-server.json` (beside the settings of this computer) says, with
//! that token. A QNC that names this server in its `ai-place.json` sends its first line
//! `{ "token", "program" }`; the server answers `{ "ok": true }` and starts that program of its
//! own AI catalog (only a program the catalog names, beside the server), then passes the plug-in's
//! JSON lines both ways unchanged. The programs find their models in this computer's AI catalog
//! (plug-in contracts version 2), so nothing of the other computer is read here.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Stdio;

use serde::{Deserialize, Serialize};

/// The setting of this computer as a server, beside its other settings.
const FILE: &str = "ai-server.json";
/// The longest first line a client may send.
const LONGEST_FIRST_LINE: u64 = 4096;

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
    let listener = TcpListener::bind(&setting.listen).map_err(|error| format!("listen {}: {error}", setting.listen))?;
    log(format!("ai-server listens on {} for {}", setting.listen, programs.join(", ")));
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let (token, programs) = (setting.token.clone(), programs.clone());
        let _ = std::thread::Builder::new().name("qnc-ai-server-client".into()).spawn(move || {
            let peer = stream.peer_addr().map(|addr| addr.to_string()).unwrap_or_default();
            match serve(stream, &token, &programs) {
                Ok(program) => log(format!("ai-server {peer} {program} ended")),
                Err(error) => log(format!("ai-server {peer} refused: {error}")),
            }
        });
    }
    Ok(())
}

/// One client: its first line, then its program's lines both ways until either ends.
fn serve(stream: TcpStream, token: &str, programs: &[String]) -> Result<String, String> {
    let mut reader = BufReader::new(stream.try_clone().map_err(|error| error.to_string())?);
    let mut writer = stream;
    let mut first = String::new();
    (&mut reader).take(LONGEST_FIRST_LINE).read_line(&mut first).map_err(|error| error.to_string())?;
    let refuse = |writer: &mut TcpStream, why: String| {
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
    let mut back = writer.try_clone().map_err(|error| error.to_string())?;
    let out = std::thread::spawn(move || {
        let _ = std::io::copy(&mut stdout, &mut back);
        let _ = back.shutdown(std::net::Shutdown::Both);
    });
    // What the client sends after its first line goes to the program; the client gone ends it.
    let _ = std::io::copy(&mut reader, &mut stdin);
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
    let _ = writer.shutdown(std::net::Shutdown::Both);
    let _ = out.join();
    Ok(asked.program)
}

fn say(writer: &mut TcpStream, answer: &Answer) -> Result<(), String> {
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
