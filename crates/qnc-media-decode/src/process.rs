use crate::*;
use crossbeam_channel::{Receiver, RecvTimeoutError, SendTimeoutError, Sender, TryRecvError};
use qnc_media_stream::{CodecEndpoint, LoopbackBridge, MediaStream};
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, Stdio},
    sync::{
        Arc, Mutex,
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
type StderrCapture = Arc<Mutex<Vec<u8>>>;
const MAX_CAPTURED_STDERR_BYTES: usize = 8192;

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
    /// Asked to work below normal priority (a decoder in this process lowers its thread).
    background: Arc<AtomicBool>,
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
        Self::open_endpoint_inner(
            request,
            bridge.endpoint().clone(),
            stamp,
            config,
            Some(bridge),
        )
    }

    /// Open a decoder on a transport endpoint already validated by the storage owner.
    /// This is for local seekable owner bindings; callers must not pass public raw paths.
    pub fn open_endpoint(
        request: DecodeRequest,
        endpoint: CodecEndpoint,
        storage_stamp: String,
        config: DecoderConfig,
    ) -> Result<Self> {
        let plan = DecodePlan::new(&request, &config)?;
        config.adapter.validate(&request, &plan)?;
        if endpoint.media_uri() != request.media.media_uri {
            return Err(DecodeError::new(
                ErrorKind::Contract,
                "codec endpoint differs from saved request",
            ));
        }
        Self::open_endpoint_inner(request, endpoint, storage_stamp, config, None)
    }

    fn open_endpoint_inner(
        request: DecodeRequest,
        endpoint: CodecEndpoint,
        storage_stamp: String,
        config: DecoderConfig,
        bridge: Option<LoopbackBridge>,
    ) -> Result<Self> {
        let plan = DecodePlan::new(&request, &config)?;
        if let Some(source) = config.adapter.open_in_process(&request, &plan, &endpoint) {
            return Self::in_process(request, source?, plan, config, bridge);
        }
        let ProcessLaunch {
            command: mut cmd,
            input,
            records,
        } = config
            .adapter
            .launch(&request, &plan, &endpoint, &storage_stamp)?;
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
            // Windows starts a child at normal priority unless told: a decoder runs at the
            // priority of the process that needs its pictures (the player above the desktop,
            // a background worker at normal).
            let priority = unsafe {
                windows_sys::Win32::System::Threading::GetPriorityClass(
                    windows_sys::Win32::System::Threading::GetCurrentProcess(),
                )
            };
            cmd.creation_flags(0x08000000 | priority);
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
        let stderr_capture = Arc::new(Mutex::new(Vec::new()));
        let mut decoder = Self {
            request,
            config,
            state: State::Running,
            child: Some(child),
            cancelled: cancelled.clone(),
            events: Some(events_rx),
            workers: vec![],
            bridge,
            poll_deadline: None,
            eof_deadline: None,
            background: Arc::new(AtomicBool::new(false)),
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
        let captured = stderr_capture.clone();
        let header_worker = thread::Builder::new()
            .name("qnc-decode-records".into())
            .spawn(move || read_headers(stderr, headers_tx, stop, records, captured));
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
        let captured = stderr_capture.clone();
        let data_worker = thread::Builder::new()
            .name("qnc-decode-packets".into())
            .spawn(move || {
                read_packets(
                    stdout, headers_rx, events_tx, cancelled, request, plan, captured,
                )
            });
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

    /// The decoder works below normal priority: background work (the pictures before
    /// where the player stands) gives way to the playing decoder (Windows; other OSes keep
    /// the priority, a plain user may not raise it back).
    pub fn set_background_priority(&self) {
        self.background.store(true, Ordering::Release);
        #[cfg(windows)]
        if let Some(pid) = self.process_id() {
            use windows_sys::Win32::{
                Foundation::CloseHandle,
                System::Threading::{BELOW_NORMAL_PRIORITY_CLASS, OpenProcess, PROCESS_SET_INFORMATION, SetPriorityClass},
            };
            unsafe {
                let process = OpenProcess(PROCESS_SET_INFORMATION, 0, pid);
                if !process.is_null() {
                    SetPriorityClass(process, BELOW_NORMAL_PRIORITY_CLASS);
                    CloseHandle(process);
                }
            }
        }
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
                let Some(child) = self.child.as_mut() else {
                    return Ok(Poll::Ready(None)); // a decoder in this process ended with its stream
                };
                return match child.try_wait() {
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
        let Some(child) = self.child.as_mut() else {
            return Ok(()); // a decoder in this process ended with its stream
        };
        loop {
            match child.try_wait() {
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

fn no_packets_error(stderr: &StderrCapture) -> DecodeError {
    let captured = captured_stderr(stderr);
    if captured.is_empty() {
        stream_error("decoder returned no packets")
    } else {
        stream_error(&format!(
            "decoder returned no packets; decoder stderr: {captured}"
        ))
    }
}

fn captured_stderr(stderr: &StderrCapture) -> String {
    String::from_utf8_lossy(&stderr.lock().unwrap())
        .trim()
        .to_string()
}

fn capture_decoder_stderr(stderr: &StderrCapture, line: &str) {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("QNC_PACKET ") {
        return;
    }
    let mut captured = stderr.lock().unwrap();
    if captured.len() >= MAX_CAPTURED_STDERR_BYTES {
        return;
    }
    let remaining = MAX_CAPTURED_STDERR_BYTES - captured.len();
    let bytes = trimmed.as_bytes();
    if !captured.is_empty() {
        captured.push(b'\n');
    }
    captured.extend_from_slice(&bytes[..bytes.len().min(remaining)]);
}

fn read_headers(
    reader: impl Read,
    tx: Sender<Result<PacketHeader>>,
    stop: Arc<AtomicBool>,
    mut records: Box<dyn PacketRecordReader>,
    stderr_capture: StderrCapture,
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
            .and_then(|line| {
                capture_decoder_stderr(&stderr_capture, line);
                records.parse(line)
            });
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
    stderr_capture: StderrCapture,
) {
    let max_packet = plan.max_packet;
    let next = || -> Result<Option<(PacketHeader, Vec<u8>)>> {
        loop {
            if stop.load(Ordering::Acquire) {
                return Ok(None);
            }
            match headers.recv_timeout(Duration::from_millis(20)) {
                Ok(header) => {
                    let header = header?;
                    if header.size > max_packet {
                        return Err(stream_error(
                            "packet sequence or size differs from saved decode contract",
                        ));
                    }
                    let mut bytes = vec![0; header.size];
                    stdout
                        .read_exact(&mut bytes)
                        .map_err(|_| stream_error("truncated decoder packet"))?;
                    return Ok(Some((header, bytes)));
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => {
                    let mut extra = [0];
                    if stdout
                        .read(&mut extra)
                        .map_err(|_| stream_error("decoder stdout error"))?
                        != 0
                    {
                        return Err(stream_error("decoded bytes without packet record"));
                    }
                    return Ok(None);
                }
            }
        }
    };
    pump(next, &tx, &stop, &request, &plan, || no_packets_error(&stderr_capture));
}

/// The decoded packets of one request, from a decoder process or a decoder inside this
/// process, held to the saved decode contract (sequence, size, timestamps, sample frames,
/// and the saved frame count when the whole stream is decoded) and queued.
fn pump(
    mut next: impl FnMut() -> Result<Option<(PacketHeader, Vec<u8>)>>,
    tx: &Sender<Event>,
    stop: &AtomicBool,
    request: &DecodeRequest,
    plan: &DecodePlan,
    no_packets: impl Fn() -> DecodeError,
) {
    let result = (|| -> Result<()> {
        let mut ordinal = 0;
        let mut previous: Option<(i64, qnc_media_metadata::Rational)> = None;
        loop {
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            let Some((header, bytes)) = next()? else { break };
            header.validate()?;
            if header.ordinal != ordinal
                || header.size != bytes.len()
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
            if !send(tx, Event::Packet(packet), stop) {
                return Ok(());
            }
            ordinal += 1;
        }
        if stop.load(Ordering::Acquire) {
            return Ok(());
        }
        if ordinal == 0 {
            return Err(no_packets());
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
                    qnc_media_metadata::StreamDetails::Video(v) => v.exact_frame_count().map(|count| {
                        // At a delivered rate of twice or half the saved one, twice or half
                        // as many pictures (a half of an odd count rounds up: frame 0 is one).
                        match (request.delivered_rate, v.frame_rate.as_ref()) {
                            (Some(delivered), Some(saved))
                                if i128::from(delivered.fps_num) * i128::from(saved.value.fps_den)
                                    > i128::from(saved.value.fps_num) * i128::from(delivered.fps_den) =>
                            {
                                count * 2
                            }
                            (Some(_), Some(_)) => count.div_ceil(2),
                            _ => count,
                        }
                    }),
                    _ => None,
                });
            if expected != Some(ordinal) {
                return Err(stream_error(&format!(
                    "decoded frame count {ordinal} differs from saved record {expected:?}"
                )));
            }
        }
        Ok(())
    })();
    if !stop.load(Ordering::Acquire) {
        send(
            tx,
            match result {
                Ok(()) => Event::End,
                Err(e) => Event::Failed(e),
            },
            stop,
        );
    }
}

impl Decoder {
    /// A decoder inside this process: one thread takes its packets and queues them like
    /// the readers of a decoder process; cancel and drop stop it the same way.
    fn in_process(
        request: DecodeRequest,
        mut source: Box<dyn PictureSource>,
        plan: DecodePlan,
        config: DecoderConfig,
        bridge: Option<LoopbackBridge>,
    ) -> Result<Self> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let background = Arc::new(AtomicBool::new(false));
        let (events_tx, events_rx) = crossbeam_channel::bounded(config.queued_packets);
        let (stop, lower, asked) = (cancelled.clone(), background.clone(), request.clone());
        let worker = thread::Builder::new()
            .name("qnc-decode-in-process".into())
            .spawn(move || {
                let mut lowered = false;
                let next = || {
                    if !lowered && lower.load(Ordering::Acquire) {
                        lower_this_thread();
                        lowered = true;
                    }
                    source.next()
                };
                pump(next, &events_tx, &stop, &asked, &plan, || stream_error("decoder returned no packets"));
            })
            .map_err(|_| DecodeError::new(ErrorKind::Process, "cannot start decoder thread"))?;
        Ok(Self {
            request,
            config,
            state: State::Running,
            child: None,
            cancelled,
            events: Some(events_rx),
            workers: vec![worker],
            bridge,
            poll_deadline: None,
            eof_deadline: None,
            background,
        })
    }
}

/// The calling thread below normal priority (Windows; other OSes keep it).
fn lower_this_thread() {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL};
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_packets_error_includes_decoder_stderr_without_packet_records() {
        let capture = Arc::new(Mutex::new(Vec::new()));
        capture_decoder_stderr(&capture, "QNC_PACKET 0 0 1/50 16\n");
        capture_decoder_stderr(&capture, "Input/output error while seeking\n");

        let error = no_packets_error(&capture);
        assert_eq!(error.kind, ErrorKind::Stream);
        assert!(error.to_string().contains("decoder returned no packets"));
        assert!(
            error
                .to_string()
                .contains("Input/output error while seeking")
        );
        assert!(!error.to_string().contains("QNC_PACKET"));
    }
}
