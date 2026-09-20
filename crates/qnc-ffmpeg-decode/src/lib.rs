//! FFmpeg CLI adapter only; no playback clock, database or application policy.
use qnc_media_decode::*;
use qnc_media_metadata::{FrameTimebase, Rational};
use qnc_media_stream::CodecEndpoint;
use std::{
    ffi::OsString,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

#[derive(Debug, Clone)]
pub struct FfmpegAdapter {
    executable: PathBuf,
}
impl FfmpegAdapter {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
        }
    }
    fn command(
        &self,
        request: &DecodeRequest,
        plan: &DecodePlan,
        ep: &CodecEndpoint,
        _stamp: &str,
    ) -> Result<Command> {
        if !ep.is_seekable_file() {
            return Err(DecodeError::new(
                ErrorKind::Unsupported,
                "ffmpeg decoder requires a seekable codec endpoint",
            ));
        }
        let container = container_name(&plan.container)?;
        let mut cmd = Command::new(&self.executable);
        cmd.args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-noautorotate",
            "-copyts",
        ]);
        cmd.args([
            "-protocol_whitelist",
            ep.protocol_whitelist(),
            "-f",
            &container,
        ])
        .arg(format!("-c:{}", request.stream_index))
        .arg(&plan.codec);
        if let Some(start) = request.start {
            cmd.arg("-ss").arg(seconds(start)?);
        }
        cmd.arg("-i").arg(ep.input_arg()).args([
            "-map",
            &format!("0:{}", request.stream_index),
            "-sn",
            "-dn",
            "-map_metadata",
            "-1",
            "-stats_mux_pre",
            "pipe:2",
            "-stats_mux_pre_fmt",
            "QNC_PACKET {n} {pts} {tb} {size}",
            "-avoid_negative_ts",
            "disabled",
            "-threads",
            "1",
        ]);
        match &plan.format {
            DecodedFormat::Video { pixel_format, .. } => {
                cmd.args([
                    "-an",
                    "-vf",
                    &format!("setfield=prog,format={pixel_format}"),
                    "-c:v",
                    "rawvideo",
                    "-pix_fmt",
                    &format!("+{pixel_format}"),
                    "-fps_mode",
                    "passthrough",
                    "-enc_time_base",
                    "demux",
                    "-f",
                    "rawvideo",
                ]);
            }
            DecodedFormat::Audio { .. } => {
                cmd.args(["-vn", "-c:a", "pcm_f32le", "-f", "f32le"]);
            }
        }
        // Pipe buffering must not hold small PCM packets until a large muxer buffer fills.
        cmd.args(["-flush_packets", "1", "pipe:1"]);
        Ok(cmd)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FfmpegFilmstripMode {
    RandomSeek,
    KeyframeSeek,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FfmpegFilmstripFrame {
    pub seek_sec: f64,
}

pub fn extract_filmstrip_frames(
    executable: &Path,
    source: &Path,
    frames: &[FfmpegFilmstripFrame],
    source_timebase: FrameTimebase,
    mode: FfmpegFilmstripMode,
    thumb_size: [u32; 2],
    temp_dir: &Path,
) -> std::result::Result<(), String> {
    let cancel = AtomicBool::new(false);
    extract_filmstrip_frames_with_cancel(
        executable,
        source,
        frames,
        source_timebase,
        mode,
        thumb_size,
        temp_dir,
        &cancel,
    )
}

pub fn extract_filmstrip_frames_with_cancel(
    executable: &Path,
    source: &Path,
    frames: &[FfmpegFilmstripFrame],
    _source_timebase: FrameTimebase,
    mode: FfmpegFilmstripMode,
    thumb_size: [u32; 2],
    temp_dir: &Path,
    cancel: &AtomicBool,
) -> std::result::Result<(), String> {
    if frames.len() < 2 {
        return Err("filmstrip requires at least two frames".into());
    }
    if !source.is_file() {
        return Err(format!(
            "filmstrip media does not exist: {}",
            source.display()
        ));
    }
    let args = match mode {
        FfmpegFilmstripMode::RandomSeek => {
            filmstrip_random_seek_args(source, frames, thumb_size, temp_dir)
        }
        FfmpegFilmstripMode::KeyframeSeek => {
            filmstrip_keyframe_seek_args(source, frames, thumb_size, temp_dir)
        }
    };
    run_filmstrip_command(executable, args, cancel)
}

fn run_filmstrip_command(
    executable: &Path,
    args: Vec<OsString>,
    cancel: &AtomicBool,
) -> std::result::Result<(), String> {
    let mut command = Command::new(executable);
    command
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("filmstrip ffmpeg start: {error}"))?;
    loop {
        if cancel.load(Ordering::Acquire) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("filmstrip extraction cancelled".into());
        }
        let status = child
            .try_wait()
            .map_err(|error| format!("filmstrip ffmpeg wait: {error}"))?;
        let Some(status) = status else {
            thread::sleep(Duration::from_millis(10));
            continue;
        };
        let mut stderr = Vec::new();
        if let Some(mut pipe) = child.stderr.take() {
            let _ = pipe.read_to_end(&mut stderr);
        }
        if status.success() {
            return Ok(());
        }
        return Err(stderr_text_or_default(&stderr, "filmstrip ffmpeg failed"));
    }
}

fn stderr_text_or_default(stderr: &[u8], fallback: &str) -> String {
    let stderr = String::from_utf8_lossy(stderr);
    if stderr.trim().is_empty() {
        fallback.to_string()
    } else {
        stderr.trim().to_string()
    }
}

fn filmstrip_random_seek_args(
    source: &Path,
    frames: &[FfmpegFilmstripFrame],
    thumb_size: [u32; 2],
    temp_dir: &Path,
) -> Vec<OsString> {
    filmstrip_seek_args(source, frames, thumb_size, temp_dir, false)
}

fn filmstrip_keyframe_seek_args(
    source: &Path,
    frames: &[FfmpegFilmstripFrame],
    thumb_size: [u32; 2],
    temp_dir: &Path,
) -> Vec<OsString> {
    filmstrip_seek_args(source, frames, thumb_size, temp_dir, true)
}

fn filmstrip_seek_args(
    source: &Path,
    frames: &[FfmpegFilmstripFrame],
    thumb_size: [u32; 2],
    temp_dir: &Path,
    keyframes_only: bool,
) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("-hide_banner"),
        OsString::from("-loglevel"),
        OsString::from("error"),
        OsString::from("-nostdin"),
        OsString::from("-y"),
        OsString::from("-noautorotate"),
    ];
    for frame in frames {
        args.push(OsString::from("-ss"));
        args.push(OsString::from(format_decimal(frame.seek_sec, 6)));
        args.push(OsString::from("-noaccurate_seek"));
        if keyframes_only {
            args.push(OsString::from("-skip_frame"));
            args.push(OsString::from("nokey"));
        }
        // One frame per input: decoder threads only cost start-up and context switches.
        args.push(OsString::from("-threads"));
        args.push(OsString::from("1"));
        args.push(OsString::from("-i"));
        args.push(source.as_os_str().to_owned());
    }

    let scale = filmstrip_scale_filter(thumb_size);
    for (input_index, _) in frames.iter().enumerate() {
        args.extend([
            OsString::from("-map"),
            OsString::from(format!("{input_index}:v:0")),
            OsString::from("-an"),
            OsString::from("-sn"),
            OsString::from("-dn"),
            OsString::from("-frames:v"),
            OsString::from("1"),
            OsString::from("-vf"),
            OsString::from(&scale),
            OsString::from("-q:v"),
            OsString::from("2"),
            OsString::from("-pix_fmt"),
            OsString::from("yuvj420p"),
            OsString::from("-strict"),
            OsString::from("unofficial"),
            temp_dir
                .join(format!("{input_index:03}.jpg"))
                .into_os_string(),
        ]);
    }
    args
}

fn filmstrip_scale_filter(thumb_size: [u32; 2]) -> String {
    let [width, height] = thumb_size;
    format!(
        "scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:color=black"
    )
}

fn format_decimal(value: f64, precision: usize) -> String {
    let value = if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    };
    format!("{value:.precision$}").replace(',', ".")
}

fn seconds(t: Rational) -> Result<String> {
    if t.numerator < 0 || t.denominator <= 0 {
        return Err(invalid("invalid timestamp"));
    }
    let nanos = i128::from(t.numerator) * 1_000_000_000 / i128::from(t.denominator);
    Ok(format!(
        "{}.{:09}",
        nanos / 1_000_000_000,
        nanos % 1_000_000_000
    ))
}

fn invalid(message: &str) -> DecodeError {
    DecodeError::new(ErrorKind::Contract, message)
}
fn container_name(container: &str) -> Result<&str> {
    Ok(match container {
        "mov,mp4,m4a,3gp,3g2,mj2" | "mov" | "mp4" => "mov",
        "mxf" => "mxf",
        "matroska,webm" | "matroska" => "matroska",
        "wav" => "wav",
        _ => {
            return Err(DecodeError::new(
                ErrorKind::Unsupported,
                "unsupported saved container",
            ));
        }
    })
}
impl DecoderAdapter for FfmpegAdapter {
    fn validate(&self, _: &DecodeRequest, plan: &DecodePlan) -> Result<()> {
        container_name(&plan.container)?;
        if self.executable.as_os_str().is_empty() {
            return Err(invalid("missing decoder executable"));
        }
        Ok(())
    }
    fn launch(
        &self,
        request: &DecodeRequest,
        plan: &DecodePlan,
        endpoint: &CodecEndpoint,
        stamp: &str,
    ) -> Result<ProcessLaunch> {
        Ok(ProcessLaunch {
            command: self.command(request, plan, endpoint, stamp)?,
            input: vec![],
            records: Box::new(StatsReader::default()),
        })
    }
}

#[derive(Default)]
struct StatsReader {
    next_ordinal: u64,
}

impl PacketRecordReader for StatsReader {
    fn parse(&mut self, line: &str) -> Result<Option<PacketHeader>> {
        let Some(mut header) = parse_record(line)? else {
            return Ok(None);
        };
        header.ordinal = self.next_ordinal;
        self.next_ordinal += 1;
        Ok(Some(header))
    }
}
fn parse_record(line: &str) -> Result<Option<PacketHeader>> {
    if !line.starts_with("QNC_PACKET") {
        return Ok(None);
    }
    let bad = || DecodeError::new(ErrorKind::Stream, "malformed decoder packet record");
    let fields: Vec<_> = line.split_ascii_whitespace().collect();
    if fields.len() != 5 || fields[0] != "QNC_PACKET" {
        return Err(bad());
    }
    let (num, den) = fields[3].split_once('/').ok_or_else(bad)?;
    let header = PacketHeader {
        ordinal: fields[1].parse().map_err(|_| bad())?,
        pts: fields[2].parse().map_err(|_| bad())?,
        time_base: Rational {
            numerator: num.parse().map_err(|_| bad())?,
            denominator: den.parse().map_err(|_| bad())?,
        },
        size: fields[4].parse().map_err(|_| bad())?,
    };
    if header.pts == i64::MIN
        || header.pts == i64::MAX
        || header.time_base.numerator <= 0
        || header.time_base.denominator <= 0
        || header.size == 0
    {
        return Err(bad());
    }
    Ok(Some(header))
}

#[cfg(test)]
mod tests;
