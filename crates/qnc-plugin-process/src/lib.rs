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
    /// A part of the answer that is still coming (a line with a part is not the answer's end).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<Value>,
}

/// The request the application writes to stop the job of request `body` (a number).
pub const CANCEL: &str = "cancel";
/// How long a plug-in has to end a cancelled job before its process is stopped.
pub const CANCEL_GRACE: Duration = Duration::from_secs(2);

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

/// A command for a program of QNC that runs in the background: no console window on
/// Windows (audit 2026-10-25: written the same way in several pieces).
pub fn background_command(executable: &Path) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(executable);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // no console window
    }
    command
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
        let never = std::sync::atomic::AtomicBool::new(false);
        self.call_parts(op, body, &never, &mut |_| Err("a part where none is asked for".into()))
    }

    /// Asks `op` with `body` and hands every part of the answer to `on_part` as it comes; the
    /// timeout holds between two lines. When `cancel` is set the plug-in is asked to stop the job
    /// and, when it does not end it in [`CANCEL_GRACE`], its process is stopped.
    pub fn call_parts(
        &self,
        op: &str,
        body: Value,
        cancel: &std::sync::atomic::AtomicBool,
        on_part: &mut dyn FnMut(Value) -> Result<(), String>,
    ) -> Result<Value, String> {
        use std::sync::atomic::Ordering;
        let mut guard = self.running.lock().unwrap_or_else(|e| e.into_inner());
        let running = self.started(&mut guard)?;
        let id = running.next_id;
        running.next_id += 1;
        let program = self.spec.executable.display().to_string();
        if let Err(error) = send(&mut running.stdin, &Request { id, op: op.into(), body }) {
            *guard = None; // a plug-in that broke is started again at the next request
            return Err(format!("plug-in {program}: {error}"));
        }
        let mut cancelled_at: Option<Instant> = None;
        let mut last_line = Instant::now();
        let answer = loop {
            if cancelled_at.is_none() && cancel.load(Ordering::Relaxed) {
                if send(&mut running.stdin, &Request { id: 0, op: CANCEL.into(), body: id.into() }).is_err() {
                    *guard = None;
                    return Err("cancelled".into());
                }
                cancelled_at = Some(Instant::now());
            }
            if cancelled_at.is_some_and(|at| at.elapsed() >= CANCEL_GRACE) {
                *guard = None;
                return Err("cancelled".into());
            }
            let answer = match receive::<Answer>(&running.lines, Duration::from_millis(100)) {
                Ok(answer) => answer,
                Err(error) if error.starts_with(NO_ANSWER) => {
                    if last_line.elapsed() >= self.spec.timeout {
                        *guard = None;
                        return Err(format!("plug-in {program}: no answer in {} ms", self.spec.timeout.as_millis()));
                    }
                    continue;
                }
                Err(error) => {
                    *guard = None;
                    return Err(format!("plug-in {program}: {error}"));
                }
            };
            last_line = Instant::now();
            if answer.id != id {
                *guard = None;
                return Err(format!("plug-in {program} answered request {} for {id}", answer.id));
            }
            match answer.part {
                Some(part) if cancelled_at.is_none() => {
                    if let Err(error) = on_part(part) {
                        // A refused part ends the job as a cancel does.
                        *guard = None;
                        return Err(error);
                    }
                }
                Some(_) => {}
                None => break answer,
            }
        };
        running.used = Instant::now();
        if cancelled_at.is_some() {
            return Err("cancelled".into());
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
    let mut command = background_command(&spec.executable);
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
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

const NO_ANSWER: &str = "no answer in";

fn receive<T: serde::de::DeserializeOwned>(lines: &Receiver<Result<String, String>>, timeout: Duration) -> Result<T, String> {
    let line = match lines.recv_timeout(timeout) {
        Ok(line) => line?,
        Err(RecvTimeoutError::Timeout) => return Err(format!("{NO_ANSWER} {} ms", timeout.as_millis())),
        Err(RecvTimeoutError::Disconnected) => return Err("the plug-in ended".into()),
    };
    serde_json::from_str(line.trim_end()).map_err(|e| format!("an answer outside the contract: {e}"))
}

/// The plug-in's side: says who it is when the application's contract and version are `ready`'s,
/// then answers each request with `handle` until stdin ends.
pub fn serve(ready: Ready, mut handle: impl FnMut(&str, Value) -> Result<Value, String>) -> Result<(), String> {
    serve_parts(ready, |op, body, _, _| handle(op, body))
}

/// Whether the application cancelled the running job of [`serve_parts`].
pub type Cancelled = std::sync::atomic::AtomicBool;
/// Sends a part of the running job's answer before its end.
pub type PartSender<'a> = dyn FnMut(Value) -> Result<(), String> + 'a;

/// [`serve`] for jobs that answer in parts or can be cancelled: one job at a time; a [`CANCEL`]
/// request for the running job sets the flag `handle` is given, and the job ends early.
pub fn serve_parts(
    ready: Ready,
    mut handle: impl FnMut(&str, Value, &Cancelled, &mut PartSender) -> Result<Value, String>,
) -> Result<(), String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    let mut stdout = std::io::stdout().lock();
    // Lines are read beside the job, so that a cancel reaches the job that runs.
    let current = Arc::new(AtomicU64::new(0));
    let cancelled = Arc::new(Cancelled::new(false));
    let (send_line, lines) = mpsc::channel::<Result<String, String>>();
    {
        let (current, cancelled) = (current.clone(), cancelled.clone());
        std::thread::Builder::new()
            .name("qnc-plugin-requests".into())
            .spawn(move || {
                let stdin = std::io::stdin();
                for line in stdin.lock().lines() {
                    let line = line.map_err(|e| e.to_string());
                    if let Ok(text) = &line {
                        if let Ok(cancel) = serde_json::from_str::<Request>(text) {
                            if cancel.op == CANCEL {
                                if cancel.body.as_u64() == Some(current.load(Ordering::SeqCst)) {
                                    cancelled.store(true, Ordering::SeqCst);
                                }
                                continue;
                            }
                        }
                    }
                    if send_line.send(line).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
    }
    let mut lines = lines.into_iter();
    let hello: Hello = match lines.next() {
        Some(line) => serde_json::from_str(&line?).map_err(|e| e.to_string())?,
        None => return Ok(()),
    };
    let mut answer = ready.clone();
    if hello.contract != ready.contract || hello.version != ready.version {
        // It says what it speaks; the application refuses it.
        answer.about = Value::Null;
    }
    write_line(&mut stdout, &answer)?;
    for line in lines {
        let request: Request = serde_json::from_str(&line?).map_err(|e| e.to_string())?;
        cancelled.store(false, Ordering::SeqCst);
        current.store(request.id, Ordering::SeqCst);
        let id = request.id;
        let mut parts_failed: Option<String> = None;
        let result = {
            let out = &mut stdout;
            let failed = &mut parts_failed;
            let mut send_part = |part: Value| {
                let sent = write_line(out, &Answer { id, ok: None, error: None, part: Some(part) });
                if let Err(error) = &sent {
                    *failed = Some(error.clone());
                }
                sent
            };
            handle(&request.op, request.body, &cancelled, &mut send_part)
        };
        current.store(0, Ordering::SeqCst);
        if let Some(error) = parts_failed {
            return Err(error);
        }
        let answer = match result {
            Ok(ok) => Answer { id, ok: Some(ok), error: None, part: None },
            Err(error) => Answer { id, ok: None, error: Some(error), part: None },
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
