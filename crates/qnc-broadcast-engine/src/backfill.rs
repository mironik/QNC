//! The pictures before where the player stands, decoded in the background and held in
//! memory (user 2026-10-11: "the best solution when the camera made no proxy"; "the same
//! for Wrap"; arrow steps and drags go smoothly). Once the player has stood still a moment,
//! a thread of its own decodes the second and a half before the frame (about the GOP
//! before) with decoders and converters of its own, at a lower priority, and keeps the
//! pictures; a step back takes them without reopening the player's decoder. Ahead of the
//! frame the player's decoder already reads on. It stops as soon as the player plays.
//! Nothing is coded, nothing written: the media is decoded once, as the player would.
//!
//! A clip is one piece; a story program is its pieces in program frames (a segment or a
//! cover reads its clip from its source frame; an Off piece has no picture, the player
//! makes its black at once and the background leaves it).

use crate::*;
use qnc_pixel_convert::ConversionSpec;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

/// A picture made in the background: its raster size and pixels.
pub(crate) type Made = ([u32; 2], Arc<[u8]>);
type Pictures = Arc<Mutex<BTreeMap<u64, Made>>>;
type Converters = Arc<Mutex<Vec<(ConversionSpec, conversion::Raster)>>>;

/// About the GOP before where the player stands.
const BEHIND_SECONDS: f64 = 1.5;
/// A new background job only for at least this part of a second of missing frames (each
/// job opens a decoder and reads from a key picture), unless the frame just before is missing.
const REFILL_SECONDS: f64 = 0.75;
/// The background starts this long after the player stopped at a frame.
const START_DELAY: std::time::Duration = std::time::Duration::from_millis(600);

/// Frames `record.0..record.1` of what the player plays; with a picture, read from
/// `source_in` of the clip `plan`/`input` (program frames map to source frames 1:1).
pub(crate) struct Piece {
    pub record: (u64, u64),
    pub picture: Option<(InputPlan, Rc<DecodeInput>, u64)>,
}

struct Job {
    /// The frames the job makes: `from..until`.
    range: (u64, u64),
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

pub(crate) struct BackFill {
    pictures: Pictures,
    /// The converters of the background, one per picture format, prepared by the jobs.
    converters: Converters,
    pieces: Vec<Piece>,
    job: Option<Job>,
    /// The job to start once the player stood still long enough: (when, from, until, frame).
    due: Option<(Instant, u64, u64, u64)>,
    /// How many frames before the standing frame are kept, and the least a job makes.
    behind: u64,
    refill: u64,
}

impl BackFill {
    /// `per_second`: the frames per second of what the player plays.
    pub fn new(pieces: Vec<Piece>, per_second: f64) -> Self {
        Self {
            pictures: Arc::default(),
            converters: Arc::default(),
            pieces,
            job: None,
            due: None,
            behind: (BEHIND_SECONDS * per_second).round() as u64,
            refill: (REFILL_SECONDS * per_second).round() as u64,
        }
    }

    /// One clip, all its frames from itself.
    pub fn for_clip(plan: &InputPlan, input: Rc<DecodeInput>) -> Self {
        let per_second = plan.source.timebase.fps_num as f64 / plan.source.timebase.fps_den.max(1) as f64;
        let piece = Piece { record: (0, plan.source.duration_frames), picture: Some((plan.clone(), input, 0)) };
        Self::new(vec![piece], per_second)
    }

    /// The picture of `frame`, when the background made it.
    pub fn picture(&self, frame: u64) -> Option<Made> {
        self.pictures.lock().ok()?.get(&frame).cloned()
    }

    /// Whether `frame` has no picture of its own (an Off piece: the player makes it black).
    fn without_picture(&self, frame: u64) -> bool {
        self.pieces.iter().any(|p| (p.record.0..p.record.1).contains(&frame) && p.picture.is_none())
    }

    /// The player stands at `frame`: keep what is around it; what is missing before it is
    /// made once the player stood still `START_DELAY` (one job at a time; a job that no
    /// longer helps stops). `held`: the player holds that frame itself.
    pub fn stand(&mut self, frame: u64, held: impl Fn(u64) -> bool) {
        let from = frame.saturating_sub(self.behind);
        if let Ok(mut pictures) = self.pictures.lock() {
            pictures.retain(|kept, _| *kept + self.behind + self.refill >= frame && *kept <= frame + self.behind);
        }
        self.due = None;
        let have = |f: u64| held(f) || self.without_picture(f) || self.picture(f).is_some();
        // The frames before the standing one not yet held, from the first missing one up
        // to the next held one.
        let Some(from) = (from..frame).find(|f| !have(*f)) else { return };
        let until = (from..frame).find(|f| have(*f)).unwrap_or(frame);
        let urgent = frame > 0 && !have(frame - 1);
        if until - from < self.refill && !urgent {
            return;
        }
        if let Some(job) = &self.job {
            let (a, b) = job.range;
            if !job.thread.is_finished() && a <= from + self.refill && b + self.refill >= until {
                return; // the running job makes (nearly) these
            }
            job.cancel.store(true, Ordering::Release);
        }
        self.due = Some((Instant::now() + START_DELAY, from, until, frame));
    }

    /// From the player's loop: starts the job that is due.
    pub fn tick(&mut self) {
        let Some((when, from, until, frame)) = self.due else { return };
        if Instant::now() < when {
            return;
        }
        self.due = None;
        // The pieces with a picture in `from..until`, the one nearest the frame first.
        let mut parts = Vec::new();
        for piece in &self.pieces {
            let (start, end) = (piece.record.0.max(from), piece.record.1.min(until));
            let Some((plan, input, source_in)) = piece.picture.as_ref().filter(|_| start < end) else { continue };
            let source = source_in + (start - piece.record.0);
            let opened = seek_start(&plan.source, source).and_then(|seek| input.open(plan.video_index, seek));
            let decoder = match opened {
                Ok(decoder) => decoder,
                Err(why) => {
                    diag(format!("backfill open failed from={start} why={why}"));
                    return;
                }
            };
            decoder.set_background_priority();
            parts.push(Part {
                decoder,
                record: (start, end),
                source,
                origin: plan.origin,
                fps: (plan.source.timebase.fps_num, plan.source.timebase.fps_den),
                video_index: plan.video_index,
                spec: plan.spec.clone(),
            });
        }
        parts.reverse();
        let cancel = Arc::new(AtomicBool::new(false));
        let work = Work {
            parts,
            range: (from, until),
            pictures: self.pictures.clone(),
            converters: self.converters.clone(),
            cancel: cancel.clone(),
        };
        if let Ok(thread) = std::thread::Builder::new().name("player-backfill".into()).spawn(move || work.run()) {
            diag(format!("backfill start from={from} until={until} stand={frame}"));
            if let Some(old) = self.job.replace(Job { range: (from, until), cancel, thread }) {
                old.cancel.store(true, Ordering::Release);
                retire(old.thread);
            }
        }
    }

    /// The player plays: the background stops, it gives the processor to the playing
    /// decoder (it starts again at the next stand).
    pub fn pause(&mut self) {
        self.due = None;
        if let Some(job) = self.job.take() {
            job.cancel.store(true, Ordering::Release);
            diag(format!("backfill paused {}..{}", job.range.0, job.range.1));
            retire(job.thread);
        }
    }
}

impl Drop for BackFill {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            job.cancel.store(true, Ordering::Release);
        }
    }
}

/// One piece of a job: program frames `record`, read from `source` of a clip.
struct Part {
    decoder: qnc_media_decode::Decoder,
    record: (u64, u64),
    source: u64,
    origin: (i64, qnc_media_metadata::Rational),
    fps: (i64, i64),
    video_index: u32,
    spec: ConversionSpec,
}

struct Work {
    parts: Vec<Part>,
    range: (u64, u64),
    pictures: Pictures,
    converters: Converters,
    cancel: Arc<AtomicBool>,
}

impl Work {
    fn run(mut self) {
        let started = Instant::now();
        let mut made = Ok(0);
        for part in std::mem::take(&mut self.parts) {
            match self.part(part) {
                Ok(count) => made = made.map(|total| total + count),
                Err(why) => {
                    made = Err(why);
                    break;
                }
            }
            if self.cancel.load(Ordering::Acquire) {
                break;
            }
        }
        diag(format!("backfill {}..{} made={made:?} ms={}", self.range.0, self.range.1, started.elapsed().as_millis()));
    }

    /// The converter for `spec`, taken from the shared ones or prepared like the player's.
    fn converter(&self, spec: &ConversionSpec) -> std::result::Result<conversion::Raster, String> {
        let taken = self.converters.lock().ok().and_then(|mut all| {
            let at = all.iter().position(|(s, _)| s == spec)?;
            Some(all.swap_remove(at).1)
        });
        match taken {
            Some(converter) => Ok(converter),
            None => qnc_gpu_raster::GpuRasterConverter::prepare(spec.clone(), input::preview_raster_bounds(spec.width, spec.height))
                .map(conversion::Raster::Gpu)
                .map_err(|e| e.to_string()),
        }
    }

    fn part(&mut self, mut part: Part) -> std::result::Result<u64, String> {
        let mut converter = self.converter(&part.spec)?;
        let made = self.decode(&mut part, &mut converter);
        if let Ok(mut all) = self.converters.lock() {
            all.push((part.spec.clone(), converter));
        }
        made
    }

    fn decode(&self, part: &mut Part, converter: &mut conversion::Raster) -> std::result::Result<u64, String> {
        let mut made = 0;
        let last = part.source + (part.record.1 - part.record.0);
        loop {
            if self.cancel.load(Ordering::Acquire) {
                return Ok(made);
            }
            let packet = match part.decoder.try_next_packet().map_err(|e| e.to_string())? {
                Poll::Pending => {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                    continue;
                }
                Poll::Ready(None) => return Ok(made),
                Poll::Ready(Some(packet)) => packet,
            };
            if packet.stream_index != part.video_index {
                continue;
            }
            let source = relative_position(packet.pts, packet.time_base, part.origin, part.fps.0, part.fps.1)
                .map_err(|e| e.to_string())?;
            if source < part.source {
                continue;
            }
            if source >= last {
                return Ok(made);
            }
            let mut rgba = vec![0u8; converter.output_bytes()];
            converter.convert(&packet.bytes, &mut rgba).map_err(|e| e.to_string())?;
            let frame = part.record.0 + (source - part.source);
            if let Ok(mut pictures) = self.pictures.lock() {
                pictures.insert(frame, (converter.size(), Arc::from(rgba)));
            }
            made += 1;
        }
    }
}

fn diag(text: String) {
    if qnc_dev_diagnostics::player_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, text);
    }
}
