//! The loudness of a story's A1 and A2 (user 2026-10-24: normalised to the project's EBU R 128
//! targets). Every clip's channels were measured once, in 100 ms segments, in the pass of its
//! wave (`qnc-loudness`); here the segments (A1) and covers (A2) of the program take theirs by
//! their source IN/OUT, at their level (off is silent), into 100 ms segments of the program, and
//! those give the gated integrated loudness and the true peak of each output. It reads, plays
//! and writes nothing.

use qnc_loudness::ChannelLoudness;
use qnc_program_waveform::Placement;

pub const MODULE_ID: &str = "qnc.module.program-loudness";

/// The loudness of A1 and A2 of the program; `None` where nothing passes the gates.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ProgramLoudness {
    pub a1_lufs: Option<f64>,
    pub a2_lufs: Option<f64>,
    pub a1_peak_dbtp: Option<f64>,
    pub a2_peak_dbtp: Option<f64>,
    /// Every part that sounds had its clip measured (else the values are of what was).
    pub complete: bool,
}

// The values are finite (or none), so equality is total.
impl Eq for ProgramLoudness {}

/// What was measured of one clip: one entry per source channel (`None`: not measured) and
/// the clip's length in the frames its IN/OUT use.
pub struct ClipMeasure<'a> {
    pub channels: &'a [Option<ChannelLoudness>],
    pub duration_frames: u64,
}

/// The loudness of a program of `total_frames` at `story_rate` (frames per second as
/// num/den): segments on A1, covers on A2.
pub fn measure<'a>(
    total_frames: u64,
    story_rate: (u32, u32),
    segments: &[Placement],
    covers: &[Placement],
    clip: impl Fn(&str) -> Option<ClipMeasure<'a>>,
) -> ProgramLoudness {
    let (num, den) = (u64::from(story_rate.0), u64::from(story_rate.1));
    if total_frames == 0 || num == 0 || den == 0 {
        return ProgramLoudness::default();
    }
    // The 100 ms segment of the program a frame falls in.
    let bin = |frame: u64| (frame * den * 10 / num) as usize;
    let bins = bin(total_frames - 1) + 1;
    let mut frames_in = vec![0u32; bins];
    for frame in 0..total_frames {
        frames_in[bin(frame)] += 1;
    }
    let mut complete = true;
    let mut lane = |placements: &[Placement]| {
        let mut sums = vec![0f64; bins];
        let mut peak = 0f64;
        for at in placements.iter().filter(|at| !at.off && at.program_end > at.program_start) {
            let Some(measure) = clip(&at.clip_id).filter(|measure| measure.duration_frames > 0) else {
                complete = false;
                continue;
            };
            let Some(channel) = measure.channels.get(usize::from(at.channel)).and_then(Option::as_ref).filter(|c| !c.energies.is_empty()) else {
                complete = false;
                continue;
            };
            let gain = 10f64.powf(f64::from(at.level_db) / 20.0);
            let segments = channel.energies.len() as u64;
            let (length, source) = (at.program_end - at.program_start, at.source_out.saturating_sub(at.source_in));
            for frame in at.program_start..at.program_end.min(total_frames) {
                let source_frame = at.source_in + (frame - at.program_start) * source / length;
                let index = (source_frame * segments / measure.duration_frames).min(segments - 1) as usize;
                sums[bin(frame)] += f64::from(channel.energies[index]) * gain * gain;
                peak = peak.max(f64::from(channel.true_peaks[index]) * gain);
            }
        }
        let energies: Vec<f64> = sums.iter().zip(&frames_in).map(|(sum, count)| sum / f64::from((*count).max(1))).collect();
        (qnc_loudness::integrated(&energies), (peak > 0.0).then(|| qnc_loudness::peak_db(peak)))
    };
    let (a1_lufs, a1_peak_dbtp) = lane(segments);
    let (a2_lufs, a2_peak_dbtp) = lane(covers);
    ProgramLoudness { a1_lufs, a2_lufs, a1_peak_dbtp, a2_peak_dbtp, complete }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, seconds: f64, amplitude: f64) -> ChannelLoudness {
        let mut meter = qnc_loudness::Meter::new(rate).unwrap();
        for n in 0..(f64::from(rate) * seconds) as usize {
            meter.push((amplitude * (2.0 * std::f64::consts::PI * 997.0 * n as f64 / f64::from(rate)).sin()) as f32);
        }
        meter.finish()
    }

    fn at(clip: &str, program: (u64, u64), source: (u64, u64), level_db: i8, off: bool) -> Placement {
        Placement {
            clip_id: clip.into(),
            channel: 0,
            program_start: program.0,
            program_end: program.1,
            source_in: source.0,
            source_out: source.1,
            level_db,
            off,
        }
    }

    /// A clip of -23 LUFS heard 6 dB louder on A1 is -17 on A1; a silent lead-in (A1 off)
    /// does not pull it down; A2 has its own; a clip not measured makes it incomplete.
    #[test]
    fn the_program_takes_each_part_at_its_level_and_off_is_silent() {
        let channels = vec![Some(tone(48_000, 20.0, 0.1))];
        let quiet = vec![Some(tone(48_000, 20.0, 0.01))];
        let clip = |id: &str| match id {
            "loud" => Some(ClipMeasure { channels: &channels, duration_frames: 1000 }),
            "quiet" => Some(ClipMeasure { channels: &quiet, duration_frames: 1000 }),
            _ => None,
        };
        let segments = [at("loud", (0, 100), (0, 100), 0, true), at("loud", (100, 600), (200, 700), 6, false)];
        let covers = [at("quiet", (100, 600), (0, 500), 0, false)];
        let measured = measure(600, (50, 1), &segments, &covers, clip);
        assert!((measured.a1_lufs.unwrap() + 17.0).abs() < 0.15, "{measured:?}");
        assert!((measured.a2_lufs.unwrap() + 43.0).abs() < 0.15, "{measured:?}");
        assert!((measured.a1_peak_dbtp.unwrap() - (-20.0 + 6.0)).abs() < 0.3, "{measured:?}");
        assert!(measured.complete);
        let missing = measure(600, (50, 1), &[at("unknown", (0, 600), (0, 600), 0, false)], &[], clip);
        assert!(!missing.complete);
        assert_eq!(missing.a1_lufs, None);
    }
}
