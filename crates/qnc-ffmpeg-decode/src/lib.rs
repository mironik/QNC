//! FFmpeg CLI adapter only; no playback clock, database or application policy.
use qnc_media_decode::*;
use qnc_media_metadata::Rational;
use qnc_media_stream::HttpEndpoint;
use std::{path::PathBuf, process::Command};

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
        ep: &HttpEndpoint,
        stamp: &str,
    ) -> Result<Command> {
        let container = container_name(&plan.container)?;
        let mut cmd = Command::new(&self.executable);
        // MXF interleaves large picture packets and small mono packets. Reuse the
        // bounded HTTP read window; retain the MP4 soft-seek workaround elsewhere.
        let short_seek_size = if container == "mxf" { "1048576" } else { "1" };
        cmd.args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-xerror",
            "-nofind_stream_info",
            "-noautorotate",
            "-copyts",
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
            "-short_seek_size",
            short_seek_size,
            "-multiple_requests",
            "1",
            "-protocol_whitelist",
            "http,tcp",
            "-headers",
        ])
        .arg(format!(
            "Authorization: {}\r\n{}: {stamp}\r\n",
            ep.authorization_header(),
            qnc_media_stream::EXPECTED_STAMP_HEADER
        ))
        .args(["-f", &container])
        .arg(format!("-c:{}", request.stream_index))
        .arg(&plan.codec);
        if let Some(start) = request.start {
            cmd.arg("-ss").arg(seconds(start)?);
        }
        cmd.arg("-i").arg(ep.url()).args([
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
        endpoint: &HttpEndpoint,
        stamp: &str,
    ) -> Result<ProcessLaunch> {
        Ok(ProcessLaunch {
            command: self.command(request, plan, endpoint, stamp)?,
            input: vec![],
            records: Box::new(StatsReader),
        })
    }
}
struct StatsReader;
impl PacketRecordReader for StatsReader {
    fn parse(&mut self, line: &str) -> Result<Option<PacketHeader>> {
        parse_record(line)
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
