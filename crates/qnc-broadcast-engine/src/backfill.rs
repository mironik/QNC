//! The pictures before where the player stands, decoded in the background and held in
//! memory (user 2026-10-11: "the best solution when the camera made no proxy"; arrow steps
//! and drags go smoothly). While the player stands at a frame, a thread of its own decodes
//! the second and a half before it (about the GOP before) with a decoder and a converter of
//! its own and keeps the pictures; a step back takes them without reopening the decoder of
//! the player. Ahead of the frame the decoder of the player already reads on. Nothing is
//! coded, nothing written: the media is decoded once, as the player would.

use crate::*;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

type Pictures = Arc<Mutex<BTreeMap<u64, Arc<[u8]>>>>;

/// About the GOP before where the player stands.
const BEHIND_SECONDS: f64 = 1.5;
/// A new background job only for at least this part of a second of missing frames (each
/// job opens a decoder and reads from a key picture), unless the frame just before is missing.
const REFILL_SECONDS: f64 = 0.75;
/// The background starts this long after the player stopped at a frame.
const START_DELAY: std::time::Duration = std::time::Duration::from_millis(600);

struct Job {
    /// The frames the job makes: `from..until`.
    range: (u64, u64),
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

pub(crate) struct BackFill {
    pictures: Pictures,
    /// The converter of the background (prepared by the first job, like the player's),
    /// returned by each job for the next one.
    converter: Arc<Mutex<Option<conversion::Raster>>>,
    plan: InputPlan,
    input: Rc<DecodeInput>,
    job: Option<Job>,
    /// The job to start once the player stood still long enough: (when, from, until, frame).
    due: Option<(Instant, u64, u64, u64)>,
    /// How many frames before the standing frame are kept, and the least a job makes.
    behind: u64,
    refill: u64,
}

impl BackFill {
    pub fn new(plan: &InputPlan, input: Rc<DecodeInput>) -> Self {
        let per_second = plan.source.timebase.fps_num as f64 / plan.source.timebase.fps_den.max(1) as f64;
        Self {
            pictures: Arc::default(),
            converter: Arc::default(),
            plan: plan.clone(),
            input,
            job: None,
            due: None,
            behind: (BEHIND_SECONDS * per_second).round() as u64,
            refill: (REFILL_SECONDS * per_second).round() as u64,
        }
    }

    /// The picture of `frame`, when the background made it.
    pub fn picture(&self, frame: u64) -> Option<Arc<[u8]>> {
        self.pictures.lock().ok()?.get(&frame).cloned()
    }

    /// The player stands at `frame`: keep what is around it; what is missing before it is
    /// made once the player stood still `START_DELAY` (one job at a time; a job that no
    /// longer helps stops). `held`: the player holds that frame itself.
    pub fn stand(&mut self, frame: u64, held: impl Fn(u64) -> bool) {
        let from = frame.saturating_sub(self.behind);
        if let Ok(mut pictures) = self.pictures.lock() {
            pictures.retain(|kept, _| *kept + self.behind + self.refill >= frame && *kept <= frame + self.behind);
        }
        let have = |f: u64| held(f) || self.picture(f).is_some();
        // The frames before the standing one not yet held, up to the first held one.
        let until = (from..frame).find(|f| have(*f)).unwrap_or(frame);
        let urgent = frame > 0 && !have(frame - 1);
        self.due = None;
        if until <= from || (until - from < self.refill && !urgent) {
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
        let plan = &self.plan;
        let decoder = match seek_start(&plan.source, from).and_then(|start| self.input.open(plan.video_index, start)) {
            Ok(decoder) => decoder,
            Err(why) => {
                diag(format!("backfill open failed from={from} why={why}"));
                return;
            }
        };
        decoder.set_background_priority();
        let cancel = Arc::new(AtomicBool::new(false));
        let work = Work {
            decoder,
            range: (from, until),
            origin: plan.origin,
            fps: (plan.source.timebase.fps_num, plan.source.timebase.fps_den),
            video_index: plan.video_index,
            spec: plan.spec.clone(),
            pictures: self.pictures.clone(),
            converter: self.converter.clone(),
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
}

impl BackFill {
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

struct Work {
    decoder: qnc_media_decode::Decoder,
    range: (u64, u64),
    origin: (i64, qnc_media_metadata::Rational),
    fps: (i64, i64),
    video_index: u32,
    spec: qnc_pixel_convert::ConversionSpec,
    pictures: Pictures,
    converter: Arc<Mutex<Option<conversion::Raster>>>,
    cancel: Arc<AtomicBool>,
}

impl Work {
    fn run(mut self) {
        let started = Instant::now();
        let taken = self.converter.lock().ok().and_then(|mut slot| slot.take());
        let converter = match taken {
            Some(converter) => Ok(converter),
            None => qnc_gpu_raster::GpuRasterConverter::prepare(
                self.spec.clone(),
                input::preview_raster_bounds(self.spec.width, self.spec.height),
            )
            .map(conversion::Raster::Gpu)
            .map_err(|e| e.to_string()),
        };
        let made = converter.and_then(|mut converter| {
            let made = self.decode(&mut converter);
            if let Ok(mut slot) = self.converter.lock() {
                *slot = Some(converter);
            }
            made
        });
        diag(format!("backfill {}..{} made={made:?} ms={}", self.range.0, self.range.1, started.elapsed().as_millis()));
    }

    fn decode(&mut self, converter: &mut conversion::Raster) -> std::result::Result<u64, String> {
        let mut made = 0;
        loop {
            if self.cancel.load(Ordering::Acquire) {
                return Ok(made);
            }
            let packet = match self.decoder.try_next_packet().map_err(|e| e.to_string())? {
                Poll::Pending => {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                    continue;
                }
                Poll::Ready(None) => return Ok(made),
                Poll::Ready(Some(packet)) => packet,
            };
            if packet.stream_index != self.video_index {
                continue;
            }
            let frame = relative_position(packet.pts, packet.time_base, self.origin, self.fps.0, self.fps.1)
                .map_err(|e| e.to_string())?;
            if frame < self.range.0 {
                continue;
            }
            if frame >= self.range.1 {
                return Ok(made);
            }
            let mut rgba = vec![0u8; converter.output_bytes()];
            converter.convert(&packet.bytes, &mut rgba).map_err(|e| e.to_string())?;
            if let Ok(mut pictures) = self.pictures.lock() {
                pictures.insert(frame, Arc::from(rgba));
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
