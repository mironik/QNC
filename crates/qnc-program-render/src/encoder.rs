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

    /// The picture layout the encoder takes and its arguments (delivery quality).
    pub(crate) fn args(self) -> (&'static str, Vec<&'static str>) {
        match self {
            Self::IntelQsv => ("nv12", vec!["-c:v", "h264_qsv", "-preset", "medium", "-global_quality", "18", "-look_ahead", "0", "-profile:v", "high"]),
            Self::NvidiaNvenc => ("yuv420p", vec!["-c:v", "h264_nvenc", "-preset", "p5", "-rc", "vbr", "-cq", "18", "-profile:v", "high"]),
            Self::AmdAmf => ("yuv420p", vec!["-c:v", "h264_amf", "-quality", "quality", "-rc", "cqp", "-qp_i", "18", "-qp_p", "20", "-profile:v", "high"]),
            Self::AppleVideoToolbox => ("yuv420p", vec!["-c:v", "h264_videotoolbox", "-q:v", "70", "-profile:v", "high", "-allow_sw", "0"]),
            Self::Software => ("yuv420p", vec!["-c:v", "libx264", "-preset", "superfast", "-crf", "16", "-profile:v", "high", "-flags", "+cgop", "-sc_threshold", "0"]),
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
    let (layout, args) = encoder.args();
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
