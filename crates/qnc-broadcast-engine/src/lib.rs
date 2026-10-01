//! One player owner composing public media/output adapters. No application workflow or DB writes.
#[cfg(test)]
mod av_sync;
mod conversion;
mod decode_input;
mod input;
mod output;
mod program;
pub use decode_input::DecodeMediaAccess;
use decode_input::{DecodeInput, seek_start};
pub use input::InputPlan;
use input::{relative_position, sample_boundary};
use output::{Audio, Presenter, SharedVideo, VideoSink};
pub use program::ProgramPlan;
use program::{ProgramAudio, ProgramVideo};
use qnc_broadcast_player::*;
pub use qnc_broadcast_player::{BroadcastEngineError, BroadcastEngineErrorKind};
use qnc_media_decode::{DecodeRequest, DecodedFormat, Decoder, DecoderConfig};
use qnc_media_stream::MediaStream;
use qnc_pixel_convert::{Converter, RasterConverter};
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
    header: FrameHeader,
    rgba: std::sync::Arc<[u8]>,
}
type Picture = Rc<PictureData>;
type Engine = TransportEngine<Source, VideoPath, SplitAvPlayoutOutput<AudioPath, Presenter>>;

/// The picture of the session: one clip, or a story program as one source.
enum VideoPath {
    Clip(Video),
    Program(ProgramVideo),
}
impl VideoDecodeAdapter for VideoPath {
    type VideoFrame = Picture;
    fn cue_video(&mut self, request: EngineFrameRequest) -> Result<()> {
        match self {
            Self::Clip(video) => video.cue_video(request),
            Self::Program(video) => video.cue_video(request),
        }
    }
    fn prepare_video(&mut self, source: &EngineSourceHandle) -> Result<Vec<BroadcastEvent>> {
        match self {
            Self::Clip(video) => video.prepare_video(source),
            Self::Program(video) => video.prepare_video(source),
        }
    }
    fn decode_video_frame(
        &mut self,
        request: EngineFrameRequest,
    ) -> Result<DecodedVideoFrame<Picture>> {
        match self {
            Self::Clip(video) => video.decode_video_frame(request),
            Self::Program(video) => video.decode_video_frame(request),
        }
    }
    fn stop_video(&mut self) -> Result<Vec<BroadcastEvent>> {
        match self {
            Self::Clip(video) => video.stop_video(),
            Self::Program(video) => video.stop_video(),
        }
    }
}

/// The sound of the session: one clip, or a story program as one source.
enum AudioPath {
    Clip(Audio),
    Program(ProgramAudio),
}
macro_rules! audio_path {
    ($self:ident, $audio:ident => $call:expr) => {
        match $self {
            AudioPath::Clip($audio) => $call,
            AudioPath::Program($audio) => $call,
        }
    };
}
impl AudioOutputAdapter for AudioPath {
    type AudioPacket = std::sync::Arc<[f32]>;
    fn cue_audio(&mut self, request: EngineFrameRequest) -> Result<()> {
        audio_path!(self, audio => audio.cue_audio(request))
    }
    fn prepare_audio(&mut self, source: &EngineSourceHandle) -> Result<Vec<BroadcastEvent>> {
        audio_path!(self, audio => audio.prepare_audio(source))
    }
    fn render_audio_for_frame(
        &mut self,
        request: EngineFrameRequest,
    ) -> Result<AudioFramePacket<std::sync::Arc<[f32]>>> {
        audio_path!(self, audio => audio.render_audio_for_frame(request))
    }
    fn submit_audio_packet(
        &mut self,
        packet: AudioFramePacket<std::sync::Arc<[f32]>>,
    ) -> Result<Vec<BroadcastEvent>> {
        audio_path!(self, audio => audio.submit_audio_packet(packet))
    }
    fn begin_audio_preroll(&mut self) -> Result<Vec<BroadcastEvent>> {
        audio_path!(self, audio => audio.begin_audio_preroll())
    }
    fn commit_audio_preroll(&mut self) -> Result<Vec<BroadcastEvent>> {
        audio_path!(self, audio => audio.commit_audio_preroll())
    }
    fn start_audio(&mut self) -> Result<Vec<BroadcastEvent>> {
        audio_path!(self, audio => audio.start_audio())
    }
    fn pause_audio(&mut self) -> Result<Vec<BroadcastEvent>> {
        audio_path!(self, audio => audio.pause_audio())
    }
    fn stop_audio(&mut self) -> Result<Vec<BroadcastEvent>> {
        audio_path!(self, audio => audio.stop_audio())
    }
}

/// Created and driven on the one player owner thread, never on an application form thread.
pub struct Runtime {
    engine: Engine,
    gpu: SharedVideo,
    audio: Option<Rc<RefCell<qnc_audio_output::AudioOutput>>>,
    epoch: Instant,
    /// An underrun rearm could not start audio yet; retried until it does.
    audio_rearm_pending: bool,
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
        let prebuffer_frames = if video_output.is_none() {
            input::monitor_prebuffer_frames(plan.source.timebase)?
        } else {
            input::PREBUFFER_FRAMES
        };
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
            || output_config.slots < input::OUTPUT_SLOTS
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
        let converter = if video_output.is_none() {
            conversion::Raster::Gpu(
                qnc_gpu_raster::GpuRasterConverter::prepare(
                    plan.spec.clone(),
                    input::preview_raster_bounds(plan.spec.width, plan.spec.height),
                )
                .map_err(error)?,
            )
        } else {
            let converter =
                Converter::prepare(plan.spec.clone(), plan.spec.scratch_bytes().map_err(error)?)
                    .map_err(error)?;
            conversion::Raster::Cpu(RasterConverter::prepare(converter, None).map_err(error)?)
        };
        let raster_size = converter.size();
        let rgba = (0..usize::from(output_config.slots).max(prebuffer_frames + 4) + STEP_BACK_FRAMES as usize)
            .map(|_| Some(std::sync::Arc::from(vec![0; converter.output_bytes()])))
            .collect();
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
        let device = audio.sink.device.clone();
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
            monitor: None,
            monitor_pending: Default::default(),
        }));
        let source = plan.source.clone();
        let mut engine = TransportEngine::new(
            Source(source.clone()),
            VideoPath::Clip(Video {
                plan,
                decoder: video_decoder,
                input: decode_input,
                pending_seek: None,
                discard_before: None,
                converter: conversion::ConversionWorker::with_capacity(
                    converter,
                    input::CONVERT_IN_FLIGHT.min(prebuffer_frames),
                )
                .map_err(error)?,
                raster_size,
                rgba,
                ready: BTreeMap::new(),
                next_decode_frame: 0,
                prefetch_frames: prebuffer_frames,
                gpu: gpu.clone(),
                kept: BTreeMap::new(),
                cue_anchor: 0,
            }),
            AudioPath::Clip(audio),
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
            audio_rearm_pending: false,
        })
    }

    /// A story program as one source for the passive monitor: the same transport,
    /// clock and commands as a clip, program frames instead of source frames.
    pub fn open_program_monitor(
        plan: ProgramPlan,
        output_config: OutputConfig,
        decoder_config: DecoderConfig,
        audio_device_id: Option<String>,
        open_media: impl FnMut(&str) -> std::io::Result<DecodeMediaAccess> + 'static,
    ) -> Result<Self> {
        output_config.validate().map_err(error)?;
        let prebuffer_frames = input::monitor_prebuffer_frames(plan.source.timebase)?;
        let first = plan
            .clips
            .first()
            .ok_or_else(|| error("program has no clip"))?;
        let root = DecodeInput::new_access(first.media.clone(), decoder_config, open_media);
        let inputs: Vec<_> = plan
            .clips
            .iter()
            .map(|clip| Rc::new(root.for_media(clip.media.clone())))
            .collect();
        // The decoder refuses an unsupported saved clip before playback, never mid-program.
        for (clip, input) in plan.clips.iter().zip(&inputs) {
            input.validate(clip.video_index)?;
        }
        let gpu = Rc::new(RefCell::new(VideoSink {
            output: None,
            config: output_config,
            images: BTreeMap::new(),
            sequence: 0,
            inflight: false,
            submit_us: 0,
            conversion_us: 0,
            upload_us: 0,
            converted: 0,
            monitor: None,
            monitor_pending: Default::default(),
        }));
        let video = ProgramVideo::open(&plan, &inputs, gpu.clone(), prebuffer_frames)?;
        let audio = ProgramAudio::open(&plan, &inputs, audio_device_id, prebuffer_frames)?;
        let device = audio.sink.device.clone();
        let source = plan.source.clone();
        let mut engine = TransportEngine::new(
            Source(source.clone()),
            VideoPath::Program(video),
            AudioPath::Program(audio),
            Presenter(gpu.clone()),
        )
        .with_decode_burst_frames(4)
        .with_min_prebuffer_frames(prebuffer_frames);
        engine.load_source(&source, None)?;
        Ok(Self {
            engine,
            gpu,
            audio: device,
            epoch: Instant::now(),
            audio_rearm_pending: false,
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
    /// Only a picture actually submitted by the player, never a predicted frame.
    pub fn monitor_frame(&self) -> Option<(FrameHeader, std::sync::Arc<[u8]>)> {
        self.gpu.borrow().monitor.clone()
    }
    /// Drain submitted monitor frames in order. `true` means the monitor was
    /// explicitly cleared, usually after a cue or source switch.
    pub fn take_monitor_frames(&mut self) -> (bool, Vec<(FrameHeader, std::sync::Arc<[u8]>)>) {
        let mut gpu = self.gpu.borrow_mut();
        let frames = gpu.take_monitor_pending();
        (gpu.monitor.is_none(), frames)
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
        let events = self.engine.cue_frame(frame, present)?;
        self.gpu.borrow_mut().clear_pending_monitor();
        Ok(events)
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
            match self.engine.rearm_audio_after_underrun() {
                Ok(()) => self.audio_rearm_pending = false,
                Err(e) => {
                    // The preroll keeps filling while playing; finish it on a later tick.
                    self.audio_rearm_pending = self.audio_waits_for_pcm();
                    if qnc_dev_diagnostics::player_diagnostics_enabled() {
                        qnc_dev_diagnostics::log_line(
                            qnc_dev_diagnostics::DiagnosticsStream::Player,
                            format!("player-rebuffer audio_rearm_failed={e}"),
                        );
                    }
                }
            }
        } else if self.audio_rearm_pending {
            match self.engine.finish_audio_rearm() {
                Ok(()) => {
                    self.audio_rearm_pending = false;
                    if qnc_dev_diagnostics::player_diagnostics_enabled() {
                        qnc_dev_diagnostics::log_line(
                            qnc_dev_diagnostics::DiagnosticsStream::Player,
                            format!(
                                "player-rebuffer audio_resumed frame={}",
                                self.engine.state().carrier_frame
                            ),
                        );
                    }
                }
                // Too little PCM yet: the queue is still being prepared.
                Err(_) if self.audio_waits_for_pcm() => {}
                Err(e) => {
                    self.audio_rearm_pending = false;
                    return Err(self.enrich_tick_error(e));
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

    /// The audio queue of a rearm exists but has not been started yet.
    fn audio_waits_for_pcm(&self) -> bool {
        self.audio.as_ref().is_some_and(|device| {
            device.borrow().telemetry().status == qnc_audio_output::Status::Preparing
        })
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
    converter: conversion::ConversionWorker,
    raster_size: [u32; 2],
    rgba: Vec<Option<std::sync::Arc<[u8]>>>,
    ready: BTreeMap<u64, DecodedVideoFrame<Picture>>,
    next_decode_frame: u64,
    prefetch_frames: usize,
    gpu: SharedVideo,
    /// Pictures this decoder already made (user rule 2026-10-01: the decoder pauses, it
    /// is not closed and opened again for a frame step). A step to a kept picture or
    /// just ahead of the decoder needs no new decoder.
    kept: BTreeMap<u64, DecodedVideoFrame<Picture>>,
    /// The frame the last cue went to: the kept pictures are the ones around it, and a
    /// cue before it is a step back.
    cue_anchor: u64,
}

/// Pictures kept behind the last asked frame, so frame steps back need no new decoder.
const STEP_BACK_FRAMES: u64 = 25;
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
        let mut stepping_back = false;
        if let Some(frame) = self.pending_seek {
            if request.frame != frame {
                return Err(error("seek target mismatch"));
            }
            stepping_back = frame < self.cue_anchor;
            self.cue_anchor = frame;
            self.forget_far_from(frame);
            if self.reached_without_reopen(frame) {
                // The decoder stays open and paused: a kept picture, or read on.
                self.pending_seek = None;
                self.ready.retain(|kept, _| *kept >= frame);
            }
        }
        if let Some(frame) = self.pending_seek {
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
            // Stepping back: the new decoder also keeps the pictures before the frame, so
            // the next steps back come from them.
            let from = if stepping_back { frame.saturating_sub(STEP_BACK_FRAMES) } else { frame };
            self.decoder.cancel();
            self.decoder = self
                .input
                .open(self.plan.video_index, seek_start(&self.plan.source, from)?)?;
            self.discard_before = Some(from);
            self.ready.clear();
            self.kept.clear();
            self.next_decode_frame = from;
            self.pending_seek = None;
        }
        self.drain_conversions()?;
        if let Some(frame) = self.ready.remove(&request.frame) {
            self.forget_far_from(request.frame);
            return self.stamped(frame).ok_or_else(|| error("frame sequence exhausted"));
        }
        if let Some(frame) = self.kept_again(request.frame) {
            self.forget_far_from(request.frame);
            return Ok(frame);
        }
        if request.frame < self.next_decode_frame && self.pending_seek.is_none() && !self.converter.busy() {
            // Behind the open decoder and no longer kept: only then a new decoder.
            self.pending_seek = Some(request.frame);
            return self.decode_video_frame(request);
        }
        self.fill_conversion_queue(&request)?;
        self.drain_conversions()?;
        if let Some(frame) = self.ready.remove(&request.frame) {
            self.forget_far_from(request.frame);
            return self.stamped(frame).ok_or_else(|| error("frame sequence exhausted"));
        }
        Err(pending())
    }
}

impl Video {
    /// A cue the open decoder reaches: a kept picture, or a frame not far ahead of it.
    fn reached_without_reopen(&self, frame: u64) -> bool {
        let reusable = self.gpu.borrow().output.is_none();
        reusable
            && (self.kept.contains_key(&frame)
                || (self.discard_before.is_none()
                    && frame >= self.next_decode_frame
                    && frame < self.next_decode_frame + self.prefetch_frames as u64))
    }

    /// A kept picture shown again.
    fn kept_again(&mut self, frame: u64) -> Option<DecodedVideoFrame<Picture>> {
        let kept = self.kept.get(&frame)?.clone();
        self.stamped(kept)
    }

    /// Every picture handed out takes the next sequence when it is handed out, so the
    /// monitor never sees a lower one after a step back (it drops those as stale).
    fn stamped(&mut self, kept: DecodedVideoFrame<Picture>) -> Option<DecodedVideoFrame<Picture>> {
        if kept.payload.token.is_some() {
            return Some(kept); // an output token keeps its own sequence
        }
        let mut gpu = self.gpu.borrow_mut();
        let mut header = kept.payload.header.clone();
        gpu.sequence = gpu.sequence.checked_add(1)?;
        header.sequence = gpu.sequence;
        let rgba = kept.payload.rgba.clone();
        Some(DecodedVideoFrame { payload: Rc::new(PictureData { token: None, header, rgba }), ..kept })
    }

    /// Keeps the pictures around the frame the last cue went to only.
    fn forget_far_from(&mut self, _served: u64) {
        let frame = self.cue_anchor;
        let (back, ahead) = (frame.saturating_sub(STEP_BACK_FRAMES), frame + 2 * self.prefetch_frames as u64);
        self.kept.retain(|kept, _| *kept >= back && *kept <= ahead);
    }

    fn drain_conversions(&mut self) -> Result<()> {
        while self.converter.busy() {
            let completed = match self.converter.poll().map_err(error)? {
                Poll::Pending => break,
                Poll::Ready(completed) => completed,
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
        let target_end = request
            .frame
            .saturating_add(self.prefetch_frames as u64)
            .min(self.plan.source.duration_frames);
        while self.converter.can_accept() && self.next_decode_frame < target_end {
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
            let packet = match self.decoder.try_next_packet().map_err(error)? {
                Poll::Pending => break,
                Poll::Ready(None) => {
                    if self.next_decode_frame >= self.plan.source.duration_frames {
                        break;
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
            self.converter
                .submit(conversion::Job {
                    generation: self.gpu.borrow().config.generation,
                    frame,
                    slot,
                    input: packet.bytes,
                    rgba: self.rgba[slot].take().expect("exclusive frame buffer"),
                })
                .map_err(error)?;
            self.next_decode_frame += 1;
        }
        Ok(())
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
            header,
            rgba: completed.rgba,
        });
        gpu.images.insert(frame, token.clone());
        gpu.upload_us += upload_start.elapsed().as_micros();
        gpu.converted += 1;
        let picture = DecodedVideoFrame {
            source_id: self.plan.source.source_id.clone(),
            frame,
            video_format: self.plan.source.video_format.clone(),
            payload: token,
        };
        self.kept.insert(frame, picture.clone());
        self.ready.insert(frame, picture);
        Ok(())
    }
}
