//! Opt-in read-only byte-transport diagnostic, NOT a playback runtime or application.
//! Usage: decode_saved ROOT OWNER_SOURCE_DIRECTORY CLIP_ID [FFMPEG_EXECUTABLE]
use qnc_json_transport::Credentials;
use qnc_media_metadata::{Signal, StreamDetails};
use qnc_media_stream::{HttpEndpoint, LocalSource, MediaStream, SourceReference};
use qnc_player_input::InputReader;
use qnc_work_settings::SettingsReader;
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const LIMIT: u64 = 64 * 1024 * 1024;

struct Host {
    base: String,
    token: String,
    stop: Arc<AtomicBool>,
    requests: Arc<AtomicUsize>,
    ranges: Arc<AtomicUsize>,
    workers: Vec<thread::JoinHandle<()>>,
}
impl Host {
    fn new(source: LocalSource) -> Result<Self> {
        let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").map_err(|_| "bind failed")?);
        let base = format!("http://{}", server.server_addr());
        let token = uuid::Uuid::new_v4().simple().to_string();
        let credentials = Arc::new(Credentials::new(
            &token,
            &uuid::Uuid::new_v4().simple().to_string(),
        )?);
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(AtomicUsize::new(0));
        let ranges = Arc::new(AtomicUsize::new(0));
        let workers = (0..2)
            .map(|_| {
                let (server, source, credentials, stop, requests, ranges) = (
                    server.clone(),
                    source.clone(),
                    credentials.clone(),
                    stop.clone(),
                    requests.clone(),
                    ranges.clone(),
                );
                thread::spawn(move || {
                    while !stop.load(Ordering::Acquire) {
                        match server.recv_timeout(Duration::from_millis(20)) {
                            Ok(Some(request)) => {
                                if std::env::var_os("QNC_STREAM_TRACE").is_some() {
                                    eprintln!(
                                        "{} {}",
                                        request.method(),
                                        request
                                            .headers()
                                            .iter()
                                            .find(|h| h.field.equiv("Range"))
                                            .map(|h| h.value.as_str())
                                            .unwrap_or("no range")
                                    );
                                }
                                requests.fetch_add(1, Ordering::Relaxed);
                                if request.headers().iter().any(|h| h.field.equiv("Range")) {
                                    ranges.fetch_add(1, Ordering::Relaxed);
                                }
                                if let Err(error) = qnc_media_stream::server::respond(
                                    request,
                                    &source,
                                    &credentials,
                                ) && std::env::var_os("QNC_STREAM_TRACE").is_some()
                                {
                                    eprintln!("send: {error}");
                                }
                            }
                            Ok(None) => (),
                            Err(_) => break,
                        }
                    }
                })
            })
            .collect();
        Ok(Self {
            base,
            token,
            stop,
            requests,
            ranges,
            workers,
        })
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

fn decode(mut command: Command, token: &str) -> Result<Vec<u8>> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn()?;
    let stdout = child.stdout.take().ok_or("missing decoder stdout")?;
    let stderr = child.stderr.take().ok_or("missing decoder stderr")?;
    let output = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(LIMIT + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let errors = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr
            .take(64 * 1024)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break Err("decoder failed or timed out");
            }
        }
    };
    let bytes = output.join().map_err(|_| "stdout worker failed")??;
    let stderr = errors.join().map_err(|_| "stderr worker failed")??;
    // Do not dump commands/HTTP credentials from decoder diagnostics.
    if !status?.success() {
        return Err(format!(
            "decoder failed: {}",
            String::from_utf8_lossy(&stderr).replace(token, "[redacted]")
        )
        .into());
    }
    if bytes.is_empty() || bytes.len() as u64 > LIMIT {
        return Err("invalid/oversized decoded output".into());
    }
    Ok(bytes)
}

fn command(
    exe: &Path,
    endpoint: &HttpEndpoint,
    container: &str,
    stamp: &str,
    seconds: &str,
    stream_index: u32,
    decoder: &str,
) -> Command {
    let mut command = Command::new(exe);
    let level = if std::env::var_os("QNC_STREAM_TRACE").is_some() {
        "debug"
    } else {
        "error"
    };
    command
        .args([
            "-hide_banner",
            "-loglevel",
            level,
            "-nostdin",
            "-xerror",
            "-nofind_stream_info",
            "-noautorotate",
            "-rw_timeout",
            "5000000",
            "-max_redirects",
            "0",
            "-reconnect",
            "0",
            "-request_size",
            "1048576",
            "-initial_request_size",
            "1048576",
            "-multiple_requests",
            "1",
            "-protocol_whitelist",
            "http,tcp",
            "-headers",
        ])
        .arg(format!(
            "Authorization: {}\r\n{}: {stamp}\r\n",
            endpoint.authorization_header(),
            qnc_media_stream::EXPECTED_STAMP_HEADER
        ))
        .arg(format!("-c:{stream_index}"))
        .arg(decoder)
        .args(["-f", container, "-ss", seconds])
        .arg("-i")
        .arg(endpoint.url());
    command
}

fn saved_decoder(stream: &qnc_media_metadata::MediaStream) -> Result<&str> {
    match stream.codec.as_ref().map(|fact| &fact.value) {
        Some(Signal::Known(codec)) if !codec.is_empty() => Ok(codec),
        _ => Err("missing saved decoder codec; no discovery fallback".into()),
    }
}

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let root = PathBuf::from(args.next().ok_or("missing ROOT")?);
    let source_dir = PathBuf::from(args.next().ok_or("missing OWNER_SOURCE_DIRECTORY")?);
    let clip_id = args
        .next()
        .ok_or("missing CLIP_ID")?
        .into_string()
        .map_err(|_| "invalid clip id")?;
    let exe = PathBuf::from(args.next().unwrap_or_else(|| "ffmpeg".into()));
    if args.next().is_some() {
        return Err("too many arguments".into());
    }
    let settings_reader = SettingsReader::from_root(&root)?;
    let settings = settings_reader.read()?;
    let input =
        InputReader::new(settings_reader.clone()).load(&settings.workspace_db_uri, &clip_id)?;
    let media = input.media()?;
    let reference = SourceReference::from_uri(&media.media_uri)?;
    // Explicit diagnostic owner binding, not a new consumer-side path convention.
    let source = LocalSource::new(reference.source_uri(), &source_dir)?;
    let mut stream = MediaStream::local(&source, &media.media_uri)?;
    let info = stream.info().clone();
    let mut hash = Sha256::new();
    let mut block = [0; 64 * 1024];
    loop {
        let n = stream.read(&mut block)?;
        if n == 0 {
            break;
        }
        hash.update(&block[..n]);
    }
    let before = hash.finalize();
    let host = Host::new(source)?;
    let endpoint = HttpEndpoint::for_owner_endpoint(&host.base, &media.media_uri, &host.token)?;
    let container = match media
        .container
        .as_ref()
        .ok_or("no saved container")?
        .value
        .as_str()
    {
        "mov,mp4,m4a,3gp,3g2,mj2" | "mp4" | "mov" => "mov",
        "mxf" => "mxf",
        _ => return Err("diagnostic supports saved MOV/MP4 or MXF only".into()),
    };
    let video = input.layout.video.as_ref().ok_or("no saved video")?;
    let saved = media
        .streams
        .iter()
        .find(|s| {
            s.index
                .as_ref()
                .is_some_and(|i| i.value == video.stream_index)
        })
        .ok_or("missing stream")?;
    let StreamDetails::Video(v) = &saved.details else {
        return Err("not video".into());
    };
    let frame_bytes = usize::try_from(
        u64::from(v.width.as_ref().ok_or("no width")?.value)
            * u64::from(v.height.as_ref().ok_or("no height")?.value)
            * 3,
    )?;
    if frame_bytes == 0 || frame_bytes as u64 > LIMIT / 2 {
        return Err("diagnostic frame limit".into());
    }
    let nanos = u128::from(video.duration_frames / 2)
        * u128::try_from(video.timebase.fps_den)?
        * 1_000_000_000
        / u128::try_from(video.timebase.fps_num)?;
    let midpoint = format!("{}.{:09}", nanos / 1_000_000_000, nanos % 1_000_000_000);
    let mut hashes = Vec::new();
    for (seconds, count) in [("0", 2_usize), (midpoint.as_str(), 1)] {
        let mut cmd = command(
            &exe,
            &endpoint,
            container,
            &info.storage_stamp,
            seconds,
            video.stream_index,
            saved_decoder(saved)?,
        );
        cmd.args([
            "-map",
            &format!("0:{}", video.stream_index),
            "-an",
            "-sn",
            "-dn",
            "-frames:v",
            &count.to_string(),
            "-fps_mode",
            "passthrough",
            "-pix_fmt",
            "rgb24",
            "-f",
            "rawvideo",
            "pipe:1",
        ]);
        let bytes = decode(cmd, &host.token)?;
        if bytes.len() != frame_bytes * count {
            return Err("unexpected decoded frame size/count".into());
        }
        for frame in bytes.chunks_exact(frame_bytes) {
            hashes.push(format!("{:x}", Sha256::digest(frame)));
        }
    }
    if hashes
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        < 2
    {
        return Err("diagnostic did not observe distinct frames".into());
    }
    let mut audio = Vec::new();
    for saved in &media.streams {
        let StreamDetails::Audio(a) = &saved.details else {
            continue;
        };
        let index = saved.index.as_ref().ok_or("no audio index")?.value;
        let channels = a.channels.as_ref().ok_or("no saved channel count")?.value;
        let rate = a
            .sample_rate_hz
            .as_ref()
            .ok_or("no saved sample rate")?
            .value;
        let mut cmd = command(
            &exe,
            &endpoint,
            container,
            &info.storage_stamp,
            "0",
            index,
            saved_decoder(saved)?,
        );
        cmd.args([
            "-map",
            &format!("0:{index}"),
            "-vn",
            "-sn",
            "-dn",
            "-t",
            "0.1",
            "-c:a",
            "pcm_f32le",
            "-f",
            "f32le",
            "pipe:1",
        ]);
        let bytes = decode(cmd, &host.token)?;
        if bytes.len() as u64 != u64::from(rate) * u64::from(channels) * 4 / 10 {
            return Err("unexpected audio sample count".into());
        }
        let mut peak = 0_f32;
        for b in bytes.chunks_exact(4) {
            let sample = f32::from_le_bytes(b.try_into()?);
            if !sample.is_finite() {
                return Err("nonfinite audio sample".into());
            }
            peak = peak.max(sample.abs());
        }
        audio.push(serde_json::json!({"stream":index,"channels":channels,"sample_rate":rate,"bytes":bytes.len(),"peak":peak}));
    }
    stream.seek(SeekFrom::Start(0))?;
    let mut hash = Sha256::new();
    loop {
        let n = stream.read(&mut block)?;
        if n == 0 {
            break;
        }
        hash.update(&block[..n]);
    }
    if hash.finalize() != before || settings_reader.read()? != settings {
        return Err("source/settings changed during diagnostic".into());
    }
    println!(
        "{}",
        serde_json::json!({"project":settings.project_name,"clip_id":clip_id,"representation":input.representation,
        "decoded_frames":hashes.len(),"frame_hashes":hashes,"audio":audio,"http_requests":host.requests.load(Ordering::Relaxed),
        "http_ranges":host.ranges.load(Ordering::Relaxed),"source_unchanged":true,"database_writes":0,
        "player_runtime":false,"audio_device_output":false})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_forces_saved_demuxer_decoder_and_disables_stream_discovery() {
        let ep = HttpEndpoint::for_owner_endpoint(
            "http://127.0.0.1:1",
            "qnc://local/source/card/file/clip",
            "test-token",
        )
        .unwrap();
        let cmd = command(
            Path::new("ffmpeg"),
            &ep,
            "mxf",
            "8-abc",
            "0",
            3,
            "pcm_s24le",
        );
        let args: Vec<_> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert!(args.iter().any(|a| a == "-nofind_stream_info"));
        for pair in [
            ["-f", "mxf"],
            ["-c:3", "pcm_s24le"],
            ["-multiple_requests", "1"],
            ["-max_redirects", "0"],
        ] {
            assert!(args.windows(2).any(|w| w[0] == pair[0] && w[1] == pair[1]));
        }
    }
}
