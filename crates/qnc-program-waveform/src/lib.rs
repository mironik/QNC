//! Passive program waveform (v5 `editorial/program_waveform.rs` and the row slice of
//! `qnc_segment_timeline.rs`). It never decodes media, never writes and never drives
//! playback: it maps the waves already stored for each clip onto the program axis by
//! the source IN/OUT of each segment (A1) and cover (A2), and cuts that program wave
//! for one Wrap row.
//!
//! QNC difference from v5 (user rule 2026-09-30): the lane drawn is the source
//! channel the segment or cover plays (`a1_source_channel`, `a2_source_channel`), so
//! the picture matches what is heard. Channel 1 keeps the v5 fallback to the next lane
//! when the first is empty.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub use qnc_timeline_assets::TimelineArtifactRead;

const PROGRAM_PEAK_BUCKETS: usize = 1200;
const MIN_PROGRAM_PEAK_BUCKETS: usize = 24;
const WAVE_RETRY_DELAY: Duration = Duration::from_secs(2);

/// The program wave: A1 (segments) and A2 (covers) peaks over the whole program.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProgramPeaks {
    pub a1: Vec<f32>,
    pub a2: Vec<f32>,
}

// Peaks are finite amplitudes 0..=1 (never NaN), so equality is total.
impl Eq for ProgramPeaks {}

/// A piece of a clip placed on the program axis: program frames `[start, end)` play
/// source frames `[source_in, source_out)` of `clip_id`, heard from `channel`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Placement {
    pub clip_id: String,
    pub channel: u16,
    pub program_start: u64,
    pub program_end: u64,
    pub source_in: u64,
    pub source_out: u64,
}

/// The stored wave of one clip: one peak lane per source channel, over `duration_frames`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClipWave {
    pub lanes: Vec<Vec<f32>>,
    pub duration_frames: u64,
}

impl ClipWave {
    /// The lane of `channel`; channel 1 falls back to the next lane (v5
    /// `primary_source_peaks`), another missing channel draws nothing.
    fn lane(&self, channel: u16) -> &[f32] {
        let lane = |index: usize| self.lanes.get(index).map_or(&[][..], Vec::as_slice);
        match (channel, lane(channel as usize)) {
            (0, []) => lane(1),
            (_, peaks) => peaks,
        }
    }
}

/// v5 `compose_program_waveform`: segments on A1, covers on A2.
pub fn compose(
    total_frames: u64,
    segments: &[Placement],
    covers: &[Placement],
    waves: &HashMap<String, ClipWave>,
) -> ProgramPeaks {
    if total_frames == 0 {
        return ProgramPeaks::default();
    }
    let buckets = (total_frames as usize).clamp(MIN_PROGRAM_PEAK_BUCKETS, PROGRAM_PEAK_BUCKETS);
    let mut out = ProgramPeaks { a1: vec![0.0; buckets], a2: vec![0.0; buckets] };
    for (lane, placements) in [(&mut out.a1, segments), (&mut out.a2, covers)] {
        for placement in placements {
            if let Some(wave) = waves.get(&placement.clip_id) {
                fill(lane, total_frames, placement, wave.lane(placement.channel), wave.duration_frames);
            }
        }
    }
    out
}

/// v5 `fill_program_peaks`.
fn fill(out: &mut [f32], program_duration: u64, at: &Placement, peaks: &[f32], source_duration: u64) {
    if out.is_empty() || peaks.is_empty() {
        return;
    }
    let program_start = at.program_start as f64;
    let program_end = at.program_end.max(at.program_start + 1) as f64;
    let source_duration = source_duration.max(1);
    let source_in = at.source_in.min(source_duration);
    let source_out = at.source_out.max(source_in + 1).min(source_duration);
    if source_out <= source_in {
        return;
    }
    let (duration, len) = (program_duration.max(1) as f64, out.len() as f64);
    let first = ((program_start / duration) * len).floor().clamp(0.0, len) as usize;
    let last = ((program_end / duration) * len).ceil().clamp(0.0, len) as usize;
    for bucket in first..last {
        let bucket_start = bucket as f64 * duration / len;
        let bucket_end = (bucket + 1) as f64 * duration / len;
        let from = bucket_start.max(program_start);
        let to = bucket_end.min(program_end);
        if to <= from {
            continue;
        }
        let map = |frame: f64| {
            let t = ((frame - program_start) / (program_end - program_start).max(1.0)).clamp(0.0, 1.0);
            source_in as f64 + t * (source_out - source_in).max(1) as f64
        };
        out[bucket] = out[bucket].max(max_peak(peaks, source_duration, map(from), map(to)));
    }
}

/// v5 `max_source_peak` / `max_program_peak`: the loudest peak in `[start, end)` of a
/// lane spread over `duration` frames.
fn max_peak(peaks: &[f32], duration: u64, start: f64, end: f64) -> f32 {
    if peaks.is_empty() {
        return 0.0;
    }
    let (duration, len) = (duration.max(1) as f64, peaks.len() as f64);
    let first = ((start.max(0.0) / duration) * len).floor().clamp(0.0, len) as usize;
    let last = ((end.max(start + 1.0) / duration) * len).ceil().clamp(0.0, len) as usize;
    let last = last.max(first + 1).min(peaks.len());
    let first = first.min(last.saturating_sub(1));
    peaks[first..last].iter().copied().fold(0.0, f32::max)
}

/// v5 `local_peaks_for_row`: the program wave between program frames `start` and `end`.
pub fn row_peaks(program: &[f32], total_frames: u64, start: u64, end: u64) -> Vec<f32> {
    if program.is_empty() {
        return Vec::new();
    }
    let row = end.saturating_sub(start).max(1);
    let buckets = (row as usize).clamp(MIN_PROGRAM_PEAK_BUCKETS, program.len().max(MIN_PROGRAM_PEAK_BUCKETS));
    (0..buckets)
        .map(|bucket| {
            let from = start as f64 + bucket as f64 * row as f64 / buckets as f64;
            let to = (start as f64 + (bucket + 1) as f64 * row as f64 / buckets as f64).min(end as f64);
            max_peak(program, total_frames, from, to)
        })
        .collect()
}

/// The waves of the clips a program uses, read from the project database through the
/// public timeline artifact reader (v5 `ProgramWaveformAssets`): a clip whose wave is
/// not there yet is asked again after two seconds, the composed program is kept until
/// its placements or waves change. The reads run on a thread of their own, so the
/// form's thread never waits on the database while a background job writes it.
#[derive(Default)]
pub struct ProgramWaves {
    reader: Option<Arc<dyn TimelineArtifactRead>>,
    waves: HashMap<String, ClipWave>,
    retry_after: HashMap<String, Instant>,
    /// Waves read on the reading thread, by clip (None: not there yet), until taken.
    read: Arc<std::sync::Mutex<Vec<(String, Option<Vec<Vec<f32>>>)>>>,
    asked: std::collections::HashSet<String>,
    composed: Option<(u64, Vec<Placement>, Vec<Placement>, usize, ProgramPeaks)>,
}

impl ProgramWaves {
    pub fn new() -> Self {
        Self::default()
    }

    /// The reader of the active project; a new project forgets the old waves.
    pub fn set_reader(&mut self, reader: Option<Arc<dyn TimelineArtifactRead>>) {
        *self = Self { reader, ..Self::default() };
    }

    /// The program wave for these placements; `duration_frames` gives each clip's
    /// length in the frames its IN/OUT use.
    pub fn peaks(
        &mut self,
        total_frames: u64,
        segments: &[Placement],
        covers: &[Placement],
        duration_frames: impl Fn(&str) -> Option<u64>,
    ) -> ProgramPeaks {
        let now = Instant::now();
        let read = self.read.lock().map(|mut read| std::mem::take(&mut *read)).unwrap_or_default();
        for (clip_id, lanes) in read {
            self.asked.remove(&clip_id);
            match (lanes, duration_frames(&clip_id)) {
                (Some(lanes), Some(duration_frames)) if duration_frames > 0 => {
                    self.waves.insert(clip_id, ClipWave { lanes, duration_frames });
                }
                _ => _ = self.retry_after.insert(clip_id, now + WAVE_RETRY_DELAY),
            }
        }
        let mut wanted = Vec::new();
        for placement in segments.iter().chain(covers) {
            let clip_id = placement.clip_id.as_str();
            if self.waves.contains_key(clip_id)
                || self.asked.contains(clip_id)
                || self.retry_after.get(clip_id).is_some_and(|at| *at > now)
            {
                continue;
            }
            self.asked.insert(clip_id.to_string());
            wanted.push(clip_id.to_string());
        }
        self.ask(wanted);
        let loaded = self.waves.len();
        if let Some((total, a1, a2, count, peaks)) = &self.composed {
            if *total == total_frames && a1 == segments && a2 == covers && *count == loaded {
                return peaks.clone();
            }
        }
        let peaks = compose(total_frames, segments, covers, &self.waves);
        self.composed = Some((total_frames, segments.to_vec(), covers.to_vec(), loaded, peaks.clone()));
        peaks
    }

    /// Whether waves are still being read.
    pub fn reading(&self) -> bool {
        !self.asked.is_empty()
    }

    /// Reads the waves of these clips on a thread of its own.
    fn ask(&mut self, clips: Vec<String>) {
        if clips.is_empty() {
            return;
        }
        let Some(reader) = self.reader.clone() else {
            self.asked.clear();
            return;
        };
        let mailbox = self.read.clone();
        let started = std::thread::Builder::new().name("qnc-program-waves".into()).spawn(move || {
            for clip_id in clips {
                let lanes = read_wave(reader.as_ref(), &clip_id);
                if let Ok(mut read) = mailbox.lock() {
                    read.push((clip_id, lanes));
                }
            }
        });
        if started.is_err() {
            self.asked.clear();
        }
    }
}

fn read_wave(reader: &dyn TimelineArtifactRead, clip_id: &str) -> Option<Vec<Vec<f32>>> {
    let record = reader.read_wave(clip_id).ok()??;
    let peaks = record.peaks()?;
    let lanes = [peaks.a1_peaks, peaks.a2_peaks, peaks.a3_peaks, peaks.a4_peaks];
    let lanes: Vec<Vec<f32>> = lanes.into_iter().collect();
    lanes.iter().any(|lane| !lane.is_empty()).then_some(lanes)
}

#[cfg(test)]
mod tests;
