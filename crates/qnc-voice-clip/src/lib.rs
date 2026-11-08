//! A voice-over as a clip (user 2026-10-28: "the sound we record gets a timecode in QNC and
//! a video title of its text"; the sound recorded elsewhere, a camera used as a recorder, is
//! brought the same way). The picture is the title card of the text at the project's size and
//! frame rate, the sound is the recordings as they are (24-bit PCM, their own sample rate), the
//! timecode is the time the first was recorded. Written as an MXF OP1a with XDCAM HD422 picture
//! (the Broadcast Player and every NLE read it) by the ffmpeg of the decoder catalog.
//!
//! Every recording is one mono channel of the clip, in the order given (user 2026-10-30: each
//! take of an OFF is one more channel of one record, and the editor chooses which one is used,
//! as A1 of a camera clip). They all start at the start of the clip; a shorter one ends in
//! silence, and the clip is as long as the longest.
//!
//! The clip is made in the system's temporary folder and then written over its file inside the
//! project folder: the project folder lets no file be deleted or renamed, but a file may be
//! written again, so a clip with one more channel replaces the one before under its own name.
//!
//! It knows no form, script or database; the caller says what goes where.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use qnc_media_metadata::ClipMetadata;

pub const MODULE_ID: &str = "qnc.module.voice-clip";

/// The most channels a voice-over clip has (as many as an XDCAM card records).
pub const MAX_CHANNELS: usize = 8;

pub type Result<T> = std::result::Result<T, String>;

/// What a voice clip is made of.
#[derive(Debug, Clone)]
pub struct VoiceClip<'a> {
    /// The sounds, one channel each (WAV recordings, or any sound files ffmpeg reads).
    pub sounds: &'a [PathBuf],
    /// The length of the longest sound, seconds (the clip is as long).
    pub seconds: f64,
    /// The text on its picture and the small line above it.
    pub text: &'a str,
    pub heading: &'a str,
    /// The picture is this numbered sequence of files (as ffmpeg names them, `dir/p_%02d.png`,
    /// from 1) instead of a title card: pictures QNC made itself (user 2026-11-08: the
    /// in-between pictures of a smooth cut).
    pub sequence: Option<&'a Path>,
    /// The project picture: its size and frame rate.
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    /// The project's scan (user 2026-10-30: "a project may be 50i or 50p"): `None`
    /// progressive, `Some(true)` interlaced upper field first, `Some(false)` lower first.
    pub fields: Option<bool>,
    /// The sample rate every channel is written at (the project's).
    pub sample_rate: u32,
    /// The timecode of its first frame (`HH:MM:SS:FF`).
    pub timecode: &'a str,
    /// The clip to write (inside `project_dir`; one there already is written over).
    pub out: &'a Path,
    pub project_dir: &'a Path,
}

/// Makes the clip; returns its path.
pub fn make(clip: &VoiceClip<'_>) -> Result<PathBuf> {
    inside(clip.project_dir, clip.out)?;
    if clip.sounds.is_empty() || clip.sounds.len() > MAX_CHANNELS {
        return Err(qnc_ui_text::fill("voice.clip_channels", &[("most", &MAX_CHANNELS.to_string())]));
    }
    if let Some(missing) = clip.sounds.iter().find(|sound| !sound.is_file()) {
        return Err(format!("{}: {}", missing.display(), qnc_ui_text::text("voice.take_missing")));
    }
    if !(clip.fps.is_finite() && clip.fps > 0.0) {
        return Err(qnc_ui_text::text("voice.clip_rate").into());
    }
    if !(clip.seconds.is_finite() && clip.seconds > 0.0) {
        return Err(qnc_ui_text::text("voice.clip_empty").into());
    }
    let ffmpeg = qnc_decoder_catalog::installed_deployment().map_err(|e| e.to_string())?.executable;
    // The card and the clip are made in the system's temporary folder; only the finished clip
    // goes into the project.
    let stem = format!("qnc-voice-{}-{}", std::process::id(), clip.out.file_stem().and_then(|s| s.to_str()).unwrap_or("clip"));
    let card = std::env::temp_dir().join(format!("{stem}.png"));
    let made = std::env::temp_dir().join(format!("{stem}.mxf"));
    if clip.sequence.is_none() {
        qnc_title_card::write_png(&card, clip.text, clip.heading, clip.width, clip.height)?;
    }
    let result = run(&ffmpeg, clip, &card, &made).and_then(|()| std::fs::copy(&made, clip.out).map(drop).map_err(|e| format!("{}: {e}", clip.out.display())));
    let _ = std::fs::remove_file(&card);
    let _ = std::fs::remove_file(&made);
    result.map(|()| clip.out.to_path_buf())
}

/// The record of the clip `make` writes, as the catalog keeps it (user 2026-10-30: the voice
/// record is a clip of the project the Broadcast Player plays). Every fact is what this module
/// asks the encoder for, so nothing is probed: stream 0 the XDCAM HD422 picture (BT.709, tv
/// range, progressive, constant rate, its exact frame count), then one mono 24-bit PCM stream
/// per sound, all as long as the picture and starting with it; the timecode of the first frame.
pub fn describe(clip: &VoiceClip<'_>, clip_id: &str, media_uri: &str) -> Result<ClipMetadata> {
    use qnc_media_metadata::*;
    if !(clip.fps.is_finite() && clip.fps > 0.0) || clip.sample_rate == 0 {
        return Err(qnc_ui_text::text("voice.clip_rate").into());
    }
    let rate = qnc_frame_timebase::rational_fps(clip.fps);
    let frames = frames(clip.seconds, clip.fps);
    fn fact<T>(value: T) -> Option<Fact<T>> {
        Some(Fact { value, evidence_id: EVIDENCE.into(), locator: "qnc-voice-clip".into() })
    }
    let ticks = |per_second: i64| (frames as i64 * per_second * rate.fps_den + rate.fps_num - 1) / rate.fps_num;
    let known = |text: &str| Signal::Known(text.to_string());
    let picture = MediaStream {
        index: fact(0),
        codec: fact(known("mpeg2video")),
        profile: fact("4:2:2".to_string()),
        time_base: fact(Rational { numerator: rate.fps_den, denominator: rate.fps_num }),
        start_pts: fact(0),
        duration_ts: fact(frames as i64),
        details: StreamDetails::Video(Box::new(VideoMetadata {
            width: fact(clip.width),
            height: fact(clip.height),
            frame_rate: fact(rate),
            frame_rate_mode: fact(FrameRateMode::Constant),
            frame_count: fact(FrameCount::Exact(frames)),
            scan_mode: fact(match clip.fields {
                None => ScanMode::Progressive,
                Some(true) => ScanMode::InterlacedTopFieldFirst,
                Some(false) => ScanMode::InterlacedBottomFieldFirst,
            }),
            pixel_format: fact("yuv422p".to_string()),
            sample_aspect_ratio: fact(Rational { numerator: 1, denominator: 1 }),
            rotation_degrees: fact(Signal::Unspecified),
            color: ColorMetadata { primaries: fact(known("bt709")), transfer: fact(known("bt709")), matrix: fact(known("bt709")), range: fact(known("tv")) },
        })),
    };
    let sound = |index: u32| MediaStream {
        index: fact(index),
        codec: fact(known("pcm_s24le")),
        profile: None,
        time_base: fact(Rational { numerator: 1, denominator: clip.sample_rate as i64 }),
        start_pts: fact(0),
        duration_ts: fact(ticks(clip.sample_rate as i64)),
        details: StreamDetails::Audio(Box::new(AudioMetadata {
            sample_rate_hz: fact(clip.sample_rate),
            channels: fact(1),
            sample_format: fact(known("s32")),
            channel_layout: fact(Signal::Unspecified),
            bits_per_sample: fact(24),
        })),
    };
    let streams = std::iter::once(picture).chain((1..=clip.sounds.len() as u32).map(sound)).collect();
    Ok(ClipMetadata {
        contract_id: CONTRACT_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        clip_id: clip_id.into(),
        evidence: vec![Evidence { id: EVIDENCE.into(), kind: EvidenceKind::QncWritten, document_uri: media_uri.into(), media_uri: media_uri.into() }],
        original: MediaRepresentation {
            media_uri: media_uri.into(),
            container: fact("mxf".to_string()),
            duration_seconds: fact(Rational { numerator: frames as i64 * rate.fps_den, denominator: rate.fps_num }),
            streams_complete: fact(true),
            streams,
            tags: [("timecode".to_string(), fact(clip.timecode.to_string()).unwrap())].into(),
        },
        proxy: None,
    })
}

const EVIDENCE: &str = "qnc-written";

/// The frames of the clip: the longest sound, rounded up to a whole frame.
pub fn frames(seconds: f64, fps: f64) -> u64 {
    (seconds * fps - 1e-6).ceil().max(1.0) as u64
}

fn run(ffmpeg: &Path, clip: &VoiceClip<'_>, card: &Path, out: &Path) -> Result<()> {
    let rate = rate_text(clip.fps);
    let frames = frames(clip.seconds, clip.fps);
    let length = format!("{:.6}", frames as f64 / clip.fps);
    let mut command = Command::new(ffmpeg);
    command.args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"]);
    match clip.sequence {
        Some(pattern) => command.args(["-start_number", "1", "-framerate", &rate]).arg("-i").arg(pattern),
        None => command.args(["-loop", "1", "-framerate", &rate]).arg("-i").arg(card),
    };
    for sound in clip.sounds {
        command.arg("-i").arg(sound);
    }
    // Every sound starts with the clip and is padded with silence to its end.
    let pads: Vec<String> = (0..clip.sounds.len()).map(|index| format!("[{}:a:0]apad[a{index}]", index + 1)).collect();
    command.args(["-filter_complex", &pads.join(";")]).args(["-map", "0:v:0"]);
    for index in 0..clip.sounds.len() {
        command.args(["-map", &format!("[a{index}]")]);
    }
    command
        .args(["-frames:v", &frames.to_string(), "-t", &length])
        .args(["-c:v", "mpeg2video", "-pix_fmt", "yuv422p", "-profile:v", "0", "-level:v", "2"])
        .args(["-b:v", "50M", "-minrate", "50M", "-maxrate", "50M", "-bufsize", "17825792", "-rc_init_occupancy", "17825792"])
        .args(["-g", "12", "-bf", "2", "-sc_threshold", "1000000000", "-dc", "10", "-intra_vlc", "1", "-non_linear_quant", "1", "-qmin", "1", "-qmax", "12"])
        .args(["-color_primaries", "bt709", "-color_trc", "bt709", "-colorspace", "bt709", "-vf", "format=yuv422p,setsar=1"]);
    if let Some(top) = clip.fields {
        // Interlaced as the project: field DCT and motion, its field order (XDCAM HD422 50i).
        command.args(["-flags", "+ildct+ilme", "-top", if top { "1" } else { "0" }, "-field_order", if top { "tt" } else { "bb" }]);
    }
    command
        .args(["-c:a", "pcm_s24le", "-ac", "1", "-ar", &clip.sample_rate.to_string()])
        .args(["-timecode", clip.timecode])
        .args(["-f", "mxf"])
        .arg(out);
    let output = command.output().map_err(|e| format!("{}: {e}", ffmpeg.display()))?;
    if output.status.success() && out.is_file() {
        return Ok(());
    }
    Err(qnc_ui_text::fill("voice.clip_failed", &[("error", String::from_utf8_lossy(&output.stderr).trim())]))
}

/// A frame rate as ffmpeg takes it (`50`, `30000/1001`).
fn rate_text(fps: f64) -> String {
    for (num, den) in [(24000u32, 1001u32), (30000, 1001), (60000, 1001)] {
        if (fps - num as f64 / den as f64).abs() < 0.001 {
            return format!("{num}/{den}");
        }
    }
    format!("{}", fps.round() as u32)
}

fn inside(project_dir: &Path, path: &Path) -> Result<()> {
    let plain = path.components().all(|part| !matches!(part, std::path::Component::ParentDir));
    if plain && path.starts_with(project_dir) {
        Ok(())
    } else {
        Err(qnc_ui_text::text("import.writing_allowed_only_into_project_folder").into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip<'a>(sounds: &'a [PathBuf], out: &'a Path, project_dir: &'a Path) -> VoiceClip<'a> {
        VoiceClip { sounds, seconds: 2.0, text: "x", heading: "", sequence: None, width: 1920, height: 1080, fps: 50.0, fields: None, sample_rate: 48_000, timecode: "10:00:00:00", out, project_dir }
    }

    #[test]
    fn rates_are_given_as_ffmpeg_takes_them() {
        assert_eq!(rate_text(50.0), "50");
        assert_eq!(rate_text(25.0), "25");
        assert_eq!(rate_text(29.97), "30000/1001");
    }

    #[test]
    fn the_clip_is_as_long_as_the_longest_sound_in_whole_frames() {
        assert_eq!(frames(2.0, 50.0), 100);
        assert_eq!(frames(2.001, 50.0), 101);
        assert_eq!(frames(0.001, 25.0), 1);
    }

    #[test]
    fn a_clip_outside_the_project_no_sound_or_too_many_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("p");
        let sound = vec![PathBuf::from("a.wav")];
        assert!(make(&clip(&sound, &dir.path().join("x.mxf"), &project)).is_err());
        assert!(make(&clip(&[], &project.join("x.mxf"), &project)).is_err());
        let many = vec![PathBuf::from("a.wav"); MAX_CHANNELS + 1];
        assert!(make(&clip(&many, &project.join("x.mxf"), &project)).is_err());
    }
}
