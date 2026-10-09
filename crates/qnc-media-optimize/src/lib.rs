//! The optimized copy of an original (user 2026-10-09): a laptop without a GPU that
//! decodes XAVC (H.264 10-bit 4:2:2) edits on an H.264 8-bit 4:2:0 copy of the same size,
//! which any GPU decodes. The copy keeps every frame, the timecode and the sound of the
//! original bit for bit (`-c:a copy`, every channel); only the picture is coded again. The
//! export takes the original. Procedure from v5 (`qnc-media-ffmpeg/src/proxy.rs`: encoder
//! chosen from those the computer has, H.264 without B frames, constant frames, then the
//! frames checked), with the QNC decisions: full sound instead of AAC stereo, exact frames
//! instead of +/-2, standard H.264 High (no nonstandard formats).
//!
//! This module only makes the file. Checking it (one probe, frames against the saved
//! original) and writing the record belong to the import.

use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

pub const MODULE_ID: &str = "qnc.module.media-optimize";
pub const VERSION: &str = "0.1.0";

/// The H.264 encoders the optimized copy may use, in the order tried on this OS. A GPU
/// encoder is used only when a test encode works on this computer (an encoder ffmpeg
/// lists may still have no device).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoder {
    IntelQsv,
    NvidiaNvenc,
    AmdAmf,
    AppleVideoToolbox,
    Software,
}

impl Encoder {
    pub fn label(self) -> &'static str {
        match self {
            Self::IntelQsv => "h264_qsv",
            Self::NvidiaNvenc => "h264_nvenc",
            Self::AmdAmf => "h264_amf",
            Self::AppleVideoToolbox => "h264_videotoolbox",
            Self::Software => "libx264",
        }
    }

    fn order() -> &'static [Encoder] {
        if cfg!(target_os = "macos") {
            &[Self::AppleVideoToolbox, Self::Software]
        } else if cfg!(windows) {
            &[Self::IntelQsv, Self::NvidiaNvenc, Self::AmdAmf, Self::Software]
        } else {
            &[Self::NvidiaNvenc, Self::IntelQsv, Self::Software]
        }
    }

    /// 8-bit 4:2:0 H.264 High, a key frame every 10 pictures, no B frames, about
    /// 35 Mbit/s for 1080p50.
    fn video_args(self) -> Vec<&'static str> {
        let rate = ["-g", "10", "-bf", "0", "-b:v", "35M", "-maxrate", "40M", "-bufsize", "70M"];
        let mut args: Vec<&'static str> = match self {
            Self::IntelQsv => vec!["-vf", "format=nv12", "-c:v", "h264_qsv", "-preset", "veryfast", "-look_ahead", "0"],
            Self::NvidiaNvenc => vec!["-pix_fmt", "yuv420p", "-c:v", "h264_nvenc", "-preset", "p4"],
            Self::AmdAmf => vec!["-pix_fmt", "yuv420p", "-c:v", "h264_amf", "-quality", "speed"],
            Self::AppleVideoToolbox => vec!["-pix_fmt", "yuv420p", "-c:v", "h264_videotoolbox", "-allow_sw", "0"],
            Self::Software => vec!["-pix_fmt", "yuv420p", "-c:v", "libx264", "-preset", "veryfast"],
        };
        args.extend(["-profile:v", "high"]);
        args.extend(rate);
        args
    }
}

/// Makes optimized copies with one encoder chosen for this computer.
#[derive(Debug, Clone)]
pub struct Optimizer {
    ffmpeg: PathBuf,
    encoder: Encoder,
}

impl Optimizer {
    /// The ffmpeg of the installed decoder catalog (the same toolchain the player uses)
    /// and the first encoder of this OS that passes a short test encode.
    pub fn installed() -> Result<Self, String> {
        let deployment = qnc_decoder_catalog::installed_deployment().map_err(|e| e.to_string())?;
        Self::with_ffmpeg(deployment.executable)
    }

    pub fn with_ffmpeg(ffmpeg: PathBuf) -> Result<Self, String> {
        let encoder = Encoder::order()
            .iter()
            .copied()
            .find(|encoder| works(&ffmpeg, *encoder))
            .ok_or("Nijedan H.264 enkoder ne radi na ovom racunalu.")?;
        Ok(Self { ffmpeg, encoder })
    }

    pub fn encoder(&self) -> Encoder {
        self.encoder
    }

    /// Writes the optimized copy of `source` to `dest` (a QuickTime file). The process runs
    /// below normal priority so playback keeps the processor; `cancel` stops it. No other
    /// encoder is tried when this one fails.
    pub fn make(&self, source: &Path, dest: &Path, cancel: &AtomicBool) -> Result<(), String> {
        let mut command = Command::new(&self.ffmpeg);
        command
            .args(["-y", "-hide_banner", "-nostdin", "-v", "error", "-i"])
            .arg(source)
            .args(["-map", "0:v:0", "-map", "0:a?", "-map_metadata", "0"])
            .args(self.encoder.video_args())
            .args(["-fps_mode", "passthrough", "-c:a", "copy", "-f", "mov"])
            .arg(dest)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        below_normal(&mut command);
        let child = command.spawn().map_err(|e| format!("ffmpeg optimize: {e}"))?;
        wait(child, cancel)
    }
}

/// A test encode of a few frames of a generated picture: does this encoder work here?
fn works(ffmpeg: &Path, encoder: Encoder) -> bool {
    let mut command = Command::new(ffmpeg);
    command
        .args(["-hide_banner", "-nostdin", "-v", "error", "-f", "lavfi", "-i", "testsrc2=size=1280x720:rate=50", "-frames:v", "5"])
        .args(encoder.video_args())
        .args(["-f", "null", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    below_normal(&mut command);
    let Ok(child) = command.spawn() else { return false };
    wait(child, &AtomicBool::new(false)).is_ok()
}

fn wait(mut child: Child, cancel: &AtomicBool) -> Result<(), String> {
    let mut stderr = child.stderr.take();
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Izrada optimizirane kopije je prekinuta.".into());
        }
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) if status.success() => return Ok(()),
            Some(status) => {
                let mut text = String::new();
                if let Some(stderr) = stderr.as_mut() {
                    let _ = stderr.take(4096).read_to_string(&mut text);
                }
                return Err(format!("ffmpeg optimize: {status} {}", text.trim()));
            }
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

/// The copy is background work: below normal priority (Windows), so the player and the
/// desktop keep the processor. Other OSes keep the default (a plain user may lower it,
/// but the process here is not niced; nothing is guessed).
fn below_normal(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        command.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
    }
    #[cfg(not(windows))]
    let _ = command;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_encoder_makes_8_bit_4_2_0_high_with_short_gop_and_no_b_frames() {
        for encoder in [Encoder::IntelQsv, Encoder::NvidiaNvenc, Encoder::AmdAmf, Encoder::AppleVideoToolbox, Encoder::Software] {
            let args = encoder.video_args().join(" ");
            assert!(args.contains("-g 10") && args.contains("-bf 0"), "{args}");
            assert!(args.contains("-profile:v high"), "{args}");
            assert!(args.contains("nv12") || args.contains("yuv420p"), "{args}");
        }
        assert_eq!(*Encoder::order().last().unwrap(), Encoder::Software, "the processor is the last one tried");
    }

    /// Makes an optimized copy of `QNC_OPTIMIZE_SOURCE` next to it (a local file, never a
    /// card: the copy is written beside the source). Run with `--ignored --nocapture`.
    #[test]
    #[ignore = "needs ffmpeg and a local source file in QNC_OPTIMIZE_SOURCE"]
    fn makes_a_copy_of_a_given_file() {
        let source = PathBuf::from(std::env::var("QNC_OPTIMIZE_SOURCE").unwrap());
        let optimizer = Optimizer::installed().unwrap();
        let dest = source.with_extension("optimized.mov");
        let started = std::time::Instant::now();
        optimizer.make(&source, &dest, &AtomicBool::new(false)).unwrap();
        println!("encoder={} ms={}", optimizer.encoder().label(), started.elapsed().as_millis());
        assert!(dest.is_file());
    }
}
