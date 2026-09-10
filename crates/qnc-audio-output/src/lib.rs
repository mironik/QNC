//! Prepared PCM -> device output. No application, database, media decoder or playback clock.
mod channel_map;
mod device;
pub use channel_map::ChannelMap;
mod model;
mod queue;
pub use device::DeviceInfo;
pub use model::*;
use queue::Queue;
use std::{
    sync::{atomic::Ordering, mpsc},
    thread,
    time::{Duration, Instant},
};
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const ACK_TIMEOUT: Duration = Duration::from_secs(3);

pub struct AudioOutput {
    queue: Queue,
    info: DeviceInfo,
    stop: Option<mpsc::Sender<()>>,
    worker: Option<thread::JoinHandle<()>>,
}
impl AudioOutput {
    /// Runs device creation on its owning worker, starts silence, and confirms a callback.
    /// Synchronous preparation: call from a player/module worker, never a UI thread.
    pub fn open(config: Config) -> Result<Self> {
        let (queue, callback) = Queue::new(config.clone())?;
        let (stop, receiver) = mpsc::channel();
        let (ready, result) = mpsc::sync_channel(1);
        let shared = queue.shared.clone();
        let worker = thread::Builder::new()
            .name("qnc-audio-device".into())
            .spawn(
                move || match device::open(&config, callback, shared.clone()) {
                    Ok((stream, info)) => {
                        if ready.send(Ok(info)).is_ok() {
                            let _ = receiver.recv();
                        }
                        drop(stream);
                    }
                    Err(error) => {
                        shared.device_failed.store(true, Ordering::Release);
                        let _ = ready.send(Err(error));
                    }
                },
            )
            .map_err(|e| Error::new(Code::Device, e.to_string()))?;
        let opened = result
            .recv()
            .map_err(|_| Error::new(Code::Device, "device worker ended"));
        match opened.and_then(|value| value) {
            Ok(info) => {
                let mut output = Self {
                    queue,
                    info,
                    stop: Some(stop),
                    worker: Some(worker),
                };
                output.wait_ack()?;
                Ok(output)
            }
            Err(error) => {
                drop(stop);
                let _ = worker.join();
                Err(error)
            }
        }
    }
    pub fn devices() -> Result<Vec<DeviceInfo>> {
        device::list()
    }
    pub fn device(&self) -> &DeviceInfo {
        &self.info
    }
    /// Start a new sample sequence after the callback has discarded the previous generation.
    pub fn begin(&mut self, first_sample_frame: u64) -> Result<u64> {
        let generation = self.queue.reset(first_sample_frame, Status::Preparing)?;
        self.wait_ack()?;
        Ok(generation)
    }
    pub fn queue(
        &mut self,
        generation: u64,
        first_sample_frame: u64,
        samples: &[f32],
    ) -> Result<()> {
        self.queue.push(generation, first_sample_frame, samples)
    }
    pub fn finish(&mut self, generation: u64) -> Result<()> {
        self.queue.finish(generation)
    }
    pub fn commit(&mut self, generation: u64) -> Result<()> {
        self.queue.commit(generation)
    }
    pub fn start(&mut self, generation: u64) -> Result<()> {
        self.queue.start(generation)
    }
    /// Gate silence and await the flush acknowledgement; keep the existing device stream.
    pub fn pause(&mut self) -> Result<()> {
        self.queue.reset(0, Status::Paused)?;
        self.wait_ack()
    }
    pub fn telemetry(&self) -> Telemetry {
        self.queue.telemetry()
    }
    pub fn driver_timing(&self) -> Option<DriverTiming> {
        self.queue.driver_timing()
    }
    /// Device-estimated media time. Does not advance past submitted PCM or invent a new clock.
    pub fn playback_position_ns(&self) -> u128 {
        self.queue.playback_position_ns()
    }
    fn wait_ack(&mut self) -> Result<()> {
        let deadline = Instant::now() + ACK_TIMEOUT;
        loop {
            if self.queue.shared.device_failed.load(Ordering::Acquire) {
                return Err(Error::new(Code::Device, "device failed during preparation"));
            }
            if self.queue.acknowledged() {
                return self.queue.health();
            }
            if Instant::now() >= deadline {
                self.queue
                    .shared
                    .device_failed
                    .store(true, Ordering::Release);
                return Err(Error::new(
                    Code::Timeout,
                    "audio callback did not acknowledge preparation",
                ));
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
}
impl Drop for AudioOutput {
    fn drop(&mut self) {
        self.queue.shared.gate.fetch_and(!1, Ordering::AcqRel);
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests;
