use qnc_json_transport::{Access, Credentials};
use qnc_player_contract::{
    Timebase,
    session::{
        MAX_CONTROL_BYTES, MAX_FRAME_BYTES, MonitorHeader, MonitorQuery, SessionReply,
        SessionRequest, WIRE_HEADER_BYTES, WIRE_KIND_CONTROL, WIRE_KIND_FRAME, WIRE_MAGIC,
        WIRE_MAX_TOKEN_BYTES, WIRE_SCHEME, WIRE_STATUS_ACCESS_DENIED, WIRE_STATUS_BAD_REQUEST,
        WIRE_STATUS_OK, WIRE_STATUS_PROTOCOL, WIRE_STATUS_TOO_LARGE, WIRE_VERSION,
    },
};
use std::{
    collections::VecDeque,
    io::{ErrorKind, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const MAX_MONITOR_QUEUE: usize = 8;

pub struct Pending {
    pub request: SessionRequest,
    pub response: SyncSender<SessionReply>,
    pub expires: Instant,
}
#[derive(Default)]
struct FrameState {
    current: Option<(MonitorHeader, Arc<[u8]>)>,
    pending: VecDeque<(MonitorHeader, Arc<[u8]>)>,
}
type FrameSlot = Mutex<FrameState>;
pub struct Control {
    pub requests: Receiver<Pending>,
    pub address: String,
    stop: Arc<AtomicBool>,
    io: Option<JoinHandle<()>>,
    frame: Arc<FrameSlot>,
}
impl Control {
    pub fn publish_frames(
        &self,
        session: &str,
        generation: u64,
        source_timebase: Option<Timebase>,
        clear: bool,
        pictures: Vec<(qnc_video_output::FrameHeader, Arc<[u8]>)>,
    ) {
        let mut frames = self.frame.lock().unwrap();
        if clear {
            frames.current = None;
            frames.pending.clear();
        }
        for (h, bytes) in pictures {
            let Some(timebase) = source_timebase else {
                break;
            };
            let frame = (
                MonitorHeader {
                    contract_version: qnc_player_contract::VERSION.into(),
                    session_id: session.into(),
                    source_generation: generation,
                    output_generation: h.generation,
                    sequence: h.sequence,
                    source_id: h.source_id,
                    frame: h.frame_number,
                    timebase,
                    width: h.width,
                    height: h.height,
                },
                bytes,
            );
            frames.pending.push_back(frame);
            while frames.pending.len() > MAX_MONITOR_QUEUE {
                frames.pending.pop_front();
            }
        }
    }

    #[cfg(test)]
    pub fn set_frame(
        &self,
        session: &str,
        generation: u64,
        picture: Option<(qnc_video_output::FrameHeader, Arc<[u8]>)>,
    ) {
        match picture {
            Some(picture) => self.publish_frames(
                session,
                generation,
                Some(Timebase::new(50, 1).unwrap()),
                true,
                vec![picture],
            ),
            None => self.publish_frames(session, generation, None, true, Vec::new()),
        }
    }
    pub fn open(port: u16, credentials: Credentials) -> crate::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let address = format!(
            "{WIRE_SCHEME}{}",
            listener.local_addr().map_err(|e| e.to_string())?
        );
        let (sender, requests) = mpsc::sync_channel(8);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let frame = Arc::new(Mutex::new(FrameState::default()));
        let credentials = Arc::new(credentials);
        let accept_frame = frame.clone();
        let accept_credentials = credentials.clone();
        let io = thread::spawn(move || {
            while !stopping.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = stream.set_nodelay(true);
                        let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
                        let sender = sender.clone();
                        let credentials = accept_credentials.clone();
                        let frame = accept_frame.clone();
                        let stop = stopping.clone();
                        thread::spawn(move || {
                            handle_socket(stream, sender, credentials, frame, stop)
                        });
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            requests,
            address,
            stop,
            io: Some(io),
            frame,
        })
    }
}

struct WireRequest {
    kind: u8,
    token: String,
    body: Vec<u8>,
}

fn handle_socket(
    mut stream: TcpStream,
    sender: SyncSender<Pending>,
    credentials: Arc<Credentials>,
    frame: Arc<FrameSlot>,
    stop: Arc<AtomicBool>,
) {
    while !stop.load(Ordering::Acquire) {
        let request = match read_wire_request(&mut stream) {
            Ok(request) => request,
            Err(status) => {
                if status != WIRE_STATUS_OK {
                    let _ = write_wire_response(&mut stream, status, &[]);
                }
                break;
            }
        };
        let access = credentials.access(&format!("Bearer {}", request.token));
        let Some(access) = access else {
            let _ = write_wire_response(&mut stream, WIRE_STATUS_ACCESS_DENIED, &[]);
            continue;
        };
        let response = match request.kind {
            WIRE_KIND_CONTROL => respond_control(request.body, access, &sender),
            WIRE_KIND_FRAME => respond_frame_packet(request.body, &frame),
            _ => (WIRE_STATUS_BAD_REQUEST, Vec::new()),
        };
        if write_wire_response(&mut stream, response.0, &response.1).is_err() {
            break;
        }
    }
}

fn respond_control(body: Vec<u8>, access: Access, sender: &SyncSender<Pending>) -> (u8, Vec<u8>) {
    let Ok(request) = serde_json::from_slice::<SessionRequest>(&body) else {
        return (WIRE_STATUS_BAD_REQUEST, Vec::new());
    };
    let reply: SessionReply =
        if access != Access::ReadWrite && matches!(request, SessionRequest::Command(_)) {
            Err("command access denied".into())
        } else {
            let (response, receiver) = mpsc::sync_channel(1);
            let pending = Pending {
                request,
                response,
                expires: Instant::now() + Duration::from_secs(2),
            };
            match sender.try_send(pending) {
                Ok(()) => receiver
                    .recv_timeout(Duration::from_secs(2))
                    .unwrap_or_else(|_| {
                        Err("player response unavailable; do not replay command".into())
                    }),
                Err(_) => Err("player busy or closed".into()),
            }
        };
    match serde_json::to_vec(&reply) {
        Ok(bytes) if bytes.len() <= MAX_CONTROL_BYTES => (WIRE_STATUS_OK, bytes),
        Ok(_) => (WIRE_STATUS_TOO_LARGE, Vec::new()),
        Err(_) => (WIRE_STATUS_PROTOCOL, Vec::new()),
    }
}

fn respond_frame_packet(body: Vec<u8>, frame: &FrameSlot) -> (u8, Vec<u8>) {
    if body.len() > 8192 {
        return (WIRE_STATUS_TOO_LARGE, Vec::new());
    }
    let Ok(query) = serde_json::from_slice::<MonitorQuery>(&body) else {
        return (WIRE_STATUS_BAD_REQUEST, Vec::new());
    };
    let data = next_frame(frame, query.after);
    let Some((header, rgba)) = data else {
        return (WIRE_STATUS_OK, Vec::new());
    };
    if header
        .validate(&query.session, &header.source_id, rgba.len())
        .is_err()
    {
        return (WIRE_STATUS_PROTOCOL, Vec::new());
    }
    if query.after == Some((header.output_generation, header.sequence)) {
        return (WIRE_STATUS_OK, vec![0; 4]);
    }
    let Ok(json) = serde_json::to_vec(&header) else {
        return (WIRE_STATUS_PROTOCOL, Vec::new());
    };
    if json.len() > 8192 || json.len() + 4 + rgba.len() > MAX_FRAME_BYTES {
        return (WIRE_STATUS_TOO_LARGE, Vec::new());
    }
    let mut bytes = (json.len() as u32).to_le_bytes().to_vec();
    bytes.extend(json);
    bytes.extend_from_slice(&rgba);
    (WIRE_STATUS_OK, bytes)
}

fn read_wire_request(stream: &mut TcpStream) -> std::result::Result<WireRequest, u8> {
    let mut header = [0; WIRE_HEADER_BYTES];
    if let Err(error) = stream.read_exact(&mut header) {
        return Err(match error.kind() {
            ErrorKind::UnexpectedEof
            | ErrorKind::ConnectionAborted
            | ErrorKind::ConnectionReset
            | ErrorKind::TimedOut => WIRE_STATUS_OK,
            _ => WIRE_STATUS_PROTOCOL,
        });
    }
    if &header[..8] != WIRE_MAGIC || header[8] != WIRE_VERSION {
        return Err(WIRE_STATUS_PROTOCOL);
    }
    let kind = header[9];
    let token_len = u16::from_le_bytes(header[10..12].try_into().unwrap()) as usize;
    let body_len = u32::from_le_bytes(header[12..16].try_into().unwrap()) as usize;
    if token_len == 0
        || token_len > WIRE_MAX_TOKEN_BYTES
        || body_len > MAX_CONTROL_BYTES
        || !matches!(kind, WIRE_KIND_CONTROL | WIRE_KIND_FRAME)
    {
        return Err(WIRE_STATUS_BAD_REQUEST);
    }
    let mut token = vec![0; token_len];
    let mut body = vec![0; body_len];
    stream
        .read_exact(&mut token)
        .map_err(|_| WIRE_STATUS_PROTOCOL)?;
    stream
        .read_exact(&mut body)
        .map_err(|_| WIRE_STATUS_PROTOCOL)?;
    let token = String::from_utf8(token).map_err(|_| WIRE_STATUS_ACCESS_DENIED)?;
    Ok(WireRequest { kind, token, body })
}

fn write_wire_response(stream: &mut TcpStream, status: u8, body: &[u8]) -> std::io::Result<()> {
    let mut header = [0; WIRE_HEADER_BYTES];
    header[..8].copy_from_slice(WIRE_MAGIC);
    header[8] = WIRE_VERSION;
    header[9] = status;
    header[12..16].copy_from_slice(&(body.len() as u32).to_le_bytes());
    let mut response = Vec::with_capacity(WIRE_HEADER_BYTES + body.len());
    response.extend_from_slice(&header);
    response.extend_from_slice(body);
    stream.write_all(&response)?;
    stream.flush()
}

fn next_frame(frame: &FrameSlot, after: Option<(u64, u64)>) -> Option<(MonitorHeader, Arc<[u8]>)> {
    let mut frames = frame.lock().unwrap();
    if let Some(cursor) = after {
        while frames
            .pending
            .front()
            .is_some_and(|(header, _)| frame_key(header) <= cursor)
        {
            frames.pending.pop_front();
        }
    }
    if let Some(next) = frames.pending.pop_front() {
        frames.current = Some(next.clone());
        return Some(next);
    }
    frames.current.clone()
}

fn frame_key(header: &MonitorHeader) -> (u64, u64) {
    (header.output_generation, header.sequence)
}
impl Drop for Control {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(io) = self.io.take() {
            let until = Instant::now() + Duration::from_millis(250);
            while !io.is_finished() && Instant::now() < until {
                thread::sleep(Duration::from_millis(1));
            }
            // A slow socket client must never hold process shutdown or media resources.
            if io.is_finished() {
                let _ = io.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_player_contract::{
        BroadcastPlayerProtocolCommand, VERSION,
        envelope::{CommandEnvelope, EventEnvelope},
        session::SessionQuery,
    };

    fn post<Q: serde::Serialize>(
        control: &Control,
        kind: u8,
        token: &str,
        request: &Q,
        limit: usize,
    ) -> std::result::Result<Vec<u8>, u8> {
        let body = serde_json::to_vec(request).unwrap();
        let mut stream =
            TcpStream::connect(control.address.strip_prefix(WIRE_SCHEME).unwrap()).unwrap();
        stream.set_nodelay(true).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        write_request(&mut stream, kind, token, &body).unwrap();
        read_response(&mut stream, limit)
    }

    fn write_request(
        stream: &mut TcpStream,
        kind: u8,
        token: &str,
        body: &[u8],
    ) -> std::io::Result<()> {
        let mut header = [0; WIRE_HEADER_BYTES];
        header[..8].copy_from_slice(WIRE_MAGIC);
        header[8] = WIRE_VERSION;
        header[9] = kind;
        header[10..12].copy_from_slice(&(token.len() as u16).to_le_bytes());
        header[12..16].copy_from_slice(&(body.len() as u32).to_le_bytes());
        let mut request = Vec::with_capacity(WIRE_HEADER_BYTES + token.len() + body.len());
        request.extend_from_slice(&header);
        request.extend_from_slice(token.as_bytes());
        request.extend_from_slice(body);
        stream.write_all(&request)?;
        stream.flush()
    }

    fn read_response(stream: &mut TcpStream, limit: usize) -> std::result::Result<Vec<u8>, u8> {
        let mut header = [0; WIRE_HEADER_BYTES];
        stream
            .read_exact(&mut header)
            .map_err(|_| WIRE_STATUS_PROTOCOL)?;
        if &header[..8] != WIRE_MAGIC || header[8] != WIRE_VERSION {
            return Err(WIRE_STATUS_PROTOCOL);
        }
        let status = header[9];
        let body_len = u32::from_le_bytes(header[12..16].try_into().unwrap()) as usize;
        if body_len > limit {
            return Err(WIRE_STATUS_TOO_LARGE);
        }
        let mut body = vec![0; body_len];
        stream
            .read_exact(&mut body)
            .map_err(|_| WIRE_STATUS_PROTOCOL)?;
        if status == WIRE_STATUS_OK {
            Ok(body)
        } else {
            Err(status)
        }
    }

    fn frame(
        sequence: u64,
        frame_number: u64,
        value: u8,
    ) -> (qnc_video_output::FrameHeader, Arc<[u8]>) {
        (
            qnc_video_output::FrameHeader {
                version: qnc_video_output::VERSION.into(),
                session_id: "s".into(),
                generation: 2,
                sequence,
                source_id: "clip".into(),
                frame_number,
                width: 2,
                height: 1,
                pixel_format: qnc_video_output::PixelFormat::Rgba8Srgb,
            },
            vec![value; 8].into(),
        )
    }

    #[test]
    fn read_token_cannot_enqueue_commands_and_bad_token_cannot_read() {
        let control =
            Control::open(0, Credentials::new("read-test", "write-test").unwrap()).unwrap();
        let command = SessionRequest::Command(CommandEnvelope {
            contract_version: VERSION.into(),
            session_id: "s".into(),
            request_id: "r".into(),
            source_generation: 1,
            sequence: 1,
            command: BroadcastPlayerProtocolCommand::Play,
        });
        let reply: SessionReply = serde_json::from_slice(
            &post(
                &control,
                WIRE_KIND_CONTROL,
                "read-test",
                &command,
                MAX_CONTROL_BYTES,
            )
            .unwrap(),
        )
        .unwrap();
        assert!(reply.is_err());
        assert!(control.requests.try_recv().is_err());
        let query = SessionRequest::State(SessionQuery {
            contract_version: VERSION.into(),
            session_id: "s".into(),
            source_generation: 1,
        });
        assert_eq!(
            post(
                &control,
                WIRE_KIND_CONTROL,
                "bad-test",
                &query,
                MAX_CONTROL_BYTES,
            ),
            Err(WIRE_STATUS_ACCESS_DENIED)
        );
        assert!(control.requests.try_recv().is_err());
    }

    #[test]
    fn monitor_is_authenticated_bounded_and_does_not_enqueue_player_work() {
        let control =
            Control::open(0, Credentials::new("read-test", "write-test").unwrap()).unwrap();
        control.set_frame(
            "s",
            1,
            Some((
                qnc_video_output::FrameHeader {
                    version: qnc_video_output::VERSION.into(),
                    session_id: "s".into(),
                    generation: 2,
                    sequence: 7,
                    source_id: "clip".into(),
                    frame_number: 3,
                    width: 2,
                    height: 1,
                    pixel_format: qnc_video_output::PixelFormat::Rgba8Srgb,
                },
                vec![255; 8].into(),
            )),
        );
        let mut query = MonitorQuery {
            session: SessionQuery {
                contract_version: VERSION.into(),
                session_id: "s".into(),
                source_generation: 1,
            },
            after: None,
        };
        assert_eq!(
            post(
                &control,
                WIRE_KIND_FRAME,
                "bad-token",
                &query,
                MAX_FRAME_BYTES,
            ),
            Err(WIRE_STATUS_ACCESS_DENIED)
        );
        let bytes = post(
            &control,
            WIRE_KIND_FRAME,
            "read-test",
            &query,
            MAX_FRAME_BYTES,
        )
        .unwrap();
        let length = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        let header: MonitorHeader = serde_json::from_slice(&bytes[4..4 + length]).unwrap();
        assert_eq!(header.frame, 3);
        assert_eq!(&bytes[4 + length..], &[255; 8]);
        query.after = Some((2, 7));
        assert_eq!(
            post(
                &control,
                WIRE_KIND_FRAME,
                "read-test",
                &query,
                MAX_FRAME_BYTES,
            )
            .unwrap(),
            [0; 4]
        );
        query.session.session_id = "wrong".into();
        assert!(
            post(
                &control,
                WIRE_KIND_FRAME,
                "read-test",
                &query,
                MAX_FRAME_BYTES,
            )
            .is_err()
        );
        assert!(control.requests.try_recv().is_err());
    }

    #[test]
    fn monitor_frame_uses_exact_socket_payload_without_http_headers() {
        let control =
            Control::open(0, Credentials::new("read-test", "write-test").unwrap()).unwrap();
        let pixels = vec![123; 960 * 540 * 4];
        control.set_frame(
            "s",
            1,
            Some((
                qnc_video_output::FrameHeader {
                    version: qnc_video_output::VERSION.into(),
                    session_id: "s".into(),
                    generation: 2,
                    sequence: 7,
                    source_id: "clip".into(),
                    frame_number: 3,
                    width: 960,
                    height: 540,
                    pixel_format: qnc_video_output::PixelFormat::Rgba8Srgb,
                },
                pixels.clone().into(),
            )),
        );
        let query = MonitorQuery {
            session: SessionQuery {
                contract_version: VERSION.into(),
                session_id: "s".into(),
                source_generation: 1,
            },
            after: None,
        };
        let body = post(
            &control,
            WIRE_KIND_FRAME,
            "read-test",
            &query,
            MAX_FRAME_BYTES,
        )
        .unwrap();
        let prefix = u32::from_le_bytes(body[..4].try_into().unwrap()) as usize;
        let header: MonitorHeader = serde_json::from_slice(&body[4..4 + prefix]).unwrap();
        assert_eq!(header.frame, 3);
        assert_eq!(&body[4 + prefix..], pixels.as_slice());
        assert_eq!(body.len(), 4 + prefix + pixels.len());
        assert!(control.requests.try_recv().is_err());
    }

    #[test]
    fn monitor_returns_next_queued_frame_instead_of_only_latest() {
        let control =
            Control::open(0, Credentials::new("read-test", "write-test").unwrap()).unwrap();
        control.publish_frames(
            "s",
            1,
            Some(Timebase::new(50, 1).unwrap()),
            true,
            vec![frame(7, 3, 7), frame(8, 4, 8)],
        );
        let query = |after| MonitorQuery {
            session: SessionQuery {
                contract_version: VERSION.into(),
                session_id: "s".into(),
                source_generation: 1,
            },
            after,
        };
        let first = post(
            &control,
            WIRE_KIND_FRAME,
            "read-test",
            &query(None),
            MAX_FRAME_BYTES,
        )
        .unwrap();
        let length = u32::from_le_bytes(first[..4].try_into().unwrap()) as usize;
        let header: MonitorHeader = serde_json::from_slice(&first[4..4 + length]).unwrap();
        assert_eq!((header.sequence, header.frame), (7, 3));
        let next = post(
            &control,
            WIRE_KIND_FRAME,
            "read-test",
            &query(Some((2, 7))),
            MAX_FRAME_BYTES,
        )
        .unwrap();
        let length = u32::from_le_bytes(next[..4].try_into().unwrap()) as usize;
        let header: MonitorHeader = serde_json::from_slice(&next[4..4 + length]).unwrap();
        assert_eq!((header.sequence, header.frame), (8, 4));
    }

    #[test]
    fn authorized_query_crosses_only_the_bounded_handoff() {
        let control =
            Control::open(0, Credentials::new("read-test", "write-test").unwrap()).unwrap();
        let query = SessionRequest::State(SessionQuery {
            contract_version: VERSION.into(),
            session_id: "s".into(),
            source_generation: 1,
        });
        let address = control.address.clone();
        let caller = thread::spawn(move || {
            let body = serde_json::to_vec(&query).unwrap();
            let mut stream =
                TcpStream::connect(address.strip_prefix(WIRE_SCHEME).unwrap()).unwrap();
            write_request(&mut stream, WIRE_KIND_CONTROL, "read-test", &body).unwrap();
            read_response(&mut stream, MAX_CONTROL_BYTES)
                .map(|bytes| serde_json::from_slice::<SessionReply>(&bytes).unwrap())
        });
        let pending = control
            .requests
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        assert!(pending.expires > Instant::now());
        assert!(matches!(pending.request, SessionRequest::State(_)));
        pending
            .response
            .send(Ok(EventEnvelope {
                contract_version: VERSION.into(),
                session_id: "s".into(),
                source_generation: 1,
                sequence: 1,
                events: Vec::new(),
            }))
            .unwrap();
        assert!(caller.join().unwrap().unwrap().is_ok());
    }
}
