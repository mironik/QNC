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

pub type Result<T> = std::result::Result<T, String>;

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
}

impl Delivery {
    pub fn from_export(export: &Value) -> Result<Self> {
        let text = |key: &str| export.get(key).and_then(Value::as_str).map(str::to_string);
        let number = |key: &str| export.get(key).and_then(Value::as_f64);
        let missing = |key: &str| format!("Exportne postavke nemaju {key}.");
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
        };
        if delivery.width == 0 || delivery.height == 0 || delivery.fps <= 0.0 || delivery.audio_channels == 0 {
            return Err("Exportne postavke imaju neispravan format.".into());
        }
        delivery.encoder_args()?;
        Ok(delivery)
    }

    /// The encoder of the delivery codec. A codec not named here is refused.
    fn encoder_args(&self) -> Result<Vec<String>> {
        let gop = self.fps.round().max(1.0) as u32;
        let args: Vec<&str> = match self.video_codec.as_str() {
            "h264" => vec![
                "-c:v", "libx264", "-preset", "veryfast", "-crf", "16", "-profile:v", "high",
                "-pix_fmt", "yuv420p", "-flags", "+cgop", "-sc_threshold", "0",
            ],
            "mpeg2_422_50mbit" => vec![
                "-c:v", "mpeg2video", "-profile:v", "0", "-level:v", "2", "-pix_fmt", "yuv422p",
                "-b:v", "50M", "-minrate", "50M", "-maxrate", "50M", "-bufsize", "17825792",
                "-flags", "+cgop", "-bf", "2", "-dc", "10", "-intra_vlc", "1", "-non_linear_quant", "1",
                "-qmin", "1", "-qmax", "12",
            ],
            other => return Err(format!("Prerender jos ne zna kodek predaje '{other}'.")),
        };
        let mut args: Vec<String> = args.into_iter().map(str::to_string).collect();
        args.extend(["-g".to_string(), gop.to_string()]);
        Ok(args)
    }

    /// Whether the unit's frames are the delivery's frames (same rate, progressive).
    fn check(&self, unit: &RenderUnit) -> Result<()> {
        if self.interlaced {
            return Err("Prerender jos ne radi isprepletenu predaju (field_order); prica je progresivna.".into());
        }
        let program = unit.program_rate.fps_num as f64 / unit.program_rate.fps_den.max(1) as f64;
        if (program - self.fps).abs() > 0.01 {
            return Err(format!("Stopa price {program:.3} nije stopa predaje {:.3}.", self.fps));
        }
        if unit.sample_rate != self.audio_sample_rate {
            return Err(format!(
                "Frekvencija zvuka price {} nije frekvencija predaje {}.",
                unit.sample_rate, self.audio_sample_rate
            ));
        }
        if unit.channels > self.audio_channels {
            return Err(format!(
                "Prica ima {} izlaznih kanala, predaja {}.",
                unit.channels, self.audio_channels
            ));
        }
        Ok(())
    }

    /// The fingerprint of a unit rendered in this delivery: its file name.
    pub fn file_name(&self, unit: &RenderUnit) -> String {
        format!("{}.mov", unit.fingerprint(&self.saved))
    }
}

/// Where a unit's media is read: its endpoint and storage stamp (given by the host that
/// resolves the QNC URI; a card endpoint is read only).
pub type MediaAccess<'a> = dyn Fn(&MediaRepresentation) -> io::Result<(CodecEndpoint, String)> + 'a;

/// Renders `unit` into `out` (a `.mov` file, overwritten). The sound of the unit waits in
/// the system's temporary folder while the encoder reads it, and goes after.
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
    let sound = sound_of(unit, delivery, decoders, access, cancel)?;
    let audio_file = temporary_sound(out);
    std::fs::write(&audio_file, sound).map_err(|e| format!("Zvuk prerendera se ne moze zapisati: {e}"))?;
    let result = encode(unit, delivery, decoders, access, ffmpeg, &audio_file, out, cancel);
    let _ = std::fs::remove_file(&audio_file);
    result
}

fn temporary_sound(out: &Path) -> PathBuf {
    let name = out.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    std::env::temp_dir().join(format!("qnc-prerender-{}-{name}.pcm", std::process::id()))
}

fn open(
    media: &MediaRepresentation,
    stream_index: u32,
    start: Option<qnc_media_metadata::Rational>,
    progressive: bool,
    delivered_rate: Option<qnc_media_metadata::FrameTimebase>,
    decoders: &DecoderConfig,
    access: &MediaAccess,
) -> Result<Decoder> {
    let (endpoint, stamp) = access(media).map_err(|e| format!("Medij prerendera nije dostupan: {e}"))?;
    let request = DecodeRequest {
        version: VERSION.into(),
        media: media.clone(),
        stream_index,
        start,
        output_pixel_format: None,
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
        if output >= channels {
            return Err(format!("Izlazni kanal {} nije u predaji.", output + 1));
        }
        let mut decoder = open(&part.media, part.stream_index, part.start, false, None, decoders, access)?;
        let width = usize::from(part.stream_channels);
        let mut filled = 0usize;
        while filled < samples {
            if cancel.load(Ordering::Relaxed) {
                return Err("Prerender je prekinut.".into());
            }
            let packet = decoder
                .next_packet()
                .map_err(|e| e.to_string())?
                .ok_or("Zvuk izvora zavrsava prije kraja jedinice.")?;
            if packet.format
                != (DecodedFormat::Audio {
                    sample_rate_hz: unit.sample_rate,
                    channels: u32::from(part.stream_channels),
                    sample_format: "f32le".into(),
                })
            {
                return Err("Zvuk izvora nije u formatu price.".into());
            }
            let at = position(packet.pts, packet.time_base, part.origin, i64::from(unit.sample_rate), 1)
                .ok_or("Vremenska oznaka zvuka nije na mrezi uzoraka.")?;
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
                    return Err("Zvuk izvora preskace uzorke.".into());
                }
                let offset = usize::from(part.channel_index) * 4;
                pcm[slot * channels + output] = f32::from_le_bytes(frame[offset..offset + 4].try_into().expect("sample"));
                filled += 1;
            }
        }
    }
    Ok(pcm.iter().flat_map(|sample| sample.to_le_bytes()).collect())
}

#[allow(clippy::too_many_arguments)]
fn encode(
    unit: &RenderUnit,
    delivery: &Delivery,
    decoders: &DecoderConfig,
    access: &MediaAccess,
    ffmpeg: &Path,
    audio_file: &Path,
    partial: &Path,
    cancel: &AtomicBool,
) -> Result<()> {
    let rate = format!("{}/{}", unit.program_rate.fps_num, unit.program_rate.fps_den);
    let size = format!("{}x{}", delivery.width, delivery.height);
    let mut command = Command::new(ffmpeg);
    command.args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"]);
    let mut video = None;
    let mut first = None;
    match &unit.video {
        Some(part) => {
            let mut decoder = open(&part.media, part.stream_index, part.start, part.progressive, part.delivered_rate, decoders, access)?;
            // The first picture of the unit tells the layout the encoder reads.
            let packet = next_picture(&mut decoder, part, part.first_frame, cancel)?;
            let DecodedFormat::Video { width, height, pixel_format } = &packet.format else {
                return Err("Slika izvora nije slika.".into());
            };
            command.args(["-f", "rawvideo", "-pix_fmt", pixel_format, "-s", &format!("{width}x{height}"), "-r", &rate, "-i", "pipe:0"]);
            first = Some(packet.bytes);
            video = Some(decoder);
        }
        // An Off without a picture is black, as the player shows it.
        None => {
            command.args(["-f", "lavfi", "-i", &format!("color=c=black:s={size}:r={rate}")]);
        }
    }
    command.args(["-f", "f32le", "-ar", &unit.sample_rate.to_string(), "-ac", &delivery.audio_channels.to_string()]);
    command.arg("-i").arg(audio_file);
    command.args(["-map", "0:v:0", "-map", "1:a:0", "-frames:v", &unit.frames().to_string()]);
    command.args(["-vf", &format!("scale={size}:flags=bicubic,setsar=1")]);
    command.args(delivery.encoder_args()?);
    command.args(["-c:a", "pcm_s24le", "-f", "mov"]);
    command.arg(partial);
    command.stdin(if video.is_some() { Stdio::piped() } else { Stdio::null() });
    command.stdout(Stdio::null()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| format!("Koder prerendera se ne moze pokrenuti: {e}"))?;
    let fed = match (&unit.video, video, first) {
        (Some(part), Some(mut decoder), Some(first)) => {
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
        _ => Ok(()),
    };
    if fed.is_err() {
        let _ = child.kill();
    }
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    fed?;
    if !output.status.success() {
        return Err(format!(
            "Koder prerendera je javio gresku: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
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
            return Err("Prerender je prekinut.".into());
        }
        let packet = decoder
            .next_packet()
            .map_err(|e| e.to_string())?
            .ok_or("Slika izvora zavrsava prije kraja jedinice.")?;
        let at = position(packet.pts, packet.time_base, part.origin, part.frame_rate.0, part.frame_rate.1)
            .ok_or("Vremenska oznaka slike nije na mrezi slika.")?;
        if at < frame {
            continue;
        }
        if at != frame {
            return Err("Dekoder je preskocio sliku izvora.".into());
        }
        return Ok(packet);
    }
}

#[cfg(test)]
mod tests;
