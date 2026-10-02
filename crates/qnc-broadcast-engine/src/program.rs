//! A story program played as one virtual source (v5
//! `qnc-player-runtime/program_playlist.rs`).
//!
//! The transport, clock, cue, Play and Pause stay the ones of a single clip: the
//! program is one source on the program frame axis. The picture adapter decodes
//! program frames in order across cuts (the prefetch reaches the next clip before
//! the cut, so its decoder starts early); an item without picture (Off) is black.
//! The sound adapter builds every program frame from the channel of each bus
//! (A1, A2) and keeps the other outputs silent. No probe, no database, no new
//! decoder choice: the clips are the saved inputs of `ProgramInput`.

use crate::decode_input::{DecodeInput, seek_start};
use crate::input::{AudioStreamPlan, InputPlan, native_audio_layout, preview_raster_bounds};
use crate::output::{DeviceSink, PcmTrack, SharedVideo, device_sink_adapter};
use crate::*;
use qnc_player_input::ProgramInput;
use std::{collections::BTreeMap, sync::Arc};

/// A program item's picture: which clip, from which source frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VideoSpan {
    record_in: u64,
    record_out: u64,
    video: Option<(usize, u64)>,
}

/// One program output channel fed from one channel of a clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bus {
    output: u16,
    clip: usize,
    source_in: u64,
    stream_index: u32,
    channel_index: u16,
    stream_channels: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AudioSpan {
    record_in: u64,
    record_out: u64,
    buses: Vec<Bus>,
}

/// The program checked against the saved clips: its virtual source, the clip
/// plans and, per item, the picture and the sound buses.
pub struct ProgramPlan {
    pub(crate) source: SourceRuntime,
    pub(crate) clips: Vec<InputPlan>,
    video: Vec<VideoSpan>,
    audio: Vec<AudioSpan>,
    channels: u16,
}

impl ProgramPlan {
    pub fn new(input: &ProgramInput) -> Result<Self> {
        input.validate_for(&input.workspace_db_uri).map_err(error)?;
        let ids: Vec<&str> = input.clips.keys().map(String::as_str).collect();
        let clips = input
            .clips
            .iter()
            .map(|(id, clip)| InputPlan::for_program(clip, &input.workspace_db_uri, id))
            .collect::<Result<Vec<_>>>()?;
        let playlist = &input.playlist;
        let timebase = Timebase::new(
            playlist.program_timebase.fps_num,
            playlist.program_timebase.fps_den,
        )
        .map_err(error)?;
        let frame = |value: i64| u64::try_from(value).map_err(error);
        let channels = input.project_audio.channels;
        let rate = input.project_audio.sample_rate_hz;
        let (video, audio) = program_spans(
            playlist,
            &ids,
            |clip_id| {
                let prepared = &input.clips[clip_id];
                let (streams, format) = native_audio_layout(prepared.audio_media())?;
                Ok(ClipSound {
                    channels: prepared.layout.audio_channels.clone(),
                    streams,
                    rate: format.map(|format| format.sample_rate_hz),
                })
            },
            rate,
        )?;
        let first_picture = video
            .iter()
            .find_map(|span| span.video.map(|(clip, _)| clip))
            .unwrap_or(0);
        let picture_format = clips
            .get(first_picture)
            .and_then(|plan| plan.source.video_format.clone())
            .ok_or_else(|| error("program has no saved picture format"))?;
        let source = SourceRuntime::new(
            playlist.playlist_id.as_str(),
            frame(playlist.duration_frames)?,
            timebase,
        )
        .map_err(error)?
        .with_video_format(picture_format)
        .with_audio_format(AudioFormat::new(rate, channels).map_err(error)?);
        source.validate().map_err(error)?;
        Ok(Self {
            source,
            clips,
            video,
            audio,
            channels,
        })
    }

    pub fn source(&self) -> &SourceRuntime {
        &self.source
    }

    /// Output pool of the picture the program opens with.
    pub fn output_config(&self, session_id: &str, generation: u64) -> Result<OutputConfig> {
        let first = self
            .video
            .iter()
            .find_map(|span| span.video.map(|(clip, _)| clip))
            .unwrap_or(0);
        self.clips
            .get(first)
            .ok_or_else(|| error("program has no clip"))?
            .output_config(session_id, generation)
    }
}

struct Worker {
    spec: qnc_pixel_convert::ConversionSpec,
    conversion: conversion::ConversionWorker,
    raster: [u32; 2],
    rgba: Vec<Option<Arc<[u8]>>>,
}

struct ActiveDecoder {
    clip: usize,
    decoder: Decoder,
    next_source: u64,
    discard_before: Option<u64>,
}

/// Program picture: one decode sequence in program frames.
pub(crate) struct ProgramVideo {
    source: SourceRuntime,
    spans: Vec<VideoSpan>,
    clips: Vec<(InputPlan, Rc<DecodeInput>, usize)>,
    workers: Vec<Worker>,
    active: Option<ActiveDecoder>,
    /// The decoder of the next cut to another picture, opened ahead of it.
    upcoming: Option<ActiveDecoder>,
    next_decode_frame: u64,
    ready: BTreeMap<u64, DecodedVideoFrame<Picture>>,
    pending_seek: Option<u64>,
    prefetch_frames: usize,
    gpu: SharedVideo,
}

impl ProgramVideo {
    pub fn open(
        plan: &ProgramPlan,
        inputs: &[Rc<DecodeInput>],
        gpu: SharedVideo,
        prefetch_frames: usize,
    ) -> Result<Self> {
        let mut workers: Vec<Worker> = Vec::new();
        let mut clips = Vec::new();
        for (clip, input) in plan.clips.iter().zip(inputs) {
            let worker = match workers.iter().position(|w| w.spec == clip.spec) {
                Some(index) => index,
                None => {
                    let converter = conversion::Raster::Gpu(
                        qnc_gpu_raster::GpuRasterConverter::prepare(
                            clip.spec.clone(),
                            preview_raster_bounds(clip.spec.width, clip.spec.height),
                        )
                        .map_err(error)?,
                    );
                    let raster = converter.size();
                    let rgba = (0..prefetch_frames + 4)
                        .map(|_| Some(Arc::from(vec![0; converter.output_bytes()])))
                        .collect();
                    workers.push(Worker {
                        spec: clip.spec.clone(),
                        conversion: conversion::ConversionWorker::with_capacity(
                            converter,
                            input::CONVERT_IN_FLIGHT.min(prefetch_frames),
                        )
                        .map_err(error)?,
                        raster,
                        rgba,
                    });
                    workers.len() - 1
                }
            };
            clips.push((clip.clone(), input.clone(), worker));
        }
        Ok(Self {
            source: plan.source.clone(),
            spans: plan.video.clone(),
            clips,
            workers,
            active: None,
            upcoming: None,
            next_decode_frame: 0,
            ready: BTreeMap::new(),
            pending_seek: None,
            prefetch_frames,
            gpu,
        })
    }

    fn span_at(&self, frame: u64) -> Result<VideoSpan> {
        self.spans
            .iter()
            .find(|span| (span.record_in..span.record_out).contains(&frame))
            .copied()
            .ok_or_else(|| error("program frame outside its items"))
    }

    fn header(&mut self, frame: u64, raster: [u32; 2], generation: u64) -> Result<FrameHeader> {
        let mut gpu = self.gpu.borrow_mut();
        let header = FrameHeader {
            version: qnc_video_output::VERSION.into(),
            session_id: gpu.config.session_id.clone(),
            generation,
            sequence: gpu.sequence,
            source_id: self.source.source_id.clone(),
            frame_number: frame,
            width: raster[0],
            height: raster[1],
            pixel_format: PixelFormat::Rgba8Srgb,
        };
        gpu.sequence = gpu
            .sequence
            .checked_add(1)
            .ok_or_else(|| error("frame sequence exhausted"))?;
        Ok(header)
    }

    fn picture(
        &self,
        frame: u64,
        header: FrameHeader,
        rgba: Arc<[u8]>,
    ) -> DecodedVideoFrame<Picture> {
        DecodedVideoFrame {
            source_id: self.source.source_id.clone(),
            frame,
            video_format: self.source.video_format.clone(),
            payload: Rc::new(PictureData {
                token: None,
                header,
                rgba,
            }),
        }
    }

    /// Off: no picture, black in the raster of the program's first picture.
    fn black(&mut self, frame: u64) -> Result<()> {
        let raster = self
            .workers
            .first()
            .map(|worker| worker.raster)
            .ok_or_else(|| error("program has no picture raster"))?;
        let mut rgba = vec![0u8; raster[0] as usize * raster[1] as usize * 4];
        rgba.chunks_exact_mut(4).for_each(|pixel| pixel[3] = 255);
        let generation = self.gpu.borrow().config.generation;
        let header = self.header(frame, raster, generation)?;
        let picture = self.picture(frame, header, Arc::from(rgba));
        self.ready.insert(frame, picture);
        Ok(())
    }

    fn drain(&mut self) -> Result<()> {
        for index in 0..self.workers.len() {
            while self.workers[index].conversion.busy() {
                let completed = match self.workers[index].conversion.poll().map_err(error)? {
                    Poll::Pending => break,
                    Poll::Ready(completed) => completed,
                };
                self.workers[index].rgba[completed.slot] = Some(completed.rgba.clone());
                if completed.generation != self.gpu.borrow().config.generation {
                    continue;
                }
                completed.result.as_ref().map_err(error)?;
                {
                    let mut gpu = self.gpu.borrow_mut();
                    gpu.conversion_us += completed.elapsed_us;
                    gpu.converted += 1;
                }
                let raster = self.workers[index].raster;
                let header = self.header(completed.frame, raster, completed.generation)?;
                let picture = self.picture(completed.frame, header, completed.rgba);
                self.ready.insert(completed.frame, picture);
            }
        }
        Ok(())
    }

    fn open_decoder(&self, clip: usize, source_frame: u64) -> Result<ActiveDecoder> {
        let (plan, input, _) = &self.clips[clip];
        Ok(ActiveDecoder {
            clip,
            decoder: input.open(plan.video_index, seek_start(&plan.source, source_frame)?)?,
            next_source: source_frame,
            discard_before: Some(source_frame),
        })
    }

    /// Opens the decoder of the next cut to another picture source within the lookahead
    /// (user report 2026-10-02: the picture stalled just before a cover, its decoder
    /// started only half a second ahead). A cut that continues the same source needs none.
    fn open_upcoming(&mut self, frame: u64) -> Result<()> {
        let timebase = self.source.timebase;
        let seconds = i64::try_from(PICTURE_LOOKAHEAD_SECONDS).map_err(error)?;
        let lookahead = u64::try_from(seconds * timebase.fps_num / timebase.fps_den.max(1)).unwrap_or(0);
        let Some((clip, source_in)) = next_picture_cut(&self.spans, frame, lookahead) else {
            return Ok(());
        };
        if self.upcoming.as_ref().is_some_and(|up| up.clip == clip && up.next_source == source_in) {
            return Ok(());
        }
        diag(format!("program-video upcoming clip={clip} source={source_in} at_frame={frame}"));
        self.upcoming = Some(self.open_decoder(clip, source_in)?);
        Ok(())
    }

    fn fill(&mut self, frame: u64) -> Result<()> {
        let target_end = frame
            .saturating_add(self.prefetch_frames as u64)
            .min(self.source.duration_frames);
        self.next_decode_frame = self.next_decode_frame.max(frame);
        self.open_upcoming(frame)?;
        while self.next_decode_frame < target_end {
            let next = self.next_decode_frame;
            if self.ready.contains_key(&next) {
                self.next_decode_frame += 1;
                continue;
            }
            let span = self.span_at(next)?;
            let Some((clip, source_in)) = span.video else {
                self.black(next)?;
                self.next_decode_frame += 1;
                continue;
            };
            let source_frame = source_in + (next - span.record_in);
            let worker = self.clips[clip].2;
            if !self.workers[worker].conversion.can_accept() {
                break;
            }
            let Some(slot) = self.workers[worker]
                .rgba
                .iter()
                .position(|b| b.as_ref().is_some_and(|b| Arc::strong_count(b) == 1))
            else {
                break;
            };
            let continues = self
                .active
                .as_ref()
                .is_some_and(|a| a.clip == clip && a.next_source == source_frame);
            if !continues {
                // The decoder opened ahead for this cut takes over; else one opens now.
                let ahead = if self.upcoming.as_ref().is_some_and(|up| up.clip == clip && up.next_source == source_frame) {
                    self.upcoming.take()
                } else {
                    None
                };
                diag(format!("program-video cut frame={next} clip={clip} opened_ahead={}", ahead.is_some()));
                self.active = match ahead {
                    Some(up) => Some(up),
                    None => Some(self.open_decoder(clip, source_frame)?),
                };
            }
            let active = self.active.as_mut().expect("decoder just opened");
            let packet = match active.decoder.try_next_packet().map_err(error)? {
                Poll::Pending => break,
                Poll::Ready(None) => return Err(error("video ended before saved frame boundary")),
                Poll::Ready(Some(packet)) => packet,
            };
            let plan = &self.clips[clip].0;
            let decoded = input::relative_position(
                packet.pts,
                packet.time_base,
                plan.origin,
                plan.source.timebase.fps_num,
                plan.source.timebase.fps_den,
            )?;
            let spec = &plan.spec;
            if packet.stream_index != plan.video_index
                || packet.media_uri != plan.media.media_uri
                || packet.format
                    != (DecodedFormat::Video {
                        width: spec.width,
                        height: spec.height,
                        pixel_format: spec.layout.name().into(),
                    })
            {
                return Err(error("decoded frame does not match saved input/request"));
            }
            if active.discard_before.is_some_and(|before| decoded < before) {
                continue;
            }
            active.discard_before = None;
            if decoded < active.next_source {
                continue;
            }
            if decoded != active.next_source {
                return Err(error("decoder skipped requested source frame"));
            }
            let generation = self.gpu.borrow().config.generation;
            let rgba = self.workers[worker].rgba[slot]
                .take()
                .expect("exclusive frame buffer");
            self.workers[worker]
                .conversion
                .submit(conversion::Job {
                    generation,
                    frame: next,
                    slot,
                    input: packet.bytes,
                    rgba,
                })
                .map_err(error)?;
            active.next_source += 1;
            self.next_decode_frame += 1;
        }
        Ok(())
    }
}

impl VideoDecodeAdapter for ProgramVideo {
    type VideoFrame = Picture;
    fn cue_video(&mut self, request: EngineFrameRequest) -> Result<()> {
        if request.source_id != self.source.source_id || request.timebase != self.source.timebase {
            return Err(error("seek source mismatch"));
        }
        seek_start(&self.source, request.frame)?;
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
        if request.source_id != self.source.source_id || request.timebase != self.source.timebase {
            return Err(error("video request differs from the program"));
        }
        if let Some(frame) = self.pending_seek {
            if request.frame != frame {
                return Err(error("seek target mismatch"));
            }
            let mut gpu = self.gpu.borrow_mut();
            gpu.config.generation = gpu
                .config
                .generation
                .checked_add(1)
                .ok_or_else(|| error("GPU generation exhausted"))?;
            gpu.sequence = 0;
            gpu.images.clear();
            drop(gpu);
            self.active = None;
            self.upcoming = None;
            self.ready.clear();
            self.next_decode_frame = frame;
            self.pending_seek = None;
        }
        self.ready.retain(|frame, _| *frame >= request.frame);
        self.drain()?;
        if let Some(frame) = self.ready.remove(&request.frame) {
            return Ok(frame);
        }
        self.fill(request.frame)?;
        self.drain()?;
        let picture = self.ready.remove(&request.frame);
        if picture.is_none() {
            diag(format!("program-video pending frame={} next_decode={}", request.frame, self.next_decode_frame));
        }
        picture.ok_or_else(pending)
    }
}

/// A lane is one channel of a clip played with one offset between program and source
/// frames: the same channel in a later item with another offset is another lane, so it
/// can be opened before its cut while the current one still plays.
type LaneKey = (usize, u32, u16, i128);

/// How far ahead (seconds of program) the sound of the next items is opened, so the
/// decoder of a cover has started before its cut instead of on it (a cut used to
/// empty the audio queue: `audio_underrun` on every cover).
const LANE_LOOKAHEAD_SECONDS: u64 = 2;

/// How far ahead (seconds of program) the picture decoder of the next cut is opened.
const PICTURE_LOOKAHEAD_SECONDS: u64 = 2;

fn lane_key(span: &AudioSpan, bus: &Bus) -> LaneKey {
    (
        bus.clip,
        bus.stream_index,
        bus.channel_index,
        i128::from(bus.source_in) - i128::from(span.record_in),
    )
}

/// The lanes of the items starting in `(frame, horizon]` that are not playing now, each
/// once, at the first item that uses it. An item cut in two (a cover over the end of a
/// segment) gives the same lane twice; opening it at both starts made each undo the
/// other on every frame, so the sound of the cover was never ready at its cut.
fn next_lanes(spans: &[AudioSpan], current: &[LaneKey], frame: u64, horizon: u64) -> Vec<(LaneKey, Bus)> {
    let mut next: Vec<(LaneKey, Bus)> = Vec::new();
    for span in spans.iter().filter(|span| span.record_in > frame && span.record_in <= horizon) {
        for bus in &span.buses {
            let key = lane_key(span, bus);
            if !current.contains(&key) && !next.iter().any(|(seen, _)| *seen == key) {
                next.push((key, *bus));
            }
        }
    }
    next
}

/// A diagnostics line of program playback (only with player diagnostics on).
fn diag(text: String) {
    if qnc_dev_diagnostics::player_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, text);
    }
}

/// The first cut within `(frame, frame + lookahead]` to a picture that does not continue
/// the one before it: (clip, its source frame at the cut).
fn next_picture_cut(spans: &[VideoSpan], frame: u64, lookahead: u64) -> Option<(usize, u64)> {
    spans.windows(2).find_map(|pair| {
        let (before, after) = (pair[0], pair[1]);
        if after.record_in <= frame || after.record_in > frame.saturating_add(lookahead) {
            return None;
        }
        let (clip, source_in) = after.video?;
        let continues = before.video.is_some_and(|(prev, prev_in)| {
            prev == clip && prev_in + (after.record_in - before.record_in) == source_in
        });
        (!continues).then_some((clip, source_in))
    })
}

struct Lane {
    clip: usize,
    decoder: Option<Decoder>,
    track: PcmTrack,
    consumed_through: Option<u64>,
}

/// Program sound: A1/A2 buses per frame into the project output channels.
pub(crate) struct ProgramAudio {
    pub sink: DeviceSink,
    source: SourceRuntime,
    spans: Vec<AudioSpan>,
    clips: Vec<(InputPlan, Rc<DecodeInput>)>,
    lanes: BTreeMap<LaneKey, Lane>,
    channels: u16,
    rate: u32,
}

impl ProgramAudio {
    pub fn open(
        plan: &ProgramPlan,
        inputs: &[Rc<DecodeInput>],
        device_id: Option<String>,
        prebuffer_frames: usize,
    ) -> Result<Self> {
        let map = qnc_audio_output::ChannelMap::identity(plan.channels).map_err(error)?;
        let rate = plan
            .source
            .audio_format
            .as_ref()
            .map(|format| format.sample_rate_hz)
            .ok_or_else(|| error("program audio format missing"))?;
        Ok(Self {
            sink: DeviceSink::open(&plan.source, device_id, Some(map), prebuffer_frames)?,
            source: plan.source.clone(),
            spans: plan.audio.clone(),
            clips: plan
                .clips
                .iter()
                .zip(inputs)
                .map(|(plan, input)| {
                    (
                        plan.clone(),
                        Rc::new(input.for_media(plan.audio_media.clone())),
                    )
                })
                .collect(),
            lanes: BTreeMap::new(),
            channels: plan.channels,
            rate,
        })
    }

    /// Makes the lane of a bus hold the samples of its source frame; false while
    /// the decoder still prepares them.
    fn fill_lane(&mut self, key: LaneKey, bus: &Bus, source_frame: u64) -> Result<bool> {
        let (plan, input) = &self.clips[bus.clip];
        let start = sample_boundary(source_frame, plan.source.timebase, self.rate)?;
        let end = sample_boundary(source_frame + 1, plan.source.timebase, self.rate)?;
        let lane = self.lanes.entry(key).or_insert_with(|| Lane {
            clip: bus.clip,
            decoder: None,
            track: PcmTrack::new(&AudioStreamPlan {
                stream_index: bus.stream_index,
                source_channels: bus.stream_channels,
                selected_channels: vec![bus.channel_index],
            }),
            consumed_through: None,
        });
        if lane.consumed_through != Some(start) {
            diag(format!("program-audio open clip={} stream={} source={source_frame}", bus.clip, bus.stream_index));
            lane.decoder =
                Some(input.open(bus.stream_index, seek_start(&plan.source, source_frame)?)?);
            lane.track.samples.clear();
            lane.track.decoded_through = None;
            lane.track.discard_before = start;
            lane.consumed_through = Some(start);
        }
        let needed = usize::try_from(end - start).map_err(error)?;
        let decoder = lane
            .decoder
            .as_mut()
            .ok_or_else(|| error("audio lane without decoder"))?;
        for _ in 0..2 {
            if lane.track.samples.len() >= needed {
                break;
            }
            match decoder.try_next_packet().map_err(error)? {
                Poll::Pending => break,
                Poll::Ready(None) => return Err(error("audio ended before saved video boundary")),
                Poll::Ready(Some(packet)) => lane.track.push(
                    packet,
                    &plan.audio_media.media_uri,
                    plan.audio_origin,
                    self.rate,
                )?,
            }
        }
        Ok(lane.track.samples.len() >= needed)
    }

    /// Opens the lanes of the items starting within the lookahead (each one at its
    /// first frame) and drops the lanes no item near the playhead uses.
    fn open_next_lanes(&mut self, current: &AudioSpan, frame: u64) -> Result<()> {
        let timebase = self.source.timebase;
        let seconds = i64::try_from(LANE_LOOKAHEAD_SECONDS).map_err(error)?;
        let lookahead = u64::try_from(seconds * timebase.fps_num / timebase.fps_den.max(1)).unwrap_or(0);
        let horizon = frame.saturating_add(lookahead);
        let current_keys: Vec<LaneKey> = current
            .buses
            .iter()
            .map(|bus| lane_key(current, bus))
            .collect();
        let next = next_lanes(&self.spans, &current_keys, frame, horizon);
        self.lanes.retain(|key, _| {
            current_keys.contains(key) || next.iter().any(|(next_key, _)| next_key == key)
        });
        for (key, bus) in next {
            self.fill_lane(key, &bus, bus.source_in)?;
        }
        Ok(())
    }
}

impl AudioOutputAdapter for ProgramAudio {
    type AudioPacket = Arc<[f32]>;
    fn cue_audio(&mut self, request: EngineFrameRequest) -> Result<()> {
        if request.source_id != self.source.source_id || request.timebase != self.source.timebase {
            return Err(error("audio seek source mismatch"));
        }
        seek_start(&self.source, request.frame)?;
        Ok(())
    }
    fn prepare_audio(&mut self, _: &EngineSourceHandle) -> Result<Vec<BroadcastEvent>> {
        Ok(Vec::new())
    }
    fn render_audio_for_frame(
        &mut self,
        request: EngineFrameRequest,
    ) -> Result<AudioFramePacket<Arc<[f32]>>> {
        if request.source_id != self.source.source_id || request.timebase != self.source.timebase {
            return Err(error("audio request differs from the program"));
        }
        let frame = request.frame;
        let start = sample_boundary(frame, request.timebase, self.rate)?;
        let end = sample_boundary(frame + 1, request.timebase, self.rate)?;
        let frames = usize::try_from(end - start).map_err(error)?;
        let channels = usize::from(self.channels);
        let span = self
            .spans
            .iter()
            .find(|span| (span.record_in..span.record_out).contains(&frame))
            .cloned()
            .ok_or_else(|| error("program frame outside its items"))?;
        let mut ready = true;
        for bus in &span.buses {
            let source_frame = bus.source_in + (frame - span.record_in);
            ready &= self.fill_lane(lane_key(&span, bus), bus, source_frame)?;
        }
        self.open_next_lanes(&span, frame)?;
        if !ready {
            diag(format!("program-audio pending frame={frame}"));
            return Err(pending());
        }
        let mut samples = vec![0.0f32; frames * channels];
        let mut taken: BTreeMap<LaneKey, Vec<f32>> = BTreeMap::new();
        for bus in &span.buses {
            let key = lane_key(&span, bus);
            if !taken.contains_key(&key) {
                let source_frame = bus.source_in + (frame - span.record_in);
                let timebase = self.clips[bus.clip].0.source.timebase;
                let lane_start = sample_boundary(source_frame, timebase, self.rate)?;
                let lane_end = sample_boundary(source_frame + 1, timebase, self.rate)?;
                let count = usize::try_from(lane_end - lane_start).map_err(error)?;
                let lane = self.lanes.get_mut(&key).expect("filled lane");
                debug_assert_eq!(lane.clip, bus.clip);
                taken.insert(key, lane.track.samples.drain(..count).collect());
                lane.consumed_through = Some(lane_end);
            }
            let source = &taken[&key];
            for (index, value) in source.iter().take(frames).enumerate() {
                samples[index * channels + usize::from(bus.output)] = *value;
            }
        }
        Ok(AudioFramePacket {
            source_id: request.source_id,
            start_frame: frame,
            frame_count: 1,
            audio_format: self.source.audio_format.clone(),
            payload: samples.into(),
        })
    }
    device_sink_adapter!();
}

/// The saved sound of a clip: its channel inventory (program channel k is entry
/// k), its native streams and their sample rate.
pub(crate) struct ClipSound {
    pub channels: Vec<qnc_player_input::AudioChannel>,
    pub streams: Vec<(u32, u16)>,
    pub rate: Option<u32>,
}

/// Per program item: the top picture layer (v5: a cover over the segment
/// picture) and one bus per audio route, from the saved channel inventory.
fn program_spans(
    playlist: &qnc_program_playlist::FlatProgramPlaylist,
    ids: &[&str],
    sound: impl Fn(&str) -> Result<ClipSound>,
    rate: u32,
) -> Result<(Vec<VideoSpan>, Vec<AudioSpan>)> {
    let index = |clip_id: &str| {
        ids.iter()
            .position(|id| *id == clip_id)
            .ok_or_else(|| error("program source has no prepared clip"))
    };
    let frame = |value: i64| u64::try_from(value).map_err(error);
    let mut video = Vec::new();
    let mut audio = Vec::new();
    for item in &playlist.items {
        let (record_in, record_out) = (
            frame(item.record_range.in_frame)?,
            frame(item.record_range.out_frame)?,
        );
        let picture = item
            .sources
            .iter()
            .rev()
            .find(|source| source.has_video())
            .map(|source| {
                Ok::<_, BroadcastEngineError>((
                    index(&source.clip_id)?,
                    frame(source.source_range.source_in)?,
                ))
            })
            .transpose()?;
        video.push(VideoSpan {
            record_in,
            record_out,
            video: picture,
        });
        let mut buses = Vec::new();
        for source in &item.sources {
            for route in &source.audio_routes {
                let clip_sound = sound(&source.clip_id)?;
                let channel = clip_sound
                    .channels
                    .get(usize::from(route.source_channel))
                    .ok_or_else(|| error("program routes a channel the clip does not have"))?;
                if clip_sound.rate != Some(rate) {
                    return Err(error(
                        "Project audio sample rate requires conversion not supported by this playback adapter.",
                    ));
                }
                let stream_channels = clip_sound
                    .streams
                    .iter()
                    .find(|(stream, _)| *stream == channel.stream_index)
                    .map(|(_, count)| *count)
                    .ok_or_else(|| error("program channel references a missing stream"))?;
                buses.push(Bus {
                    output: route.output_channel,
                    clip: index(&source.clip_id)?,
                    source_in: frame(source.source_range.source_in)?,
                    stream_index: channel.stream_index,
                    channel_index: u16::try_from(channel.channel_index).map_err(error)?,
                    stream_channels,
                });
            }
        }
        audio.push(AudioSpan {
            record_in,
            record_out,
            buses,
        });
    }
    Ok((video, audio))
}

#[cfg(test)]
#[path = "program_tests.rs"]
mod tests;
