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
    if total_frames == 0 || story_rate.0 == 0 || story_rate.1 == 0 {
        return ProgramLoudness::default();
    }
    let mut complete = true;
    let mut lane = |placements: &[Placement]| {
        let (lufs, peak, whole) = lane_loudness(total_frames, story_rate, placements, &clip);
        complete &= whole;
        (lufs, peak)
    };
    let (a1_lufs, a1_peak_dbtp) = lane(segments);
    let (a2_lufs, a2_peak_dbtp) = lane(covers);
    ProgramLoudness { a1_lufs, a2_lufs, a1_peak_dbtp, a2_peak_dbtp, complete }
}

/// The gated integrated loudness and true peak of `placements` on one lane of a program of
/// `total_frames`, and whether every part that sounds had its clip measured.
fn lane_loudness<'a>(
    total_frames: u64,
    story_rate: (u32, u32),
    placements: &[Placement],
    clip: &impl Fn(&str) -> Option<ClipMeasure<'a>>,
) -> (Option<f64>, Option<f64>, bool) {
    let (num, den) = (u64::from(story_rate.0), u64::from(story_rate.1));
    // The 100 ms segment of the program a frame falls in.
    let bin = |frame: u64| (frame * den * 10 / num) as usize;
    let bins = bin(total_frames - 1) + 1;
    let mut frames_in = vec![0u32; bins];
    for frame in 0..total_frames {
        frames_in[bin(frame)] += 1;
    }
    let mut complete = true;
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
        let gain = qnc_gain::db_to_gain(f64::from(at.level_db));
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
    (qnc_loudness::integrated(&energies), (peak > 0.0).then(|| qnc_loudness::peak_db(peak)), complete)
}

/// The most one cover is raised to its target (user 2026-10-24: a quiet ambience raised more
/// carries its noise up too).
pub const COVER_GAIN_MAX_DB: f64 = 12.0;

/// The rule every cover's A2 is brought to: the project's A2 target, its tolerance and the
/// true peak limit (EBU R 128 by default).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoverRule {
    pub target_lufs: f64,
    pub tolerance_lu: f64,
    pub limit_dbtp: f64,
}

/// The whole dB one cover (`placement`, at the level the user gave it) is raised or lowered by
/// so that it alone stays within the rule (user 2026-10-24: no cover may stand out of the
/// rule): 0 within the tolerance; raised at most `COVER_GAIN_MAX_DB` and never over the peak
/// limit. `None` when the cover is off, silent or its clip not measured: it stays as edited.
pub fn cover_gain<'a>(
    story_rate: (u32, u32),
    placement: &Placement,
    rule: CoverRule,
    clip: impl Fn(&str) -> Option<ClipMeasure<'a>>,
) -> Option<i8> {
    if placement.off || placement.program_end <= placement.program_start || story_rate.0 == 0 || story_rate.1 == 0 {
        return None;
    }
    // Measured on its own, from its first frame: no silence of the program before it counts.
    let alone = Placement { program_start: 0, program_end: placement.program_end - placement.program_start, ..placement.clone() };
    let (lufs, peak, complete) = lane_loudness(alone.program_end, story_rate, std::slice::from_ref(&alone), &clip);
    let (lufs, peak) = (lufs?, peak?);
    if !complete {
        return None;
    }
    let gain = qnc_loudness::export_gain(Some(lufs), peak, rule.target_lufs, rule.tolerance_lu, rule.limit_dbtp);
    Some(gain.min(COVER_GAIN_MAX_DB).round().clamp(-60.0, COVER_GAIN_MAX_DB) as i8)
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
    /// A cover quieter than the rule is raised to it, at most 12 dB; one within the tolerance
    /// stays; one louder is lowered; an off or unmeasured cover stays as edited.
    #[test]
    fn every_cover_is_brought_into_the_rule_on_its_own() {
        let near = vec![Some(tone(48_000, 20.0, 0.1))]; // -23 LUFS
        let quiet = vec![Some(tone(48_000, 20.0, 0.03))]; // about -33.5
        let very_quiet = vec![Some(tone(48_000, 20.0, 0.003))]; // about -53.5
        let clip = |id: &str| match id {
            "near" => Some(ClipMeasure { channels: &near, duration_frames: 1000 }),
            "quiet" => Some(ClipMeasure { channels: &quiet, duration_frames: 1000 }),
            "very_quiet" => Some(ClipMeasure { channels: &very_quiet, duration_frames: 1000 }),
            _ => None,
        };
        let rule = CoverRule { target_lufs: -23.0, tolerance_lu: 0.5, limit_dbtp: -1.0 };
        let gain = |id: &str, level_db: i8, off: bool| cover_gain((50, 1), &at(id, (100, 400), (0, 300), level_db, off), rule, clip);
        assert_eq!(gain("near", 0, false), Some(0), "within the tolerance it stays");
        assert_eq!(gain("quiet", 0, false), Some(10), "raised to the target");
        assert_eq!(gain("very_quiet", 0, false), Some(12), "at most 12 dB");
        assert_eq!(gain("near", 6, false), Some(-6), "a cover made too loud is lowered");
        assert_eq!(gain("near", 0, true), None, "off stays off");
        assert_eq!(gain("unknown", 0, false), None, "not measured: as edited");
    }
}
