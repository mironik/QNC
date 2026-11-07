//! The level curve of an output channel (user 2026-11-07: the level line drawn on the source
//! timeline is heard in the clip's preview): points at absolute samples of the playout and the
//! level there in dB, a line between them, before the first and after the last theirs. Applied
//! where the samples go to the device, so a change is heard in the next callback; the callback
//! allocates nothing and takes no lock.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// The most points of one curve (as a story's).
pub const CURVE_POINTS: usize = 256;
/// How many output channels may have a curve (A1 and A2).
const CURVES: usize = 2;
const NONE_OUTPUT: u32 = u32::MAX;

struct Lane {
    output: AtomicU32,
    len: AtomicU32,
    samples: [AtomicU64; CURVE_POINTS],
    /// dB as f32 bits.
    levels: [AtomicU32; CURVE_POINTS],
}

impl Lane {
    fn new() -> Self {
        Self {
            output: AtomicU32::new(NONE_OUTPUT),
            len: AtomicU32::new(0),
            samples: std::array::from_fn(|_| AtomicU64::new(0)),
            levels: std::array::from_fn(|_| AtomicU32::new(0f32.to_bits())),
        }
    }
}

pub(crate) struct CurveShared {
    lanes: [Lane; CURVES],
}

impl CurveShared {
    pub(crate) fn new() -> Self {
        Self { lanes: std::array::from_fn(|_| Lane::new()) }
    }

    /// The curve of output `output`: (absolute sample, dB) sorted by sample; empty clears it.
    /// False when every slot holds the curve of another output.
    pub(crate) fn set(&self, output: u16, points: &[(u64, f32)]) -> bool {
        let output = u32::from(output);
        let slot = self.lanes.iter().find(|lane| lane.output.load(Ordering::Acquire) == output).or_else(|| self.lanes.iter().find(|lane| lane.output.load(Ordering::Acquire) == NONE_OUTPUT));
        let Some(lane) = slot else { return false };
        // Hidden while it is written: a callback in between plays it as none.
        lane.len.store(0, Ordering::Release);
        if points.is_empty() {
            lane.output.store(NONE_OUTPUT, Ordering::Release);
            return true;
        }
        let points = &points[..points.len().min(CURVE_POINTS)];
        for (at, (sample, db)) in points.iter().enumerate() {
            lane.samples[at].store(*sample, Ordering::Relaxed);
            lane.levels[at].store(db.to_bits(), Ordering::Relaxed);
        }
        lane.output.store(output, Ordering::Release);
        lane.len.store(points.len() as u32, Ordering::Release);
        true
    }

    /// The curves over `output` (interleaved `channels`) from absolute sample `position` on.
    pub(crate) fn apply(&self, output: &mut [f32], channels: usize, position: u64) {
        for lane in &self.lanes {
            let len = lane.len.load(Ordering::Acquire) as usize;
            let channel = lane.output.load(Ordering::Acquire);
            if len == 0 || channel == NONE_OUTPUT || channel as usize >= channels {
                continue;
            }
            let point = |at: usize| (lane.samples[at].load(Ordering::Relaxed), f32::from_bits(lane.levels[at].load(Ordering::Relaxed)));
            let mut next = 0usize;
            for (index, frame) in output.chunks_exact_mut(channels.max(1)).enumerate() {
                let sample = position + index as u64;
                while next < len && point(next).0 <= sample {
                    next += 1;
                }
                let db = match next {
                    0 => point(0).1,
                    _ if next == len => point(len - 1).1,
                    _ => {
                        let ((a, a_db), (b, b_db)) = (point(next - 1), point(next));
                        let t = (sample - a) as f32 / (b - a).max(1) as f32;
                        a_db + (b_db - a_db) * t
                    }
                };
                frame[channel as usize] *= 10f32.powf(db / 20.0);
            }
        }
    }
}
