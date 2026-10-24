//! The H.264 encoder of a prerender on this computer: a hardware encoder when one works
//! here (one short test encode, once per process), else the processor. Measured on the
//! camera original (2026-10-12, Intel Iris Xe): libx264 veryfast 33 pictures/s with the
//! decoding, Quick Sync 86 (the decoding is then the limit) and the processor stays free
//! for the player. Every unit of a process uses the one encoder found, so the units of a
//! story are put together without encoding again.

use std::{
    path::Path,
    process::{Command, Stdio},
    sync::OnceLock,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum H264 {
    IntelQsv,
    NvidiaNvenc,
    AmdAmf,
    AppleVideoToolbox,
    Software,
}

impl H264 {
    /// The order tried on this OS (hardware first, the processor last).
    fn order() -> &'static [H264] {
        if cfg!(target_os = "macos") {
            &[Self::AppleVideoToolbox, Self::Software]
        } else if cfg!(windows) {
            &[Self::IntelQsv, Self::NvidiaNvenc, Self::AmdAmf, Self::Software]
        } else {
            &[Self::NvidiaNvenc, Self::IntelQsv, Self::Software]
        }
    }

    /// The picture layout the encoder takes and its arguments: constant bitrate `mbps` Mbit/s
    /// (user 2026-10-24: a CBR delivery, 35 by default). No B pictures: decode order is
    /// picture order, so a unit's timestamps start at 0 and its description is known without
    /// probing it.
    pub(crate) fn args(self, mbps: u32) -> (&'static str, Vec<String>) {
        // A buffer of a quarter second, started half full: every unit (a story has many, some
        // only frames long) keeps to the rate from its first picture on, so the units put
        // together stay constant (measured 2026-10-24: with a one second buffer the joined
        // file swung 32-43 Mbit/s per second at 35, each unit spending its full buffer).
        let (rate, buffer, start) = (format!("{mbps}M"), format!("{}k", mbps * 250), format!("{}k", mbps * 125));
        let cbr = |args: &[&str]| {
            args.iter()
                .map(|arg| match *arg {
                    "RATE" => rate.clone(),
                    "BUFFER" => buffer.clone(),
                    "START" => start.clone(),
                    other => other.to_string(),
                })
                .collect::<Vec<String>>()
        };
        match self {
            // Quick Sync takes a constant bitrate when the target is also the floor and the ceiling.
            Self::IntelQsv => ("nv12", cbr(&["-c:v", "h264_qsv", "-preset", "medium", "-b:v", "RATE", "-minrate", "RATE", "-maxrate", "RATE", "-bufsize", "BUFFER", "-rc_init_occupancy", "START", "-look_ahead", "0", "-profile:v", "high", "-bf", "0"])),
            Self::NvidiaNvenc => ("yuv420p", cbr(&["-c:v", "h264_nvenc", "-preset", "p5", "-rc", "cbr", "-b:v", "RATE", "-maxrate", "RATE", "-bufsize", "BUFFER", "-rc_init_occupancy", "START", "-profile:v", "high", "-bf", "0"])),
            Self::AmdAmf => ("yuv420p", cbr(&["-c:v", "h264_amf", "-quality", "quality", "-rc", "cbr", "-b:v", "RATE", "-maxrate", "RATE", "-bufsize", "BUFFER", "-rc_init_occupancy", "START", "-profile:v", "high", "-bf", "0"])),
            Self::AppleVideoToolbox => ("yuv420p", cbr(&["-c:v", "h264_videotoolbox", "-b:v", "RATE", "-maxrate", "RATE", "-constant_bit_rate", "1", "-profile:v", "high", "-allow_sw", "0", "-bf", "0"])),
            Self::Software => ("yuv420p", cbr(&["-c:v", "libx264", "-preset", "superfast", "-b:v", "RATE", "-minrate", "RATE", "-maxrate", "RATE", "-bufsize", "BUFFER", "-rc_init_occupancy", "START", "-x264-params", "nal-hrd=cbr", "-profile:v", "high", "-flags", "+cgop", "-sc_threshold", "0", "-bf", "0"])),
        }
    }

    /// The first encoder of this OS that passes a test encode with `ffmpeg`.
    pub(crate) fn here(ffmpeg: &Path) -> H264 {
        static FOUND: OnceLock<H264> = OnceLock::new();
        *FOUND.get_or_init(|| {
            Self::order()
                .iter()
                .copied()
                .find(|encoder| *encoder == Self::Software || works(ffmpeg, *encoder))
                .unwrap_or(Self::Software)
        })
    }
}

/// A test encode of a few frames of a generated picture: does this encoder work here?
fn works(ffmpeg: &Path, encoder: H264) -> bool {
    let (layout, args) = encoder.args(35);
    Command::new(ffmpeg)
        .args(["-hide_banner", "-nostdin", "-v", "error", "-f", "lavfi", "-i", "testsrc2=size=1280x720:rate=50", "-frames:v", "5"])
        .args(["-vf", &format!("format={layout}")])
        .args(args)
        .args(["-f", "null", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_processor_is_the_last_one_tried() {
        assert_eq!(*H264::order().last().unwrap(), H264::Software);
    }
}
