//! A plug-in as its own process (user 2026-10-23: "blocks like a DLL, so that the application
//! is easier to service"). A plug-in is a program beside the application that speaks one
//! versioned contract as JSON lines on stdin and stdout: the application says which contract
//! and version it speaks, the plug-in answers who it is, then each request has one answer.
//! Replacing a plug-in is replacing its program. A plug-in that does not answer in time, answers
//! too much or answers another contract gives a controlled error; nothing falls back to anything.
//!
//! The process is started at its first request, serves every request of a job, and is stopped
//! when it has had nothing to do for a while, or when this handle goes.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        mpsc::{self, Receiver, RecvTimeoutError},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

pub const MODULE_ID: &str = "qnc.module.plugin-process";

/// The longest answer line a plug-in may give.
pub const MAX_LINE_BYTES: usize = 64 << 20;
/// A process with nothing to do this long is stopped (it starts again at the next request).
pub const IDLE: Duration = Duration::from_secs(30);

/// The first line the application writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    pub contract: String,
    pub version: u32,
}

/// The plug-in's first answer: who it is. `about` carries what the contract asks a plug-in to say.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ready {
    pub contract: String,
    pub version: u32,
    pub plugin_id: String,
    pub plugin_version: String,
    #[serde(default)]
    pub about: Value,
}

/// One request and its answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: u64,
    pub op: String,
    pub body: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Answer {
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ok: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// How the application reaches one plug-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSpec {
    pub executable: PathBuf,
    pub contract: String,
    pub version: u32,
    /// How long one answer may take.
    pub timeout: Duration,
}

/// A plug-in's program by its name, beside the running application (or one folder up, where a
/// test or an example of the build runs from). A name with a path in it is refused.
pub fn beside_application(name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || name.contains(['/', '\\']) {
        return Err(format!("a plug-in is named by its program name, not a path: '{name}'"));
    }
    let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    exe.ancestors()
        .skip(1)
        .take(2)
        .map(|dir| dir.join(&file))
        .find(|path| path.is_file())
        .ok_or_else(|| format!("the plug-in program '{file}' is not beside the application"))
}

struct Running {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<Result<String, String>>,
    ready: Ready,
    next_id: u64,
    used: Instant,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One plug-in, started when it is first asked.
pub struct Plugin {
    spec: PluginSpec,
    running: Arc<Mutex<Option<Running>>>,
}

impl Plugin {
    pub fn new(spec: PluginSpec) -> Self {
        let running: Arc<Mutex<Option<Running>>> = Arc::default();
        let idle = Arc::downgrade(&running);
        // Stops the process when it has had nothing to do for a while; ends with the handle.
        std::thread::Builder::new()
            .name("qnc-plugin-idle".into())
            .spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                let Some(running) = idle.upgrade() else { break };
                let mut guard = running.lock().unwrap_or_else(|e| e.into_inner());
                if guard.as_ref().is_some_and(|r| r.used.elapsed() >= IDLE) {
                    *guard = None;
                }
            })
            .ok();
        Self { spec, running }
    }

    /// Who the plug-in is (it is started for this when it is not running).
    pub fn ready(&self) -> Result<Ready, String> {
        let mut guard = self.running.lock().unwrap_or_else(|e| e.into_inner());
        Ok(self.started(&mut guard)?.ready.clone())
    }

    /// Asks `op` with `body`; the plug-in's answer, or its error.
    pub fn call(&self, op: &str, body: Value) -> Result<Value, String> {
        let mut guard = self.running.lock().unwrap_or_else(|e| e.into_inner());
        let running = self.started(&mut guard)?;
        let id = running.next_id;
        running.next_id += 1;
        let asked = send(&mut running.stdin, &Request { id, op: op.into(), body }).and_then(|()| receive::<Answer>(&running.lines, self.spec.timeout));
        let answer = match asked {
            Ok(answer) => answer,
            Err(error) => {
                *guard = None; // a plug-in that broke is started again at the next request
                return Err(format!("plug-in {}: {error}", self.spec.executable.display()));
            }
        };
        running.used = Instant::now();
        if answer.id != id {
            *guard = None;
            return Err(format!("plug-in {} answered request {} for {id}", self.spec.executable.display(), answer.id));
        }
        match (answer.ok, answer.error) {
            (_, Some(error)) => Err(error),
            (Some(ok), None) => Ok(ok),
            (None, None) => Ok(Value::Null),
        }
    }

    fn started<'a>(&self, guard: &'a mut Option<Running>) -> Result<&'a mut Running, String> {
        if guard.as_mut().is_some_and(|r| r.child.try_wait().ok().flatten().is_some()) {
            *guard = None;
        }
        if guard.is_none() {
            *guard = Some(start(&self.spec)?);
        }
        Ok(guard.as_mut().expect("started"))
    }
}

fn start(spec: &PluginSpec) -> Result<Running, String> {
    let mut command = Command::new(&spec.executable);
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // no console window
    }
    let mut child = command.spawn().map_err(|e| format!("plug-in {} does not start: {e}", spec.executable.display()))?;
    let mut stdin = child.stdin.take().ok_or("plug-in has no stdin")?;
    let stdout = child.stdout.take().ok_or("plug-in has no stdout")?;
    let (send_line, lines) = mpsc::channel();
    std::thread::Builder::new()
        .name("qnc-plugin-read".into())
        .spawn(move || read_lines(stdout, send_line))
        .map_err(|e| e.to_string())?;
    let hello = Hello { contract: spec.contract.clone(), version: spec.version };
    let ready = send(&mut stdin, &hello).and_then(|()| receive::<Ready>(&lines, spec.timeout));
    let mut running = Running { child, stdin, lines, ready: Ready { contract: String::new(), version: 0, plugin_id: String::new(), plugin_version: String::new(), about: Value::Null }, next_id: 1, used: Instant::now() };
    let ready = ready.map_err(|e| format!("plug-in {} did not say who it is: {e}", spec.executable.display()))?;
    if ready.contract != spec.contract || ready.version != spec.version {
        return Err(format!(
            "plug-in {} speaks {} {}, not {} {}",
            spec.executable.display(),
            ready.contract,
            ready.version,
            spec.contract,
            spec.version
        ));
    }
    running.ready = ready;
    Ok(running)
}

fn read_lines(stdout: impl std::io::Read, lines: mpsc::Sender<Result<String, String>>) {
    let mut reader = BufReader::new(stdout);
    loop {
        let mut line = Vec::new();
        let read = (&mut reader).take(MAX_LINE_BYTES as u64 + 1).read_until(b'\n', &mut line);
        let last = matches!(read, Ok(0) | Err(_));
        let sent = match read {
            Ok(0) => lines.send(Err("the plug-in ended".into())),
            Ok(_) if line.len() > MAX_LINE_BYTES => lines.send(Err(format!("an answer longer than {MAX_LINE_BYTES} bytes"))),
            Ok(_) => lines.send(String::from_utf8(line).map_err(|_| "an answer that is not UTF-8".to_string())),
            Err(error) => lines.send(Err(error.to_string())),
        };
        if sent.is_err() || last {
            break;
        }
    }
}

use std::io::Read as _;

fn send(stdin: &mut ChildStdin, message: &impl Serialize) -> Result<(), String> {
    let mut line = serde_json::to_vec(message).map_err(|e| e.to_string())?;
    line.push(b'\n');
    stdin.write_all(&line).and_then(|()| stdin.flush()).map_err(|e| format!("the plug-in does not listen: {e}"))
}

fn receive<T: serde::de::DeserializeOwned>(lines: &Receiver<Result<String, String>>, timeout: Duration) -> Result<T, String> {
    let line = match lines.recv_timeout(timeout) {
        Ok(line) => line?,
        Err(RecvTimeoutError::Timeout) => return Err(format!("no answer in {} ms", timeout.as_millis())),
        Err(RecvTimeoutError::Disconnected) => return Err("the plug-in ended".into()),
    };
    serde_json::from_str(line.trim_end()).map_err(|e| format!("an answer outside the contract: {e}"))
}

/// The plug-in's side: says who it is when the application's contract and version are `ready`'s,
/// then answers each request with `handle` until stdin ends.
pub fn serve(ready: Ready, mut handle: impl FnMut(&str, Value) -> Result<Value, String>) -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    let mut lines = stdin.lock().lines();
    let hello: Hello = match lines.next() {
        Some(line) => serde_json::from_str(&line.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?,
        None => return Ok(()),
    };
    let mut answer = ready.clone();
    if hello.contract != ready.contract || hello.version != ready.version {
        // It says what it speaks; the application refuses it.
        answer.about = Value::Null;
    }
    write_line(&mut stdout, &answer)?;
    for line in lines {
        let line = line.map_err(|e| e.to_string())?;
        let request: Request = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        let answer = match handle(&request.op, request.body) {
            Ok(ok) => Answer { id: request.id, ok: Some(ok), error: None },
            Err(error) => Answer { id: request.id, ok: None, error: Some(error) },
        };
        write_line(&mut stdout, &answer)?;
    }
    Ok(())
}

fn write_line(out: &mut impl Write, message: &impl Serialize) -> Result<(), String> {
    let mut line = serde_json::to_vec(message).map_err(|e| e.to_string())?;
    line.push(b'\n');
    out.write_all(&line).and_then(|()| out.flush()).map_err(|e| e.to_string())
}

/// The program of a plug-in for `spec`, as a path for messages.
pub fn program(spec: &PluginSpec) -> &Path {
    &spec.executable
}
