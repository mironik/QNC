//! The sound assistant (user 2026-11-06: "maybe an AI agent for the final fine sound"): from the
//! loudness measured with every clip's wave (100 ms segments, BS.1770) it proposes level curves
//! (docs/98 2.1) for the story, as an editor would ride the faders:
//!
//! - **ducking**: under speech on A1 every cover's A2 goes down with a ramp, only as far as it
//!   must to sit about 15 dB under the speech, and back up after it;
//! - **speech evened out**: a segment whose speech is louder or quieter than the story's (the
//!   median of the segments) is brought to it.
//!
//! An item the user gave a curve of his own is left alone. Pure: it reads and writes nothing; the
//! caller gives the placements and the measures and puts the curves where it plays them.

use qnc_program_loudness::ClipMeasure;
use qnc_program_waveform::Placement;

pub const MODULE_ID: &str = "qnc.module.sound-assist";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A 100 ms block louder than this is speech (LUFS of the block).
const SPEECH_FLOOR_LUFS: f64 = -40.0;
/// Speech closer than this to the next speech is one stretch; shorter than `SPEECH_LEAST` none.
const SPEECH_GAP_MS: u64 = 500;
const SPEECH_LEAST_MS: u64 = 300;
/// Under speech the ambience sits this much quieter (dB), lowered at most `DUCK_MOST`.
const UNDER_SPEECH_DB: f64 = 15.0;
const DUCK_MOST_DB: f64 = 20.0;
/// A duck smaller than this is not made (dB).
const DUCK_LEAST_DB: f64 = 2.0;
/// The ramp down and up of a duck (ms).
const RAMP_MS: u64 = 200;
/// A segment's speech this far from the story's is evened out (dB), at most `EVEN_MOST`.
const EVEN_LEAST_DB: f64 = 1.5;
const EVEN_MOST_DB: f64 = 10.0;

/// One item of the story the assistant may give a curve: a segment on A1 (lane 0) or a cover on
/// A2 (lane 1), where it plays and whether the user made a curve of his own for it.
#[derive(Debug, Clone)]
pub struct Item {
    pub lane: u8,
    pub at: Placement,
    pub own_curve: bool,
}

/// A curve the assistant proposes for `items[index]`: points on the placement's source frames
/// (tenths of a dB), sorted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub index: usize,
    pub points: Vec<(u64, i16)>,
}

/// The program frames per 100 ms block at `story_rate`, and the block of a frame.
struct Blocks {
    num: u64,
    den: u64,
}

impl Blocks {
    fn of(&self, frame: u64) -> usize {
        (frame * self.den * 10 / self.num.max(1)) as usize
    }

    fn frames(&self, ms: u64) -> u64 {
        (ms * self.num / (1000 * self.den.max(1))).max(1)
    }
}

/// The mean energy of `at` (with its level and curve) in every block of the program.
fn energies<'a>(total: u64, blocks: &Blocks, at: &Placement, clip: &impl Fn(&str) -> Option<ClipMeasure<'a>>) -> Option<Vec<f64>> {
    let measure = clip(&at.clip_id).filter(|measure| measure.duration_frames > 0)?;
    let channel = measure.channels.get(usize::from(at.channel))?.as_ref().filter(|channel| !channel.energies.is_empty())?;
    let mut sums = vec![0f64; blocks.of(total.saturating_sub(1)) + 1];
    let mut counts = vec![0u32; sums.len()];
    let (length, source) = (at.program_end.saturating_sub(at.program_start).max(1), at.source_out.saturating_sub(at.source_in));
    let segments = channel.energies.len() as u64;
    for frame in at.program_start..at.program_end.min(total) {
        let source_frame = at.source_in + (frame - at.program_start) * source / length;
        let index = (source_frame * segments / measure.duration_frames).min(segments - 1) as usize;
        let gain = f64::from(at.gain_at(source_frame as f64));
        sums[blocks.of(frame)] += f64::from(channel.energies[index]) * gain * gain;
        counts[blocks.of(frame)] += 1;
    }
    Some(sums.iter().zip(&counts).map(|(sum, count)| if *count == 0 { 0.0 } else { sum / f64::from(*count) }).collect())
}

/// Stretches of speech on A1, program frames `[from, to)`.
fn speech(a1: &[f64], blocks: &Blocks, total: u64) -> Vec<(u64, u64)> {
    let frame_of = |block: usize| ((block as u64) * blocks.num / (10 * blocks.den.max(1))).min(total);
    let mut stretches: Vec<(u64, u64)> = Vec::new();
    for (block, energy) in a1.iter().enumerate() {
        if qnc_loudness::lufs(*energy) <= SPEECH_FLOOR_LUFS {
            continue;
        }
        let (from, to) = (frame_of(block), frame_of(block + 1));
        match stretches.last_mut() {
            Some(last) if from.saturating_sub(last.1) <= blocks.frames(SPEECH_GAP_MS) => last.1 = to,
            _ => stretches.push((from, to)),
        }
    }
    stretches.retain(|(from, to)| to - from >= blocks.frames(SPEECH_LEAST_MS));
    stretches
}

/// The mean loudness (LUFS) of `energies` over the blocks of program frames `[from, to)`.
fn loudness(energies: &[f64], blocks: &Blocks, (from, to): (u64, u64)) -> Option<f64> {
    let (a, b) = (blocks.of(from), blocks.of(to.saturating_sub(1)).min(energies.len().saturating_sub(1)));
    let slice = energies.get(a..=b)?;
    let mean = slice.iter().sum::<f64>() / slice.len().max(1) as f64;
    (mean > 0.0).then(|| qnc_loudness::lufs(mean))
}

/// The source frame of program frame `frame` in `at`.
fn source_of(at: &Placement, frame: u64) -> u64 {
    let length = at.program_end.saturating_sub(at.program_start).max(1);
    let source = at.source_out.saturating_sub(at.source_in);
    at.source_in + frame.clamp(at.program_start, at.program_end).saturating_sub(at.program_start) * source / length
}

/// The assistant's curves for `items` of a program of `total` frames at `story_rate`.
pub fn propose<'a>(total: u64, story_rate: (u32, u32), items: &[Item], clip: impl Fn(&str) -> Option<ClipMeasure<'a>>) -> Vec<Proposal> {
    if total == 0 || story_rate.0 == 0 || story_rate.1 == 0 {
        return Vec::new();
    }
    let blocks = Blocks { num: u64::from(story_rate.0), den: u64::from(story_rate.1) };
    let measured: Vec<Option<Vec<f64>>> = items.iter().map(|item| if item.at.off { None } else { energies(total, &blocks, &item.at, &clip) }).collect();
    let mut a1 = vec![0f64; blocks.of(total.saturating_sub(1)) + 1];
    for (item, energies) in items.iter().zip(&measured) {
        if let (0, Some(energies)) = (item.lane, energies) {
            a1.iter_mut().zip(energies).for_each(|(sum, energy)| *sum += energy);
        }
    }
    let spoken = speech(&a1, &blocks, total);
    let ramp = blocks.frames(RAMP_MS);
    let mut proposals = Vec::new();
    // Speech evened out: each segment's speech to the median of the segments'.
    let speech_of = |item: &Item, energies: &[f64]| {
        let (start, end) = (item.at.program_start, item.at.program_end);
        let mut sum = 0.0;
        let mut count = 0usize;
        for (from, to) in spoken.iter().map(|(from, to)| (*from.max(&start), *to.min(&end))).filter(|(from, to)| from < to) {
            let (a, b) = (blocks.of(from), blocks.of(to - 1).min(energies.len().saturating_sub(1)));
            sum += energies[a..=b].iter().sum::<f64>();
            count += b + 1 - a;
        }
        (count > 0 && sum > 0.0).then(|| qnc_loudness::lufs(sum / count as f64))
    };
    let speeches: Vec<(usize, f64)> = items
        .iter()
        .zip(&measured)
        .enumerate()
        .filter(|(_, (item, _))| item.lane == 0)
        .filter_map(|(index, (item, energies))| Some((index, speech_of(item, energies.as_deref()?)?)))
        .collect();
    if speeches.len() > 1 {
        let mut levels: Vec<f64> = speeches.iter().map(|(_, level)| *level).collect();
        levels.sort_by(f64::total_cmp);
        let median = levels[levels.len() / 2];
        for (index, level) in &speeches {
            let offset = (median - level).clamp(-EVEN_MOST_DB, EVEN_MOST_DB);
            if offset.abs() >= EVEN_LEAST_DB && !items[*index].own_curve {
                proposals.push(Proposal { index: *index, points: vec![(items[*index].at.source_in, (offset * 10.0).round() as i16)] });
            }
        }
    }
    // Ducking: each cover under each stretch of speech it lies under.
    for (index, (item, energies)) in items.iter().zip(&measured).enumerate() {
        let (1, Some(energies), false) = (item.lane, energies, item.own_curve) else { continue };
        let (start, end) = (item.at.program_start, item.at.program_end);
        let mut points: Vec<(u64, i16)> = Vec::new();
        for (from, to) in spoken.iter().map(|(from, to)| (*from.max(&start), *to.min(&end))).filter(|(from, to)| from < to) {
            let (Some(voice), Some(ambience)) = (loudness(&a1, &blocks, (from, to)), loudness(energies, &blocks, (from, to))) else { continue };
            let duck = (ambience - (voice - UNDER_SPEECH_DB)).clamp(0.0, DUCK_MOST_DB);
            if duck < DUCK_LEAST_DB {
                continue;
            }
            let down = -(duck * 10.0).round() as i16;
            if from > start {
                points.push((source_of(&item.at, from.saturating_sub(ramp).max(start)), 0));
            }
            points.push((source_of(&item.at, from), down));
            points.push((source_of(&item.at, to), down));
            if to < end {
                points.push((source_of(&item.at, (to + ramp).min(end)), 0));
            }
        }
        points.sort_by_key(|point| point.0);
        points.dedup_by_key(|point| point.0);
        if !points.is_empty() {
            proposals.push(Proposal { index, points });
        }
    }
    proposals
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_loudness::ChannelLoudness;

    fn at(clip: &str, program: (u64, u64)) -> Placement {
        Placement {
            clip_id: clip.into(),
            channel: 0,
            program_start: program.0,
            program_end: program.1,
            source_in: 0,
            source_out: program.1 - program.0,
            level_db: 0,
            off: false,
            curve: Vec::new(),
        }
    }

    /// Loudness segments of a clip of `seconds` (50 fps): `level(second)` LUFS in each.
    fn channel(seconds: usize, level: impl Fn(usize) -> f64) -> ChannelLoudness {
        let energies: Vec<f32> = (0..seconds * 10).map(|segment| 10f64.powf((level(segment / 10) + 0.691) / 10.0) as f32).collect();
        ChannelLoudness { segment_ms: 100, true_peaks: vec![0.1; energies.len()], energies }
    }

    #[test]
    fn a_cover_ducks_under_speech_and_a_quiet_segment_is_evened_out() {
        // Speech on A1 from second 2 to 6 at -23 LUFS; the ambience on A2 at -20 LUFS throughout.
        let channels: std::collections::HashMap<&str, Vec<Option<ChannelLoudness>>> = std::collections::HashMap::from([
            ("voice", vec![Some(channel(10, |second| if (2..6).contains(&second) { -23.0 } else { -70.0 }))]),
            ("quiet", vec![Some(channel(10, |_| -33.0))]),
            ("amb", vec![Some(channel(10, |_| -20.0))]),
        ]);
        let clip = |id: &str| channels.get(id).map(|channels| ClipMeasure { channels, duration_frames: 500 });
        let items = vec![
            Item { lane: 0, at: at("voice", (0, 500)), own_curve: false },
            Item { lane: 1, at: at("amb", (0, 500)), own_curve: false },
        ];
        let proposals = propose(500, (50, 1), &items, clip);
        let duck = proposals.iter().find(|proposal| proposal.index == 1).expect("the cover is ducked");
        // -20 against -23 - 15 = -38: 18 dB down, from second 2 (frame 100) to 6 (frame 300).
        assert_eq!(duck.points, vec![(90, 0), (100, -180), (300, -180), (310, 0)]);
        // The user's own curve is left alone.
        let own = vec![Item { own_curve: true, ..items[1].clone() }, items[0].clone()];
        assert!(propose(500, (50, 1), &own, clip).iter().all(|proposal| proposal.index != 0));
        // Two segments: the quieter speech is brought to the louder (the median of two).
        let segments = vec![
            Item { lane: 0, at: at("voice", (0, 500)), own_curve: false },
            Item { lane: 0, at: Placement { program_start: 500, program_end: 1000, ..at("quiet", (0, 500)) }, own_curve: false },
        ];
        let evened = propose(1000, (50, 1), &segments, clip);
        assert_eq!(evened, vec![Proposal { index: 1, points: vec![(0, 100)] }]);
    }
}
