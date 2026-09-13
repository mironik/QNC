use crate::MediaStream;
use std::{
    ffi::OsStr,
    io::{self, Read, Seek, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

/// Session-private codec endpoint. It is a process-local transport address, not a
/// persisted media identity.
#[derive(Clone)]
pub struct CodecEndpoint {
    uri: String,
    address: SocketAddr,
    url: String,
    local_file: Option<PathBuf>,
}

impl std::fmt::Debug for CodecEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodecEndpoint")
            .field("media_uri", &self.uri)
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

impl CodecEndpoint {
    pub fn for_loopback(address: SocketAddr, uri: &str) -> io::Result<Self> {
        crate::reference(uri)?;
        if !address.ip().is_loopback() {
            return Err(crate::invalid());
        }
        Ok(Self {
            uri: uri.into(),
            address,
            url: format!("tcp://{address}"),
            local_file: None,
        })
    }

    pub fn for_local_file(path: impl Into<PathBuf>, uri: &str) -> io::Result<Self> {
        crate::reference(uri)?;
        let path = path.into();
        if !path.is_absolute() || !path.is_file() {
            return Err(crate::invalid());
        }
        Ok(Self {
            uri: uri.into(),
            address: SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
            url: String::new(),
            local_file: Some(path),
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn input_arg(&self) -> &OsStr {
        self.local_file
            .as_ref()
            .map(|path| path.as_os_str())
            .unwrap_or_else(|| OsStr::new(&self.url))
    }

    pub fn protocol_whitelist(&self) -> &'static str {
        if self.local_file.is_some() {
            "file"
        } else {
            "tcp"
        }
    }

    pub fn is_seekable_file(&self) -> bool {
        self.local_file.is_some()
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn media_uri(&self) -> &str {
        &self.uri
    }
}

/// Session-private codec bridge. Stop the codec/close its socket before dropping this owner.
/// Remote bytes still pass through MediaStream validation; no remote credential reaches the codec.
pub struct LoopbackBridge {
    endpoint: CodecEndpoint,
    stop: Arc<AtomicBool>,
    workers: Vec<thread::JoinHandle<()>>,
}
impl LoopbackBridge {
    pub fn new(media: MediaStream) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let endpoint =
            CodecEndpoint::for_loopback(listener.local_addr()?, &media.info().media_uri)?;
        let stop = Arc::new(AtomicBool::new(false));
        let media = Arc::new(Mutex::new(media));
        let mut bridge = Self {
            endpoint,
            stop,
            workers: vec![],
        };
        let cancelled = bridge.stop.clone();
        bridge.workers.push(
            thread::Builder::new()
                .name("qnc-media-codec-tcp-bridge".into())
                .spawn(move || {
                    while !cancelled.load(Ordering::Acquire) {
                        match listener.accept() {
                            Ok((mut socket, address)) if address.ip().is_loopback() => {
                                let _ = socket.set_nonblocking(false);
                                let _ = socket.set_nodelay(true);
                                let _ = stream_from_start(&mut socket, media.clone());
                            }
                            Ok((_socket, _address)) => {}
                            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                                thread::sleep(Duration::from_millis(5));
                            }
                            Err(_) => break,
                        }
                    }
                })?,
        );
        Ok(bridge)
    }
    pub fn endpoint(&self) -> &CodecEndpoint {
        &self.endpoint
    }
}
impl Drop for LoopbackBridge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

fn stream_from_start(writer: &mut impl Write, media: Arc<Mutex<MediaStream>>) -> io::Result<()> {
    let mut media = media
        .lock()
        .map_err(|_| io::Error::other("media reader lock"))?;
    media.rewind()?;
    let mut buffer = vec![0; crate::MAX_READ_BYTES];
    loop {
        let count = media.read(&mut buffer)?;
        if count == 0 {
            return Ok(());
        }
        if let Err(error) = write_block(writer, &buffer[..count]) {
            return match error.kind() {
                io::ErrorKind::BrokenPipe
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::ConnectionReset => Ok(()),
                _ => Err(error),
            };
        }
    }
}

fn write_block(writer: &mut impl Write, mut bytes: &[u8]) -> io::Result<()> {
    while !bytes.is_empty() {
        match writer.write(bytes) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "codec bridge socket closed",
                ));
            }
            Ok(written) => bytes = &bytes[written..],
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
