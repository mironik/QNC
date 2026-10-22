use crate::*;
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
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
    /// The loudest sample of each output channel handed to the device since the last
    /// `take_peaks` (f32 bits of the absolute value; a level meter, user 2026-10-16).
    peaks: [AtomicU32; METER_CHANNELS],
    /// The gain of each output channel (f32 bits, 1.0 as recorded), applied where the
    /// samples go to the device, so a change is heard at once (user 2026-10-16).
    gains: [AtomicU32; METER_CHANNELS],
    /// The source channel each output channel plays (user 2026-10-19: a channel chosen
    /// for A1/A2 is a route, heard in the next callback; nothing is prepared again).
    routes: [AtomicU32; METER_CHANNELS],
    /// One part of the playout with its own gain on one output channel (user 2026-10-22:
    /// the level of one cover is heard at once, not the whole output): the output (NONE_PART
    /// for none), absolute samples `[start, end)` and the gain (f32 bits).
    part_output: AtomicU32,
    part_start: AtomicU64,
    part_end: AtomicU64,
    part_gain: AtomicU32,
    /// The absolute sample position of the first sample of the current generation.
    pub anchor: AtomicU64,
    /// A bed under the playout (`bed`): the story's A1 under a Sync (user 2026-10-20).
    pub bed: crate::bed::BedShared,
}
/// How many output channels the level meter follows.
pub const METER_CHANNELS: usize = 8;
/// No part with its own gain.
const NONE_PART: u32 = u32::MAX;
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
            peaks: Default::default(),
            gains: std::array::from_fn(|_| AtomicU32::new(1f32.to_bits())),
            routes: std::array::from_fn(|channel| AtomicU32::new(channel as u32)),
            part_output: AtomicU32::new(NONE_PART),
            part_start: AtomicU64::new(0),
            part_end: AtomicU64::new(0),
            part_gain: AtomicU32::new(1f32.to_bits()),
            anchor: AtomicU64::new(0),
            bed: crate::bed::BedShared::new(),
        }
    }
    /// The gain of the first `gains.len()` output channels; the others keep theirs.
    pub fn set_gains(&self, gains: &[f32]) {
        for (slot, gain) in self.gains.iter().zip(gains) {
            slot.store(gain.max(0.0).to_bits(), Ordering::Release);
        }
    }
    /// Output channel `output` over absolute samples `[start, end)` times `gain`, heard at
    /// once; `None` clears it.
    pub fn set_part_gain(&self, part: Option<(u16, u64, u64, f32)>) {
        self.part_output.store(NONE_PART, Ordering::Release);
        if let Some((output, start, end, gain)) = part {
            self.part_start.store(start, Ordering::Relaxed);
            self.part_end.store(end, Ordering::Relaxed);
            self.part_gain.store(gain.max(0.0).to_bits(), Ordering::Relaxed);
            self.part_output.store(u32::from(output), Ordering::Release);
        }
    }
    /// The loudest sample of each output channel since the last call, then zero again.
    /// The source channel of the first `routes.len()` output channels; the others keep theirs.
    pub fn set_routes(&self, routes: &[u16]) {
        for (slot, route) in self.routes.iter().zip(routes) {
            slot.store(u32::from(*route), Ordering::Release);
        }
    }
    pub fn take_peaks(&self, channels: usize) -> Vec<f32> {
        self.peaks.iter().take(channels).map(|peak| f32::from_bits(peak.swap(0, Ordering::AcqRel))).collect()
    }
    pub fn elapsed_ns(&self) -> u64 {
        self.origin.elapsed().as_nanos().min((NONE - 1) as u128) as u64
    }
}

pub(crate) struct Queue {
    producer: Producer<f32>,
    /// One frame of sound heard while the transport is paused (a frame step).
    audition: Producer<f32>,
    pub shared: Arc<Shared>,
    pub config: Config,
    pub generation: u64,
    anchor: u64,
    next: u64,
    pub status: Status,
    finished: bool,
    pub bed: crate::bed::BedProducer,
}
pub(crate) struct Callback {
    consumer: Consumer<f32>,
    audition: Consumer<f32>,
    shared: Arc<Shared>,
    channels: usize,
    /// Samples of one queued frame (every source channel).
    width: usize,
    generation: u64,
    submitted: u64,
    /// The absolute sample position of the generation's first sample.
    anchor: u64,
    bed: crate::bed::BedConsumer,
}
impl Queue {
    pub fn new(config: Config) -> Result<(Self, Callback)> {
        let capacity = config.validate()?;
        let (producer, consumer) = RingBuffer::new(capacity);
        let (audition, audition_out) = RingBuffer::new(capacity);
        // Four seconds of a mono bed.
        let (bed, bed_out) = crate::bed::bed(config.format.sample_rate_hz as usize * 4);
        let shared = Arc::new(Shared::new());
        let callback = Callback {
            consumer,
            audition: audition_out,
            shared: shared.clone(),
            channels: config.format.channels as usize,
            width: config.width(),
            generation: 0,
            submitted: 0,
            anchor: 0,
            bed: bed_out,
        };
        Ok((
            Self {
                producer,
                audition,
                shared,
                config,
                generation: 1,
                anchor: 0,
                next: 0,
                status: Status::Paused,
                finished: false,
                bed,
            },
            callback,
        ))
    }
    /// Sound of one frame step, heard only while the gate is closed (paused); what
    /// does not fit is dropped. It never touches the playout queue or its clock.
    pub fn audition(&mut self, samples: &[f32]) {
        let channels = self.config.width();
        let fit = self.audition.slots().min(samples.len());
        let fit = fit - fit % channels.max(1);
        if let Ok(chunk) = self.audition.write_chunk_uninit(fit) {
            let written = chunk.fill_from_iter(samples[..fit].iter().copied());
            debug_assert_eq!(written, fit);
        }
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
        self.shared.anchor.store(anchor, Ordering::Release);
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
        let channels = self.config.width();
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
            queued_frames: ((self.config.capacity_frames as usize * self.config.width() - self.producer.slots())
                / self.config.width()) as u64,
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
    /// Queued frames (`width` samples each, in `a` then `b`) into the output channels by
    /// their routes (no allocation; a plain copy when each output plays its own channel).
    fn route_into(shared: &Shared, (width, channels): (usize, usize), output: &mut [f32], a: &[f32], b: &[f32]) {
        let (width, channels) = (width.max(1), channels.max(1));
        let mut routes = [0usize; METER_CHANNELS];
        for (channel, route) in routes.iter_mut().enumerate().take(channels.min(METER_CHANNELS)) {
            *route = (shared.routes[channel].load(Ordering::Acquire) as usize).min(width - 1);
        }
        let route = |channel: usize| if channel < METER_CHANNELS { routes[channel] } else { channel.min(width - 1) };
        if width == channels && (0..channels).all(|channel| route(channel) == channel) {
            output[..a.len()].copy_from_slice(a);
            output[a.len()..a.len() + b.len()].copy_from_slice(b);
            return;
        }
        let frames = (a.len() + b.len()) / width;
        for (frame, out) in output.chunks_exact_mut(channels).take(frames).enumerate() {
            for (channel, sample) in out.iter_mut().enumerate() {
                let at = frame * width + route(channel);
                *sample = if at < a.len() { a[at] } else { b[at - a.len()] };
            }
        }
    }

    /// Each output channel times its gain (no allocation; nothing when all are 1.0).
    fn apply_gains(&self, output: &mut [f32]) {
        let channels = self.channels.clamp(1, METER_CHANNELS);
        let mut gains = [1f32; METER_CHANNELS];
        for (gain, slot) in gains.iter_mut().zip(&self.shared.gains).take(channels) {
            *gain = f32::from_bits(slot.load(Ordering::Acquire));
        }
        if gains.iter().take(channels).all(|gain| *gain == 1.0) {
            return;
        }
        for frame in output.chunks_exact_mut(self.channels.max(1)) {
            for (sample, gain) in frame.iter_mut().zip(gains).take(channels) {
                *sample *= gain;
            }
        }
    }

    /// The part with its own gain over the samples from absolute `position` on (no
    /// allocation; nothing when there is none or it is not in them).
    fn apply_part_gain(&self, output: &mut [f32], position: u64) {
        let channel = self.shared.part_output.load(Ordering::Acquire);
        if channel == NONE_PART || channel as usize >= self.channels {
            return;
        }
        let (start, end) = (self.shared.part_start.load(Ordering::Relaxed), self.shared.part_end.load(Ordering::Relaxed));
        let gain = f32::from_bits(self.shared.part_gain.load(Ordering::Relaxed));
        for (index, frame) in output.chunks_exact_mut(self.channels.max(1)).enumerate() {
            if (start..end).contains(&(position + index as u64)) {
                frame[channel as usize] *= gain;
            }
        }
    }

    /// The loudest sample of each channel of what goes to the device (no allocation).
    fn note_peaks(&self, output: &[f32]) {
        let channels = self.channels.clamp(1, METER_CHANNELS);
        let mut loudest = [0f32; METER_CHANNELS];
        for frame in output.chunks_exact(self.channels.max(1)) {
            for (channel, sample) in frame.iter().take(channels).enumerate() {
                loudest[channel] = loudest[channel].max(sample.abs());
            }
        }
        for (peak, value) in self.shared.peaks.iter().zip(loudest).take(channels) {
            // Positive floats order as their bits do.
            peak.fetch_max(value.to_bits(), Ordering::AcqRel);
        }
    }

    /// Real-time path: no allocation, blocking synchronization, decode, or logging.
    pub fn render(&mut self, output: &mut [f32], now_ns: u64, driver_delay_ns: Option<u64>) {
        output.fill(0.0);
        self.bed.adopt(&self.shared);
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
            self.anchor = self.shared.anchor.load(Ordering::Acquire);
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
        if self.shared.device_failed.load(Ordering::Acquire) {
            return;
        }
        if gate & 1 == 0 {
            // Paused: a frame step may be heard; nothing else plays.
            let count = self.audition.slots().min(output.len() / self.channels * self.width);
            let count = count - count % self.width;
            if let Ok(chunk) = self.audition.read_chunk(count) {
                let (a, b) = chunk.as_slices();
                Self::route_into(&self.shared, (self.width, self.channels), output, a, b);
                chunk.commit_all();
            }
            self.apply_gains(output);
            self.note_peaks(output);
            return;
        }
        // Playing: a step heard before Play is not heard over it.
        let stale = self.audition.slots();
        if let Ok(chunk) = self.audition.read_chunk(stale) {
            chunk.commit_all();
        }
        if !output.len().is_multiple_of(self.channels) {
            self.shared.device_failed.store(true, Ordering::Release);
            return;
        }
        let wanted = output.len() / self.channels;
        let end = self.shared.end.load(Ordering::Acquire);
        let remaining = end.saturating_sub(self.submitted).min(usize::MAX as u64) as usize;
        let needed = wanted.min(remaining);
        let available = self.consumer.slots() / self.width;
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
            let Ok(chunk) = self.consumer.read_chunk(needed * self.width) else {
                self.shared.device_failed.store(true, Ordering::Release);
                return;
            };
            let (a, b) = chunk.as_slices();
            Self::route_into(&self.shared, (self.width, self.channels), output, a, b);
            chunk.commit_all();
        }
        // A concurrent pause invalidates the whole callback, not merely the following one.
        if self.shared.gate.load(Ordering::Acquire) != gate {
            output.fill(0.0);
            return;
        }
        self.apply_gains(output);
        self.apply_part_gain(&mut output[..needed * self.channels], self.anchor + self.submitted);
        // The bed over its output, in the same samples and clock (after the gains: its level
        // is in its samples).
        let channels = self.channels;
        self.bed.mix(&self.shared, &mut output[..needed * channels], channels, self.anchor + self.submitted);
        self.note_peaks(output);
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
