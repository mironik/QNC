use crate::*;
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub(crate) const NONE: u64 = u64::MAX;
pub(crate) struct Shared {
    pub gate: AtomicU64,
    pub ack: AtomicU64,
    pub device_failed: AtomicBool,
    pub underrun: AtomicBool,
    pub drained: AtomicBool,
    pub submitted: AtomicU64,
    pub end: AtomicU64,
    pub started_ns: AtomicU64,
    pub first_ns: AtomicU64,
    pub driver_delay_ns: AtomicU64,
    pub callback_frames: AtomicU64,
    pub origin: Instant,
    wall_origin_ns: u128,
    timing_revision: AtomicU64,
    timing_first: AtomicU64,
    timing_playback_ns: AtomicU64,
    timing_frames: AtomicU64,
    last_playback_position_ns: AtomicU64,
}
impl Shared {
    fn new() -> Self {
        Self {
            gate: AtomicU64::new(2),
            ack: AtomicU64::new(0),
            device_failed: AtomicBool::new(false),
            underrun: AtomicBool::new(false),
            drained: AtomicBool::new(false),
            submitted: AtomicU64::new(0),
            end: AtomicU64::new(NONE),
            started_ns: AtomicU64::new(NONE),
            first_ns: AtomicU64::new(NONE),
            driver_delay_ns: AtomicU64::new(NONE),
            callback_frames: AtomicU64::new(0),
            origin: Instant::now(),
            wall_origin_ns: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            timing_revision: AtomicU64::new(0),
            timing_first: AtomicU64::new(0),
            timing_playback_ns: AtomicU64::new(NONE),
            timing_frames: AtomicU64::new(0),
            last_playback_position_ns: AtomicU64::new(0),
        }
    }
    pub fn elapsed_ns(&self) -> u64 {
        self.origin.elapsed().as_nanos().min((NONE - 1) as u128) as u64
    }
}

pub(crate) struct Queue {
    producer: Producer<f32>,
    pub shared: Arc<Shared>,
    pub config: Config,
    pub generation: u64,
    anchor: u64,
    next: u64,
    pub status: Status,
    finished: bool,
}
pub(crate) struct Callback {
    consumer: Consumer<f32>,
    shared: Arc<Shared>,
    channels: usize,
    generation: u64,
    submitted: u64,
}
impl Queue {
    pub fn new(config: Config) -> Result<(Self, Callback)> {
        let capacity = config.validate()?;
        let (producer, consumer) = RingBuffer::new(capacity);
        let shared = Arc::new(Shared::new());
        let callback = Callback {
            consumer,
            shared: shared.clone(),
            channels: config.format.channels as usize,
            generation: 0,
            submitted: 0,
        };
        Ok((
            Self {
                producer,
                shared,
                config,
                generation: 1,
                anchor: 0,
                next: 0,
                status: Status::Paused,
                finished: false,
            },
            callback,
        ))
    }
    pub fn health(&self) -> Result<()> {
        if self.shared.callback_frames.load(Ordering::Acquire) > self.config.capacity_frames as u64
        {
            return Err(Error::new(
                Code::Unsupported,
                "device callback exceeds configured queue capacity",
            ));
        }
        if self.shared.device_failed.load(Ordering::Acquire) || self.producer.is_abandoned() {
            return Err(Error::new(Code::Device, "audio device unavailable"));
        }
        if self.shared.underrun.load(Ordering::Acquire) {
            return Err(Error::new(
                Code::Underrun,
                "audio queue underrun; explicit preparation required",
            ));
        }
        Ok(())
    }
    fn require_generation(&self, generation: u64) -> Result<()> {
        if generation != self.generation {
            return Err(Error::new(Code::StaleGeneration, "stale audio generation"));
        }
        self.health()
    }
    pub fn reset(&mut self, anchor: u64, status: Status) -> Result<u64> {
        self.generation = self
            .generation
            .checked_add(1)
            .filter(|g| *g < NONE / 2)
            .ok_or_else(|| Error::new(Code::Contract, "generation exhausted"))?;
        self.status = status;
        self.anchor = anchor;
        self.next = anchor;
        self.finished = false;
        self.shared
            .gate
            .store(self.generation << 1, Ordering::Release);
        self.shared.last_playback_position_ns.store(
            samples_to_ns(anchor, self.config.format.sample_rate_hz),
            Ordering::Release,
        );
        Ok(self.generation)
    }
    pub fn acknowledged(&self) -> bool {
        self.shared.ack.load(Ordering::Acquire) == self.generation
    }
    pub fn push(&mut self, generation: u64, first_frame: u64, samples: &[f32]) -> Result<()> {
        self.require_generation(generation)?;
        if !self.acknowledged()
            || !matches!(self.status, Status::Preparing | Status::Playing)
            || self.finished
        {
            return Err(Error::new(Code::NotReady, "queue is not accepting samples"));
        }
        let channels = self.config.format.channels as usize;
        let frames = samples.len() / channels;
        let next = self
            .next
            .checked_add(frames as u64)
            .ok_or_else(|| Error::new(Code::Contract, "sample position overflow"))?;
        if samples.is_empty()
            || !samples.len().is_multiple_of(channels)
            || first_frame != self.next
            || samples
                .iter()
                .any(|s| !s.is_finite() || !(-1.0..=1.0).contains(s))
        {
            return Err(Error::new(
                Code::Contract,
                "invalid PCM block or discontinuous sample position",
            ));
        }
        let mut chunk = self
            .producer
            .write_chunk(samples.len())
            .map_err(|_| Error::new(Code::Full, "audio queue full"))?;
        let (a, b) = chunk.as_mut_slices();
        let split = a.len();
        a.copy_from_slice(&samples[..split]);
        b.copy_from_slice(&samples[split..]);
        chunk.commit_all();
        self.next = next;
        Ok(())
    }
    pub fn finish(&mut self, generation: u64) -> Result<()> {
        self.require_generation(generation)?;
        if !self.acknowledged()
            || self.next == self.anchor
            || !matches!(
                self.status,
                Status::Preparing | Status::Ready | Status::Playing
            )
        {
            return Err(Error::new(Code::NotReady, "no prepared audio to finish"));
        }
        self.shared
            .end
            .store(self.next - self.anchor, Ordering::Release);
        self.finished = true;
        Ok(())
    }
    pub fn commit(&mut self, generation: u64) -> Result<()> {
        self.require_generation(generation)?;
        if !self.acknowledged()
            || self.status != Status::Preparing
            || self.next == self.anchor
            || (!self.finished
                && self.next - self.anchor
                    < (self.config.ready_frames as u64)
                        .max(self.shared.callback_frames.load(Ordering::Acquire)))
        {
            return Err(Error::new(Code::NotReady, "initial PCM queue is not ready"));
        }
        self.status = Status::Ready;
        Ok(())
    }
    pub fn start(&mut self, generation: u64) -> Result<()> {
        self.require_generation(generation)?;
        if self.status != Status::Ready || !self.acknowledged() {
            return Err(Error::new(
                Code::NotReady,
                "audio must be prepared before Start",
            ));
        }
        self.shared
            .started_ns
            .store(self.shared.elapsed_ns(), Ordering::Relaxed);
        self.shared
            .gate
            .compare_exchange(
                generation << 1,
                (generation << 1) | 1,
                Ordering::Release,
                Ordering::Relaxed,
            )
            .map_err(|_| Error::new(Code::NotReady, "audio gate changed"))?;
        self.status = Status::Playing;
        Ok(())
    }
    pub fn telemetry(&self) -> Telemetry {
        let status = if self.shared.device_failed.load(Ordering::Acquire)
            || self.shared.underrun.load(Ordering::Acquire)
        {
            Status::Failed
        } else if self.shared.drained.load(Ordering::Acquire) {
            Status::Drained
        } else {
            self.status
        };
        let first = self.shared.first_ns.load(Ordering::Acquire);
        let start = self.shared.started_ns.load(Ordering::Acquire);
        let delay = self.shared.driver_delay_ns.load(Ordering::Acquire);
        Telemetry {
            generation: self.generation,
            status,
            submitted_frames: self.shared.submitted.load(Ordering::Acquire),
            queued_frames: ((self.config.capacity_frames as usize
                * self.config.format.channels as usize
                - self.producer.slots())
                / self.config.format.channels as usize) as u64,
            start_to_first_callback_ns: (first != NONE && start != NONE)
                .then(|| first.saturating_sub(start)),
            first_driver_delay_ns: (first != NONE && delay != NONE).then_some(delay),
        }
    }

    pub fn driver_timing(&self) -> Option<DriverTiming> {
        if self.status != Status::Playing || !self.acknowledged() || self.health().is_err() {
            return None;
        }
        // Never wait for the real-time callback; discard an overlapping observation.
        let revision = self.shared.timing_revision.load(Ordering::SeqCst);
        if revision & 1 != 0 {
            return None;
        }
        let first = self.shared.timing_first.load(Ordering::SeqCst);
        let playback = self.shared.timing_playback_ns.load(Ordering::SeqCst);
        if playback == NONE || revision != self.shared.timing_revision.load(Ordering::SeqCst) {
            return None;
        }
        Some(DriverTiming {
            generation: self.generation,
            first_sample_frame: self.anchor.checked_add(first)?,
            sample_rate_hz: self.config.format.sample_rate_hz,
            playback_unix_ns: self.shared.wall_origin_ns + u128::from(playback),
        })
    }

    /// Media position at the device, using monotonic callback timing, not wall time.
    pub fn playback_position_ns(&self) -> u128 {
        self.playback_position_at(self.shared.elapsed_ns())
    }

    pub(crate) fn playback_position_at(&self, now_ns: u64) -> u128 {
        let rate = u128::from(self.config.format.sample_rate_hz);
        let anchor = u128::from(samples_to_ns(
            self.anchor,
            self.config.format.sample_rate_hz,
        ));
        if !self.acknowledged() || self.status != Status::Playing {
            return anchor;
        }
        let revision = self.shared.timing_revision.load(Ordering::SeqCst);
        let first = self.shared.timing_first.load(Ordering::SeqCst);
        let playback = self.shared.timing_playback_ns.load(Ordering::SeqCst);
        let frames = self.shared.timing_frames.load(Ordering::SeqCst);
        if revision & 1 != 0
            || playback == NONE
            || revision != self.shared.timing_revision.load(Ordering::SeqCst)
        {
            return u128::from(
                self.shared
                    .last_playback_position_ns
                    .load(Ordering::Acquire)
                    .max(anchor.min(u128::from(u64::MAX)) as u64),
            );
        }
        let first_ns = u128::from(first) * 1_000_000_000 / rate;
        let end_ns = u128::from(first + frames) * 1_000_000_000 / rate;
        let position = if now_ns >= playback {
            first_ns.saturating_add(u128::from(now_ns - playback))
        } else {
            first_ns.saturating_sub(u128::from(playback - now_ns))
        };
        let absolute = anchor + position.min(end_ns);
        let current = absolute.min(u128::from(u64::MAX)) as u64;
        self.shared
            .last_playback_position_ns
            .store(current, Ordering::Release);
        absolute
    }
}

fn samples_to_ns(samples: u64, rate: u32) -> u64 {
    (u128::from(samples) * 1_000_000_000 / u128::from(rate)).min(u128::from(u64::MAX)) as u64
}

impl Callback {
    /// Real-time path: no allocation, blocking synchronization, decode, or logging.
    pub fn render(&mut self, output: &mut [f32], now_ns: u64, driver_delay_ns: Option<u64>) {
        output.fill(0.0);
        self.shared
            .callback_frames
            .fetch_max((output.len() / self.channels) as u64, Ordering::Release);
        let gate = self.shared.gate.load(Ordering::Acquire);
        let generation = gate >> 1;
        if generation != self.generation {
            // The producer cannot publish this generation until the flush is acknowledged.
            let count = self.consumer.slots();
            if let Ok(chunk) = self.consumer.read_chunk(count) {
                chunk.commit_all();
            }
            self.generation = generation;
            self.submitted = 0;
            self.shared.submitted.store(0, Ordering::Relaxed);
            self.shared.end.store(NONE, Ordering::Relaxed);
            self.shared.first_ns.store(NONE, Ordering::Relaxed);
            self.shared.started_ns.store(NONE, Ordering::Relaxed);
            self.shared.driver_delay_ns.store(NONE, Ordering::Relaxed);
            self.shared.timing_playback_ns.store(NONE, Ordering::SeqCst);
            self.shared.underrun.store(false, Ordering::Relaxed);
            self.shared.drained.store(false, Ordering::Relaxed);
            self.shared.ack.store(generation, Ordering::Release);
            return;
        }
        if gate & 1 == 0 || self.shared.device_failed.load(Ordering::Acquire) {
            return;
        }
        if !output.len().is_multiple_of(self.channels) {
            self.shared.device_failed.store(true, Ordering::Release);
            return;
        }
        let wanted = output.len() / self.channels;
        let end = self.shared.end.load(Ordering::Acquire);
        let remaining = end.saturating_sub(self.submitted).min(usize::MAX as u64) as usize;
        let needed = wanted.min(remaining);
        let available = self.consumer.slots() / self.channels;
        if available < needed {
            self.shared.underrun.store(true, Ordering::Release);
            let _ = self.shared.gate.compare_exchange(
                gate,
                gate & !1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            );
            return;
        }
        if needed > 0 {
            let Ok(chunk) = self.consumer.read_chunk(needed * self.channels) else {
                self.shared.device_failed.store(true, Ordering::Release);
                return;
            };
            let (a, b) = chunk.as_slices();
            output[..a.len()].copy_from_slice(a);
            output[a.len()..a.len() + b.len()].copy_from_slice(b);
            chunk.commit_all();
        }
        // A concurrent pause invalidates the whole callback, not merely the following one.
        if self.shared.gate.load(Ordering::Acquire) != gate {
            output.fill(0.0);
            return;
        }
        if needed > 0 {
            self.shared.timing_revision.fetch_add(1, Ordering::SeqCst);
            self.shared
                .timing_first
                .store(self.submitted, Ordering::SeqCst);
            self.shared
                .timing_frames
                .store(needed as u64, Ordering::SeqCst);
            self.shared.timing_playback_ns.store(
                driver_delay_ns
                    .and_then(|delay| now_ns.checked_add(delay))
                    .unwrap_or(NONE),
                Ordering::SeqCst,
            );
            self.shared.timing_revision.fetch_add(1, Ordering::SeqCst);
            if self.submitted == 0 {
                self.shared
                    .driver_delay_ns
                    .store(driver_delay_ns.unwrap_or(NONE), Ordering::Relaxed);
                self.shared.first_ns.store(now_ns, Ordering::Release);
            }
            self.submitted += needed as u64;
            self.shared
                .submitted
                .store(self.submitted, Ordering::Release);
        }
        if end != NONE && self.submitted == end {
            self.shared.drained.store(true, Ordering::Release);
            let _ = self.shared.gate.compare_exchange(
                gate,
                gate & !1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            );
        }
    }
}
