//! A voice-over as a clip (user 2026-10-28: "the sound we record gets a timecode in QNC and
//! a video title of its text"; the sound recorded elsewhere, a camera used as a recorder, is
//! brought the same way). The picture is the title card of the text at the project's size and
//! frame rate, the sound is the recording as it is (24-bit PCM, its own sample rate), the
//! timecode is the time it was recorded. Written as an MXF OP1a with XDCAM HD422 picture (the
//! Broadcast Player and every NLE read it) by the ffmpeg of the decoder catalog, straight
//! under its final name inside the project folder (the project folder does not let a file be
//! renamed).
//!
//! It knows no form, script or database; the caller says what goes where.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub const MODULE_ID: &str = "qnc.module.voice-clip";

pub type Result<T> = std::result::Result<T, String>;

/// What a voice clip is made of.
#[derive(Debug, Clone)]
pub struct VoiceClip<'a> {
    /// The sound (a WAV recording, or any sound file ffmpeg reads).
    pub sound: &'a Path,
    /// The text on its picture and the small line above it.
    pub text: &'a str,
    pub heading: &'a str,
    /// The project picture: its size and frame rate.
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    /// The timecode of its first frame (`HH:MM:SS:FF`).
    pub timecode: &'a str,
    /// The clip to write (inside `project_dir`, not existing yet).
    pub out: &'a Path,
    pub project_dir: &'a Path,
}

/// Makes the clip; returns its path.
pub fn make(clip: &VoiceClip<'_>) -> Result<PathBuf> {
    inside(clip.project_dir, clip.out)?;
    if clip.out.exists() {
        return Err(format!("{}: {}", clip.out.display(), qnc_ui_text::text("voice.clip_exists")));
    }
    if !(clip.fps.is_finite() && clip.fps > 0.0) {
        return Err(qnc_ui_text::text("voice.clip_rate").into());
    }
    let ffmpeg = qnc_decoder_catalog::installed_deployment().map_err(|e| e.to_string())?.executable;
    // The card goes through the system's temporary folder; only the clip is the project's.
    let temp = std::env::temp_dir().join(format!("qnc-voice-card-{}-{}.png", std::process::id(), clip.out.file_stem().and_then(|s| s.to_str()).unwrap_or("card")));
    qnc_title_card::write_png(&temp, clip.text, clip.heading, clip.width, clip.height)?;
    let result = run(&ffmpeg, clip, &temp);
    let _ = std::fs::remove_file(&temp);
    result.map(|()| clip.out.to_path_buf())
}

fn run(ffmpeg: &Path, clip: &VoiceClip<'_>, card: &Path) -> Result<()> {
    let rate = rate_text(clip.fps);
    let mut command = Command::new(ffmpeg);
    command
        .args(["-hide_banner", "-loglevel", "error", "-nostdin"])
        .args(["-loop", "1", "-framerate", &rate])
        .arg("-i")
        .arg(card)
        .arg("-i")
        .arg(clip.sound)
        .args(["-map", "0:v:0", "-map", "1:a:0", "-shortest"])
        .args(["-c:v", "mpeg2video", "-pix_fmt", "yuv422p", "-profile:v", "0", "-level:v", "2"])
        .args(["-b:v", "50M", "-minrate", "50M", "-maxrate", "50M", "-bufsize", "17825792", "-rc_init_occupancy", "17825792"])
        .args(["-g", "12", "-bf", "2", "-sc_threshold", "1000000000", "-dc", "10", "-intra_vlc", "1", "-non_linear_quant", "1", "-qmin", "1", "-qmax", "12"])
        .args(["-color_primaries", "bt709", "-color_trc", "bt709", "-colorspace", "bt709", "-vf", "format=yuv422p,setsar=1"])
        .args(["-c:a", "pcm_s24le", "-ac", "1"])
        .args(["-timecode", clip.timecode])
        .args(["-f", "mxf"])
        .arg(clip.out);
    let output = command.output().map_err(|e| format!("{}: {e}", ffmpeg.display()))?;
    if output.status.success() && clip.out.is_file() {
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

    #[test]
    fn rates_are_given_as_ffmpeg_takes_them() {
        assert_eq!(rate_text(50.0), "50");
        assert_eq!(rate_text(25.0), "25");
        assert_eq!(rate_text(29.97), "30000/1001");
    }

    #[test]
    fn a_clip_outside_the_project_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let clip = VoiceClip { sound: Path::new("a.wav"), text: "x", heading: "", width: 1920, height: 1080, fps: 50.0, timecode: "10:00:00:00", out: &dir.path().join("x.mxf"), project_dir: &dir.path().join("p") };
        assert!(make(&clip).is_err());
    }
}
