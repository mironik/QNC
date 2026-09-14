//! One player owner composing public media/output adapters. No application workflow or DB writes.
#[cfg(test)]
mod av_sync;
mod conversion;
mod decode_input;
mod input;
mod output;
pub use decode_input::DecodeMediaAccess;
use decode_input::{DecodeInput, seek_start};
pub use input::InputPlan;
use input::{relative_position, sample_boundary};
use output::{Audio, Presenter, SharedVideo, VideoSink};
use qnc_broadcast_player::*;
pub use qnc_broadcast_player::{BroadcastEngineError, BroadcastEngineErrorKind};
use qnc_media_decode::{DecodeRequest, DecodedFormat, Decoder, DecoderConfig};
use qnc_media_stream::MediaStream;
use qnc_video_output::{FrameHeader, OutputConfig, PixelFormat, PreparedFrame, VideoOutput};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, task::Poll, time::Instant};

pub type Result<T> = std::result::Result<T, BroadcastEngineError>;
fn error(e: impl std::fmt::Display) -> BroadcastEngineError {
    BroadcastEngineError::new(BroadcastEngineErrorKind::Contract, e.to_string())
}
fn pending() -> BroadcastEngineError {
    BroadcastEngineError::new(
        BroadcastEngineErrorKind::NotReady,
        "packet is still being prepared",
    )
}
struct PictureData {
    token: Option<PreparedFrame>,
    _buffer: std::sync::Arc<[u8]>,
}
type Picture = Rc<PictureData>;
type Engine = TransportEngine<Source, Video, SplitAvPlayoutOutput<Audio, Presenter>>;

enum VideoRaster {
    Worker(conversion::ConversionWorker),
    Direct(qnc_gpu_raster::GpuTextureRasterizer),
}

/// Created and driven on the one player owner thread, never on an application form thread.
pub struct Runtime {
    engine: Engine,
    gpu: SharedVideo,
    audio: Option<Rc<RefCell<qnc_audio_output::AudioOutput>>>,
    epoch: Instant,
}
impl Runtime {
    pub fn open(
        plan: InputPlan,
        video_output: VideoOutput,
        output_config: OutputConfig,
        decoder_config: DecoderConfig,
        audio_device_id: Option<String>,
        mut open_media: impl FnMut(&str) -> std::io::Result<MediaStream> + 'static,
    ) -> Result<Self> {
        Self::open_access(
            plan,
            video_output,
            output_config,
            decoder_config,
            audio_device_id,
            move |uri| open_media(uri).map(DecodeMediaAccess::Stream),
        )
    }
    pub fn open_access(
        plan: InputPlan,
        video_output: VideoOutput,
        output_config: OutputConfig,
        decoder_config: DecoderConfig,
        audio_device_id: Option<String>,
        open_media: impl FnMut(&str) -> std::io::Result<DecodeMediaAccess> + 'static,
    ) -> Result<Self> {
        Self::open_output(
            plan,
            Some(video_output),
            output_config,
            decoder_config,
            audio_device_id,
            open_media,
        )
    }
    /// Publish confirmed frames for a passive monitor without an offscreen GPU draw.
    pub fn open_monitor(
        plan: InputPlan,
        output_config: OutputConfig,
        decoder_config: DecoderConfig,
        audio_device_id: Option<String>,
        mut open_media: impl FnMut(&str) -> std::io::Result<MediaStream> + 'static,
    ) -> Result<Self> {
        Self::open_monitor_access(
            plan,
            output_config,
            decoder_config,
            audio_device_id,
            move |uri| open_media(uri).map(DecodeMediaAccess::Stream),
        )
    }
    pub fn open_monitor_access(
        plan: InputPlan,
        output_config: OutputConfig,
        decoder_config: DecoderConfig,
        audio_device_id: Option<String>,
        open_media: impl FnMut(&str) -> std::io::Result<DecodeMediaAccess> + 'static,
    ) -> Result<Self> {
        Self::open_output(
            plan,
            None,
            output_config,
            decoder_config,
            audio_device_id,
            open_media,
        )
    }
    fn open_output(
        plan: InputPlan,
        video_output: Option<VideoOutput>,
        output_config: OutputConfig,
        decoder_config: DecoderConfig,
        audio_device_id: Option<String>,
        open_media: impl FnMut(&str) -> std::io::Result<DecodeMediaAccess> + 'static,
    ) -> Result<Self> {
        output_config.validate().map_err(error)?;
        let prebuffer_frames = input::monitor_prebuffer_frames(plan.source.timebase)?;
        if qnc_dev_diagnostics::player_diagnostics_enabled() {
            qnc_dev_diagnostics::log_line(
                qnc_dev_diagnostics::DiagnosticsStream::Player,
                format!(
                    "player-open source={} media={} codec={} pixel={} timebase={}/{} frames={} prebuffer={}",
                    plan.source.source_id,
                    plan.media.media_uri,
                    saved_video_codec(&plan.media, plan.video_index),
                    plan.spec.layout.name(),
                    plan.source.timebase.fps_num,
                    plan.source.timebase.fps_den,
                    plan.source.duration_frames,
                    prebuffer_frames,
                ),
            );
        }
        if output_config.width != plan.spec.width
            || output_config.height != plan.spec.height
            || output_config.slots < prebuffer_frames.saturating_add(4)
        {
            return Err(error(
                "output configuration differs from saved input or required pool",
            ));
        }
        let request = |media: &qnc_media_metadata::MediaRepresentation, index| DecodeRequest {
            version: qnc_media_decode::VERSION.into(),
            media: media.clone(),
            stream_index: index,
            start: None,
        };
        request(&plan.media, plan.video_index)
            .validate(&decoder_config)
            .map_err(error)?;
        output::validate_channel_map(
            plan.source.audio_format.as_ref(),
            plan.audio_channels.as_ref(),
        )?;
        for stream in &plan.audio_streams {
            request(&plan.audio_media, stream.stream_index)
                .validate(&decoder_config)
                .map_err(error)?;
        }
        let (raster, raster_size, output_bytes) = if let Some(output) = video_output.as_ref() {
            let direct = qnc_gpu_raster::GpuTextureRasterizer::prepare_on_device(
                output.device(),
                output.queue(),
                plan.spec.clone(),
                [plan.spec.width, plan.spec.height],
            )
            .map_err(error)?;
            (
                VideoRaster::Direct(direct),
                [plan.spec.width, plan.spec.height],
                0,
            )
        } else {
            let converter = conversion::Raster::Gpu(
                qnc_gpu_raster::GpuRasterConverter::prepare(
                    plan.spec.clone(),
                    input::preview_raster_bounds(plan.spec.width, plan.spec.height),
                )
                .map_err(error)?,
            );
            let raster_size = converter.size();
            let output_bytes = converter.output_bytes();
            (
                VideoRaster::Worker(
                    conversion::ConversionWorker::with_capacity(
                        converter,
                        input::CONVERT_IN_FLIGHT.min(prebuffer_frames),
                    )
                    .map_err(error)?,
                ),
                raster_size,
                output_bytes,
            )
        };
        let rgba = if output_bytes == 0 {
            Vec::new()
        } else {
            (0..output_config.slots.max(prebuffer_frames + 4))
                .map(|_| Some(std::sync::Arc::from(vec![0; output_bytes])))
                .collect()
        };
        let decode_input = Rc::new(DecodeInput::new_access(
            plan.media.clone(),
            decoder_config,
            open_media,
        ));
        let audio_input = Rc::new(decode_input.for_media(plan.audio_media.clone()));
        let audio = Audio::open(
            &plan,
            audio_input,
            audio_device_id,
            plan.audio_channels.clone(),
            prebuffer_frames,
        )?;
        let video_decoder = decode_input.open(plan.video_index, None)?;
        let device = audio.device.clone();
        let gpu = Rc::new(RefCell::new(VideoSink {
            output: video_output,
            config: output_config,
            images: BTreeMap::new(),
            sequence: 0,
            inflight: false,
            submit_us: 0,
            conversion_us: 0,
            upload_us: 0,
            converted: 0,
        }));
        let source = plan.source.clone();
        let mut engine = TransportEngine::new(
            Source(source.clone()),
            Video {
                plan,
                decoder: video_decoder,
                input: decode_input,
                pending_seek: None,
                discard_before: None,
                raster,
                raster_size,
                rgba,
                ready: BTreeMap::new(),
                next_decode_frame: 0,
                prefetch_frames: prebuffer_frames,
                gpu: gpu.clone(),
            },
            audio,
            Presenter(gpu.clone()),
        )
        // Catch up bounded source-frame gaps while the converter stays queued ahead.
        .with_decode_burst_frames(4)
        .with_min_prebuffer_frames(prebuffer_frames);
        engine.load_source(&source, None)?;
        Ok(Self {
            engine,
            gpu,
            audio: device,
            epoch: Instant::now(),
        })
    }
    pub fn state(&self) -> &TransportEngineState {
        self.engine.state()
    }
    pub fn audio_telemetry(&self) -> Option<qnc_audio_output::Telemetry> {
        self.audio.as_ref().map(|d| d.borrow().telemetry())
    }
    pub fn audio_driver_timing(&self) -> Option<qnc_audio_output::DriverTiming> {
        self.audio.as_ref().and_then(|d| d.borrow().driver_timing())
    }
    pub fn last_submission_us(&self) -> u128 {
        self.gpu.borrow().submit_us
    }
    pub fn play(&mut self) -> Result<Vec<BroadcastEvent>> {
        self.engine.play(self.playback_tick())
    }
    fn playback_tick(&self) -> u128 {
        self.audio.as_ref().map_or_else(
            || self.epoch.elapsed().as_nanos(),
            |device| device.borrow().playback_position_ns(),
        )
    }
    pub fn pause(&mut self) -> Result<Vec<BroadcastEvent>> {
        self.engine.pause()
    }
    pub fn stop(&mut self) -> Result<Vec<BroadcastEvent>> {
        self.engine.stop()
    }
    pub fn cue_frame(&mut self, frame: u64, present: bool) -> Result<Vec<BroadcastEvent>> {
        self.engine.cue_frame(frame, present)
    }
    pub fn tick(&mut self) -> Result<Vec<BroadcastEvent>> {
        if let Some(device) = &self.audio
            && device.borrow().telemetry().status == qnc_audio_output::Status::Failed
        {
            if qnc_dev_diagnostics::player_diagnostics_enabled() {
                let gpu = self.gpu.borrow();
                qnc_dev_diagnostics::log_line(
                    qnc_dev_diagnostics::DiagnosticsStream::Player,
                    format!(
                        "player-rebuffer reason=audio_underrun frame={} converted={} conversion_avg_us={}",
                        self.engine.state().carrier_frame,
                        gpu.converted,
                        gpu.conversion_us / u128::from(gpu.converted.max(1))
                    ),
                );
            }
            if let Err(e) = self.engine.rearm_audio_after_underrun() {
                if qnc_dev_diagnostics::player_diagnostics_enabled() {
                    qnc_dev_diagnostics::log_line(
                        qnc_dev_diagnostics::DiagnosticsStream::Player,
                        format!("player-rebuffer audio_rearm_failed={e}"),
                    );
                }
            }
        }
        self.gpu.borrow_mut().poll_and_collect()?;
        if self.engine.state().at_end {
            return Ok(Vec::new());
        }
        // Native video submission may still be pending. Continue bounded AV
        // preparation; an unavailable output at its deadline is an error, not a
        // reason to stop servicing the audio queue or advance a second clock.
        let events = match self.engine.tick(self.playback_tick()) {
            Ok(events) => events,
            Err(e) if e.kind == BroadcastEngineErrorKind::NotReady => {
                let frame = self.engine.state().carrier_frame;
                if qnc_dev_diagnostics::player_diagnostics_enabled() {
                    let gpu = self.gpu.borrow();
                    qnc_dev_diagnostics::log_line(
                        qnc_dev_diagnostics::DiagnosticsStream::Player,
                        format!(
                            "player-rebuffer reason=not_ready frame={} converted={} conversion_avg_us={} upload_avg_us={}",
                            frame,
                            gpu.converted,
                            gpu.conversion_us / u128::from(gpu.converted.max(1)),
                            gpu.upload_us / u128::from(gpu.converted.max(1))
                        ),
                    );
                }
                return Ok(Vec::new());
            }
            Err(e) => return Err(self.enrich_tick_error(e)),
        };
        if events.iter().any(|event| {
            matches!(
                event,
                BroadcastEvent::PlaybackError { .. } | BroadcastEvent::DecodeWarning { .. }
            )
        }) {
            return Err(error(format!("player preparation failed: {events:?}")));
        }
        Ok(events)
    }

    fn enrich_tick_error(&self, e: BroadcastEngineError) -> BroadcastEngineError {
        let gpu = self.gpu.borrow();
        BroadcastEngineError::new(
            e.kind,
            format!(
                "{e}; converted={}; conversion_avg_us={}; upload_avg_us={}",
                gpu.converted,
                gpu.conversion_us / u128::from(gpu.converted.max(1)),
                gpu.upload_us / u128::from(gpu.converted.max(1))
            ),
        )
        .with_frame(e.frame.unwrap_or(self.engine.state().carrier_frame))
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.engine.pause();
    }
}

fn saved_video_codec(media: &qnc_media_metadata::MediaRepresentation, stream_index: u32) -> String {
    media
        .streams
        .iter()
        .find_map(
            |stream| match (&stream.details, &stream.codec, &stream.index) {
                (qnc_media_metadata::StreamDetails::Video(_), Some(codec), Some(index))
                    if index.value == stream_index =>
                {
                    match &codec.value {
                        qnc_media_metadata::Signal::Known(value) => Some(value.clone()),
                        qnc_media_metadata::Signal::Unspecified => Some("unspecified".into()),
                    }
                }
                _ => None,
            },
        )
        .unwrap_or_else(|| "missing".into())
}

struct Source(SourceRuntime);
impl SourceOpenAdapter for Source {
    fn open_source(
        &mut self,
        source: &SourceRuntime,
        revision: Option<u64>,
    ) -> Result<EngineSourceHandle> {
        if source != &self.0 || revision.is_some() {
            return Err(error("source differs from saved input"));
        }
        Ok(EngineSourceHandle::from_source_runtime(source, revision))
    }
    fn close_source(&mut self, _: &str) -> Result<()> {
        Ok(())
    }
}
struct Video {
    plan: InputPlan,
    decoder: Decoder,
    input: Rc<DecodeInput>,
    pending_seek: Option<u64>,
    discard_before: Option<u64>,
    raster: VideoRaster,
    raster_size: [u32; 2],
    rgba: Vec<Option<std::sync::Arc<[u8]>>>,
    ready: BTreeMap<u64, DecodedVideoFrame<Picture>>,
    next_decode_frame: u64,
    prefetch_frames: usize,
    gpu: SharedVideo,
}
impl VideoDecodeAdapter for Video {
    type VideoFrame = Picture;
    fn cue_video(&mut self, request: EngineFrameRequest) -> Result<()> {
        if request.source_id != self.plan.source.source_id
            || request.timebase != self.plan.source.timebase
        {
            return Err(error("seek source mismatch"));
        }
        seek_start(&self.plan.source, request.frame)?;
        self.pending_seek = Some(request.frame);
        Ok(())
    }
    fn prepare_video(&mut self, _: &EngineSourceHandle) -> Result<Vec<BroadcastEvent>> {
        Ok(Vec::new())
    }
    fn decode_video_frame(
        &mut self,
        request: EngineFrameRequest,
    ) -> Result<DecodedVideoFrame<Picture>> {
        if let Some(frame) = self.pending_seek {
            if request.frame != frame {
                return Err(error("seek target mismatch"));
            }
            let mut gpu = self.gpu.borrow_mut();
            let generation = gpu
                .config
                .generation
                .checked_add(1)
                .ok_or_else(|| error("GPU generation exhausted"))?;
            if let Some(output) = &mut gpu.output {
                match output.reset(generation) {
                    Ok(()) => (),
                    Err(qnc_video_output::OutputError::Busy) => return Err(pending()),
                    Err(e) => return Err(error(e)),
                }
            }
            gpu.config.generation = generation;
            gpu.sequence = 0;
            gpu.images.clear();
            drop(gpu);
            self.decoder.cancel();
            self.decoder = self
                .input
                .open(self.plan.video_index, seek_start(&self.plan.source, frame)?)?;
            self.discard_before = Some(frame);
            self.ready.clear();
            self.next_decode_frame = frame;
            self.pending_seek = None;
        }
        self.drain_conversions()?;
        if let Some(frame) = self.ready.remove(&request.frame) {
            return Ok(frame);
        }
        self.fill_conversion_queue(&request)?;
        self.drain_conversions()?;
        if let Some(frame) = self.ready.remove(&request.frame) {
            return Ok(frame);
        }
        Err(pending())
    }
}

impl Video {
    fn drain_conversions(&mut self) -> Result<()> {
        loop {
            let completed = {
                let VideoRaster::Worker(worker) = &mut self.raster else {
                    return Ok(());
                };
                if !worker.busy() {
                    break;
                }
                match worker.poll().map_err(error)? {
                    Poll::Pending => break,
                    Poll::Ready(completed) => completed,
                }
            };
            self.rgba[completed.slot] = Some(completed.rgba.clone());
            // A seek can supersede conversion already running; recycle, never present it.
            if completed.generation != self.gpu.borrow().config.generation {
                continue;
            }
            completed.result.as_ref().map_err(error)?;
            self.store_completed(completed)?;
        }
        Ok(())
    }

    fn fill_conversion_queue(&mut self, request: &EngineFrameRequest) -> Result<()> {
        if request.source_id != self.plan.source.source_id
            || request.timebase != self.plan.source.timebase
        {
            return Err(error("video request differs from saved input"));
        }
        match &self.raster {
            VideoRaster::Worker(_) => self.fill_worker_queue(request),
            VideoRaster::Direct(_) => self.fill_direct_queue(request),
        }
    }

    fn fill_worker_queue(&mut self, request: &EngineFrameRequest) -> Result<()> {
        let target_end = request
            .frame
            .saturating_add(self.prefetch_frames as u64)
            .min(self.plan.source.duration_frames);
        while matches!(&self.raster, VideoRaster::Worker(worker) if worker.can_accept())
            && self.next_decode_frame < target_end
        {
            if self.ready.contains_key(&self.next_decode_frame) {
                self.next_decode_frame += 1;
                continue;
            }
            let Some(slot) = self.rgba.iter().position(|buffer| {
                buffer
                    .as_ref()
                    .is_some_and(|b| std::sync::Arc::strong_count(b) == 1)
            }) else {
                break;
            };
            let Some((frame, input)) = self.next_decoded_payload(request)? else {
                break;
            };
            let generation = self.gpu.borrow().config.generation;
            let VideoRaster::Worker(worker) = &mut self.raster else {
                return Err(error("raster path changed while filling worker queue"));
            };
            worker
                .submit(conversion::Job {
                    generation,
                    frame,
                    slot,
                    input,
                    rgba: self.rgba[slot].take().expect("exclusive frame buffer"),
                })
                .map_err(error)?;
        }
        Ok(())
    }

    fn fill_direct_queue(&mut self, request: &EngineFrameRequest) -> Result<()> {
        let target_end = request
            .frame
            .saturating_add(self.prefetch_frames as u64)
            .min(self.plan.source.duration_frames);
        while self.next_decode_frame < target_end {
            if self.ready.contains_key(&self.next_decode_frame) {
                self.next_decode_frame += 1;
                continue;
            }
            let has_slot = self
                .gpu
                .borrow()
                .output
                .as_ref()
                .ok_or_else(|| error("missing native video output"))?
                .has_free_slot()
                .map_err(error)?;
            if !has_slot {
                break;
            }
            let Some((frame, input)) = self.next_decoded_payload(request)? else {
                break;
            };
            self.store_direct(frame, input)?;
        }
        Ok(())
    }

    fn next_decoded_payload(
        &mut self,
        request: &EngineFrameRequest,
    ) -> Result<Option<(u64, Vec<u8>)>> {
        loop {
            let packet = match self.decoder.try_next_packet().map_err(error)? {
                Poll::Pending => return Ok(None),
                Poll::Ready(None) => {
                    if self.next_decode_frame >= self.plan.source.duration_frames {
                        return Ok(None);
                    }
                    return Err(error("video ended before saved frame boundary"));
                }
                Poll::Ready(Some(packet)) => packet,
            };
            let spec = &self.plan.spec;
            let frame = relative_position(
                packet.pts,
                packet.time_base,
                self.plan.origin,
                request.timebase.fps_num,
                request.timebase.fps_den,
            )?;
            if packet.stream_index != self.plan.video_index
                || packet.media_uri != self.plan.media.media_uri
                || packet.format
                    != (DecodedFormat::Video {
                        width: spec.width,
                        height: spec.height,
                        pixel_format: spec.layout.name().into(),
                    })
            {
                return Err(error("decoded frame does not match saved input/request"));
            }
            if let Some(discard_before) = self.discard_before
                && frame < discard_before
            {
                continue;
            }
            self.discard_before = None;
            if frame < self.next_decode_frame {
                continue;
            }
            if frame != self.next_decode_frame {
                return Err(error("decoder skipped requested source frame"));
            }
            self.next_decode_frame += 1;
            return Ok(Some((frame, packet.bytes)));
        }
    }

    fn store_completed(&mut self, completed: conversion::Completed) -> Result<()> {
        let frame = completed.frame;
        let mut gpu = self.gpu.borrow_mut();
        gpu.conversion_us += completed.elapsed_us;
        let upload_start = Instant::now();
        let header = FrameHeader {
            version: qnc_video_output::VERSION.into(),
            session_id: gpu.config.session_id.clone(),
            generation: completed.generation,
            sequence: gpu.sequence,
            source_id: self.plan.source.source_id.clone(),
            frame_number: frame,
            width: self.raster_size[0],
            height: self.raster_size[1],
            pixel_format: PixelFormat::Rgba8Srgb,
        };
        gpu.sequence = gpu
            .sequence
            .checked_add(1)
            .ok_or_else(|| error("frame sequence exhausted"))?;
        let token = Rc::new(PictureData {
            token: gpu
                .output
                .as_mut()
                .map(|output| output.prepare(header.clone(), &completed.rgba))
                .transpose()
                .map_err(error)?,
            _buffer: completed.rgba,
        });
        gpu.images.insert(frame, token.clone());
        gpu.upload_us += upload_start.elapsed().as_micros();
        gpu.converted += 1;
        self.ready.insert(
            frame,
            DecodedVideoFrame {
                source_id: self.plan.source.source_id.clone(),
                frame,
                video_format: self.plan.source.video_format.clone(),
                payload: token,
            },
        );
        Ok(())
    }

    fn store_direct(&mut self, frame: u64, input: Vec<u8>) -> Result<()> {
        let VideoRaster::Direct(raster) = &mut self.raster else {
            return Err(error("direct raster path required"));
        };
        let mut gpu = self.gpu.borrow_mut();
        let upload_start = Instant::now();
        let header = FrameHeader {
            version: qnc_video_output::VERSION.into(),
            session_id: gpu.config.session_id.clone(),
            generation: gpu.config.generation,
            sequence: gpu.sequence,
            source_id: self.plan.source.source_id.clone(),
            frame_number: frame,
            width: self.raster_size[0],
            height: self.raster_size[1],
            pixel_format: PixelFormat::Rgba8Srgb,
        };
        let token = match gpu
            .output
            .as_mut()
            .ok_or_else(|| error("missing native video output"))?
            .prepare_external(header.clone(), |texture| {
                raster
                    .convert_to_texture(&input, texture)
                    .map_err(|e| e.to_string())
            }) {
            Ok(token) => token,
            Err(qnc_video_output::OutputError::Full | qnc_video_output::OutputError::Busy) => {
                return Err(pending());
            }
            Err(e) => return Err(error(e)),
        };
        gpu.sequence = gpu
            .sequence
            .checked_add(1)
            .ok_or_else(|| error("frame sequence exhausted"))?;
        gpu.conversion_us += raster.enqueue_us();
        let token = Rc::new(PictureData {
            token: Some(token),
            _buffer: std::sync::Arc::<[u8]>::from(Vec::<u8>::new()),
        });
        gpu.images.insert(frame, token.clone());
        gpu.upload_us += upload_start.elapsed().as_micros();
        gpu.converted += 1;
        self.ready.insert(
            frame,
            DecodedVideoFrame {
                source_id: self.plan.source.source_id.clone(),
                frame,
                video_format: self.plan.source.video_format.clone(),
                payload: token,
            },
        );
        Ok(())
    }
}
