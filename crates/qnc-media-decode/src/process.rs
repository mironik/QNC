use crate::*;
use crossbeam_channel::{Receiver, RecvTimeoutError, SendTimeoutError, Sender, TryRecvError};
use qnc_media_stream::{LoopbackBridge, MediaStream};
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

enum Event {
    Packet(DecodedPacket),
    End,
    Failed(DecodeError),
}
enum State {
    Running,
    Finished,
    Failed(DecodeError),
}

pub struct Decoder {
    request: DecodeRequest,
    config: DecoderConfig,
    state: State,
    child: Option<Child>,
    cancelled: Arc<AtomicBool>,
    events: Option<Receiver<Event>>,
    workers: Vec<thread::JoinHandle<()>>,
    bridge: Option<LoopbackBridge>,
    poll_deadline: Option<Instant>,
    eof_deadline: Option<Instant>,
}
impl Decoder {
    pub fn open(request: DecodeRequest, media: MediaStream, config: DecoderConfig) -> Result<Self> {
        let plan = DecodePlan::new(&request, &config)?;
        config.adapter.validate(&request, &plan)?;
        if media.info().media_uri != request.media.media_uri {
            return Err(DecodeError::new(
                ErrorKind::Contract,
                "opened media differs from saved request",
            ));
        }
        let stamp = media.info().storage_stamp.clone();
        let bridge = LoopbackBridge::new(media)
            .map_err(|_| DecodeError::new(ErrorKind::Stream, "cannot open codec byte bridge"))?;
        let ProcessLaunch {
            command: mut cmd,
            input,
            records,
        } = config
            .adapter
            .launch(&request, &plan, bridge.endpoint(), &stamp)?;
        if input.len() > MAX_OPEN_BYTES {
            return Err(DecodeError::new(
                ErrorKind::Contract,
                "decoder open request too large",
            ));
        }
        cmd.stdin(if input.is_empty() {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let mut child = cmd.spawn().map_err(|_| {
            DecodeError::new(
                ErrorKind::Process,
                "cannot start configured decoder executable",
            )
        })?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        let cancelled = Arc::new(AtomicBool::new(false));
        let (events_tx, events_rx) = crossbeam_channel::bounded(config.queued_packets);
        let (headers_tx, headers_rx) = crossbeam_channel::bounded(4);
        let mut decoder = Self {
            request,
            config,
            state: State::Running,
            child: Some(child),
            cancelled: cancelled.clone(),
            events: Some(events_rx),
            workers: vec![],
            bridge: Some(bridge),
            poll_deadline: None,
            eof_deadline: None,
        };
        if let Some(mut stdin) = stdin {
            let worker = thread::Builder::new()
                .name("qnc-decode-open".into())
                .spawn(move || {
                    let _ = stdin.write_all(&input);
                });
            decoder.workers.push(worker.map_err(|_| {
                DecodeError::new(ErrorKind::Process, "cannot start decoder request writer")
            })?);
        }
        let stop = cancelled.clone();
        let header_worker = thread::Builder::new()
            .name("qnc-decode-records".into())
            .spawn(move || read_headers(stderr, headers_tx, stop, records));
        match header_worker {
            Ok(worker) => decoder.workers.push(worker),
            Err(_) => {
                return Err(DecodeError::new(
                    ErrorKind::Process,
                    "cannot start packet record reader",
                ));
            }
        };
        let request = decoder.request.clone();
        let data_worker = thread::Builder::new()
            .name("qnc-decode-packets".into())
            .spawn(move || read_packets(stdout, headers_rx, events_tx, cancelled, request, plan));
        match data_worker {
            Ok(worker) => decoder.workers.push(worker),
            Err(_) => {
                return Err(DecodeError::new(
                    ErrorKind::Process,
                    "cannot start packet reader",
                ));
            }
        };
        Ok(decoder)
    }
    pub fn request(&self) -> &DecodeRequest {
        &self.request
    }
    pub fn process_id(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    /// Consume the existing bounded queue without waiting, opening or joining.
    /// Cleanup remains explicit through cancel/Drop on the owning module thread.
    pub fn try_next_packet(&mut self) -> Result<std::task::Poll<Option<DecodedPacket>>> {
        use std::task::Poll;
        match &self.state {
            State::Finished => return Ok(Poll::Ready(None)),
            State::Failed(error) => return Err(error.clone()),
            State::Running => (),
        }
        let result = (|| {
            if let Some(deadline) = self.eof_deadline {
                return match self.child.as_mut().expect("running child").try_wait() {
                    Ok(Some(status)) if status.success() => Ok(Poll::Ready(None)),
                    Ok(Some(_)) => Err(DecodeError::new(
                        ErrorKind::Process,
                        "decoder exited unsuccessfully",
                    )),
                    Ok(None) if Instant::now() < deadline => Ok(Poll::Pending),
                    _ => Err(DecodeError::new(
                        ErrorKind::Timeout,
                        "decoder did not terminate after EOF",
                    )),
                };
            }
            match self.events.as_ref().expect("running receiver").try_recv() {
                Ok(Event::Packet(packet)) => {
                    self.poll_deadline = None;
                    Ok(Poll::Ready(Some(packet)))
                }
                Ok(Event::End) => {
                    self.eof_deadline = Some(Instant::now() + self.config.read_timeout);
                    Ok(Poll::Pending)
                }
                Ok(Event::Failed(error)) => Err(error),
                Err(TryRecvError::Empty) => {
                    let deadline = self
                        .poll_deadline
                        .get_or_insert_with(|| Instant::now() + self.config.read_timeout);
                    if Instant::now() < *deadline {
                        Ok(Poll::Pending)
                    } else {
                        Err(DecodeError::new(
                            ErrorKind::Timeout,
                            "decoder packet timeout",
                        ))
                    }
                }
                Err(TryRecvError::Disconnected) => Err(DecodeError::new(
                    ErrorKind::Process,
                    "decoder reader ended without completion",
                )),
            }
        })();
        match &result {
            Ok(Poll::Ready(None)) => {
                self.child.take();
                self.state = State::Finished;
            }
            Err(error) => self.state = State::Failed(error.clone()),
            _ => (),
        }
        result
    }

    /// Backpressure stops decode when the bounded queue is full; this does not run a playback clock.
    pub fn next_packet(&mut self) -> Result<Option<DecodedPacket>> {
        match &self.state {
            State::Finished => return Ok(None),
            State::Failed(e) => return Err(e.clone()),
            State::Running => (),
        }
        let event = if self.eof_deadline.is_some() {
            Ok(Event::End)
        } else {
            self.events
                .as_ref()
                .expect("running receiver")
                .recv_timeout(self.config.read_timeout)
        };
        let result = match event {
            Ok(Event::Packet(packet)) => {
                self.poll_deadline = None;
                return Ok(Some(packet));
            }
            Ok(Event::End) => self.finish(),
            Ok(Event::Failed(error)) => Err(error),
            Err(RecvTimeoutError::Timeout) => Err(DecodeError::new(
                ErrorKind::Timeout,
                "decoder packet timeout",
            )),
            Err(RecvTimeoutError::Disconnected) => Err(DecodeError::new(
                ErrorKind::Process,
                "decoder reader ended without completion",
            )),
        };
        match &result {
            Ok(()) => self.state = State::Finished,
            Err(e) => self.state = State::Failed(e.clone()),
        }
        self.shutdown();
        result.map(|_| None)
    }
    fn finish(&mut self) -> Result<()> {
        let deadline = self
            .eof_deadline
            .unwrap_or_else(|| Instant::now() + self.config.read_timeout);
        loop {
            match self.child.as_mut().expect("running child").try_wait() {
                Ok(Some(status)) if status.success() => return Ok(()),
                Ok(Some(_)) => {
                    return Err(DecodeError::new(
                        ErrorKind::Process,
                        "decoder exited unsuccessfully",
                    ));
                }
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
                _ => {
                    return Err(DecodeError::new(
                        ErrorKind::Timeout,
                        "decoder did not terminate after EOF",
                    ));
                }
            }
        }
    }
    pub fn cancel(&mut self) {
        if matches!(self.state, State::Running) {
            self.state = State::Failed(DecodeError::new(ErrorKind::Cancelled, "decode cancelled"));
        }
        self.shutdown();
    }
    fn shutdown(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.events.take();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
        // The codec has closed its sockets before the byte bridge is joined.
        self.bridge.take();
    }
}
impl Drop for Decoder {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn send<T>(tx: &Sender<T>, mut value: T, stop: &AtomicBool) -> bool {
    while !stop.load(Ordering::Acquire) {
        match tx.send_timeout(value, Duration::from_millis(20)) {
            Ok(()) => return true,
            Err(SendTimeoutError::Disconnected(_)) => return false,
            Err(SendTimeoutError::Timeout(v)) => value = v,
        }
    }
    false
}
fn stream_error(message: &str) -> DecodeError {
    DecodeError::new(ErrorKind::Stream, message)
}

fn read_headers(
    reader: impl Read,
    tx: Sender<Result<PacketHeader>>,
    stop: Arc<AtomicBool>,
    mut records: Box<dyn PacketRecordReader>,
) {
    let mut reader = BufReader::new(reader);
    while !stop.load(Ordering::Acquire) {
        let mut line = Vec::new();
        match (&mut reader)
            .take(MAX_RECORD_BYTES as u64 + 1)
            .read_until(b'\n', &mut line)
        {
            Ok(0) => {
                if let Err(error) = records.finish() {
                    send(&tx, Err(error), &stop);
                }
                return;
            }
            Ok(n) if n <= MAX_RECORD_BYTES => (),
            _ => {
                send(
                    &tx,
                    Err(stream_error("invalid or oversized decoder record line")),
                    &stop,
                );
                return;
            }
        }
        let parsed = std::str::from_utf8(&line)
            .map_err(|_| stream_error("invalid decoder record encoding"))
            .and_then(|line| records.parse(line));
        match parsed {
            Ok(None) => (),
            Ok(Some(header)) => {
                if let Err(error) = header.validate() {
                    send(&tx, Err(error), &stop);
                    return;
                }
                if !send(&tx, Ok(header), &stop) {
                    return;
                }
            }
            Err(e) => {
                send(&tx, Err(e), &stop);
                return;
            }
        }
    }
}

fn read_packets(
    mut stdout: impl Read,
    headers: Receiver<Result<PacketHeader>>,
    tx: Sender<Event>,
    stop: Arc<AtomicBool>,
    request: DecodeRequest,
    plan: DecodePlan,
) {
    let result = (|| -> Result<()> {
        let mut ordinal = 0;
        let mut previous: Option<(i64, qnc_media_metadata::Rational)> = None;
        loop {
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            let header = match headers.recv_timeout(Duration::from_millis(20)) {
                Ok(header) => header?,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            };
            if header.ordinal != ordinal
                || header.size > plan.max_packet
                || plan.exact_packet.is_some_and(|size| size != header.size)
            {
                return Err(stream_error(
                    "packet sequence or size differs from saved decode contract",
                ));
            }
            if previous.is_some_and(|(pts, tb)| tb != header.time_base || header.pts <= pts) {
                return Err(stream_error(
                    "decoder timestamps do not advance consistently",
                ));
            }
            if let DecodedFormat::Audio { channels, .. } = plan.format
                && !header.size.is_multiple_of(channels as usize * 4)
            {
                return Err(stream_error("partial audio sample frame"));
            }
            let mut bytes = vec![0; header.size];
            stdout
                .read_exact(&mut bytes)
                .map_err(|_| stream_error("truncated decoder packet"))?;
            if matches!(plan.format, DecodedFormat::Audio { .. })
                && bytes
                    .chunks_exact(4)
                    .any(|b| !f32::from_le_bytes(b.try_into().expect("four bytes")).is_finite())
            {
                return Err(stream_error("nonfinite PCM sample"));
            }
            previous = Some((header.pts, header.time_base));
            let packet = DecodedPacket {
                version: VERSION.into(),
                media_uri: request.media.media_uri.clone(),
                stream_index: request.stream_index,
                ordinal,
                pts: header.pts,
                time_base: header.time_base,
                format: plan.format.clone(),
                bytes,
            };
            if !send(&tx, Event::Packet(packet), &stop) {
                return Ok(());
            }
            ordinal += 1;
        }
        let mut extra = [0];
        if stdout
            .read(&mut extra)
            .map_err(|_| stream_error("decoder stdout error"))?
            != 0
        {
            return Err(stream_error("decoded bytes without packet record"));
        }
        if ordinal == 0 {
            return Err(stream_error("decoder returned no packets"));
        }
        if request.start.is_none() && matches!(plan.format, DecodedFormat::Video { .. }) {
            let expected = request
                .media
                .streams
                .iter()
                .find(|s| {
                    s.index
                        .as_ref()
                        .is_some_and(|i| i.value == request.stream_index)
                })
                .and_then(|s| match &s.details {
                    qnc_media_metadata::StreamDetails::Video(v) => v.exact_frame_count(),
                    _ => None,
                });
            if expected != Some(ordinal) {
                return Err(stream_error(
                    "decoded frame count differs from saved record",
                ));
            }
        }
        Ok(())
    })();
    if !stop.load(Ordering::Acquire) {
        send(
            &tx,
            match result {
                Ok(()) => Event::End,
                Err(e) => Event::Failed(e),
            },
            &stop,
        );
    }
}
