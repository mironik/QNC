//! Explicit read-only native-process test. No application or player executor is linked here.
use qnc_media_stream::{LocalSource, MediaStream, SourceReference};
use qnc_player_contract::{
    BroadcastPlayerProtocolCommand as Command, BroadcastPlayerProtocolEvent as Event, VERSION,
    envelope::{CommandEnvelope, EventEnvelope},
    session::{
        MAX_CONTROL_BYTES, SessionQuery, SessionReply, SessionRequest, WIRE_HEADER_BYTES,
        WIRE_KIND_CONTROL, WIRE_MAGIC, WIRE_MAX_TOKEN_BYTES, WIRE_SCHEME, WIRE_STATUS_OK,
        WIRE_VERSION,
    },
};
use qnc_player_input::InputReader;
use qnc_work_settings::SettingsReader;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{BufRead, BufReader, Read, Seek, Write},
    net::TcpStream,
    path::PathBuf,
    process::{Child, Command as Process, Stdio},
    thread,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

struct ChildPlayer(Child);
impl Drop for ChildPlayer {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct PlayerWireClient {
    stream: TcpStream,
    token: String,
}
impl PlayerWireClient {
    fn connect(wire_url: &str, token: &str) -> Result<Self> {
        if token.is_empty()
            || token.len() > WIRE_MAX_TOKEN_BYTES
            || !token.bytes().all(|b| (33..=126).contains(&b))
        {
            return Err("invalid player wire token".into());
        }
        let address = wire_url
            .strip_prefix(WIRE_SCHEME)
            .ok_or("unsupported player wire protocol")?;
        let stream = TcpStream::connect(address)?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        Ok(Self {
            stream,
            token: token.into(),
        })
    }
    fn post<Q: serde::Serialize, R: serde::de::DeserializeOwned>(
        &mut self,
        request: &Q,
    ) -> Result<R> {
        let request = serde_json::to_vec(request)?;
        if request.len() > MAX_CONTROL_BYTES {
            return Err("player request too large".into());
        }
        let mut header = [0; WIRE_HEADER_BYTES];
        header[..8].copy_from_slice(WIRE_MAGIC);
        header[8] = WIRE_VERSION;
        header[9] = WIRE_KIND_CONTROL;
        header[10..12].copy_from_slice(&(self.token.len() as u16).to_le_bytes());
        header[12..16].copy_from_slice(&(request.len() as u32).to_le_bytes());
        self.stream.write_all(&header)?;
        self.stream.write_all(self.token.as_bytes())?;
        self.stream.write_all(&request)?;
        self.stream.flush()?;
        let mut response = [0; WIRE_HEADER_BYTES];
        self.stream.read_exact(&mut response)?;
        if &response[..8] != WIRE_MAGIC || response[8] != WIRE_VERSION {
            return Err("player wire protocol mismatch".into());
        }
        let status = response[9];
        let len = u32::from_le_bytes(response[12..16].try_into().unwrap()) as usize;
        if len > MAX_CONTROL_BYTES {
            return Err("player response too large".into());
        }
        let mut body = vec![0; len];
        self.stream.read_exact(&mut body)?;
        if status != WIRE_STATUS_OK {
            return Err(format!("player wire status {status}").into());
        }
        Ok(serde_json::from_slice(&body)?)
    }
}

struct Client {
    id: String,
    command: u64,
    event: u64,
    writer: PlayerWireClient,
    reader: PlayerWireClient,
}
impl Client {
    fn start(mut boot: Value, timeout_ms: u64) -> Result<(ChildPlayer, Self)> {
        let id = uuid::Uuid::new_v4().to_string();
        let read_token = uuid::Uuid::new_v4().to_string();
        let command_token = uuid::Uuid::new_v4().to_string();
        boot["session_id"] = id.clone().into();
        boot["read_token"] = read_token.clone().into();
        boot["command_token"] = command_token.clone().into();
        boot["idle_timeout_ms"] = timeout_ms.into();
        let current = std::env::current_exe()?;
        let exe = current
            .parent()
            .and_then(|p| p.parent())
            .ok_or("example directory")?
            .join("qnc-broadcast-player")
            .with_extension(std::env::consts::EXE_EXTENSION);
        let mut child = ChildPlayer(
            Process::new(exe)
                .arg("--native-output")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()?,
        );
        let mut stdin = child.0.stdin.take().ok_or("stdin")?;
        serde_json::to_writer(&mut stdin, &boot)?;
        stdin.flush()?;
        drop(stdin);
        let stdout = child.0.stdout.take().ok_or("stdout")?;
        let mut line = String::new();
        BufReader::new(stdout)
            .take(64 * 1024)
            .read_line(&mut line)?;
        let started: Value = serde_json::from_str(&line)?;
        if started["session_id"] != id
            || started["contract_version"] != VERSION
            || started["source_generation"] != 1
        {
            return Err("invalid process announcement".into());
        }
        let wire_url = started["wire_url"].as_str().ok_or("endpoint")?;
        let client = Self {
            id,
            command: 0,
            event: 0,
            writer: PlayerWireClient::connect(wire_url, &command_token)?,
            reader: PlayerWireClient::connect(wire_url, &read_token)?,
        };
        println!("Native Broadcast Player PID {} started", child.0.id());
        Ok((child, client))
    }
    fn query(&self) -> SessionRequest {
        SessionRequest::State(SessionQuery {
            contract_version: VERSION.into(),
            session_id: self.id.clone(),
            source_generation: 1,
        })
    }
    fn checked(&mut self, reply: SessionReply) -> Result<EventEnvelope> {
        let reply = reply?;
        reply.validate_for(&self.id, 1, self.event)?;
        self.event = reply.sequence;
        if reply.events.iter().any(|e| {
            matches!(
                e,
                Event::FramePresented { .. } | Event::PlaybackError { .. }
            )
        }) {
            return Err(format!("unexpected presentation/error: {:?}", reply.events).into());
        }
        Ok(reply)
    }
    fn state(&mut self) -> Result<EventEnvelope> {
        let query = self.query();
        let reply = self.reader.post(&query)?;
        self.checked(reply)
    }
    fn envelope(&self, command: Command) -> CommandEnvelope {
        CommandEnvelope {
            contract_version: VERSION.into(),
            session_id: self.id.clone(),
            request_id: format!("r{}", self.command + 1),
            source_generation: 1,
            sequence: self.command + 1,
            command,
        }
    }
    fn command(&mut self, command: Command) -> Result<EventEnvelope> {
        let envelope = self.envelope(command);
        self.command = envelope.sequence;
        let started = Instant::now();
        let reply = self
            .writer
            .post(&SessionRequest::Command(envelope.clone()))?;
        let reply = self.checked(reply)?;
        if !reply.events.iter().any(|e| matches!(e, Event::CommandAccepted { command_id, .. } if command_id == &envelope.request_id)) {
            return Err(format!("command rejected: {:?}", reply.events).into());
        }
        println!(
            "{}: socket roundtrip {} us",
            envelope.command.command_name(),
            started.elapsed().as_micros()
        );
        Ok(reply)
    }
    fn wait(&mut self, predicate: impl Fn(&EventEnvelope) -> bool) -> Result<EventEnvelope> {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let state = self.state()?;
            if predicate(&state) {
                return Ok(state);
            }
            if Instant::now() > deadline {
                return Err(format!("state timeout: {:?}", state.events).into());
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}
fn ready(reply: &EventEnvelope) -> bool {
    reply
        .events
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::PlaybackReadinessChanged { ready, .. } => Some(*ready),
            _ => None,
        })
        .unwrap_or(false)
}
fn position(reply: &EventEnvelope) -> u64 {
    reply
        .events
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::CarrierPositionChanged { frame, .. } => Some(*frame),
            _ => None,
        })
        .expect("player snapshot position")
}
fn ended(reply: &EventEnvelope) -> bool {
    reply
        .events
        .iter()
        .any(|e| matches!(e, Event::PlaybackBoundaryReached { .. }))
}
fn wait_exit(child: &mut ChildPlayer) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.0.try_wait()? {
            if !status.success() {
                return Err("player exited with error".into());
            }
            return Ok(());
        }
        if Instant::now() > deadline {
            return Err("player did not release its process".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}
fn hash(stream: &mut MediaStream) -> Result<String> {
    stream.rewind()?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("Usage: live_control ROOT SOURCE_DIRECTORY CLIP_ID".into());
    }
    let reader = SettingsReader::from_root(&PathBuf::from(&args[0]))?;
    let settings = reader.read()?;
    let input = InputReader::new(reader.clone()).load(
        &settings.workspace_db_uri,
        args[2].to_str().ok_or("clip id")?,
    )?;
    let frames = input
        .layout
        .video
        .as_ref()
        .ok_or("video required")?
        .duration_frames;
    if frames < 30 {
        return Err("diagnostic requires at least 30 saved frames".into());
    }
    let source_uri = SourceReference::from_uri(&input.media()?.media_uri)?
        .source_uri()
        .to_string();
    let source = LocalSource::new(&source_uri, PathBuf::from(&args[1]))?;
    let mut media = MediaStream::local(&source, &input.media()?.media_uri)?;
    let before = hash(&mut media)?;
    let boot = json!({ "contract_version": VERSION, "source_generation": 1, "input": input,
        "media_binding": { "kind": "local", "source_uri": source_uri, "root": PathBuf::from(&args[1]) },
        "listen_port": 0 });
    let (mut process, mut client) = Client::start(boot.clone(), 30000)?;
    client.wait(ready)?;
    let forbidden = SessionRequest::Command(client.envelope(Command::Play));
    assert!(client.reader.post::<_, SessionReply>(&forbidden)?.is_err());
    let mut stale = client.envelope(Command::Play);
    stale.source_generation = 2;
    assert!(
        client
            .writer
            .post::<_, SessionReply>(&SessionRequest::Command(stale))?
            .is_err()
    );
    let mut wrong = client.envelope(Command::Play);
    wrong.session_id = "not-this-session".into();
    assert!(
        client
            .writer
            .post::<_, SessionReply>(&SessionRequest::Command(wrong))?
            .is_err()
    );
    client.command(Command::Play)?;
    client.wait(|state| position(state) >= 10)?;
    let paused = client.command(Command::Pause)?;
    let at = position(&paused);
    thread::sleep(Duration::from_millis(150));
    assert_eq!(position(&client.state()?), at);
    let (mut independent, mut second) = Client::start(boot, 2000)?;
    assert_eq!(position(&second.wait(ready)?), 0);
    let mut cross = client.envelope(Command::Play);
    cross.session_id = second.id.clone();
    assert!(
        client
            .writer
            .post::<_, SessionReply>(&SessionRequest::Command(cross))?
            .is_err()
    );
    let cue = Command::CueFrame {
        frame: frames / 2,
        present_frame: true,
    };
    let replay = client.envelope(cue.clone());
    let ack = client.command(cue)?;
    assert_eq!(position(&ack), at);
    assert!(!ready(&ack));
    client.wait(|state| ready(state) && position(state) == frames / 2)?;
    assert!(
        client
            .writer
            .post::<_, SessionReply>(&SessionRequest::Command(replay))?
            .is_err()
    );
    assert_eq!(position(&second.state()?), 0);
    wait_exit(&mut independent)?;
    println!("Independent session unchanged; inactivity released its process");
    client.command(Command::Play)?;
    client.wait(|state| position(state) >= frames / 2 + 5)?;
    client.command(Command::Stop)?;
    client.command(Command::CueFrame {
        frame: frames - 1,
        present_frame: true,
    })?;
    client.wait(|state| ready(state) && position(state) == frames - 1)?;
    client.command(Command::Play)?;
    client.wait(ended)?;
    client.command(Command::CueFrame {
        frame: 0,
        present_frame: true,
    })?;
    client.wait(|state| ready(state) && position(state) == 0)?;
    client.command(Command::Play)?;
    client.wait(|state| position(state) >= 5)?;
    client.command(Command::Pause)?;
    let held = position(&client.state()?);
    let invalid = client.envelope(Command::CueFrame {
        frame: frames,
        present_frame: true,
    });
    client.command = invalid.sequence;
    let reply = client.writer.post(&SessionRequest::Command(invalid))?;
    let reply = client.checked(reply)?;
    assert!(
        reply
            .events
            .iter()
            .any(|e| matches!(e, Event::CommandRejected { .. }))
    );
    assert_eq!(position(&reply), held);
    // Keep native pixels visible for the visual check while the owner remains alive.
    let hold = Instant::now();
    while hold.elapsed() < Duration::from_secs(15) {
        client.state()?;
        thread::sleep(Duration::from_millis(100));
    }
    client.command(Command::Shutdown)?;
    wait_exit(&mut process)?;
    assert_eq!(hash(&mut media)?, before);
    assert_eq!(reader.read()?, settings);
    println!(
        "PASS: real process/native AV, command permissions/isolation/replay, seek/end/replay, explicit and idle shutdown; source/settings unchanged. No probe or DB writes."
    );
    Ok(())
}
