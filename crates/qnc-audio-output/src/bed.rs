//! A bed under the playout (user 2026-10-20: the story's A1 under a Sync/B-roll, heard in the
//! same device and clock as the source, sample exact): mono samples the producer hands ahead,
//! played on one output channel from an absolute sample position of the playout on. Before
//! that position, and when no bed is set, the output plays what the queue routes to it.
//!
//! An epoch starts every bed content: the callback empties what is left of the previous one
//! when it sees a new epoch, and the producer hands samples only once it was seen. A bed that
//! is late (its samples come after their position) is dropped up to where the playout is, so
//! it never runs behind the source.

use crate::queue::{NONE, Shared};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::atomic::{AtomicU64, Ordering};

/// What the producer and the callback share of a bed.
pub(crate) struct BedShared {
    /// The epoch of the bed content (raised by the producer).
    pub epoch: AtomicU64,
    /// The epoch the callback emptied the ring for.
    pub ack: AtomicU64,
    /// The absolute sample position where bed sample 0 plays; `NONE`: no bed.
    pub start: AtomicU64,
    /// The output channel the bed plays on.
    pub output: AtomicU64,
}

impl BedShared {
    pub fn new() -> Self {
        Self { epoch: AtomicU64::new(0), ack: AtomicU64::new(0), start: AtomicU64::new(NONE), output: AtomicU64::new(0) }
    }
}

/// The producer side, on the player's thread.
pub(crate) struct BedProducer {
    producer: Producer<f32>,
    epoch: u64,
}

/// The callback side, real time: no allocation, blocking or logging.
pub(crate) struct BedConsumer {
    consumer: Consumer<f32>,
    epoch: u64,
    /// Bed samples taken (played or dropped) in this epoch.
    taken: u64,
}

pub(crate) fn bed(capacity: usize) -> (BedProducer, BedConsumer) {
    let (producer, consumer) = RingBuffer::new(capacity.max(1));
    (BedProducer { producer, epoch: 0 }, BedConsumer { consumer, epoch: 0, taken: 0 })
}

impl BedProducer {
    /// A new bed content: nothing plays until `start`; what was handed before is emptied.
    pub fn reset(&mut self, shared: &Shared) {
        self.epoch += 1;
        shared.bed.start.store(NONE, Ordering::Release);
        shared.bed.epoch.store(self.epoch, Ordering::Release);
    }

    /// Whether the callback emptied the previous content (samples may be handed).
    pub fn ready(&self, shared: &Shared) -> bool {
        shared.bed.ack.load(Ordering::Acquire) == self.epoch
    }

    /// Room for this many samples.
    pub fn room(&self) -> usize {
        self.producer.slots()
    }

    /// Hands samples of the current content; what does not fit is left (the count handed).
    pub fn push(&mut self, shared: &Shared, samples: &[f32]) -> usize {
        if !self.ready(shared) {
            return 0;
        }
        let fit = self.producer.slots().min(samples.len());
        if let Ok(chunk) = self.producer.write_chunk_uninit(fit) {
            chunk.fill_from_iter(samples[..fit].iter().copied());
        }
        fit
    }

    /// Bed sample 0 plays at absolute sample `position` on `output`.
    pub fn start(&self, shared: &Shared, position: u64, output: u16) {
        shared.bed.output.store(u64::from(output), Ordering::Release);
        shared.bed.start.store(position, Ordering::Release);
    }

    /// The bed stops at once; the output plays its routed channel again.
    pub fn stop(&self, shared: &Shared) {
        shared.bed.start.store(NONE, Ordering::Release);
    }
}

impl BedConsumer {
    /// A new epoch empties the ring (every callback, playing or not).
    pub fn adopt(&mut self, shared: &Shared) {
        let epoch = shared.bed.epoch.load(Ordering::Acquire);
        if epoch == self.epoch {
            return;
        }
        let count = self.consumer.slots();
        if let Ok(chunk) = self.consumer.read_chunk(count) {
            chunk.commit_all();
        }
        (self.epoch, self.taken) = (epoch, 0);
        shared.bed.ack.store(epoch, Ordering::Release);
    }

    /// The bed over `output` (interleaved, `channels` wide), whose first frame plays at
    /// absolute sample `first`. A frame with no bed sample yet is silent on that output.
    pub fn mix(&mut self, shared: &Shared, output: &mut [f32], channels: usize, first: u64) {
        let start = shared.bed.start.load(Ordering::Acquire);
        let channel = shared.bed.output.load(Ordering::Acquire) as usize;
        if start == NONE || channel >= channels || channels == 0 {
            return;
        }
        for (index, frame) in output.chunks_exact_mut(channels).enumerate() {
            let position = first + index as u64;
            if position < start {
                continue;
            }
            let wanted = position - start;
            // Late: drop what should have played already.
            while self.taken < wanted {
                if self.consumer.pop().is_err() {
                    break;
                }
                self.taken += 1;
            }
            frame[channel] = match (self.taken == wanted).then(|| self.consumer.pop().ok()).flatten() {
                Some(sample) => {
                    self.taken += 1;
                    sample
                }
                None => 0.0,
            };
        }
    }
}
