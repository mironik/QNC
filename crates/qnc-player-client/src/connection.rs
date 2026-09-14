use super::*;
use qnc_player_contract::{
    BroadcastPlayerProtocolCommand as Command, BroadcastPlayerProtocolEvent as Event, VERSION,
    envelope::CommandEnvelope,
    session::{
        MAX_CONTROL_BYTES, SessionQuery, SessionReply, SessionRequest, WIRE_HEADER_BYTES,
        WIRE_KIND_CONTROL, WIRE_MAGIC, WIRE_MAX_TOKEN_BYTES, WIRE_SCHEME,
        WIRE_STATUS_ACCESS_DENIED, WIRE_STATUS_BAD_REQUEST, WIRE_STATUS_BUSY, WIRE_STATUS_OK,
        WIRE_STATUS_PROTOCOL, WIRE_STATUS_TOO_LARGE, WIRE_VERSION,
    },
};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    process::{Child, Command as Process, Stdio},
    time::{Duration, Instant},
};

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct PlayerWireClient {
    address: String,
    stream: Option<TcpStream>,
    token: String,
    kind: u8,
    max_bytes: usize,
    persistent: bool,
}
impl PlayerWireClient {
    fn connect_one_shot(wire_url: &str, kind: u8, token: &str, max_bytes: usize) -> Result<Self> {
        Self::connect(wire_url, kind, token, max_bytes, false)
    }

    fn connect(
        wire_url: &str,
        kind: u8,
        token: &str,
        max_bytes: usize,
        persistent: bool,
    ) -> Result<Self> {
        if token.is_empty()
            || token.len() > WIRE_MAX_TOKEN_BYTES
            || !token.bytes().all(|b| (33..=126).contains(&b))
            || kind != WIRE_KIND_CONTROL
            || !(1..=MAX_CONTROL_BYTES).contains(&max_bytes)
        {
            return Err("Invalid player wire client configuration.".into());
        }
        let address = wire_url
            .strip_prefix(WIRE_SCHEME)
            .ok_or("Unsupported player wire protocol.")?;
        let stream = persistent.then(|| Self::open_stream(address)).transpose()?;
        Ok(Self {
            address: address.into(),
            stream,
            token: token.into(),
            kind,
            max_bytes,
            persistent,
        })
    }

    fn open_stream(address: &str) -> Result<TcpStream> {
        let stream = TcpStream::connect(address).map_err(|e| protocol_io_error("connect", e))?;
        stream.set_nodelay(true).map_err(|e| e.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(|e| e.to_string())?;
        Ok(stream)
    }

    fn post<Q: serde::Serialize, R: serde::de::DeserializeOwned>(
        &mut self,
        request: &Q,
    ) -> Result<R> {
        let bytes = self.post_body(request)?;
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())
    }

    fn post_body<Q: serde::Serialize>(&mut self, request: &Q) -> Result<Vec<u8>> {
        let body = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        if body.len() > MAX_CONTROL_BYTES {
            return Err("Player wire request exceeds limit.".into());
        }
        let mut header = [0; WIRE_HEADER_BYTES];
        header[..8].copy_from_slice(WIRE_MAGIC);
        header[8] = WIRE_VERSION;
        header[9] = self.kind;
        header[10..12].copy_from_slice(&(self.token.len() as u16).to_le_bytes());
        header[12..16].copy_from_slice(&(body.len() as u32).to_le_bytes());
        if self.persistent {
            let stream = self
                .stream
                .as_mut()
                .ok_or("Player socket is not connected.")?;
            return Self::send_request(
                stream,
                self.max_bytes,
                &header,
                self.token.as_bytes(),
                &body,
            );
        }
        let mut stream = Self::open_stream(&self.address)?;
        Self::send_request(
            &mut stream,
            self.max_bytes,
            &header,
            self.token.as_bytes(),
            &body,
        )
    }

    fn send_request(
        stream: &mut TcpStream,
        max_bytes: usize,
        header: &[u8; WIRE_HEADER_BYTES],
        token: &[u8],
        body: &[u8],
    ) -> Result<Vec<u8>> {
        let mut request = Vec::with_capacity(WIRE_HEADER_BYTES + token.len() + body.len());
        request.extend_from_slice(header);
        request.extend_from_slice(token);
        request.extend_from_slice(body);
        stream
            .write_all(&request)
            .map_err(|e| protocol_io_error("write", e))?;
        stream.flush().map_err(|e| protocol_io_error("flush", e))?;
        let mut response = [0; WIRE_HEADER_BYTES];
        stream
            .read_exact(&mut response)
            .map_err(|e| protocol_io_error("read header", e))?;
        if &response[..8] != WIRE_MAGIC || response[8] != WIRE_VERSION {
            return Err("Player wire protocol mismatch.".into());
        }
        let status = response[9];
        let body_len = u32::from_le_bytes(response[12..16].try_into().unwrap()) as usize;
        if body_len > max_bytes {
            return Err("Player wire response exceeds limit.".into());
        }
        let mut body = vec![0; body_len];
        stream
            .read_exact(&mut body)
            .map_err(|e| protocol_io_error("read body", e))?;
        if status == WIRE_STATUS_OK {
            return Ok(body);
        }
        Err(wire_status_error(status, &body))
    }
}

fn protocol_io_error(op: &str, error: std::io::Error) -> String {
    match (error.kind(), error.raw_os_error()) {
        (_, Some(10053 | 10054 | 104)) => {
            format!("Player protocol: control connection closed ({op}).")
        }
        (std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock, _) => {
            format!("Player protocol: control {op} timed out.")
        }
        _ => format!("Player protocol: control {op} failed."),
    }
}

fn wire_status_error(status: u8, body: &[u8]) -> String {
    if let Ok(message) = std::str::from_utf8(body)
        && !message.trim().is_empty()
    {
        return message.to_owned();
    }
    match status {
        WIRE_STATUS_ACCESS_DENIED => "Player socket access denied.".into(),
        WIRE_STATUS_BAD_REQUEST => "Player socket rejected the request.".into(),
        WIRE_STATUS_BUSY => "Player socket is busy.".into(),
        WIRE_STATUS_TOO_LARGE => "Player socket payload exceeds limit.".into(),
        WIRE_STATUS_PROTOCOL => "Player socket protocol error.".into(),
        _ => "Player socket returned an unknown status.".into(),
    }
}

pub(super) struct Connection {
    child: ChildGuard,
    control: PlayerWireClient,
    query: SessionQuery,
    sequence: u64,
    last_event: u64,
    video_visible: bool,
    last_report: Instant,
    cached_reply: Option<EventEnvelope>,
    last_state_poll: Instant,
    pending_cue_frame: Option<u64>,
}
impl Connection {
    pub fn launch(launch: Launch, generation: u64) -> Result<Self> {
        let session = uuid::Uuid::new_v4().to_string();
        let read = uuid::Uuid::new_v4().to_string();
        let write = uuid::Uuid::new_v4().to_string();
        launch
            .input
            .validate_for(
                &launch.input.workspace_db_uri,
                &launch.input.snapshot.metadata.clip_id,
            )
            .map_err(|e| e.to_string())?;
        if !qnc_player_frame_transport::active_gpu_dma_monitor_available() {
            return Err(format!(
                "GPU/DMA preview monitor is required (preferred backend: {}); CPU RGBA frame-map fallback is disabled",
                qnc_player_frame_transport::preferred_dma_backend_for_current_os()
                    .map(qnc_player_frame_transport::FrameTransportBackend::wire_name)
                    .unwrap_or("none")
            ));
        }
        let boot = serde_json::to_vec(&serde_json::json!({
            "contract_version": VERSION, "session_id": session, "source_generation": generation,
            "input": launch.input, "media_binding": launch.media_binding,
            "read_token": read, "command_token": write, "idle_timeout_ms": 300000, "listen_port": 0
        }))
        .map_err(|e| e.to_string())?;
        if boot.len() > 4 * 1024 * 1024 {
            return Err("Player bootstrap exceeds limit.".into());
        }
        let mut command = Process::new(&launch.executable);
        command
            .arg("--monitor-output")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = ChildGuard(
            command
                .spawn()
                .map_err(|e| format!("Broadcast Player: {e}"))?,
        );
        let stderr = child.0.stderr.take().ok_or("missing player error pipe")?;
        let errors = Arc::new(Mutex::new(Vec::new()));
        let log = errors.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut buffer = [0; 1024];
            while let Ok(n) = reader.read(&mut buffer) {
                if n == 0 {
                    break;
                }
                if qnc_dev_diagnostics::player_diagnostics_enabled() {
                    qnc_dev_diagnostics::log_line(
                        qnc_dev_diagnostics::DiagnosticsStream::Player,
                        String::from_utf8_lossy(&buffer[..n]),
                    );
                }
                let mut text = log.lock().unwrap();
                let keep = n.min(8192usize.saturating_sub(text.len()));
                text.extend_from_slice(&buffer[..keep]);
            }
        });
        let stdout = child.0.stdout.take().ok_or("missing player startup pipe")?;
        let stdin = child.0.stdin.take().ok_or("missing player input pipe")?;
        let (send, receive) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let mut input = stdin;
            let result = (|| -> std::io::Result<Vec<u8>> {
                input.write_all(&boot)?;
                drop(input);
                let mut line = Vec::new();
                BufReader::new(stdout.take(8193)).read_until(b'\n', &mut line)?;
                Ok(line)
            })();
            let _ = send.send(result);
        });
        let line = receive
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| "Player startup timed out.")?
            .map_err(|e| e.to_string())?;
        if line.is_empty() || line.len() > 8192 {
            return Err(format!(
                "Player startup failed: {}",
                String::from_utf8_lossy(&errors.lock().unwrap())
            ));
        }
        let hello: serde_json::Value = serde_json::from_slice(&line).map_err(|e| e.to_string())?;
        if hello["contract_version"] != VERSION
            || hello["session_id"] != session
            || hello["source_generation"] != generation
        {
            return Err("Player startup identity mismatch.".into());
        }
        let monitor_transport = hello["monitor_transport"]
            .as_str()
            .ok_or("missing player monitor transport")?;
        if monitor_transport != "qnc-player-frame-transport" {
            return Err("Player monitor transport is not the public frame transport.".into());
        }
        let monitor_backend = hello["monitor_backend"]
            .as_str()
            .ok_or("missing player monitor backend")?;
        let expected_dma = qnc_player_frame_transport::preferred_dma_backend_for_current_os()
            .map(qnc_player_frame_transport::FrameTransportBackend::wire_name)
            .ok_or("This OS has no GPU/DMA preview backend contract.")?;
        if monitor_backend != expected_dma {
            return Err("Player preview reported a different GPU/DMA backend.".into());
        }
        if qnc_dev_diagnostics::player_diagnostics_enabled() {
            qnc_dev_diagnostics::log_line(
                qnc_dev_diagnostics::DiagnosticsStream::Player,
                format!(
                    "player-preview transport={monitor_transport} backend={monitor_backend} preferred_dma={} acceptance=gpu_dma_required",
                    hello["preferred_dma_backend"].as_str().unwrap_or("none")
                ),
            );
        }
        let wire_url = hello["wire_url"]
            .as_str()
            .ok_or("missing player wire endpoint")?;
        let control = PlayerWireClient::connect_one_shot(
            wire_url,
            WIRE_KIND_CONTROL,
            &write,
            MAX_CONTROL_BYTES,
        )?;
        let query = SessionQuery {
            contract_version: VERSION.into(),
            session_id: session,
            source_generation: generation,
        };
        Ok(Self {
            child,
            control,
            query,
            sequence: 0,
            last_event: 0,
            video_visible: false,
            last_report: Instant::now(),
            cached_reply: None,
            last_state_poll: Instant::now(),
            pending_cue_frame: None,
        })
    }
    fn request(&mut self, request: SessionRequest) -> Result<EventEnvelope> {
        let reply: SessionReply = self.control.post(&request).map_err(|e| e.to_string())?;
        let reply = reply?;
        reply.validate_for(
            &self.query.session_id,
            self.query.source_generation,
            self.last_event,
        )?;
        self.last_event = reply.sequence;
        Ok(reply)
    }
    fn command(&mut self, command: Command) -> Result<EventEnvelope> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or("player command sequence exhausted")?;
        self.request(SessionRequest::Command(CommandEnvelope {
            contract_version: VERSION.into(),
            session_id: self.query.session_id.clone(),
            source_generation: self.query.source_generation,
            sequence: self.sequence,
            request_id: self.sequence.to_string(),
            command,
        }))
    }
    pub fn poll(&mut self, action: Option<Action>) -> Result<View> {
        if self
            .child
            .0
            .try_wait()
            .map_err(|e| e.to_string())?
            .is_some()
        {
            return Err("Player protocol: process closed.".into());
        }
        let control_start = Instant::now();
        let mut reply = if self.cached_reply.is_none()
            || action.is_some_and(action_requires_preflight_state)
            || self.last_state_poll.elapsed()
                >= state_poll_interval(self.pending_cue_frame, self.cached_reply.as_ref())
        {
            let reply = self.request(SessionRequest::State(self.query.clone()))?;
            self.last_state_poll = Instant::now();
            reply
        } else {
            self.cached_reply.clone().expect("observed player state")
        };
        if let Some(action) = action {
            let command = action_command(&reply, action)?;
            let measuring_play = matches!(command, Command::Play);
            if let Command::CueFrame { frame, .. } = command {
                self.pending_cue_frame = Some(frame);
            }
            reply = self.command(command)?;
            let _ = measuring_play;
            self.last_state_poll = Instant::now();
        }
        update_pending_cue(&mut self.pending_cue_frame, &reply);
        self.cached_reply = Some(reply.clone());
        let control_us = control_start.elapsed().as_micros();
        let error = reply.events.iter().find_map(|e| match e {
            Event::PlaybackError { message }
            | Event::CommandRejected {
                reason: message, ..
            } => Some(message.clone()),
            _ => None,
        });
        self.video_visible = update_video_visible(self.video_visible, &reply.events);
        if qnc_dev_diagnostics::player_diagnostics_enabled()
            && (action.is_some() || self.last_report.elapsed() >= Duration::from_secs(2))
        {
            qnc_dev_diagnostics::log_line(
                qnc_dev_diagnostics::DiagnosticsStream::Player,
                format!(
                    "player-client action={action:?} visible={} control_us={} events={:?}",
                    self.video_visible, control_us, reply.events
                ),
            );
            self.last_report = Instant::now();
        }
        Ok(View {
            preparing: false,
            video_visible: self.video_visible,
            reply: Some(reply),
            error,
        })
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        let _ = self.command(Command::Shutdown);
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until {
            if self.child.0.try_wait().ok().flatten().is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

fn update_video_visible(current: bool, events: &[Event]) -> bool {
    let source_cleared = events
        .iter()
        .any(|event| matches!(event, Event::ActiveSourceChanged { source_id: None }));
    if source_cleared {
        return false;
    }
    current
}

fn action_command(reply: &EventEnvelope, action: Action) -> Result<Command> {
    let carrier = reply
        .events
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::CarrierPositionChanged {
                frame,
                range: Some(range),
                timebase: Some(_),
                status,
                ..
            } => Some((*frame, *range, *status)),
            _ => None,
        })
        .ok_or("Player has no confirmed position.")?;
    Ok(match action {
        Action::TogglePlayPause if carrier.2 == TransportStatus::Playing => Command::Pause,
        Action::TogglePlayPause => Command::Play,
        Action::Cue(frame) => Command::CueFrame {
            frame,
            present_frame: true,
        },
        Action::Step(delta) => Command::CueFrame {
            frame: carrier
                .0
                .saturating_add_signed(delta)
                .clamp(carrier.1.start_frame, carrier.1.end_frame.saturating_sub(1)),
            present_frame: true,
        },
    })
}

fn action_requires_preflight_state(action: Action) -> bool {
    !matches!(action, Action::Cue(_))
}

fn state_poll_interval(pending_cue_frame: Option<u64>, reply: Option<&EventEnvelope>) -> Duration {
    if pending_cue_frame.is_some() {
        return Duration::from_millis(1);
    }
    let Some(frame_interval) = reply_source_frame_interval(reply) else {
        return Duration::from_millis(50);
    };
    if reply_transport_status(reply) == Some(TransportStatus::Playing) {
        return frame_interval.saturating_mul(4);
    }
    frame_interval
}

fn reply_source_frame_interval(reply: Option<&EventEnvelope>) -> Option<Duration> {
    let timebase = reply?.events.iter().rev().find_map(|event| match event {
        Event::CarrierPositionChanged {
            timebase: Some(timebase),
            ..
        } => Some(*timebase),
        _ => None,
    })?;
    frame_interval(timebase)
}

fn reply_transport_status(reply: Option<&EventEnvelope>) -> Option<TransportStatus> {
    reply?.events.iter().rev().find_map(|event| match event {
        Event::TransportStatusChanged { status } => Some(*status),
        Event::CarrierPositionChanged { status, .. } => Some(*status),
        _ => None,
    })
}

fn update_pending_cue(pending: &mut Option<u64>, reply: &EventEnvelope) {
    let Some(target) = *pending else {
        return;
    };
    if reply.events.iter().rev().any(|event| {
        matches!(
            event,
            Event::CarrierPositionChanged { frame, .. } if *frame == target
        )
    }) {
        *pending = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monitor_visibility_requires_gpu_dma_frame_descriptor() {
        assert!(!update_video_visible(false, &[]));
        assert!(!update_video_visible(
            false,
            &[Event::TransportStatusChanged {
                status: TransportStatus::Ready
            }]
        ));
        assert!(!update_video_visible(
            false,
            &[Event::TransportStatusChanged {
                status: TransportStatus::Playing
            }]
        ));
        assert!(update_video_visible(
            true,
            &[Event::TransportStatusChanged {
                status: TransportStatus::Playing
            }]
        ));
    }

    #[test]
    fn existing_actions_use_only_confirmed_player_position_and_status() {
        let mut reply = EventEnvelope {
            contract_version: VERSION.into(),
            session_id: "s".into(),
            source_generation: 1,
            sequence: 1,
            events: vec![Event::CarrierPositionChanged {
                source_id: Some("clip".into()),
                frame: 5,
                range: Some(qnc_player_contract::FrameRange::new(0, 10).unwrap()),
                timebase: Some(qnc_player_contract::Timebase::new(50, 1).unwrap()),
                status: TransportStatus::Playing,
            }],
        };
        assert_eq!(
            action_command(&reply, Action::TogglePlayPause).unwrap(),
            Command::Pause
        );
        assert_eq!(
            action_command(&reply, Action::Step(1)).unwrap(),
            Command::CueFrame {
                frame: 6,
                present_frame: true
            }
        );
        assert_eq!(
            action_command(&reply, Action::Step(-99)).unwrap(),
            Command::CueFrame {
                frame: 0,
                present_frame: true
            }
        );
        assert_eq!(
            action_command(&reply, Action::Step(99)).unwrap(),
            Command::CueFrame {
                frame: 9,
                present_frame: true
            }
        );
        reply.events.clear();
        assert!(action_command(&reply, Action::TogglePlayPause).is_err());
        reply.events.push(Event::CarrierPositionChanged {
            source_id: Some("clip".into()),
            frame: 5,
            range: Some(qnc_player_contract::FrameRange::new(0, 10).unwrap()),
            timebase: None,
            status: TransportStatus::Paused,
        });
        assert!(action_command(&reply, Action::Step(1)).is_err());
    }

    #[test]
    fn cue_command_uses_cached_confirmed_state_without_preflight_state() {
        assert!(!action_requires_preflight_state(Action::Cue(25)));
        assert!(action_requires_preflight_state(Action::Step(1)));
        assert!(action_requires_preflight_state(Action::TogglePlayPause));
    }

    #[test]
    fn state_poll_interval_comes_from_source_timebase_except_pending_cue() {
        let reply = EventEnvelope {
            contract_version: VERSION.into(),
            session_id: "s".into(),
            source_generation: 1,
            sequence: 1,
            events: vec![Event::CarrierPositionChanged {
                source_id: Some("clip".into()),
                frame: 5,
                range: Some(qnc_player_contract::FrameRange::new(0, 100).unwrap()),
                timebase: Some(qnc_player_contract::Timebase::new(50, 1).unwrap()),
                status: TransportStatus::Playing,
            }],
        };

        assert_eq!(
            state_poll_interval(None, Some(&reply)),
            Duration::from_millis(80)
        );
        assert_eq!(
            state_poll_interval(Some(25), Some(&reply)),
            Duration::from_millis(1)
        );
        let paused = EventEnvelope {
            events: vec![Event::CarrierPositionChanged {
                source_id: Some("clip".into()),
                frame: 5,
                range: Some(qnc_player_contract::FrameRange::new(0, 100).unwrap()),
                timebase: Some(qnc_player_contract::Timebase::new(50, 1).unwrap()),
                status: TransportStatus::Paused,
            }],
            ..reply
        };
        assert_eq!(
            state_poll_interval(None, Some(&paused)),
            Duration::from_millis(20)
        );
    }

    #[test]
    fn pending_cue_clears_only_after_confirmed_player_position() {
        let mut pending = Some(25);
        let old = EventEnvelope {
            contract_version: VERSION.into(),
            session_id: "s".into(),
            source_generation: 1,
            sequence: 1,
            events: vec![Event::CarrierPositionChanged {
                source_id: Some("clip".into()),
                frame: 5,
                range: Some(qnc_player_contract::FrameRange::new(0, 100).unwrap()),
                timebase: Some(qnc_player_contract::Timebase::new(50, 1).unwrap()),
                status: TransportStatus::Preparing,
            }],
        };
        update_pending_cue(&mut pending, &old);
        assert_eq!(pending, Some(25));

        let confirmed = EventEnvelope {
            events: vec![Event::CarrierPositionChanged {
                source_id: Some("clip".into()),
                frame: 25,
                range: Some(qnc_player_contract::FrameRange::new(0, 100).unwrap()),
                timebase: Some(qnc_player_contract::Timebase::new(50, 1).unwrap()),
                status: TransportStatus::Paused,
            }],
            ..old
        };
        update_pending_cue(&mut pending, &confirmed);
        assert_eq!(pending, None);
    }

    #[test]
    fn control_client_uses_one_socket_per_request() {
        let client = PlayerWireClient::connect_one_shot(
            "qnc-player+tcp://127.0.0.1:9",
            WIRE_KIND_CONTROL,
            "token",
            MAX_CONTROL_BYTES,
        )
        .unwrap();

        assert!(!client.persistent);
        assert!(client.stream.is_none());
    }

    #[test]
    fn socket_abort_is_protocol_error_not_os_text() {
        let error = std::io::Error::from_raw_os_error(10053);
        assert_eq!(
            protocol_io_error("read header", error),
            "Player protocol: control connection closed (read header)."
        );
    }
}
