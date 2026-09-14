use qnc_json_transport::{Access, Credentials};
use qnc_player_contract::session::{
    MAX_CONTROL_BYTES, SessionReply, SessionRequest, WIRE_HEADER_BYTES, WIRE_KIND_CONTROL,
    WIRE_MAGIC, WIRE_MAX_TOKEN_BYTES, WIRE_SCHEME, WIRE_STATUS_ACCESS_DENIED,
    WIRE_STATUS_BAD_REQUEST, WIRE_STATUS_OK, WIRE_STATUS_PROTOCOL, WIRE_STATUS_TOO_LARGE,
    WIRE_VERSION,
};
use std::{
    io::{ErrorKind, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub struct Pending {
    pub request: SessionRequest,
    pub response: SyncSender<SessionReply>,
    pub expires: Instant,
}
pub struct Control {
    pub requests: Receiver<Pending>,
    pub address: String,
    stop: Arc<AtomicBool>,
    io: Option<JoinHandle<()>>,
}
impl Control {
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
        let credentials = Arc::new(credentials);
        let accept_credentials = credentials.clone();
        let io = thread::spawn(move || {
            while !stopping.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = stream.set_nodelay(true);
                        let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
                        let sender = sender.clone();
                        let credentials = accept_credentials.clone();
                        let stop = stopping.clone();
                        thread::spawn(move || handle_socket(stream, sender, credentials, stop));
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
        || kind != WIRE_KIND_CONTROL
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
    fn frame_socket_is_disabled_until_gpu_dma_descriptor_backend_exists() {
        let control =
            Control::open(0, Credentials::new("read-test", "write-test").unwrap()).unwrap();
        let body = serde_json::to_vec(&SessionRequest::State(SessionQuery {
            contract_version: VERSION.into(),
            session_id: "s".into(),
            source_generation: 1,
        }))
        .unwrap();
        let mut stream =
            TcpStream::connect(control.address.strip_prefix(WIRE_SCHEME).unwrap()).unwrap();
        write_request(&mut stream, 2, "read-test", &body).unwrap();
        assert_eq!(
            read_response(&mut stream, MAX_CONTROL_BYTES),
            Err(WIRE_STATUS_BAD_REQUEST)
        );
        assert!(control.requests.try_recv().is_err());
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
