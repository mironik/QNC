use crate::input::AudioStreamPlan;
use crate::*;
use qnc_audio_output::{AudioOutput, ChannelMap, Config, Format};
use qnc_media_metadata::Rational;
use std::{collections::VecDeque, sync::Arc};

pub(crate) struct VideoSink {
    pub output: Option<VideoOutput>,
    pub config: OutputConfig,
    pub images: BTreeMap<u64, Picture>,
    pub sequence: u64,
    pub inflight: bool,
    pub submit_us: u128,
    pub conversion_us: u128,
    pub upload_us: u128,
    pub converted: u64,
    pub monitor: Option<(FrameHeader, Arc<[u8]>)>,
    pub(crate) monitor_pending: VecDeque<(FrameHeader, Arc<[u8]>)>,
    /// The transport plays (set by Play, cleared by Pause, Stop and a cue): background
    /// work of the picture gives way while it plays.
    pub(crate) playing: bool,
    /// The pictures before where the clip stands (`backfill`), started from the player loop.
    pub(crate) back: Option<Rc<RefCell<crate::backfill::BackFill>>>,
}
pub(crate) type SharedVideo = Rc<RefCell<VideoSink>>;
impl VideoSink {
    pub fn push_monitor(&mut self, header: FrameHeader, rgba: Arc<[u8]>) {
        let frame = (header, rgba);
        self.monitor = Some(frame.clone());
        self.monitor_pending.clear();
        self.monitor_pending.push_back(frame);
    }

    pub fn clear_pending_monitor(&mut self) {
        self.monitor_pending.clear();
    }

    pub fn take_monitor_pending(&mut self) -> Vec<(FrameHeader, Arc<[u8]>)> {
        self.monitor_pending.drain(..).collect()
    }

    pub fn poll_and_collect(&mut self) -> Result<()> {
        if let Some(output) = &mut self.output
            && output.poll().map_err(error)?.is_some()
        {
            self.inflight = false;
        }
        if self.inflight {
            return Ok(());
        }
        let expired: Vec<_> = self
            .images
            .iter()
            .filter(|(_, image)| Rc::strong_count(image) == 1)
            .map(|(f, _)| *f)
            .collect();
        for frame in expired {
            let image = self.images.get(&frame).expect("retained image");
            if let Some(output) = &mut self.output {
                match output.release(
                    image
                        .token
                        .as_ref()
                        .ok_or_else(|| error("missing native frame"))?,
                ) {
                    Ok(()) => (),
                    Err(qnc_video_output::OutputError::Busy) => continue,
                    Err(e) => return Err(error(e)),
                }
            }
            self.images.remove(&frame);
        }
        Ok(())
    }
}
pub(crate) struct Presenter(pub SharedVideo);
impl FramePresenter for Presenter {
    type VideoFrame = Picture;
    fn prepare_presentation(&mut self, _: &EngineSourceHandle) -> Result<Vec<BroadcastEvent>> {
        Ok(Vec::new())
    }
    fn prepare_start_frame(&mut self, frame: &DecodedVideoFrame<Picture>) -> Result<bool> {
        let sink = self.0.borrow();
        match (&sink.output, &frame.payload.token) {
            (Some(output), Some(token)) => output.is_ready(token).map_err(error),
            (None, None) => Ok(true),
            _ => Err(error("frame output mismatch")),
        }
    }
    fn present_frame(&mut self, frame: DecodedVideoFrame<Picture>) -> Result<Vec<BroadcastEvent>> {
        let mut gpu = self.0.borrow_mut();
        let start = Instant::now();
        match (&mut gpu.output, &frame.payload.token) {
            (Some(output), Some(token)) => {
                let submitted = output.submit(token).map_err(error)?;
                if submitted.frame.frame_number != frame.frame {
                    return Err(error("expected matching picture submission"));
                }
                gpu.inflight = true;
            }
            (None, None) => (),
            _ => return Err(error("frame output mismatch")),
        }
        gpu.submit_us = start.elapsed().as_micros();
        gpu.push_monitor(frame.payload.header.clone(), frame.payload.rgba.clone());
        Ok(vec![BroadcastEvent::VideoFrameSubmitted {
            frame: frame.frame,
        }])
    }
}

/// The audio device queue of one session: the clip sound and the program sound
/// both hand it packets in the session source's frames.
pub(crate) struct DeviceSink {
    pub device: Option<Rc<RefCell<AudioOutput>>>,
    source: SourceRuntime,
    generation: Option<u64>,
    channel_map: Option<ChannelMap>,
    /// The frame a cue went to: its sound is heard once, frame by frame (user rule
    /// 2026-10-01: stepping by frames is cut by ear).
    audition_frame: Option<u64>,
}
impl DeviceSink {
    pub fn open(
        source: &SourceRuntime,
        device_id: Option<String>,
        channel_map: Option<ChannelMap>,
        prebuffer_frames: usize,
    ) -> Result<Self> {
        validate_channel_map(source.audio_format.as_ref(), channel_map.as_ref())?;
        let device = source
            .audio_format
            .as_ref()
            .map(|format| {
                let ready = sample_boundary(
                    source.duration_frames.min(prebuffer_frames as u64),
                    source.timebase,
                    format.sample_rate_hz,
                )?;
                let map = channel_map.as_ref().expect("validated map");
                // The queue carries every decoded channel; the device plays the routed ones.
                let output = AudioOutput::open(Config {
                    version: qnc_audio_output::VERSION.into(),
                    format: Format {
                        sample_rate_hz: format.sample_rate_hz,
                        channels: map.output_channels().len() as u16,
                    },
                    device_id,
                    // Two seconds: the sound runs 1.5 s ahead of the picture while playing.
                    capacity_frames: format.sample_rate_hz * 2,
                    ready_frames: u32::try_from(ready).map_err(error)?,
                    source_channels: map.source_channels(),
                })
                .map_err(error)?;
                output.set_routes(map.output_channels());
                Ok(Rc::new(RefCell::new(output)))
            })
            .transpose()?;
        Ok(Self {
            device,
            source: source.clone(),
            generation: None,
            channel_map,
            audition_frame: None,
        })
    }

    /// The device outputs at these levels (percent; user 2026-10-14: level 0 of A1/A2),
    /// applied by the device where the samples leave, so a later change is heard at once.
    pub(crate) fn with_levels(self, levels: Vec<u16>) -> Self {
        if let Some(device) = &self.device {
            device.borrow().set_gains(&gains_of(&levels));
        }
        self
    }

    /// The next packet of `frame` is also heard once while paused, one frame long.
    pub(crate) fn audition_at(&mut self, frame: u64) {
        self.audition_frame = Some(frame);
    }

    pub(crate) fn submit(
        &mut self,
        packet: AudioFramePacket<Arc<[f32]>>,
    ) -> Result<Vec<BroadcastEvent>> {
        let format = self
            .source
            .audio_format
            .as_ref()
            .ok_or_else(|| error("missing audio format"))?;
        let start = sample_boundary(
            packet.start_frame,
            self.source.timebase,
            format.sample_rate_hz,
        )?;
        let mut device = self
            .device
            .as_ref()
            .ok_or_else(|| error("missing audio device"))?
            .borrow_mut();
        let generation = match self.generation {
            Some(generation) => generation,
            None => {
                let generation = device.begin(start).map_err(audio_error)?;
                self.generation = Some(generation);
                generation
            }
        };
        // Every decoded channel goes to the device queue; the device plays the routed ones.
        device.queue(generation, start, &packet.payload).map_err(audio_error)?;
        if self.audition_frame == Some(packet.start_frame) {
            self.audition_frame = None;
            // Exactly the samples of this one frame, no more.
            let end = sample_boundary(packet.start_frame + 1, self.source.timebase, format.sample_rate_hz)?;
            let width = self.channel_map.as_ref().map_or(1, |map| usize::from(map.source_channels()));
            let samples = (end - start) as usize * width;
            device.audition(&packet.payload[..samples.min(packet.payload.len())]);
        }
        if packet.start_frame + 1 == self.source.duration_frames {
            device.finish(generation).map_err(audio_error)?;
        }
        Ok(Vec::new())
    }
    pub(crate) fn begin_preroll(&mut self) -> Result<Vec<BroadcastEvent>> {
        self.generation = None;
        Ok(Vec::new())
    }
    pub(crate) fn commit_preroll(&mut self) -> Result<Vec<BroadcastEvent>> {
        self.device
            .as_ref()
            .ok_or_else(|| error("missing audio device"))?
            .borrow_mut()
            .commit(self.generation.ok_or_else(|| error("audio not queued"))?)
            .map_err(error)?;
        Ok(Vec::new())
    }
    pub(crate) fn start(&mut self) -> Result<Vec<BroadcastEvent>> {
        self.device
            .as_ref()
            .ok_or_else(|| error("missing audio device"))?
            .borrow_mut()
            .start(self.generation.ok_or_else(|| error("audio not ready"))?)
            .map_err(error)?;
        Ok(Vec::new())
    }
    pub(crate) fn pause(&mut self) -> Result<Vec<BroadcastEvent>> {
        if let Some(device) = &self.device {
            device.borrow_mut().pause().map_err(error)?;
        }
        self.generation = None;
        Ok(Vec::new())
    }
}

/// Delegates the device part of an audio adapter to its [`DeviceSink`].
macro_rules! device_sink_adapter {
    () => {
        fn submit_audio_packet(
            &mut self,
            packet: AudioFramePacket<Arc<[f32]>>,
        ) -> Result<Vec<BroadcastEvent>> {
            self.sink.submit(packet)
        }
        fn begin_audio_preroll(&mut self) -> Result<Vec<BroadcastEvent>> {
            self.sink.begin_preroll()
        }
        fn commit_audio_preroll(&mut self) -> Result<Vec<BroadcastEvent>> {
            self.sink.commit_preroll()
        }
        fn start_audio(&mut self) -> Result<Vec<BroadcastEvent>> {
            self.sink.start()
        }
        fn pause_audio(&mut self) -> Result<Vec<BroadcastEvent>> {
            self.sink.pause()
        }
        fn stop_audio(&mut self) -> Result<Vec<BroadcastEvent>> {
            self.sink.pause()
        }
    };
}
pub(crate) use device_sink_adapter;

pub(crate) struct Audio {
    pub sink: DeviceSink,
    decoders: Vec<Decoder>,
    input: Rc<DecodeInput>,
    pending_seek: Option<u64>,
    tracks: Vec<PcmTrack>,
    source: SourceRuntime,
    origin: (i64, Rational),
    media_uri: String,
    consumed_through: u64,
}
impl Audio {
    pub fn open(
        plan: &InputPlan,
        input: Rc<DecodeInput>,
        device_id: Option<String>,
        channel_map: Option<ChannelMap>,
        prebuffer_frames: usize,
    ) -> Result<Self> {
        let sink = DeviceSink::open(&plan.source, device_id, channel_map, prebuffer_frames)?
            .with_levels(plan.audio_levels.clone());
        let decoders = plan
            .audio_streams
            .iter()
            .map(|stream| input.open(stream.stream_index, None))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            sink,
            decoders,
            input,
            pending_seek: None,
            tracks: plan.audio_streams.iter().map(PcmTrack::new).collect(),
            source: plan.source.clone(),
            origin: plan.audio_origin,
            media_uri: plan.audio_media.media_uri.clone(),
            consumed_through: 0,
        })
    }
}
impl AudioOutputAdapter for Audio {
    type AudioPacket = Arc<[f32]>;
    fn cue_audio(&mut self, request: EngineFrameRequest) -> Result<()> {
        if request.source_id != self.source.source_id || request.timebase != self.source.timebase {
            return Err(error("audio seek source mismatch"));
        }
        seek_start(&self.source, request.frame)?;
        self.pending_seek = Some(request.frame);
        self.sink.audition_at(request.frame);
        Ok(())
    }
    fn prepare_audio(&mut self, _: &EngineSourceHandle) -> Result<Vec<BroadcastEvent>> {
        Ok(Vec::new())
    }
    fn render_audio_for_frame(
        &mut self,
        request: EngineFrameRequest,
    ) -> Result<AudioFramePacket<Arc<[f32]>>> {
        let format = self
            .source
            .audio_format
            .as_ref()
            .ok_or_else(|| error("audio not declared"))?;
        let start = sample_boundary(request.frame, request.timebase, format.sample_rate_hz)?;
        let end = sample_boundary(
            request
                .frame
                .checked_add(1)
                .ok_or_else(|| error("frame overflow"))?,
            request.timebase,
            format.sample_rate_hz,
        )?;
        if let Some(frame) = self.pending_seek {
            if request.frame != frame {
                return Err(error("audio seek target mismatch"));
            }
            // The sound in memory serves the cue; else the decoder opens a little before it.
            let ahead_frames = frames_of_seconds(self.source.timebase, SOUND_AHEAD_SECONDS);
            let ahead = sample_boundary(ahead_frames, request.timebase, format.sample_rate_hz)?;
            if !self.decoders.is_empty() && self.tracks.iter().all(|track| track.can_aim(start, ahead)) {
                self.tracks.iter_mut().for_each(|track| track.aim(start));
            } else {
                let back = frame.saturating_sub(frames_of_seconds(self.source.timebase, SOUND_BACK_SECONDS));
                let seek = seek_start(&self.source, back)?;
                let from = sample_boundary(back, request.timebase, format.sample_rate_hz)?;
                retire(std::mem::take(&mut self.decoders));
                for track in &mut self.tracks {
                    track.restart(from, start);
                    self.decoders.push(self.input.open(track.stream_index, seek)?);
                }
            }
            self.consumed_through = start;
            self.pending_seek = None;
        }
        if request.source_id != self.source.source_id
            || start != self.consumed_through
            || request.timebase != self.source.timebase
        {
            return Err(error(
                "audio request requires explicit decoder repositioning",
            ));
        }
        let sample_frames = usize::try_from(end - start).map_err(error)?;
        // Poll each existing decoder fairly; drain nothing until all channels are ready.
        for (track, decoder) in self.tracks.iter_mut().zip(&mut self.decoders) {
            // On the way to a cue the decoder catches up faster.
            let packets = if track.aiming() { 64 } else { 2 };
            for _ in 0..packets {
                track.settle();
                if track.ready(sample_frames) {
                    break;
                }
                match decoder.try_next_packet().map_err(error)? {
                    Poll::Pending => break,
                    Poll::Ready(None) => {
                        return Err(error("audio ended before saved video boundary"));
                    }
                    Poll::Ready(Some(packet)) => {
                        track.push(packet, &self.media_uri, self.origin, format.sample_rate_hz)?
                    }
                }
            }
            track.settle();
        }
        let samples = interleave_tracks(&mut self.tracks, sample_frames)?;
        self.consumed_through = end;
        Ok(AudioFramePacket {
            source_id: request.source_id,
            start_frame: request.frame,
            frame_count: 1,
            audio_format: Some(format.clone()),
            payload: samples.into(),
        })
    }
    device_sink_adapter!();
}

/// An error of the sound device: running dry is its own kind, the play prepares the sound
/// again instead of stopping (audit 2026-10-25).
fn audio_error(e: qnc_audio_output::Error) -> BroadcastEngineError {
    let kind = if e.code == qnc_audio_output::Code::Underrun {
        BroadcastEngineErrorKind::AudioUnderrun
    } else {
        BroadcastEngineErrorKind::Contract
    };
    BroadcastEngineError::new(kind, e.to_string())
}

pub(crate) fn validate_channel_map(
    format: Option<&AudioFormat>,
    map: Option<&ChannelMap>,
) -> Result<()> {
    match (format, map) {
        (None, None) => Ok(()),
        (Some(format), Some(map)) => {
            map.validate().map_err(error)?;
            if map.source_channels() != format.channel_count {
                return Err(error(
                    "device channel map differs from decoded channel inventory",
                ));
            }
            Ok(())
        }
        _ => Err(error("audio requires an explicit device channel map")),
    }
}

/// Seconds of decoded sound a track may hold: a program lane reads up to two seconds
/// ahead of the frame it plays (`program.rs`), with room for the frame itself and for the
/// sound before a cue that is decoded on the way to it (`SOUND_BACK_SECONDS`).
const PCM_TRACK_SECONDS: usize = 6;

/// The sound is heard whenever the playhead moves, back or forward, frame by frame (user
/// 2026-10-21: some cuts are made by ear). Sound already played stays in memory this long,
/// so a step back finds it there and no decoder is opened again (live 2026-10-21: every cue
/// opened the sound decoder anew, 577 times in ten minutes).
const PCM_HISTORY_SECONDS: usize = 2;

/// A cue outside the sound in memory opens the decoder this much earlier, so the steps back
/// after it find their sound in memory too.
pub(crate) const SOUND_BACK_SECONDS: u64 = 1;

/// A cue at most this far after the decoded sound waits for the open decoder to reach it.
pub(crate) const SOUND_AHEAD_SECONDS: u64 = 1;

/// The frames of `seconds` at `timebase`.
pub(crate) fn frames_of_seconds(timebase: Timebase, seconds: u64) -> u64 {
    let num = u64::try_from(timebase.fps_num.max(0)).unwrap_or(0);
    let den = u64::try_from(timebase.fps_den.max(1)).unwrap_or(1);
    seconds.saturating_mul(num) / den
}

pub(crate) struct PcmTrack {
    stream_index: u32,
    source_channels: u16,
    selected_channels: Vec<u16>,
    pub(crate) samples: VecDeque<f32>,
    pub(crate) decoded_through: Option<u64>,
    pub(crate) discard_before: u64,
    /// Sound taken before the read position, newest last (`PCM_HISTORY_SECONDS`).
    history: VecDeque<f32>,
    /// The read position waits for the decoder to reach this sample.
    target: Option<u64>,
    rate: Option<u32>,
}
impl PcmTrack {
    pub(crate) fn new(plan: &AudioStreamPlan) -> Self {
        debug_assert!(!plan.selected_channels.is_empty());
        debug_assert!(
            plan.selected_channels
                .iter()
                .all(|channel| *channel < plan.source_channels)
        );
        Self {
            stream_index: plan.stream_index,
            source_channels: plan.source_channels,
            selected_channels: plan.selected_channels.clone(),
            samples: VecDeque::new(),
            decoded_through: Some(0),
            discard_before: 0,
            history: VecDeque::new(),
            target: None,
            rate: None,
        }
    }
    fn output_channels(&self) -> usize {
        self.selected_channels.len()
    }

    /// The sample the next take starts at; `None` before a reopened decoder gave any.
    fn read_position(&self) -> Option<u64> {
        let pending = (self.samples.len() / self.output_channels().max(1)) as u64;
        self.decoded_through.map(|end| end.saturating_sub(pending).max(self.discard_before.min(end)))
    }

    /// Whether `target` is in memory, or at most `ahead` samples after the decoded sound.
    pub(crate) fn can_aim(&self, target: u64, ahead: u64) -> bool {
        let channels = self.output_channels().max(1) as u64;
        match (self.read_position(), self.decoded_through) {
            (Some(read), Some(end)) => {
                let back = self.history.len() as u64 / channels;
                target >= read.saturating_sub(back) && target <= end.saturating_add(ahead)
            }
            _ => false,
        }
    }

    /// Moves the read position to `target` (`can_aim` holds): from memory at once, or once
    /// the open decoder reaches it (`settle`).
    pub(crate) fn aim(&mut self, target: u64) {
        self.target = Some(target);
        self.settle();
    }

    /// A reopened decoder starts at `from` (before the cue) and the read position waits at
    /// `target`; the sound on the way goes to memory.
    pub(crate) fn restart(&mut self, from: u64, target: u64) {
        self.samples.clear();
        self.history.clear();
        self.decoded_through = None;
        self.discard_before = from;
        self.target = Some(target);
    }

    /// Whether the read position still waits for the decoder.
    pub(crate) fn aiming(&self) -> bool {
        self.target.is_some()
    }

    /// The read position goes to the target once the sound there is decoded.
    pub(crate) fn settle(&mut self) {
        let Some(target) = self.target else { return };
        let (Some(read), Some(end)) = (self.read_position(), self.decoded_through) else {
            return;
        };
        let channels = self.output_channels();
        if target >= read {
            if target > end {
                return;
            }
            let skip = (target - read) as usize * channels;
            let moved: Vec<f32> = self.samples.drain(..skip.min(self.samples.len())).collect();
            self.keep(moved);
        } else {
            let back = ((read - target) as usize * channels).min(self.history.len());
            let moved = self.history.split_off(self.history.len() - back);
            for value in moved.into_iter().rev() {
                self.samples.push_front(value);
            }
        }
        // Packets from now on all come after the decoded end, past the read position.
        self.discard_before = target.min(end);
        self.target = None;
    }

    /// Whether `frames` sample frames are ready at the read position.
    pub(crate) fn ready(&self, frames: usize) -> bool {
        self.target.is_none() && self.samples.len() >= frames * self.output_channels()
    }

    /// Takes `frames` sample frames (`ready` holds); they stay in memory for a step back.
    pub(crate) fn take(&mut self, frames: usize) -> Vec<f32> {
        let count = (frames * self.output_channels()).min(self.samples.len());
        let taken: Vec<f32> = self.samples.drain(..count).collect();
        self.keep(taken.clone());
        taken
    }

    fn keep(&mut self, played: Vec<f32>) {
        self.history.extend(played);
        let limit = self.rate.map_or(0, |rate| rate as usize * self.output_channels() * PCM_HISTORY_SECONDS);
        let over = self.history.len().saturating_sub(limit);
        self.history.drain(..over);
    }
    pub(crate) fn push(
        &mut self,
        packet: qnc_media_decode::DecodedPacket,
        uri: &str,
        origin: (i64, Rational),
        rate: u32,
    ) -> Result<()> {
        let source_channels = usize::from(self.source_channels);
        let position = relative_position(packet.pts, packet.time_base, origin, rate.into(), 1)?;
        if self
            .decoded_through
            .map_or(position > self.discard_before, |end| position != end)
            || packet.stream_index != self.stream_index
            || packet.media_uri != uri
            || packet.format
                != (DecodedFormat::Audio {
                    sample_rate_hz: rate,
                    channels: self.source_channels.into(),
                    sample_format: "f32le".into(),
                })
            || packet.bytes.is_empty()
            || !packet.bytes.len().is_multiple_of(source_channels * 4)
        {
            return Err(error("PCM timestamp or layout differs from saved input"));
        }
        let count = (packet.bytes.len() / (source_channels * 4)) as u64;
        let skip =
            self.discard_before.saturating_sub(position).min(count) as usize * source_channels * 4;
        let sample_frames_after_skip = (packet.bytes.len() - skip) / (source_channels * 4);
        if self.samples.len() + sample_frames_after_skip * self.output_channels()
            > rate as usize * self.output_channels() * PCM_TRACK_SECONDS
        {
            return Err(error("PCM slice buffer limit exceeded"));
        }
        self.decoded_through = Some(
            position
                .checked_add(count)
                .ok_or_else(|| error("PCM position overflow"))?,
        );
        self.rate = Some(rate);
        for frame in packet.bytes[skip..].chunks_exact(source_channels * 4) {
            for channel in &self.selected_channels {
                let offset = usize::from(*channel) * 4;
                self.samples.extend([f32::from_le_bytes(
                    frame[offset..offset + 4].try_into().expect("sample"),
                )]);
            }
        }
        Ok(())
    }
}

/// Output levels in percent as device gains (100 as recorded).
pub(crate) fn gains_of(levels: &[u16]) -> Vec<f32> {
    levels.iter().map(|level| f32::from(*level) / 100.0).collect()
}

fn interleave_tracks(tracks: &mut [PcmTrack], frames: usize) -> Result<Vec<f32>> {
    if tracks.iter().any(|track| !track.ready(frames)) {
        return Err(pending());
    }
    let channels: usize = tracks.iter().map(PcmTrack::output_channels).sum();
    let taken: Vec<(usize, Vec<f32>)> = tracks
        .iter_mut()
        .map(|track| (track.output_channels(), track.take(frames)))
        .collect();
    let mut samples = Vec::with_capacity(frames * channels);
    for frame in 0..frames {
        for (width, track) in &taken {
            samples.extend_from_slice(&track[frame * width..(frame + 1) * width]);
        }
    }
    Ok(samples)
}

#[cfg(test)]
#[path = "audio_live_test.rs"]
mod audio_live_test;

#[cfg(test)]
mod tests {
    use super::*;
    fn track_plan(
        stream_index: u32,
        source_channels: u16,
        selected_channels: &[u16],
    ) -> AudioStreamPlan {
        AudioStreamPlan {
            stream_index,
            source_channels,
            selected_channels: selected_channels.to_vec(),
        }
    }

    fn pcm(pts: i64, values: &[f32]) -> qnc_media_decode::DecodedPacket {
        pcm_channels(pts, 1, values)
    }

    fn pcm_channels(pts: i64, channels: u16, values: &[f32]) -> qnc_media_decode::DecodedPacket {
        qnc_media_decode::DecodedPacket {
            version: qnc_media_decode::VERSION.into(),
            media_uri: "saved".into(),
            stream_index: 1,
            ordinal: 0,
            pts,
            time_base: Rational {
                numerator: 1,
                denominator: 48000,
            },
            format: DecodedFormat::Audio {
                sample_rate_hz: 48000,
                channels: channels.into(),
                sample_format: "f32le".into(),
            },
            bytes: values.iter().flat_map(|v| v.to_le_bytes()).collect(),
        }
    }
    #[test]
    fn seek_trims_pcm_by_sample_position_without_rounding_or_relabelling() {
        let mut track = PcmTrack::new(&track_plan(1, 1, &[0]));
        track.decoded_through = None;
        track.discard_before = 5;
        let origin = (
            0,
            Rational {
                numerator: 1,
                denominator: 48000,
            },
        );
        track
            .push(pcm(1, &[1., 2., 3.]), "saved", origin, 48000)
            .unwrap();
        assert!(track.samples.is_empty());
        track
            .push(pcm(4, &[4., 5., 6.]), "saved", origin, 48000)
            .unwrap();
        assert_eq!(track.samples, [5., 6.]);
        assert_eq!(track.decoded_through, Some(7));
        assert!(track.push(pcm(8, &[8.]), "saved", origin, 48000).is_err());
        let mut missing = PcmTrack::new(&track_plan(1, 1, &[0]));
        missing.decoded_through = None;
        missing.discard_before = 5;
        assert!(missing.push(pcm(6, &[6.]), "saved", origin, 48000).is_err());
    }

    /// Locked (user 2026-10-21: the sound is heard whenever the playhead moves, back or
    /// forward): a cue within the sound in memory is served from it, with no decoder opened
    /// again; a cue a little ahead waits for the open decoder; anything else is not in reach.
    #[test]
    fn a_cue_back_or_forward_within_the_sound_in_memory_needs_no_new_decoder() {
        let origin = (0, Rational { numerator: 1, denominator: 48000 });
        let mut track = PcmTrack::new(&track_plan(1, 1, &[0]));
        track.push(pcm(0, &[0., 1., 2., 3., 4., 5.]), "saved", origin, 48000).unwrap();
        assert_eq!(track.take(4), [0., 1., 2., 3.]);
        // Back two samples: from memory, the same sound again.
        assert!(track.can_aim(2, 0));
        track.aim(2);
        assert!(track.ready(2));
        assert_eq!(track.take(2), [2., 3.]);
        // Forward past what is decoded, within reach: waits for the decoder, keeps the sound on the way.
        assert!(track.can_aim(8, 4));
        track.aim(8);
        assert!(track.aiming() && !track.ready(1));
        track.push(pcm(6, &[6., 7., 8., 9.]), "saved", origin, 48000).unwrap();
        track.settle();
        assert_eq!(track.take(2), [8., 9.]);
        track.aim(4);
        assert_eq!(track.take(3), [4., 5., 6.], "the sound skipped on the way is in memory");
        // Too far ahead, or before what memory holds: a new decoder.
        assert!(!track.can_aim(20, 4));
        let mut fresh = PcmTrack::new(&track_plan(1, 1, &[0]));
        fresh.push(pcm(0, &[0., 1.]), "saved", origin, 48000).unwrap();
        fresh.restart(100, 150);
        assert!(!fresh.can_aim(150, 0) && fresh.aiming());
        fresh.push(pcm(100, &(0..60).map(|v| v as f32).collect::<Vec<_>>()), "saved", origin, 48000).unwrap();
        fresh.settle();
        assert_eq!(fresh.take(1), [50.], "a reopened decoder reads from before the cue");
        assert!(fresh.can_aim(120, 0), "and the sound before the cue is in memory");
    }

    #[test]
    fn levels_become_device_gains_level_0_silent_100_as_recorded() {
        assert_eq!(gains_of(&[0, 100, 200]), [0.0, 1.0, 2.0]);
        assert!(gains_of(&[]).is_empty(), "no levels: the device keeps its gains");
    }

    #[test]
    fn selected_channels_from_one_stream_become_separate_mono_lanes() {
        let origin = (
            0,
            Rational {
                numerator: 1,
                denominator: 48000,
            },
        );
        let mut a1_a2 = PcmTrack::new(&track_plan(1, 4, &[0, 1]));
        a1_a2
            .push(
                pcm_channels(0, 4, &[1.0, 2.0, 3.0, 4.0, 10.0, 20.0, 30.0, 40.0]),
                "saved",
                origin,
                48000,
            )
            .unwrap();
        assert_eq!(a1_a2.samples, [1.0, 2.0, 10.0, 20.0]);
        assert_eq!(
            interleave_tracks(&mut [a1_a2], 2).unwrap(),
            [1.0, 2.0, 10.0, 20.0]
        );

        let mut a2_only = PcmTrack::new(&track_plan(1, 4, &[1]));
        a2_only
            .push(
                pcm_channels(0, 4, &[1.0, 2.0, 3.0, 4.0, 10.0, 20.0, 30.0, 40.0]),
                "saved",
                origin,
                48000,
            )
            .unwrap();
        assert_eq!(a2_only.samples, [2.0, 20.0]);
    }

    #[test]
    fn pending_track_does_not_consume_other_channels() {
        let mut tracks: Vec<_> = (1..=4)
            .map(|i| PcmTrack::new(&track_plan(i, 1, &[0])))
            .collect();
        for (i, track) in tracks.iter_mut().enumerate().take(3) {
            track.samples.extend([i as f32, i as f32 + 10.0]);
        }
        assert_eq!(
            interleave_tracks(&mut tracks, 2).unwrap_err().kind,
            BroadcastEngineErrorKind::NotReady
        );
        assert_eq!(tracks[0].samples.len(), 2);
        tracks[3].samples.extend([3.0, 13.0, 23.0]);
        assert_eq!(
            interleave_tracks(&mut tracks, 2).unwrap(),
            [0., 1., 2., 3., 10., 11., 12., 13.]
        );
        assert_eq!(tracks[3].samples, [23.0]);
    }
    #[test]
    fn device_routing_is_explicit_and_cannot_change_source_format() {
        let format = AudioFormat::new(48000, 4).unwrap();
        assert!(validate_channel_map(Some(&format), None).is_err());
        assert!(
            validate_channel_map(
                Some(&format),
                Some(&ChannelMap::new(2, vec![0, 1]).unwrap())
            )
            .is_err()
        );
        assert!(
            validate_channel_map(
                Some(&format),
                Some(&ChannelMap::new(4, vec![2, 3]).unwrap())
            )
            .is_ok()
        );
        assert_eq!(format.channel_count, 4);
    }
}
