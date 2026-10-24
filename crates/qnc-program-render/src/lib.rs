//! Renders one unit of the story's prerender (user 2026-10-12) into the delivery format
//! of the project (`export.*`): the pictures and samples the player plays for the unit,
//! decoded through the same decode layer (and decoder catalog) as the player, at full
//! size, then encoded by the FFmpeg CLI. Every unit starts on a closed group of pictures,
//! so the units of a story are put together without encoding again; its sound is PCM, so
//! nothing is cut at a unit's edge. The unit is written at its final name: the project
//! folder denies deleting and so renaming (its delete lock is inherited by its files), and
//! whether a unit is complete is recorded in the project database by the caller, never
//! read from a file. Reads media only through the endpoints it is given (a card stays read
//! only); no probe, no database, no clock.

use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
};

use qnc_media_decode::{DecodeRequest, DecodedFormat, Decoder, DecoderConfig, VERSION};
use qnc_media_metadata::MediaRepresentation;
use qnc_media_stream::CodecEndpoint;
use qnc_render_unit::{position, RenderUnit};
use serde_json::Value;

mod encoder;

pub type Result<T> = std::result::Result<T, String>;

/// The format of a rendered unit: MOV, picture stream 0 without B pictures and with the
/// time scale of the program rate, PCM stream 1, BT.709 limited range, timestamps from 0.
/// Its description (`unit_media`) is written, never probed.
pub const UNIT_FORMAT: u32 = 3;

/// The delivery format of the project (`export.*`), strictly: what this block cannot
/// render is refused with a reason, never replaced.
#[derive(Debug, Clone, PartialEq)]
pub struct Delivery {
    pub video_codec: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub interlaced: bool,
    pub audio_sample_rate: u32,
    pub audio_channels: u16,
    /// The export block as saved (part of every unit's fingerprint).
    pub saved: Value,
    /// The encoder of this computer (`on`), part of the fingerprint too: units of other
    /// encoders are not put together.
    pub encoder: Option<String>,
}

impl Delivery {
    pub fn from_export(export: &Value) -> Result<Self> {
        let text = |key: &str| export.get(key).and_then(Value::as_str).map(str::to_string);
        let number = |key: &str| export.get(key).and_then(Value::as_f64);
        let missing = |key: &str| qnc_ui_text::fill("render.export_setting_missing", &[("key", key)]);
        let field_order = text("field_order").ok_or_else(|| missing("field_order"))?;
        let delivery = Self {
            video_codec: text("video_codec").ok_or_else(|| missing("video_codec"))?,
            width: number("width").map(|v| v as u32).ok_or_else(|| missing("width"))?,
            height: number("height").map(|v| v as u32).ok_or_else(|| missing("height"))?,
            fps: number("fps").ok_or_else(|| missing("fps"))?,
            interlaced: field_order != "progressive",
            audio_sample_rate: number("audio_sample_rate").map(|v| v as u32).ok_or_else(|| missing("audio_sample_rate"))?,
            audio_channels: number("audio_channels").map(|v| v as u16).ok_or_else(|| missing("audio_channels"))?,
            saved: export.clone(),
            encoder: None,
        };
        if delivery.width == 0 || delivery.height == 0 || delivery.fps <= 0.0 || delivery.audio_channels == 0 {
            return Err(qnc_ui_text::text("render.export_settings_have_invalid_format").into());
        }
        delivery.supported()?;
        Ok(delivery)
    }

    /// The delivery codec is one this block renders (checked when the settings are read).
    fn supported(&self) -> Result<()> {
        match self.video_codec.as_str() {
            "h264" | "mpeg2_422_50mbit" => Ok(()),
            other => Err(format!("Prerender jos ne zna kodek predaje '{other}'.")),
        }
    }

    /// The picture layout the encoder takes and its arguments: H.264 through the encoder
    /// that works on this computer (`encoder`), XDCAM HD422 through MPEG-2.
    fn encoder_args(&self, ffmpeg: &Path) -> Result<(String, Vec<String>)> {
        self.supported()?;
        let gop = self.fps.round().max(1.0) as u32;
        let (layout, args): (&str, Vec<&str>) = match self.video_codec.as_str() {
            "h264" => encoder::H264::here(ffmpeg).args(),
            _ => (
                "yuv422p",
                vec![
                    "-c:v", "mpeg2video", "-profile:v", "0", "-level:v", "2",
                    "-b:v", "50M", "-minrate", "50M", "-maxrate", "50M", "-bufsize", "17825792",
                    "-flags", "+cgop", "-sc_threshold", "1000000000", "-bf", "0", "-dc", "10", "-intra_vlc", "1", "-non_linear_quant", "1",
                    "-qmin", "1", "-qmax", "12",
                ],
            ),
        };
        let mut args: Vec<String> = args.into_iter().map(str::to_string).collect();
        args.extend(["-g".to_string(), gop.to_string()]);
        Ok((layout.to_string(), args))
    }

    /// The layout asked of the decoder: 4:2:0 for a 4:2:0 delivery (converted in the
    /// decoder, a third of the bytes through the encoder's pipe), else the saved one.
    fn decode_pixel_format(&self) -> Option<String> {
        (self.video_codec == "h264").then(|| "yuv420p".to_string())
    }

    /// Whether the unit's frames are the delivery's frames (same rate, progressive).
    fn check(&self, unit: &RenderUnit) -> Result<()> {
        if self.interlaced {
            return Err(qnc_ui_text::text("render.prerender_does_make_interlaced_delivery_yet").into());
        }
        let program = unit.program_rate.fps_num as f64 / unit.program_rate.fps_den.max(1) as f64;
        if (program - self.fps).abs() > 0.01 {
            return Err(qnc_ui_text::fill("render.rate_not_delivery", &[("story", &format!("{program:.3}")), ("delivery", &format!("{:.3}", self.fps))]));
        }
        if unit.sample_rate != self.audio_sample_rate {
            return Err(qnc_ui_text::fill("render.sample_rate_not_delivery", &[("story", &unit.sample_rate.to_string()), ("delivery", &self.audio_sample_rate.to_string())]));
        }
        if unit.channels > self.audio_channels {
            return Err(format!(
                "Prica ima {} izlaznih kanala, predaja {}.",
                unit.channels, self.audio_channels
            ));
        }
        Ok(())
    }

    /// This delivery with the encoder that works with `ffmpeg` on this computer.
    pub fn on(mut self, ffmpeg: &Path) -> Self {
        self.encoder = Some(match self.video_codec.as_str() {
            "h264" => format!("{:?}", encoder::H264::here(ffmpeg)),
            other => other.to_string(),
        });
        self
    }

    /// What a unit is rendered in: the export block, the encoder and the unit's own
    /// format (a unit of another format is another file). The player matches the units
    /// of its story by this key.
    pub fn key(&self) -> Value {
        serde_json::json!({"export": self.saved, "encoder": self.encoder, "unit_format": UNIT_FORMAT})
    }

    /// The fingerprint of a unit rendered in this delivery and encoder.
    pub fn fingerprint(&self, unit: &RenderUnit) -> String {
        unit.fingerprint(&self.key())
    }

    /// The file name of a rendered unit: its fingerprint.
    pub fn file_name(&self, unit: &RenderUnit) -> String {
        format!("{}.mov", self.fingerprint(unit))
    }
}

/// Where a unit's media is read: its endpoint and storage stamp (given by the host that
/// resolves the QNC URI; a card endpoint is read only).
pub type MediaAccess<'a> = dyn Fn(&MediaRepresentation) -> io::Result<(CodecEndpoint, String)> + Sync + 'a;

/// Renders `unit` into `out` (a `.mov` file, overwritten). The picture is encoded while
/// the sound is decoded on another thread; both wait in the system's temporary folder and
/// are put together into `out` without encoding again (measured 2026-10-12: the sound
/// decoded first and the picture decoded on one thread made a unit slower than real time).
pub fn render(
    unit: &RenderUnit,
    delivery: &Delivery,
    decoders: &DecoderConfig,
    access: &MediaAccess,
    ffmpeg: &Path,
    out: &Path,
    cancel: &AtomicBool,
) -> Result<()> {
    delivery.check(unit)?;
    if unit.frames() == 0 {
        return Err("Prazna jedinica prerendera.".into());
    }
    let (video_file, audio_file) = (temporary(out, "video.mov"), temporary(out, "pcm"));
    let result = std::thread::scope(|scope| {
        let sound = scope.spawn(|| sound_of(unit, delivery, decoders, access, cancel));
        let picture = encode_picture(unit, delivery, decoders, access, ffmpeg, &video_file, cancel);
        let sound = sound.join().map_err(|_| qnc_ui_text::text("render.prerender_sound_failed").to_string())?;
        picture?;
        std::fs::write(&audio_file, sound?).map_err(|e| qnc_ui_text::fill("render.sound_write_failed", &[("error", &e.to_string())]))?;
        mux(unit, delivery, ffmpeg, &video_file, &audio_file, out)
    });
    let _ = std::fs::remove_file(&video_file);
    let _ = std::fs::remove_file(&audio_file);
    result
}

/// The colour of every unit (the player converts BT.709 limited range).
const UNIT_TAGS: [&str; 8] = [
    "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709", "-color_range", "tv",
];

/// The description of a rendered unit, as this block wrote it (UNIT_FORMAT): the media
/// the player opens instead of the unit's sources. `media_uri` is the unit file's QNC URI.
pub fn unit_media(unit: &RenderUnit, delivery: &Delivery, media_uri: &str) -> Result<MediaRepresentation> {
    use qnc_media_metadata::*;
    fn fact<T>(value: T) -> Fact<T> {
        Fact { value, evidence_id: "prerender".into(), locator: "qnc-program-render".into() }
    }
    let rate = unit.program_rate;
    let layout = match delivery.video_codec.as_str() {
        "h264" => "yuv420p",
        "mpeg2_422_50mbit" => "yuv422p",
        other => return Err(format!("Prerender jos ne zna kodek predaje '{other}'.")),
    };
    let codec = if delivery.video_codec == "h264" { "h264" } else { "mpeg2video" };
    let frames = i64::try_from(unit.frames()).map_err(|e| e.to_string())?;
    let samples = i64::try_from(unit.samples).map_err(|e| e.to_string())?;
    let video = VideoMetadata {
        width: Some(fact(delivery.width)),
        height: Some(fact(delivery.height)),
        frame_rate: Some(fact(rate)),
        frame_rate_mode: Some(fact(FrameRateMode::Constant)),
        frame_count: Some(fact(FrameCount::Exact(unit.frames()))),
        scan_mode: Some(fact(ScanMode::Progressive)),
        pixel_format: Some(fact(layout.to_string())),
        sample_aspect_ratio: Some(fact(Rational { numerator: 1, denominator: 1 })),
        rotation_degrees: Some(fact(Signal::Known(0))),
        color: ColorMetadata {
            primaries: Some(fact(Signal::Known("bt709".into()))),
            transfer: Some(fact(Signal::Known("bt709".into()))),
            matrix: Some(fact(Signal::Known("bt709".into()))),
            range: Some(fact(Signal::Known("tv".into()))),
        },
    };
    let audio = AudioMetadata {
        sample_rate_hz: Some(fact(unit.sample_rate)),
        channels: Some(fact(u32::from(delivery.audio_channels))),
        sample_format: Some(fact(Signal::Known("s32".into()))),
        channel_layout: Some(fact(Signal::Unspecified)),
        bits_per_sample: Some(fact(24)),
    };
    Ok(MediaRepresentation {
        media_uri: media_uri.into(),
        container: Some(fact("mov,mp4,m4a,3gp,3g2,mj2".into())),
        duration_seconds: Some(fact(Rational { numerator: frames * rate.fps_den, denominator: rate.fps_num })),
        streams_complete: Some(fact(true)),
        streams: vec![
            MediaStream {
                index: Some(fact(0)),
                codec: Some(fact(Signal::Known(codec.into()))),
                profile: None,
                time_base: Some(fact(Rational { numerator: 1, denominator: rate.fps_num })),
                start_pts: Some(fact(0)),
                duration_ts: Some(fact(frames * rate.fps_den)),
                details: StreamDetails::Video(Box::new(video)),
            },
            MediaStream {
                index: Some(fact(1)),
                codec: Some(fact(Signal::Known("pcm_s24le".into()))),
                profile: None,
                time_base: Some(fact(Rational { numerator: 1, denominator: i64::from(unit.sample_rate) })),
                start_pts: Some(fact(0)),
                duration_ts: Some(fact(samples)),
                details: StreamDetails::Audio(Box::new(audio)),
            },
        ],
        tags: Default::default(),
    })
}

fn temporary(out: &Path, suffix: &str) -> PathBuf {
    let name = out.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    std::env::temp_dir().join(format!("qnc-prerender-{}-{name}.{suffix}", std::process::id()))
}

/// The encoded picture and the PCM sound into the unit's file, both copied.
fn mux(unit: &RenderUnit, delivery: &Delivery, ffmpeg: &Path, video: &Path, audio: &Path, out: &Path) -> Result<()> {
    let output = Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-i"])
        .arg(video)
        .args(["-f", "f32le", "-ar", &unit.sample_rate.to_string(), "-ac", &delivery.audio_channels.to_string(), "-i"])
        .arg(audio)
        .args(["-map", "0:v:0", "-map", "1:a:0", "-c:v", "copy", "-c:a", "pcm_s24le"])
        .args(UNIT_TAGS)
        // The index at the start of the file (user 2026-10-13: a long unit with it at the end
        // took 4.6 s to open, the reader had to reach its end first).
        .args(["-movflags", "+faststart"])
        .args(["-video_track_timescale", &unit.program_rate.fps_num.to_string(), "-f", "mov"])
        .arg(out)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Spajanje prerendera se ne moze pokrenuti: {e}"))?;
    if !output.status.success() {
        return Err(format!("Spajanje prerendera: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn open(
    media: &MediaRepresentation,
    stream_index: u32,
    start: Option<qnc_media_metadata::Rational>,
    output_pixel_format: Option<String>,
    progressive: bool,
    delivered_rate: Option<qnc_media_metadata::FrameTimebase>,
    decoders: &DecoderConfig,
    access: &MediaAccess,
) -> Result<Decoder> {
    let (endpoint, stamp) = access(media).map_err(|e| qnc_ui_text::fill("render.media_unavailable", &[("error", &e.to_string())]))?;
    let request = DecodeRequest {
        version: VERSION.into(),
        media: media.clone(),
        stream_index,
        start,
        output_pixel_format,
        progressive,
        delivered_rate,
    };
    Decoder::open_endpoint(request, endpoint, stamp, decoders.clone()).map_err(|e| e.to_string())
}

/// The unit's sound, interleaved f32 in the delivery's channels; a channel no part feeds
/// is silent (as the player's).
fn sound_of(
    unit: &RenderUnit,
    delivery: &Delivery,
    decoders: &DecoderConfig,
    access: &MediaAccess,
    cancel: &AtomicBool,
) -> Result<Vec<u8>> {
    let channels = usize::from(delivery.audio_channels);
    let samples = usize::try_from(unit.samples).map_err(|e| e.to_string())?;
    let mut pcm = vec![0f32; samples * channels];
    for part in &unit.audio {
        let output = usize::from(part.output);
        let gain = part.gain();
        if output >= channels {
            return Err(qnc_ui_text::fill("render.channel_not_in_delivery", &[("channel", &(output + 1).to_string())]));
        }
        let mut decoder = open(&part.media, part.stream_index, part.start, None, false, None, decoders, access)?;
        let width = usize::from(part.stream_channels);
        let mut filled = 0usize;
        while filled < samples {
            if cancel.load(Ordering::Relaxed) {
                return Err(qnc_ui_text::text("render.prerender_interrupted").into());
            }
            let packet = decoder
                .next_packet()
                .map_err(|e| e.to_string())?
                .ok_or(qnc_ui_text::text("render.source_sound_ends_before_end_unit"))?;
            if packet.format
                != (DecodedFormat::Audio {
                    sample_rate_hz: unit.sample_rate,
                    channels: u32::from(part.stream_channels),
                    sample_format: "f32le".into(),
                })
            {
                return Err(qnc_ui_text::text("render.source_sound_story_format").into());
            }
            let at = position(packet.pts, packet.time_base, part.origin, i64::from(unit.sample_rate), 1)
                .ok_or(qnc_ui_text::text("render.sound_timestamp_sample_grid"))?;
            for (index, frame) in packet.bytes.chunks_exact(width * 4).enumerate() {
                let sample = at + index as u64;
                if sample < part.first_sample {
                    continue;
                }
                let slot = usize::try_from(sample - part.first_sample).map_err(|e| e.to_string())?;
                if slot >= samples {
                    break;
                }
                if slot != filled {
                    return Err(qnc_ui_text::text("render.source_sound_skips_samples").into());
                }
                let offset = usize::from(part.channel_index) * 4;
                // Two A2 sounds of a crossfade share the output: they add up (user 2026-10-21).
                pcm[slot * channels + output] += f32::from_le_bytes(frame[offset..offset + 4].try_into().expect("sample")) * gain * part.fade_at(slot, samples);
                filled += 1;
            }
        }
    }
    Ok(pcm.iter().flat_map(|sample| sample.to_le_bytes()).collect())
}

/// The unit's pictures encoded into `video` (picture only).
fn encode_picture(
    unit: &RenderUnit,
    delivery: &Delivery,
    decoders: &DecoderConfig,
    access: &MediaAccess,
    ffmpeg: &Path,
    video: &Path,
    cancel: &AtomicBool,
) -> Result<()> {
    let rate = format!("{}/{}", unit.program_rate.fps_num, unit.program_rate.fps_den);
    let size = format!("{}x{}", delivery.width, delivery.height);
    let mut command = Command::new(ffmpeg);
    command.args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"]);
    let mut source = None;
    match &unit.video {
        Some(part) => {
            let mut decoder = open(
                &part.media,
                part.stream_index,
                part.start,
                delivery.decode_pixel_format(),
                part.progressive,
                part.delivered_rate,
                decoders,
                access,
            )?;
            // The first picture of the unit tells the layout the encoder reads.
            let packet = next_picture(&mut decoder, part, part.first_frame, cancel)?;
            let DecodedFormat::Video { width, height, pixel_format } = &packet.format else {
                return Err(qnc_ui_text::text("render.source_picture_picture").into());
            };
            command.args(["-f", "rawvideo", "-pix_fmt", pixel_format, "-s", &format!("{width}x{height}"), "-r", &rate, "-i", "pipe:0"]);
            source = Some((part, decoder, packet.bytes));
        }
        // An Off without a picture is black, as the player shows it.
        None => {
            command.args(["-f", "lavfi", "-i", &format!("color=c=black:s={size}:r={rate}")]);
        }
    }
    command.args(["-map", "0:v:0", "-frames:v", &unit.frames().to_string()]);
    let (layout, args) = delivery.encoder_args(ffmpeg)?;
    command.args(["-vf", &format!("scale={size}:flags=bicubic,format={layout},setsar=1")]);
    command.args(args);
    command.args(UNIT_TAGS);
    command.args(["-video_track_timescale", &unit.program_rate.fps_num.to_string()]);
    command.args(["-an", "-f", "mov"]);
    command.arg(video);
    command.stdin(if source.is_some() { Stdio::piped() } else { Stdio::null() });
    command.stdout(Stdio::null()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| format!("Koder prerendera se ne moze pokrenuti: {e}"))?;
    let fed = match source {
        Some((part, mut decoder, first)) => {
            let mut stdin = child.stdin.take().expect("encoder stdin");
            let mut feed = || -> Result<()> {
                stdin.write_all(&first).map_err(|e| e.to_string())?;
                for frame in part.first_frame + 1..part.first_frame + unit.frames() {
                    let packet = next_picture(&mut decoder, part, frame, cancel)?;
                    stdin.write_all(&packet.bytes).map_err(|e| e.to_string())?;
                }
                Ok(())
            };
            let fed = feed();
            drop(stdin);
            fed
        }
        None => Ok(()),
    };
    if fed.is_err() {
        let _ = child.kill();
    }
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    // An encoder that stopped is the reason a feed broke: its own words, not the broken pipe.
    if !output.status.success() && !cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(qnc_ui_text::fill("render.encoder_error", &[("error", String::from_utf8_lossy(&output.stderr).trim())]));
    }
    fed?;
    Ok(())
}

/// The picture of source frame `frame` (pictures before the unit are dropped, as the
/// player drops them after a seek); a skipped picture is an error.
fn next_picture(
    decoder: &mut Decoder,
    part: &qnc_render_unit::VideoPart,
    frame: u64,
    cancel: &AtomicBool,
) -> Result<qnc_media_decode::DecodedPacket> {
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(qnc_ui_text::text("render.prerender_interrupted").into());
        }
        let packet = decoder
            .next_packet()
            .map_err(|e| e.to_string())?
            .ok_or(qnc_ui_text::text("render.source_picture_ends_before_end_unit"))?;
        let at = position(packet.pts, packet.time_base, part.origin, part.frame_rate.0, part.frame_rate.1)
            .ok_or(qnc_ui_text::text("render.picture_timestamp_picture_grid"))?;
        if at < frame {
            continue;
        }
        if at != frame {
            return Err(qnc_ui_text::text("render.decoder_skipped_source_picture").into());
        }
        return Ok(packet);
    }
}

/// The endpoint of a medium of the story: its QNC URI bound to a local root of this host
/// (a card source or the project folder), the same binding the player gets.
pub fn local_access(roots: &[(String, std::path::PathBuf)], media: &MediaRepresentation) -> io::Result<(CodecEndpoint, String)> {
    let invalid = |message: String| io::Error::new(io::ErrorKind::InvalidInput, message);
    let reference = qnc_media_stream::SourceReference::from_uri(&media.media_uri).map_err(|e| invalid(e.to_string()))?;
    let (source_uri, root) = roots
        .iter()
        .find(|(uri, _)| uri == reference.source_uri())
        .ok_or_else(|| invalid(format!("no binding for {}", reference.source_uri())))?;
    let root = root.canonicalize()?;
    let path = root.join(reference.relative_path()).canonicalize()?;
    if !path.starts_with(&root) || !path.is_file() {
        return Err(invalid("media escapes its source binding".into()));
    }
    let source = qnc_media_stream::LocalSource::new(source_uri, &root).map_err(|e| invalid(e.to_string()))?;
    let stamp = qnc_media_stream::MediaStream::local(&source, &media.media_uri)?.info().storage_stamp.clone();
    Ok((CodecEndpoint::for_local_file(path, &media.media_uri)?, stamp))
}

#[cfg(test)]
mod tests;
