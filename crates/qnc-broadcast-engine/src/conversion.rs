use qnc_gpu_raster::GpuRasterConverter;
use qnc_pixel_convert::{ConversionError, RasterConverter, RasterTiming};
use std::{
    sync::{
        Arc,
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    task::Poll,
    thread::{self, JoinHandle},
    time::Instant,
};

pub(super) struct Job {
    pub generation: u64,
    pub frame: u64,
    pub slot: usize,
    pub input: Vec<u8>,
    pub rgba: Arc<[u8]>,
}

pub(super) struct Completed {
    pub generation: u64,
    pub frame: u64,
    pub slot: usize,
    pub rgba: Arc<[u8]>,
    pub elapsed_us: u128,
    pub result: Result<(), ConversionError>,
}

pub(super) enum Raster {
    #[cfg_attr(not(test), allow(dead_code))]
    Cpu(RasterConverter),
    Gpu(GpuRasterConverter),
}
impl Raster {
    pub fn size(&self) -> [u32; 2] {
        match self {
            Self::Cpu(c) => c.size(),
            Self::Gpu(c) => c.size(),
        }
    }
    pub fn output_bytes(&self) -> usize {
        match self {
            Self::Cpu(c) => c.output_bytes(),
            Self::Gpu(c) => c.output_bytes(),
        }
    }
    fn convert(&mut self, input: &[u8], rgba: &mut [u8]) -> Result<RasterTiming, ConversionError> {
        match self {
            Self::Cpu(c) => c.convert_timed(input, rgba),
            Self::Gpu(c) => {
                c.convert(input, rgba)?;
                Ok(RasterTiming::default())
            }
        }
    }
}

/// Bounded conversion pipeline using borrowed pool slots, never the playback thread.
pub(super) struct ConversionWorker {
    requests: Option<SyncSender<(Instant, Job)>>,
    results: Receiver<Completed>,
    worker: Option<JoinHandle<()>>,
    pending: usize,
    capacity: usize,
}

impl ConversionWorker {
    #[cfg(test)]
    pub fn new(converter: Raster) -> std::io::Result<Self> {
        Self::with_capacity(converter, 1)
    }

    pub fn with_capacity(mut converter: Raster, capacity: usize) -> std::io::Result<Self> {
        let capacity = capacity.max(1);
        let (requests, jobs) = mpsc::sync_channel::<(Instant, Job)>(capacity);
        let (completed, results) = mpsc::sync_channel(capacity);
        let worker = thread::Builder::new()
            .name("player-convert".into())
            .spawn(move || {
                let backend = match &converter {
                    Raster::Cpu(_) => "cpu",
                    Raster::Gpu(gpu) => {
                        if qnc_dev_diagnostics::player_diagnostics_enabled() {
                            qnc_dev_diagnostics::log_line(
                                qnc_dev_diagnostics::DiagnosticsStream::Player,
                                format!("player-converter backend=gpu adapter={}", gpu.adapter_name()),
                            );
                        }
                        "gpu"
                    }
                };
                let mut samples = 0u128;
                let mut sums = [0u128; 4];
                let mut maxima = [0u128; 4];
                let mut inflight: Option<(Job, qnc_gpu_raster::GpuReadback, Instant)> = None;
                let mut leftover: Option<(Instant, Job)> = None;
                loop {
                    let (queued, mut job) = match leftover.take() {
                        Some(next) => next,
                        None => match jobs.recv() {
                            Ok(next) => next,
                            Err(_) => break,
                        },
                    };
                    let start = Instant::now();
                    let wait_us = queued.elapsed().as_micros();
                    if let Raster::Gpu(gpu) = &mut converter {
                        match gpu.enqueue(&job.input) {
                            Ok(pending) => {
                                if let Some((mut previous, previous_wait, previous_start)) =
                                    inflight.take()
                                {
                                    let previous_result = Arc::get_mut(&mut previous.rgba)
                                        .ok_or(ConversionError::Payload)
                                        .and_then(|rgba| gpu.collect(previous_wait, rgba));
                                    let previous_elapsed = previous_start.elapsed().as_micros();
                                    if completed
                                        .try_send(Completed {
                                            generation: previous.generation,
                                            frame: previous.frame,
                                            slot: previous.slot,
                                            rgba: previous.rgba,
                                            elapsed_us: previous_elapsed,
                                            result: previous_result.map(|_| ()),
                                        })
                                        .is_err()
                                    {
                                        inflight = None;
                                        break;
                                    }
                                }
                                inflight = Some((job, pending, start));
                                match jobs.try_recv() {
                                    Ok(next) => leftover = Some(next),
                                    Err(TryRecvError::Empty) => {
                                        if let Some((
                                            mut previous,
                                            previous_wait,
                                            previous_start,
                                        )) = inflight.take()
                                        {
                                            let previous_result =
                                                Arc::get_mut(&mut previous.rgba)
                                                    .ok_or(ConversionError::Payload)
                                                    .and_then(|rgba| {
                                                        gpu.collect(previous_wait, rgba)
                                                    });
                                            if completed
                                                .try_send(Completed {
                                                    generation: previous.generation,
                                                    frame: previous.frame,
                                                    slot: previous.slot,
                                                    rgba: previous.rgba,
                                                    elapsed_us: previous_start
                                                        .elapsed()
                                                        .as_micros(),
                                                    result: previous_result.map(|_| ()),
                                                })
                                                .is_err()
                                            {
                                                break;
                                            }
                                        }
                                    }
                                    Err(TryRecvError::Disconnected) => break,
                                }
                                continue;
                            }
                            Err(error) => {
                                if completed
                                    .try_send(Completed {
                                        generation: job.generation,
                                        frame: job.frame,
                                        slot: job.slot,
                                        rgba: job.rgba,
                                        elapsed_us: start.elapsed().as_micros(),
                                        result: Err(error),
                                    })
                                    .is_err()
                                {
                                    break;
                                }
                                continue;
                            }
                        }
                    }
                    let result = Arc::get_mut(&mut job.rgba)
                        .ok_or(ConversionError::Payload)
                        .and_then(|rgba| converter.convert(&job.input, rgba));
                    let elapsed_us = start.elapsed().as_micros();
                    if qnc_dev_diagnostics::player_diagnostics_enabled()
                        && let Ok(timing) = &result
                    {
                        samples += 1;
                        for (i, value) in [timing.resize_us, timing.color_us, wait_us, elapsed_us]
                            .into_iter()
                            .enumerate()
                        {
                            sums[i] += value;
                            maxima[i] = maxima[i].max(value);
                        }
                        if samples == 100 {
                            qnc_dev_diagnostics::log_line(
                                qnc_dev_diagnostics::DiagnosticsStream::Player,
                                format!(
                                    "player-conversion backend={} frame={} samples={} resize_avg_us={} color_avg_us={} queue_avg_us={} total_avg_us={} resize_max_us={} color_max_us={} queue_max_us={} total_max_us={}",
                                    backend,
                                    job.frame,
                                    samples,
                                    sums[0] / samples,
                                    sums[1] / samples,
                                    sums[2] / samples,
                                    sums[3] / samples,
                                    maxima[0],
                                    maxima[1],
                                    maxima[2],
                                    maxima[3]
                                ),
                            );
                            samples = 0;
                            sums = [0; 4];
                            maxima = [0; 4];
                        }
                    }
                    if completed
                        .try_send(Completed {
                            generation: job.generation,
                            frame: job.frame,
                            slot: job.slot,
                            rgba: job.rgba,
                            elapsed_us,
                            result: result.map(|_| ()),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                if let Some((mut previous, previous_wait, previous_start)) = inflight.take()
                    && let Raster::Gpu(gpu) = &mut converter
                {
                    let previous_result = Arc::get_mut(&mut previous.rgba)
                        .ok_or(ConversionError::Payload)
                        .and_then(|rgba| gpu.collect(previous_wait, rgba));
                    let _ = completed.try_send(Completed {
                        generation: previous.generation,
                        frame: previous.frame,
                        slot: previous.slot,
                        rgba: previous.rgba,
                        elapsed_us: previous_start.elapsed().as_micros(),
                        result: previous_result.map(|_| ()),
                    });
                }
            })?;
        Ok(Self {
            requests: Some(requests),
            results,
            worker: Some(worker),
            pending: 0,
            capacity,
        })
    }

    pub fn busy(&self) -> bool {
        self.pending > 0
    }

    pub fn can_accept(&self) -> bool {
        self.pending < self.capacity
    }

    #[cfg(test)]
    pub fn pending(&self) -> usize {
        self.pending
    }

    pub fn submit(&mut self, job: Job) -> Result<(), &'static str> {
        if !self.can_accept() {
            return Err("conversion queue full");
        }
        self.requests
            .as_ref()
            .ok_or("conversion closed")?
            .try_send((Instant::now(), job))
            .map_err(|_| "conversion queue unavailable")?;
        self.pending += 1;
        Ok(())
    }

    pub fn poll(&mut self) -> Result<Poll<Completed>, &'static str> {
        match self.results.try_recv() {
            Ok(result) => {
                self.pending = self.pending.saturating_sub(1);
                Ok(Poll::Ready(result))
            }
            Err(TryRecvError::Empty) => Ok(Poll::Pending),
            Err(TryRecvError::Disconnected) => Err("conversion worker closed"),
        }
    }
}

impl Drop for ConversionWorker {
    fn drop(&mut self) {
        self.requests.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_pixel_convert::{ConversionSpec, Converter, PixelLayout, Range, Transfer, VERSION};

    fn worker() -> ConversionWorker {
        ConversionWorker::new(cpu_raster()).unwrap()
    }

    fn cpu_raster() -> Raster {
        let spec = ConversionSpec {
            version: VERSION.into(),
            width: 2,
            height: 2,
            layout: PixelLayout::Yuv420p,
            primaries: "bt709".into(),
            matrix: "bt709".into(),
            scan_mode: qnc_media_metadata::ScanMode::Progressive,
            range: Range::Limited,
            transfer: Transfer::Bt709,
        };
        Raster::Cpu(
            RasterConverter::prepare(
                Converter::prepare(spec.clone(), spec.scratch_bytes().unwrap()).unwrap(),
                None,
            )
            .unwrap(),
        )
    }

    fn job(generation: u64, frame: u64) -> Job {
        Job {
            generation,
            frame,
            slot: 0,
            input: vec![16, 16, 16, 16, 128, 128],
            rgba: vec![0; 16].into(),
        }
    }

    #[test]
    fn bounded_conversion_returns_same_pool_slot_and_source_generation() {
        let mut worker = worker();
        let input = job(7, 42);
        let pointer = input.rgba.as_ptr();
        worker.submit(input).unwrap();
        assert!(worker.submit(job(8, 100)).is_err());
        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if let Poll::Ready(done) = worker.poll().unwrap() {
                done.result.unwrap();
                assert_eq!((done.generation, done.frame, done.slot), (7, 42, 0));
                assert_eq!(done.rgba.as_ptr(), pointer);
                assert_eq!(
                    &*done.rgba,
                    &[0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255]
                );
                assert!(!worker.busy());
                assert_eq!(worker.pending(), 0);
                break;
            }
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
    }

    #[test]
    fn shutdown_does_not_wait_for_a_consumer_to_drain_the_result() {
        let mut worker = worker();
        worker.submit(job(1, 0)).unwrap();
        drop(worker);
    }

    #[test]
    fn bounded_pipeline_accepts_more_than_one_pending_conversion() {
        let mut worker = ConversionWorker::with_capacity(cpu_raster(), 2).unwrap();
        worker.submit(job(1, 0)).unwrap();
        worker.submit(job(1, 1)).unwrap();
        assert!(worker.submit(job(1, 2)).is_err());
        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        let mut frames = Vec::new();
        while frames.len() < 2 {
            if let Poll::Ready(done) = worker.poll().unwrap() {
                done.result.unwrap();
                frames.push(done.frame);
            }
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        assert_eq!(frames, [0, 1]);
        assert_eq!(worker.pending(), 0);
    }
}
