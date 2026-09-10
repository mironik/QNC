use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::engine_contract::{
    AudioFramePacket, AudioOutputAdapter, BroadcastEngineError, BroadcastEngineErrorKind,
    DecodedVideoFrame, EngineFrameRequest, EngineSourceHandle, FramePresenter, PlayoutFrame,
    PlayoutOutput, SourceOpenAdapter, VideoDecodeAdapter,
};
use crate::event::BroadcastEvent;
use crate::frame_clock::{ClockTick, FrameClock, FrameClockConfig, FrameClockRate};
use crate::model::{FrameNumber, FrameRange, SourceRuntime, TransportStatus};

const DEFAULT_DECODE_BURST_FRAMES: usize = 4;
const MIN_PLAYOUT_BUFFER_FRAMES: usize = 4;
const MAX_DECODE_BURST_FRAMES: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportEngineState {
    pub source: Option<EngineSourceHandle>,
    pub preloaded_sources: BTreeMap<String, EngineSourceHandle>,
    pub carrier_frame: FrameNumber,
    /// Last acknowledged output handoff, not physical display position.
    pub submitted_frame: Option<FrameNumber>,
    pub presented_frame: Option<FrameNumber>,
    pub at_end: bool,
    pub status: TransportStatus,
    pub play_ready: bool,
    pub active_range: Option<FrameRange>,
    pub playback_rate_num: i32,
    pub playback_rate_den: u32,
    pub decode_burst_frames: usize,
}

impl Default for TransportEngineState {
    fn default() -> Self {
        Self {
            source: None,
            preloaded_sources: BTreeMap::new(),
            carrier_frame: 0,
            submitted_frame: None,
            presented_frame: None,
            at_end: false,
            status: TransportStatus::Empty,
            play_ready: false,
            active_range: None,
            playback_rate_num: 1,
            playback_rate_den: 1,
            decode_burst_frames: DEFAULT_DECODE_BURST_FRAMES,
        }
    }
}

pub struct SplitAvPlayoutOutput<A, P> {
    audio_output: A,
    frame_presenter: P,
    has_audio: bool,
}

impl<A, P> SplitAvPlayoutOutput<A, P> {
    pub fn new(audio_output: A, frame_presenter: P) -> Self {
        Self {
            audio_output,
            frame_presenter,
            has_audio: false,
        }
    }
}

impl<A, P> PlayoutOutput for SplitAvPlayoutOutput<A, P>
where
    A: AudioOutputAdapter,
    P: FramePresenter,
{
    type VideoFrame = P::VideoFrame;
    type AudioPacket = A::AudioPacket;

    fn cue_output(&mut self, request: EngineFrameRequest) -> Result<(), BroadcastEngineError> {
        if self.has_audio {
            self.audio_output.cue_audio(request)?;
        }
        Ok(())
    }

    fn prepare_start_frame(
        &mut self,
        frame: &DecodedVideoFrame<Self::VideoFrame>,
    ) -> Result<bool, BroadcastEngineError> {
        self.frame_presenter.prepare_start_frame(frame)
    }

    fn append_playout_audio(
        &mut self,
        packet: AudioFramePacket<Self::AudioPacket>,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        self.audio_output.submit_audio_packet(packet)
    }

    fn prepare_output(
        &mut self,
        source: &EngineSourceHandle,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        self.has_audio = source.audio_format.is_some();
        let mut events = Vec::new();
        if source.video_format.is_some() {
            events.extend(self.frame_presenter.prepare_presentation(source)?);
        }
        if self.has_audio {
            events.extend(self.audio_output.prepare_audio(source)?);
        }
        Ok(events)
    }

    fn render_audio_for_frame(
        &mut self,
        request: EngineFrameRequest,
    ) -> Result<AudioFramePacket<Self::AudioPacket>, BroadcastEngineError> {
        self.audio_output.render_audio_for_frame(request)
    }

    fn submit_playout_frame(
        &mut self,
        frame: PlayoutFrame<Self::VideoFrame, Self::AudioPacket>,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        let mut events = Vec::new();
        if let Some(audio) = frame.audio {
            events.extend(self.audio_output.submit_audio_packet(audio)?);
        }
        if let Some(video) = frame.video {
            events.extend(self.frame_presenter.present_frame(video)?);
        }
        Ok(events)
    }

    fn begin_playout_preroll(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        if self.has_audio {
            self.audio_output.begin_audio_preroll()
        } else {
            Ok(Vec::new())
        }
    }

    fn submit_preroll_audio(
        &mut self,
        packet: AudioFramePacket<Self::AudioPacket>,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        self.audio_output.submit_audio_packet(packet)
    }

    fn commit_playout_preroll(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        if self.has_audio {
            self.audio_output.commit_audio_preroll()
        } else {
            Ok(Vec::new())
        }
    }

    fn start_playout(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        if self.has_audio {
            self.audio_output.start_audio()
        } else {
            Ok(Vec::new())
        }
    }

    fn pause_playout(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        if self.has_audio {
            self.audio_output.pause_audio()
        } else {
            Ok(Vec::new())
        }
    }

    fn stop_playout(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        let mut events = if self.has_audio {
            self.audio_output.stop_audio()?
        } else {
            Vec::new()
        };
        self.has_audio = false;
        events.extend(self.frame_presenter.reset_presentation()?);
        Ok(events)
    }
}

pub struct TransportEngine<S, V, O>
where
    S: SourceOpenAdapter,
    V: VideoDecodeAdapter,
    O: PlayoutOutput<VideoFrame = V::VideoFrame>,
{
    source_open: S,
    video_decode: V,
    playout_output: O,
    clock: Option<FrameClock>,
    prepared_anchor: Option<FrameNumber>,
    pending_cue: Option<(FrameNumber, bool)>,
    idle_prebuffer_failed: bool,
    min_prebuffer_frames: usize,
    state: TransportEngineState,
    playout: PlayoutBuffer<V::VideoFrame, O::AudioPacket>,
}

impl<S, V, A, P> TransportEngine<S, V, SplitAvPlayoutOutput<A, P>>
where
    S: SourceOpenAdapter,
    V: VideoDecodeAdapter,
    A: AudioOutputAdapter,
    P: FramePresenter<VideoFrame = V::VideoFrame>,
{
    pub fn new(source_open: S, video_decode: V, audio_output: A, frame_presenter: P) -> Self {
        Self {
            source_open,
            video_decode,
            playout_output: SplitAvPlayoutOutput::new(audio_output, frame_presenter),
            clock: None,
            prepared_anchor: None,
            pending_cue: None,
            idle_prebuffer_failed: false,
            min_prebuffer_frames: MIN_PLAYOUT_BUFFER_FRAMES,
            state: TransportEngineState::default(),
            playout: PlayoutBuffer::default(),
        }
    }
}

impl<S, V, O> TransportEngine<S, V, O>
where
    S: SourceOpenAdapter,
    V: VideoDecodeAdapter,
    O: PlayoutOutput<VideoFrame = V::VideoFrame>,
{
    pub fn with_playout_output(source_open: S, video_decode: V, playout_output: O) -> Self {
        Self {
            source_open,
            video_decode,
            playout_output,
            clock: None,
            prepared_anchor: None,
            pending_cue: None,
            idle_prebuffer_failed: false,
            min_prebuffer_frames: MIN_PLAYOUT_BUFFER_FRAMES,
            state: TransportEngineState::default(),
            playout: PlayoutBuffer::default(),
        }
    }

    pub fn with_decode_burst_frames(mut self, decode_burst_frames: usize) -> Self {
        self.state.decode_burst_frames = decode_burst_frames.clamp(1, MAX_DECODE_BURST_FRAMES);
        self
    }

    pub fn state(&self) -> &TransportEngineState {
        &self.state
    }

    /// Reserve AV before Ready without increasing per-tick work.
    pub fn with_min_prebuffer_frames(mut self, frames: usize) -> Self {
        self.min_prebuffer_frames = frames.clamp(MIN_PLAYOUT_BUFFER_FRAMES, 64);
        self
    }

    pub fn load_source(
        &mut self,
        source: &SourceRuntime,
        source_revision: Option<u64>,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        source.validate().map_err(contract_error)?;
        let handle = self.source_open.open_source(source, source_revision)?;
        require_matching_handle(source, source_revision, &handle)?;
        let mut events = self.interrupt_motion()?;
        self.close_active_source()?;
        self.close_preloaded_source(&source.source_id)?;
        events.extend(self.activate_source_handle(handle)?);
        Ok(events)
    }

    pub fn preload_source(
        &mut self,
        source: &SourceRuntime,
        source_revision: Option<u64>,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        source.validate().map_err(contract_error)?;
        let handle = self.source_open.open_source(source, source_revision)?;
        require_matching_handle(source, source_revision, &handle)?;
        // A preload is only an open source handle, not an active prepared output.
        let mut events = Vec::new();
        self.close_preloaded_source(&source.source_id)?;
        let source_id = handle.source_id.clone();
        self.state
            .preloaded_sources
            .insert(source_id.clone(), handle);
        events.push(BroadcastEvent::SourcePreloaded { source_id });
        Ok(events)
    }

    pub fn set_active_source(
        &mut self,
        source: &SourceRuntime,
        source_revision: Option<u64>,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        source.validate().map_err(contract_error)?;
        if let Some(handle) = self.state.preloaded_sources.get(&source.source_id) {
            require_matching_handle(source, source_revision, handle)?;
        }
        if let Some(handle) = self.state.preloaded_sources.remove(&source.source_id) {
            let mut events = self.interrupt_motion()?;
            self.close_active_source()?;
            events.extend(self.activate_source_handle(handle)?);
            return Ok(events);
        }
        self.load_source(source, source_revision)
    }

    pub fn unload_source(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        let mut events = self.interrupt_motion()?;
        self.close_active_source()?;
        self.close_preloaded_sources()?;
        self.playout.clear();
        self.state.carrier_frame = 0;
        self.state.presented_frame = None;
        self.state.submitted_frame = None;
        self.state.at_end = false;
        self.state.status = TransportStatus::Empty;
        self.state.active_range = None;
        events.push(BroadcastEvent::ActiveSourceChanged { source_id: None });
        events.push(BroadcastEvent::RangeChanged { range: None });
        events.push(BroadcastEvent::TransportStatusChanged {
            status: TransportStatus::Empty,
        });
        events.push(self.position_event());
        Ok(events)
    }

    pub fn play(
        &mut self,
        now_tick: ClockTick,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        let range = {
            let source = self.require_source()?;
            self.active_range_or_source(source)?
        };
        if self.state.status == TransportStatus::Playing {
            return Ok(Vec::new());
        }
        let anchor = if self.state.at_end {
            range.start_frame
        } else {
            self.state.carrier_frame
        };
        if !self.state.play_ready || self.prepared_anchor != Some(anchor) {
            return Err(BroadcastEngineError::new(
                BroadcastEngineErrorKind::NotReady,
                "playback preparation is not complete",
            ));
        }
        // No source open, decode, queue fill or deferred preroll is allowed on this path.
        let result = (|| {
            let mut events = self.present_buffered_frame(anchor, false)?;
            events.extend(self.playout_output.start_playout()?);
            self.start_play_clock(now_tick, anchor)?;
            Ok::<_, BroadcastEngineError>(events)
        })();
        let mut events = match result {
            Ok(events) => events,
            Err(error) => {
                let _ = self.suspend_motion();
                self.idle_prebuffer_failed = true;
                self.state.status = TransportStatus::Paused;
                return Err(error);
            }
        };
        self.state.at_end = false;
        self.state.status = TransportStatus::Playing;
        self.state.play_ready = false;
        self.prepared_anchor = None;
        events.push(self.readiness_event());
        events.push(BroadcastEvent::TransportStatusChanged {
            status: TransportStatus::Playing,
        });
        events.push(self.position_event());
        Ok(events)
    }

    pub fn pause(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        self.require_source()?;
        let mut events = self.suspend_motion()?;
        self.state.status = TransportStatus::Paused;
        events.push(BroadcastEvent::TransportStatusChanged {
            status: TransportStatus::Paused,
        });
        Ok(events)
    }

    pub fn stop(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        self.require_source()?;
        let mut events = self.suspend_motion()?;
        self.state.status = TransportStatus::Stopped;
        events.push(BroadcastEvent::TransportStatusChanged {
            status: TransportStatus::Stopped,
        });
        Ok(events)
    }

    pub fn sync_range_runtime(
        &mut self,
        active_range: Option<FrameRange>,
        carrier_frame: FrameNumber,
        present_frame: bool,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        let was_moving = matches!(self.state.status, TransportStatus::Playing);
        let resolved_range = {
            let source = self.require_source()?;
            let range = active_range.unwrap_or(self.active_range_or_source(source)?);
            self.require_range_inside_source(range)?;
            range
        };
        if !resolved_range.contains_position(carrier_frame) {
            return Err(contract_error("requested frame is outside execution range"));
        }
        let mut events = self.suspend_motion()?;
        self.pending_cue = None;
        self.idle_prebuffer_failed = false;
        let carrier_changed = self.state.carrier_frame != carrier_frame;
        let previous_range = self.state.active_range;
        self.state.active_range = Some(resolved_range);
        self.playout.reset_for_frame(carrier_frame);

        events.push(BroadcastEvent::RangeChanged {
            range: Some(resolved_range),
        });

        if was_moving || carrier_changed || present_frame {
            self.state.status = TransportStatus::Paused;
            events.push(BroadcastEvent::TransportStatusChanged {
                status: TransportStatus::Paused,
            });
        } else if self.state.status == TransportStatus::Ready {
            self.state.status = TransportStatus::Preparing;
            events.push(BroadcastEvent::TransportStatusChanged {
                status: TransportStatus::Preparing,
            });
        }

        if carrier_changed || present_frame {
            let result = self
                .decode_frame_to_buffer(carrier_frame)
                .and_then(|()| self.present_buffered_frame(carrier_frame, false));
            match result {
                Ok(presented) => events.extend(presented),
                Err(error) => {
                    self.state.active_range = previous_range;
                    self.playout.clear();
                    return Err(error);
                }
            }
        } else {
            events.push(self.position_event());
        }
        self.state.at_end = false;
        Ok(events)
    }

    pub fn apply_request_rate(
        &mut self,
        rate_num: i32,
        rate_den: u32,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        self.require_source()?;
        if rate_num <= 0 {
            return Err(contract_error("playback rate must be positive"));
        }
        FrameClockRate::new(rate_num, rate_den)
            .map_err(|err| BroadcastEngineError::new(BroadcastEngineErrorKind::Contract, err))?;
        let mut events = self.suspend_motion()?;
        self.state.playback_rate_num = rate_num;
        self.state.playback_rate_den = rate_den;
        self.state.status = TransportStatus::Preparing;
        events.push(BroadcastEvent::TransportStatusChanged {
            status: TransportStatus::Preparing,
        });
        Ok(events)
    }

    /// Confirm the new position only after preparation, not when the request arrives.
    pub fn cue_frame(
        &mut self,
        frame: FrameNumber,
        present: bool,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        let range = self.current_range()?;
        if frame < range.start_frame || frame >= range.end_frame {
            return Err(contract_error("cue is outside execution range"));
        }
        let mut events = self.suspend_motion()?;
        self.pending_cue = None;
        self.playout.reset_for_frame(frame);
        self.state.status = TransportStatus::Preparing;
        self.state.at_end = false;
        if let Err(error) = self.queue_cue(frame, present) {
            self.idle_prebuffer_failed = true;
            self.state.status = TransportStatus::Paused;
            return Err(error);
        }
        events.push(BroadcastEvent::TransportStatusChanged {
            status: TransportStatus::Preparing,
        });
        Ok(events)
    }

    pub fn tick(
        &mut self,
        now_tick: ClockTick,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        let mut events = Vec::new();
        if self.clock.is_none() {
            if self.state.source.is_some()
                && !matches!(self.state.status, TransportStatus::Empty)
                && !self.state.play_ready
                && !self.idle_prebuffer_failed
            {
                match self.advance_preparation() {
                    Ok(refill_events) => events.extend(refill_events),
                    Err(error) => {
                        let _ = self.suspend_motion();
                        self.idle_prebuffer_failed = true;
                        self.state.status = TransportStatus::Paused;
                        events.push(error.to_event());
                        events.push(self.readiness_event());
                        events.push(BroadcastEvent::TransportStatusChanged {
                            status: TransportStatus::Paused,
                        });
                    }
                }
            }
            return Ok(events);
        };
        match self.tick_playing(now_tick) {
            Ok(events) => Ok(events),
            Err(error) => {
                let _ = self.suspend_motion();
                self.idle_prebuffer_failed = true;
                self.state.status = TransportStatus::Paused;
                Err(error)
            }
        }
    }

    fn tick_playing(
        &mut self,
        now_tick: ClockTick,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        let end = self
            .state
            .carrier_frame
            .saturating_add(self.healthy_buffer_frames() as u64 + 1)
            .min(self.current_range()?.end_frame);
        let mut events = self.refill_playout_buffer(self.state.carrier_frame, end)?;
        let max_due = self.state.decode_burst_frames.max(1);
        for _ in 0..max_due {
            let Some((scheduled, advanced_clock)) = self.peek_next_due_frame(now_tick) else {
                break;
            };

            let range = self.current_range()?;
            // OUT is an exclusive time boundary, never a decode request. Hold
            // the final presented frame for its full interval before pausing.
            if scheduled.frame >= range.end_frame {
                events.extend(self.apply_playback_boundary()?);
                return Ok(events);
            }
            let frame = scheduled.frame;
            if !self.playout_ready(frame)? {
                return Err(BroadcastEngineError::new(
                    BroadcastEngineErrorKind::NotReady,
                    "due AV frame is not prepared; output paused",
                )
                .with_frame(frame));
            }

            events.extend(self.present_buffered_frame(frame, true)?);
            self.clock = Some(advanced_clock);
        }
        Ok(events)
    }

    fn peek_next_due_frame(
        &self,
        now_tick: ClockTick,
    ) -> Option<(crate::ScheduledFrame, FrameClock)> {
        let mut clock = self.clock.clone()?;
        let scheduled = clock.next_due_frame(now_tick)?;
        Some((scheduled, clock))
    }

    fn apply_playback_boundary(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        let range = self.current_range()?;
        let mut events = vec![BroadcastEvent::PlaybackBoundaryReached {
            frame: range.end_frame,
        }];
        events.extend(self.suspend_motion()?);
        self.pending_cue = None;
        self.playout.reset_for_frame(range.start_frame);
        self.state.carrier_frame = range.start_frame;
        self.state.at_end = false;
        self.state.status = TransportStatus::Preparing;
        if let Err(error) = self.queue_cue(range.start_frame, true) {
            self.idle_prebuffer_failed = true;
            self.state.status = TransportStatus::Paused;
            return Err(error);
        }
        events.push(BroadcastEvent::TransportStatusChanged {
            status: TransportStatus::Preparing,
        });
        events.push(self.position_event());
        Ok(events)
    }

    fn queue_cue(&mut self, frame: FrameNumber, present: bool) -> Result<(), BroadcastEngineError> {
        let source = self.require_source()?.clone();
        let request = EngineFrameRequest::new(&source, frame)?;
        if source.video_format.is_some() {
            self.video_decode.cue_video(request.clone())?;
        }
        self.playout_output.cue_output(request)?;
        self.pending_cue = Some((frame, present));
        Ok(())
    }

    fn interrupt_motion(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        self.pending_cue = None;
        self.prepared_anchor = None;
        self.state.play_ready = false;
        self.clock = None;
        let mut events = self.playout_output.stop_playout()?;
        events.extend(self.video_decode.stop_video()?);
        self.playout.clear();
        events.push(self.readiness_event());
        Ok(events)
    }

    fn suspend_motion(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        self.prepared_anchor = None;
        self.state.play_ready = false;
        self.clock = None;
        self.idle_prebuffer_failed = false;
        self.playout.primed_audio.clear();
        let mut events = self.playout_output.pause_playout()?;
        events.push(self.readiness_event());
        Ok(events)
    }

    fn prepare_source_handle(
        &mut self,
        handle: &EngineSourceHandle,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        if handle.video_format.is_none() && handle.audio_format.is_none() {
            return Err(BroadcastEngineError::new(
                BroadcastEngineErrorKind::Contract,
                "source has no playable tracks",
            )
            .with_source_id(handle.source_id.clone()));
        }

        let mut events = Vec::new();
        if handle.video_format.is_some() {
            events.extend(self.video_decode.prepare_video(handle)?);
        }
        events.extend(self.playout_output.prepare_output(handle)?);
        Ok(events)
    }

    fn activate_source_handle(
        &mut self,
        handle: EngineSourceHandle,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        self.state.source = Some(handle.clone());
        self.pending_cue = None;
        self.state.carrier_frame = 0;
        self.state.presented_frame = None;
        self.state.submitted_frame = None;
        self.state.at_end = false;
        self.state.status = TransportStatus::Preparing;
        self.state.play_ready = false;
        self.prepared_anchor = None;
        self.state.active_range = source_range(&handle);
        self.state.playback_rate_num = 1;
        self.state.playback_rate_den = 1;
        self.idle_prebuffer_failed = false;
        self.playout.reset_for_frame(0);
        let mut events = match self.prepare_source_handle(&handle) {
            Ok(events) => events,
            Err(error) => {
                let _ = self.interrupt_motion();
                let _ = self.close_active_source();
                self.state.active_range = None;
                self.idle_prebuffer_failed = true;
                self.state.status = TransportStatus::Empty;
                return Err(error);
            }
        };
        events.extend([
            BroadcastEvent::ActiveSourceChanged {
                source_id: Some(handle.source_id),
            },
            BroadcastEvent::RangeChanged {
                range: self.state.active_range,
            },
            BroadcastEvent::TransportStatusChanged {
                status: TransportStatus::Preparing,
            },
            self.readiness_event(),
            self.position_event(),
        ]);
        Ok(events)
    }

    fn close_active_source(&mut self) -> Result<(), BroadcastEngineError> {
        if let Some(active_source) = self.state.source.take() {
            self.source_open.close_source(&active_source.source_id)?;
        }
        self.playout.clear();
        Ok(())
    }

    fn close_preloaded_source(&mut self, source_id: &str) -> Result<(), BroadcastEngineError> {
        if let Some(source) = self.state.preloaded_sources.remove(source_id) {
            self.source_open.close_source(&source.source_id)?;
        }
        Ok(())
    }

    fn close_preloaded_sources(&mut self) -> Result<(), BroadcastEngineError> {
        let source_ids: Vec<String> = self.state.preloaded_sources.keys().cloned().collect();
        for source_id in source_ids {
            self.close_preloaded_source(&source_id)?;
        }
        Ok(())
    }

    fn decode_frame_to_buffer(&mut self, frame: FrameNumber) -> Result<(), BroadcastEngineError> {
        let audio = self.render_audio_to_buffer(frame);
        let video = self.decode_video_to_buffer(frame);
        audio?;
        video
    }

    fn decode_video_to_buffer(&mut self, frame: FrameNumber) -> Result<(), BroadcastEngineError> {
        let source = self.require_source()?.clone();
        let request = EngineFrameRequest::new(&source, frame)?;
        if source.video_format.is_some() && !self.playout.video.contains_key(&frame) {
            let video_frame = self.video_decode.decode_video_frame(request.clone())?;
            if video_frame.source_id != source.source_id || video_frame.frame != frame {
                return Err(contract_error(
                    "decoder returned a different source or frame",
                ));
            }
            self.playout.video.insert(frame, video_frame);
        }
        Ok(())
    }

    fn render_audio_to_buffer(&mut self, frame: FrameNumber) -> Result<(), BroadcastEngineError> {
        let source = self.require_source()?.clone();
        let request = EngineFrameRequest::new(&source, frame)?;
        if source.audio_format.is_some() && !self.playout.audio.contains_key(&frame) {
            let audio_packet = self.playout_output.render_audio_for_frame(request)?;
            if audio_packet.source_id != source.source_id
                || audio_packet.start_frame != frame
                || audio_packet.frame_count != 1
            {
                return Err(contract_error(
                    "audio output returned a different source or frame range",
                ));
            }
            self.playout.audio.insert(frame, audio_packet);
        }
        Ok(())
    }

    fn present_buffered_frame(
        &mut self,
        frame: FrameNumber,
        submit_audio: bool,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        let source = self.require_source()?.clone();
        let audio = if source.audio_format.is_some() && submit_audio {
            if self.playout.primed_audio.contains(&frame) {
                None
            } else {
                Some(self.playout.audio.get(&frame).cloned().ok_or_else(|| {
                    BroadcastEngineError::new(
                        BroadcastEngineErrorKind::AudioOutput,
                        format!("audio frame {frame} is not ready for playout"),
                    )
                    .with_source_id(source.source_id.clone())
                    .with_frame(frame)
                })?)
            }
        } else {
            None
        };
        let video = if source.video_format.is_some() {
            Some(self.playout.video.get(&frame).cloned().ok_or_else(|| {
                BroadcastEngineError::new(
                    BroadcastEngineErrorKind::VideoDecode,
                    format!("video frame {frame} is not ready for playout"),
                )
                .with_source_id(source.source_id.clone())
                .with_frame(frame)
            })?)
        } else {
            None
        };
        let mut events = self.playout_output.submit_playout_frame(PlayoutFrame {
            frame,
            video,
            audio,
        })?;
        if submit_audio && source.audio_format.is_some() {
            self.playout.primed_audio.insert(frame);
        }
        if source.video_format.is_some() {
            let mut submitted = false;
            let mut presented = false;
            for event in &events {
                let (confirmed, physical) = match event {
                    BroadcastEvent::VideoFrameSubmitted { frame } => (*frame, false),
                    BroadcastEvent::FramePresented { frame } => (*frame, true),
                    _ => continue,
                };
                if confirmed != frame {
                    return Err(contract_error(
                        "output acknowledged a different video frame",
                    ));
                }
                submitted = true;
                presented |= physical;
            }
            if !submitted {
                return Err(BroadcastEngineError::new(
                    BroadcastEngineErrorKind::VideoPresent,
                    "output did not confirm submission of the requested video frame",
                )
                .with_source_id(source.source_id)
                .with_frame(frame));
            }
            self.state.submitted_frame = Some(frame);
            if presented {
                self.state.presented_frame = Some(frame);
            }
        }
        self.state.carrier_frame = frame;
        events.push(self.position_event());
        self.playout.trim_before(frame.saturating_sub(1));
        Ok(events)
    }

    fn top_up_audio_output_queue(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        if !matches!(self.state.status, TransportStatus::Playing)
            || self.require_source()?.audio_format.is_none()
        {
            return Ok(Vec::new());
        }
        let range = self.current_range()?;
        let queue_through = self
            .state
            .carrier_frame
            .saturating_add(u64::try_from(self.healthy_buffer_frames()).unwrap_or(u64::MAX))
            .min(range.end_frame - 1);
        let frames = self
            .playout
            .audio
            .range(..=queue_through)
            .filter(|(frame, _)| {
                **frame >= self.state.carrier_frame && !self.playout.primed_audio.contains(frame)
            })
            .map(|(frame, _)| *frame)
            .collect::<Vec<_>>();
        if frames.is_empty() {
            return Ok(Vec::new());
        }
        let mut events = Vec::new();
        for frame in frames {
            let Some(packet) = self.playout.audio.get(&frame).cloned() else {
                continue;
            };
            events.extend(self.playout_output.append_playout_audio(packet)?);
            self.playout.primed_audio.insert(frame);
        }
        Ok(events)
    }

    fn advance_preparation(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        let range = self.current_range()?;
        let anchor = if let Some((frame, _)) = self.pending_cue {
            frame
        } else if self.state.at_end {
            range.start_frame
        } else {
            self.state.carrier_frame
        };
        if self.state.at_end && self.pending_cue.is_none() && !self.playout_ready(anchor)? {
            // Replaying beyond the retained window requires a real cue on both
            // adapters; rewinding counters alone would relabel consumed media.
            let source = self.require_source()?.clone();
            let request = EngineFrameRequest::new(&source, anchor)?;
            self.playout.reset_for_frame(anchor);
            if source.video_format.is_some() {
                self.video_decode.cue_video(request.clone())?;
            }
            self.playout_output.cue_output(request)?;
            self.pending_cue = Some((anchor, false));
        }
        let end = anchor
            .saturating_add(self.healthy_buffer_frames() as u64)
            .min(range.end_frame);
        self.refill_playout_buffer(anchor, end)?;
        if self.pending_cue.is_some_and(|(_, present)| present) {
            if !self.playout_ready(anchor)? {
                return Ok(Vec::new());
            }
            if let Some(video) = self.playout.video.get(&anchor)
                && !self.playout_output.prepare_start_frame(video)?
            {
                return Ok(Vec::new());
            }
            let events = self.present_buffered_frame(anchor, false)?;
            self.pending_cue = Some((anchor, false));
            // The next ticks keep building preroll before committing readiness.
            return Ok(events);
        }
        for frame in anchor..end {
            if !self.playout_ready(frame)? {
                return Ok(Vec::new());
            }
        }
        if let Some(video) = self.playout.video.get(&anchor)
            && !self.playout_output.prepare_start_frame(video)?
        {
            return Ok(Vec::new());
        }
        let mut events = self.playout_output.begin_playout_preroll()?;
        if self.require_source()?.audio_format.is_some() {
            for frame in anchor..end {
                let packet = self
                    .playout
                    .audio
                    .get(&frame)
                    .cloned()
                    .ok_or_else(|| contract_error("missing prepared audio packet"))?;
                events.extend(self.playout_output.submit_preroll_audio(packet)?);
                self.playout.primed_audio.insert(frame);
            }
        }
        events.extend(self.playout_output.commit_playout_preroll()?);
        if self.pending_cue.take().is_some() && self.state.carrier_frame != anchor {
            self.state.carrier_frame = anchor;
            events.push(self.position_event());
        }
        self.prepared_anchor = Some(anchor);
        self.state.play_ready = true;
        if self.state.status == TransportStatus::Preparing {
            self.state.status = TransportStatus::Ready;
            events.push(BroadcastEvent::SourceReady {
                source_id: self.require_source()?.source_id.clone(),
            });
            events.push(BroadcastEvent::TransportStatusChanged {
                status: TransportStatus::Ready,
            });
        }
        events.push(self.readiness_event());
        Ok(events)
    }

    fn start_play_clock(
        &mut self,
        now_tick: ClockTick,
        anchor: FrameNumber,
    ) -> Result<(), BroadcastEngineError> {
        let timebase = self.require_source()?.timebase;
        let rate = FrameClockRate::new(self.state.playback_rate_num, self.state.playback_rate_den)
            .map_err(|err| BroadcastEngineError::new(BroadcastEngineErrorKind::Contract, err))?;
        let mut clock = FrameClock::start(FrameClockConfig::new(timebase, rate), anchor, now_tick);
        // Play already handed off the anchor; the next scheduled frame is its successor.
        clock.next_due_frame(now_tick);
        self.clock = Some(clock);
        Ok(())
    }

    fn readiness_event(&self) -> BroadcastEvent {
        BroadcastEvent::PlaybackReadinessChanged {
            source_id: self
                .state
                .source
                .as_ref()
                .map(|source| source.source_id.clone()),
            frame: self.prepared_anchor.unwrap_or(self.state.carrier_frame),
            ready: self.state.play_ready,
        }
    }

    fn refill_playout_buffer(
        &mut self,
        anchor_frame: FrameNumber,
        end_frame: FrameNumber,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        let source = self.require_source()?.clone();
        // Each lane has its own bounded cursor. Pending video cannot starve PCM,
        // and pending audio cannot prevent video preparation within this horizon.
        if source.audio_format.is_some() {
            let mut frame = self
                .playout
                .next_audio_frame
                .unwrap_or(anchor_frame)
                .max(anchor_frame);
            for _ in 0..self.state.decode_burst_frames {
                if frame >= end_frame {
                    break;
                }
                match self.render_audio_to_buffer(frame) {
                    Ok(()) => (),
                    Err(error) if error.kind == BroadcastEngineErrorKind::NotReady => break,
                    Err(error) => return Err(error),
                }
                frame += 1;
                self.playout.next_audio_frame = Some(frame);
            }
        }
        let events = self.top_up_audio_output_queue()?;
        if source.video_format.is_some() {
            let mut frame = self
                .playout
                .next_video_frame
                .unwrap_or(anchor_frame)
                .max(anchor_frame);
            for _ in 0..self.state.decode_burst_frames {
                if frame >= end_frame {
                    break;
                }
                match self.decode_video_to_buffer(frame) {
                    Ok(()) => (),
                    Err(error) if error.kind == BroadcastEngineErrorKind::NotReady => break,
                    Err(error) => return Err(error),
                }
                frame += 1;
                self.playout.next_video_frame = Some(frame);
            }
        }
        Ok(events)
    }

    fn playout_ready(&self, frame: FrameNumber) -> Result<bool, BroadcastEngineError> {
        let source = self.require_source()?;
        Ok(self.playout.has_ready_frame(source, frame))
    }

    fn healthy_buffer_frames(&self) -> usize {
        self.state
            .decode_burst_frames
            .saturating_mul(4)
            .max(self.min_prebuffer_frames)
    }

    fn position_event(&self) -> BroadcastEvent {
        let source = self.state.source.as_ref();
        BroadcastEvent::CarrierPositionChanged {
            source_id: source.map(|source| source.source_id.clone()),
            frame: self.state.carrier_frame,
            range: self
                .state
                .active_range
                .or_else(|| source.and_then(source_range)),
            timebase: source.map(|source| source.timebase),
            status: self.state.status,
        }
    }

    fn require_source(&self) -> Result<&EngineSourceHandle, BroadcastEngineError> {
        self.state.source.as_ref().ok_or_else(|| {
            BroadcastEngineError::new(BroadcastEngineErrorKind::Contract, "source not loaded")
        })
    }

    fn require_range_inside_source(&self, range: FrameRange) -> Result<(), BroadcastEngineError> {
        FrameRange::new(range.start_frame, range.end_frame).map_err(contract_error)?;
        let source = self.require_source()?;
        if range.end_frame > source.duration_frames {
            return Err(BroadcastEngineError::new(
                BroadcastEngineErrorKind::Contract,
                format!(
                    "range end {} is outside source duration {}",
                    range.end_frame, source.duration_frames
                ),
            ));
        }
        Ok(())
    }

    fn current_range(&self) -> Result<FrameRange, BroadcastEngineError> {
        let source = self.require_source()?;
        self.active_range_or_source(source)
    }

    fn active_range_or_source(
        &self,
        source: &EngineSourceHandle,
    ) -> Result<FrameRange, BroadcastEngineError> {
        self.state
            .active_range
            .or_else(|| source_range(source))
            .ok_or_else(|| {
                BroadcastEngineError::new(
                    BroadcastEngineErrorKind::Contract,
                    "source duration cannot define active range",
                )
            })
    }
}

struct PlayoutBuffer<V, A> {
    video: BTreeMap<FrameNumber, DecodedVideoFrame<V>>,
    audio: BTreeMap<FrameNumber, AudioFramePacket<A>>,
    primed_audio: BTreeSet<FrameNumber>,
    next_video_frame: Option<FrameNumber>,
    next_audio_frame: Option<FrameNumber>,
}

impl<V, A> Default for PlayoutBuffer<V, A> {
    fn default() -> Self {
        Self {
            video: BTreeMap::new(),
            audio: BTreeMap::new(),
            primed_audio: BTreeSet::new(),
            next_video_frame: None,
            next_audio_frame: None,
        }
    }
}

impl<V, A> PlayoutBuffer<V, A> {
    fn clear(&mut self) {
        self.video.clear();
        self.audio.clear();
        self.primed_audio.clear();
        self.next_video_frame = None;
        self.next_audio_frame = None;
    }

    fn reset_for_frame(&mut self, frame: FrameNumber) {
        self.clear();
        self.next_video_frame = Some(frame);
        self.next_audio_frame = Some(frame);
    }

    fn has_ready_frame(&self, source: &EngineSourceHandle, frame: FrameNumber) -> bool {
        source
            .video_format
            .as_ref()
            .is_none_or(|_| self.video.contains_key(&frame))
            && source.audio_format.as_ref().is_none_or(|_| {
                self.audio.contains_key(&frame) || self.primed_audio.contains(&frame)
            })
    }

    fn trim_before(&mut self, frame: FrameNumber) {
        self.video.retain(|candidate, _| *candidate >= frame);
        self.audio.retain(|candidate, _| *candidate >= frame);
        self.primed_audio.retain(|candidate| *candidate >= frame);
    }
}

fn source_range(source: &EngineSourceHandle) -> Option<FrameRange> {
    FrameRange::new(0, source.duration_frames).ok()
}

fn contract_error(message: impl Into<String>) -> BroadcastEngineError {
    BroadcastEngineError::new(BroadcastEngineErrorKind::Contract, message)
}

fn require_matching_handle(
    source: &SourceRuntime,
    revision: Option<u64>,
    handle: &EngineSourceHandle,
) -> Result<(), BroadcastEngineError> {
    if *handle != EngineSourceHandle::from_source_runtime(source, revision) {
        return Err(contract_error(
            "source adapter or preload does not match the supplied metadata revision",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;
    use crate::engine_contract::{AudioFramePacket, DecodedVideoFrame};
    use crate::model::{AudioFormat, ColorSpace, FieldMode, Timebase, VideoFormat};

    mod readiness;

    impl<S, V, O> TransportEngine<S, V, O>
    where
        S: SourceOpenAdapter,
        V: VideoDecodeAdapter,
        O: PlayoutOutput<VideoFrame = V::VideoFrame>,
    {
        fn prepare(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            let mut events = Vec::new();
            for _ in 0..64 {
                if self.state.play_ready {
                    return Ok(events);
                }
                events.extend(self.tick(0)?);
                if self.idle_prebuffer_failed {
                    return Err(contract_error("test preparation failed"));
                }
            }
            Err(contract_error("test preparation never became ready"))
        }
        // Timing/range regressions explicitly perform the earlier idle preparation phase.
        fn play_prepared(
            &mut self,
            now: ClockTick,
        ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            self.prepare()?;
            self.play(now)
        }
    }

    #[test]
    fn single_frame_ticks_prepare_four_av_frames_before_play() {
        let mut engine = fake_engine().with_decode_burst_frames(1);
        engine.load_source(&source_runtime(), None).unwrap();
        for _ in 0..3 {
            engine.tick(0).unwrap();
            assert!(!engine.state().play_ready);
        }
        engine.tick(0).unwrap();
        assert!(engine.state().play_ready);
        assert_eq!(engine.playout.video.len(), 4);
        assert_eq!(engine.playout.audio.len(), 4);
        assert_eq!(engine.state().decode_burst_frames, 1);
        let decoded = (
            engine.playout.next_video_frame,
            engine.playout.next_audio_frame,
        );
        engine.play(0).unwrap();
        assert_eq!(
            (
                engine.playout.next_video_frame,
                engine.playout.next_audio_frame
            ),
            decoded
        );
    }

    #[test]
    fn idle_ticks_do_not_decode_the_entire_source_into_memory() {
        let mut engine = fake_engine();
        let mut source = source_runtime();
        source.duration_frames = 100_000;
        engine.load_source(&source, None).unwrap();
        for tick in 0..1000 {
            engine.tick(tick).unwrap();
        }
        assert!(engine.playout.video.len() <= engine.healthy_buffer_frames() + 1);
        assert!(engine.playout.audio.len() <= engine.healthy_buffer_frames() + 1);
        assert_eq!(engine.state().presented_frame, None);
    }

    #[test]
    fn cue_to_last_frame_plays_its_full_interval_before_end() {
        let mut engine = fake_engine();
        engine.load_source(&source_runtime(), None).unwrap();
        engine.sync_range_runtime(None, 19, true).unwrap();
        engine.play_prepared(0).unwrap();
        engine.tick(0).unwrap();
        assert_eq!(engine.state().presented_frame, Some(19));
        assert!(!engine.state().at_end);
        assert!(engine.tick(19_999_999).unwrap().is_empty());
        let boundary = engine.tick(20_000_000).unwrap();
        assert!(
            boundary
                .iter()
                .any(|e| matches!(e, BroadcastEvent::PlaybackBoundaryReached { frame: 20 }))
        );
        assert!(!engine.state().at_end);
        assert_eq!(engine.state().carrier_frame, 0);
        assert_eq!(engine.state().status, TransportStatus::Preparing);
        assert_eq!(engine.state().presented_frame, Some(19));
        let preview = engine.tick(20_000_001).unwrap();
        assert!(
            preview
                .iter()
                .any(|e| matches!(e, BroadcastEvent::FramePresented { frame: 0 }))
        );
        engine.play_prepared(30_000_000).unwrap();
        assert_eq!(engine.state().presented_frame, Some(0));
        engine.tick(30_000_000).unwrap();
        assert_eq!(engine.state().presented_frame, Some(0));
    }

    #[test]
    fn stale_preload_is_rejected_without_changing_active_state() {
        let mut engine = fake_engine();
        engine
            .load_source(&source_runtime_with_id("active"), Some(1))
            .unwrap();
        let source = source_runtime_with_id("preloaded");
        engine.preload_source(&source, Some(1)).unwrap();
        let before = engine.state().clone();
        assert!(engine.set_active_source(&source, Some(2)).is_err());
        assert_eq!(engine.state(), &before);
    }

    #[test]
    fn source_adapter_cannot_replace_source_timebase_or_duration() {
        let source = source_runtime();
        let mut handle = EngineSourceHandle::from_source_runtime(&source, Some(1));
        handle.timebase.fps_num += 1;
        assert!(require_matching_handle(&source, Some(1), &handle).is_err());
        handle = EngineSourceHandle::from_source_runtime(&source, Some(1));
        handle.duration_frames += 1;
        assert!(require_matching_handle(&source, Some(1), &handle).is_err());
    }

    #[test]
    fn invalid_range_and_rate_do_not_interrupt_playback() {
        let mut engine = fake_engine();
        engine.load_source(&source_runtime(), None).unwrap();
        engine.play_prepared(0).unwrap();
        engine.tick(0).unwrap();
        let before = engine.state().clone();
        for range in [
            FrameRange {
                start_frame: 10,
                end_frame: 10,
            },
            FrameRange {
                start_frame: 19,
                end_frame: 1,
            },
        ] {
            assert!(engine.sync_range_runtime(Some(range), 10, true).is_err());
        }
        assert!(engine.sync_range_runtime(None, 20, true).is_err());
        assert!(engine.apply_request_rate(1, 0).is_err());
        assert!(engine.apply_request_rate(-1, 1).is_err());
        assert_eq!(engine.state(), &before);
        assert!(engine.clock.as_ref().unwrap().is_running());
    }

    #[test]
    fn missing_timebase_does_not_replace_ready_source() {
        let mut engine = fake_engine();
        engine.load_source(&source_runtime(), None).unwrap();
        let before = engine.state().clone();
        let mut bad = source_runtime();
        bad.timebase.fps_den = 0;
        assert!(engine.load_source(&bad, None).is_err());
        assert!(engine.preload_source(&bad, None).is_err());
        assert!(engine.set_active_source(&bad, None).is_err());
        assert_eq!(engine.state(), &before);
    }

    #[test]
    fn one_frame_source_never_decodes_exclusive_end() {
        let mut engine = fake_engine();
        let mut source = source_runtime();
        source.duration_frames = 1;
        engine.load_source(&source, None).unwrap();
        assert_eq!(engine.state().presented_frame, None);
        let mut events = engine.play_prepared(0).unwrap();
        events.extend(engine.tick(0).unwrap());
        assert_eq!(engine.state().status, TransportStatus::Playing);
        events.extend(engine.tick(20_000_000).unwrap());
        assert!(
            events
                .iter()
                .any(|e| matches!(e, BroadcastEvent::FramePresented { frame: 0 }))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, BroadcastEvent::PlaybackBoundaryReached { frame: 1 }))
        );
        assert_eq!(engine.state().presented_frame, Some(0));
        assert_eq!(engine.state().carrier_frame, 0);
        assert_eq!(engine.state().status, TransportStatus::Preparing);
        assert!(engine.sync_range_runtime(None, 1, true).is_err());
    }

    #[test]
    fn independent_engines_do_not_share_playback_state() {
        let mut first = fake_engine();
        let mut second = fake_engine();
        first
            .load_source(&source_runtime_with_id("one"), None)
            .unwrap();
        second
            .load_source(&source_runtime_with_id("two"), None)
            .unwrap();
        let before = second.state().clone();
        first.play_prepared(0).unwrap();
        first.tick(0).unwrap();
        first.tick(40_000_000).unwrap();
        assert_eq!(second.state(), &before);
        assert_eq!(second.state().presented_frame, None);
        assert_eq!(first.state().presented_frame, Some(2));
    }

    #[test]
    fn decode_failure_does_not_confirm_requested_cue() {
        let mut engine = failing_frame_decode_engine();
        engine.load_source(&source_runtime(), None).unwrap();
        let before = engine.state().carrier_frame;
        assert!(engine.sync_range_runtime(None, 10, true).is_err());
        assert_eq!(engine.state().carrier_frame, before);
        assert_eq!(engine.state().presented_frame, None);
    }

    #[test]
    fn failed_or_unconfirmed_output_does_not_advance_position() {
        for fail in [true, false] {
            let mut engine = TransportEngine::new(
                FakeSourceOpen,
                FakeVideoDecode,
                FakeAudioOutput,
                UnconfirmedPresenter { fail },
            );
            engine.load_source(&source_runtime(), None).unwrap();
            assert!(engine.sync_range_runtime(None, 10, true).is_err());
            assert_eq!(engine.state().carrier_frame, 0);
            assert_eq!(engine.state().presented_frame, None);
            assert!(engine.play_prepared(0).is_err());
            assert_eq!(engine.state().carrier_frame, 0);
            assert_eq!(engine.state().presented_frame, None);
            assert_eq!(engine.state().status, TransportStatus::Paused);
        }
    }

    struct UnconfirmedPresenter {
        fail: bool,
    }

    impl FramePresenter for UnconfirmedPresenter {
        type VideoFrame = FrameNumber;
        fn prepare_start_frame(
            &mut self,
            _: &DecodedVideoFrame<FrameNumber>,
        ) -> Result<bool, BroadcastEngineError> {
            Ok(true)
        }
        fn prepare_presentation(
            &mut self,
            _: &EngineSourceHandle,
        ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            Ok(Vec::new())
        }
        fn present_frame(
            &mut self,
            _frame: DecodedVideoFrame<FrameNumber>,
        ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            if self.fail {
                Err(BroadcastEngineError::new(
                    BroadcastEngineErrorKind::VideoPresent,
                    "output unavailable",
                ))
            } else {
                Ok(Vec::new())
            }
        }
    }

    #[test]
    fn play_starts_prepared_audio_and_presents_anchor_without_tick() {
        let mut engine = fake_engine();
        engine.load_source(&source_runtime(), Some(1)).unwrap();
        let events = engine.play_prepared(1_000).unwrap();

        assert_eq!(engine.state().carrier_frame, 0);
        assert_event_frame(&events, 0);
        assert!(events.iter().any(|event| matches!(
            event,
            BroadcastEvent::AudioLevelChanged {
                track_id,
                peak_dbfs_x100: -900
            } if track_id == "monitor"
        )));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, BroadcastEvent::FramePresented { frame: 0 }))
        );
    }

    #[test]
    fn play_rejects_without_work_until_idle_preparation_is_complete() {
        let mut engine = fake_engine();
        let loaded = engine.load_source(&source_runtime(), Some(1)).unwrap();
        assert_eq!(engine.state.status, TransportStatus::Preparing);
        assert!(
            !loaded
                .iter()
                .any(|event| matches!(event, BroadcastEvent::SourceReady { .. }))
        );
        let before = engine.state.clone();
        assert_eq!(
            engine.play(1_000).unwrap_err().kind,
            BroadcastEngineErrorKind::NotReady
        );
        assert_eq!(engine.state, before);
        assert!(engine.clock.is_none());
        assert!(engine.playout.video.is_empty());
        assert!(engine.playout.audio.is_empty());
        let prepared = engine.prepare().unwrap();
        assert!(engine.state.play_ready);
        assert_eq!(engine.state.status, TransportStatus::Ready);
        assert!(
            prepared
                .iter()
                .any(|event| matches!(event, BroadcastEvent::SourceReady { .. }))
        );
        assert!(engine.clock.is_none());
        let frames = engine.playout.video.len();
        let events = engine.play(1_000).unwrap();
        assert_event_frame(&events, 0);
        assert!(engine.clock.is_some());
        assert_eq!(engine.playout.video.len(), frames);
        assert!(!engine.state.play_ready);
        assert!(engine.playout.primed_audio.contains(&1));
        assert!(
            !engine
                .tick(1_000)
                .unwrap()
                .iter()
                .any(|event| matches!(event, BroadcastEvent::FramePresented { frame: 0 }))
        );
    }

    #[test]
    fn pause_preserves_payloads_and_prepares_resume_without_clock() {
        let mut engine = fake_engine();
        engine.load_source(&source_runtime(), Some(1)).unwrap();
        engine.play_prepared(1_000).unwrap();

        engine.pause().unwrap();
        let events = engine.tick(2_000).unwrap();

        assert!(engine.state.play_ready);
        assert!(engine.clock.is_none());
        assert!(engine.playout.video.contains_key(&0));
        assert!(engine.playout.audio.contains_key(&0));
        assert!(events.iter().any(|event| matches!(
            event,
            BroadcastEvent::PlaybackReadinessChanged { ready: true, .. }
        )));
    }

    #[test]
    fn home_play_home_play_keeps_decoder_prepared_and_restarts_at_confirmed_zero() {
        let stop_count = Rc::new(Cell::new(0));
        let mut engine = TransportEngine::new(
            FakeSourceOpen,
            CountingVideoDecode::new(stop_count.clone()),
            FakeAudioOutput,
            FakePresenter,
        );
        engine.load_source(&source_runtime(), Some(1)).unwrap();
        let stop_count_after_load = stop_count.get();

        for tick in 0..4 {
            engine.tick(tick).unwrap();
        }
        engine.play_prepared(10_000).unwrap();
        engine.tick(10_000).unwrap();
        engine.tick(20_010_000).unwrap();
        assert_eq!(engine.state().carrier_frame, 1);

        let home = engine
            .sync_range_runtime(Some(FrameRange::new(0, 20).unwrap()), 0, true)
            .unwrap();
        assert_event_frame(&home, 0);
        assert_eq!(engine.state().carrier_frame, 0);
        assert_eq!(engine.state().status, TransportStatus::Paused);
        assert_eq!(stop_count.get(), stop_count_after_load);

        for tick in 0..4 {
            engine.tick(30_000_000 + tick).unwrap();
        }
        let second_start = engine.play_prepared(40_000_000).unwrap();
        assert_event_frame(&second_start, 0);
        assert_eq!(engine.state().carrier_frame, 0);
        assert_eq!(stop_count.get(), stop_count_after_load);

        engine.stop().unwrap();
        assert_eq!(stop_count.get(), stop_count_after_load);
        engine.unload_source().unwrap();
        assert_eq!(stop_count.get(), stop_count_after_load + 1);
    }

    #[test]
    fn preparation_failure_never_reports_ready_or_starts_play() {
        let mut engine = failing_frame_decode_engine();
        engine.load_source(&source_runtime(), Some(1)).unwrap();
        assert_eq!(
            engine.play(1_000).unwrap_err().kind,
            BroadcastEngineErrorKind::NotReady
        );

        let events = engine.tick(1_000).unwrap();

        assert!(!engine.state.play_ready);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, BroadcastEvent::SourceReady { .. }))
        );
        assert!(engine.tick(2_000).unwrap().is_empty());
        assert_eq!(
            engine.play(2_000).unwrap_err().kind,
            BroadcastEngineErrorKind::NotReady
        );
        assert!(engine.clock.is_none());
        assert_eq!(engine.state().status, TransportStatus::Paused);
        assert!(events.iter().any(|event| matches!(
            event,
            BroadcastEvent::DecodeWarning { message } if message.contains("decode failed")
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            BroadcastEvent::TransportStatusChanged {
                status: TransportStatus::Paused
            }
        )));
    }

    #[test]
    fn delayed_tick_catches_up_without_skipping_frames_after_preroll() {
        let mut engine = fake_engine();
        engine.load_source(&source_runtime(), Some(1)).unwrap();
        engine.play_prepared(0).unwrap();
        engine.tick(0).unwrap();
        engine.tick(0).unwrap();

        let events = engine.tick(100_000_000).unwrap();

        assert_eq!(engine.state().carrier_frame, 4);
        for expected in 1..=4 {
            assert!(events.iter().any(|event| {
                matches!(event, BroadcastEvent::FramePresented { frame } if *frame == expected)
            }));
        }
        assert!(events.iter().all(|event| {
            !matches!(event, BroadcastEvent::FramePresented { frame } if *frame > 4)
        }));
    }

    #[test]
    fn active_playback_refills_incrementally_with_small_decode_burst() {
        let mut engine = fake_engine().with_decode_burst_frames(1);
        engine.load_source(&source_runtime(), Some(1)).unwrap();
        engine.play_prepared(0).unwrap();

        engine.tick(0).unwrap();

        assert_eq!(engine.state().carrier_frame, 0);
        assert!(engine.playout.video.contains_key(&1));
        assert!(engine.playout.primed_audio.contains(&1));
        assert!(engine.playout.video.contains_key(&3));
        assert!(engine.playout.primed_audio.contains(&3));
        assert!(engine.playout.video.contains_key(&4));
        assert!(engine.playout.primed_audio.contains(&4));
        assert!(!engine.playout.video.contains_key(&5));
        assert!(!engine.playout.primed_audio.contains(&5));
    }

    #[test]
    fn video_only_source_does_not_call_audio_output() {
        let mut engine = video_only_engine();
        engine
            .load_source(&video_only_source_runtime(), None)
            .unwrap();
        let events = engine.play_prepared(0).unwrap();

        assert_event_frame(&events, 0);
        assert!(events.iter().any(|event| {
            matches!(event, BroadcastEvent::FramePresented { frame } if *frame == 0)
        }));
        assert!(
            events
                .iter()
                .all(|event| !matches!(event, BroadcastEvent::AudioLevelChanged { .. }))
        );
    }

    #[test]
    fn audio_only_source_does_not_call_video_decode_or_presenter() {
        let mut engine = audio_only_engine();
        engine
            .load_source(&audio_only_source_runtime(), None)
            .unwrap();
        let events = engine.play_prepared(0).unwrap();

        assert_event_frame(&events, 0);
        assert!(events.iter().any(|event| matches!(
            event,
            BroadcastEvent::AudioLevelChanged {
                track_id,
                peak_dbfs_x100: -900
            } if track_id == "monitor"
        )));
        assert!(
            events
                .iter()
                .all(|event| !matches!(event, BroadcastEvent::FramePresented { .. }))
        );
    }

    #[test]
    fn source_without_declared_tracks_is_rejected() {
        let mut engine = fake_engine();
        let source =
            SourceRuntime::new("metadata-only", 20, Timebase::new(50, 1).unwrap()).unwrap();

        let error = engine.load_source(&source, None).unwrap_err();

        assert_eq!(error.kind, BroadcastEngineErrorKind::Contract);
        assert_eq!(error.source_id.as_deref(), Some("metadata-only"));
    }

    #[test]
    fn preload_source_prepares_without_changing_active_source() {
        let mut engine = fake_engine();
        engine.load_source(&source_runtime(), Some(1)).unwrap();
        let next_source = source_runtime_with_id("src-b");

        let events = engine.preload_source(&next_source, Some(2)).unwrap();

        assert!(events.iter().any(|event| matches!(
            event,
            BroadcastEvent::SourcePreloaded { source_id } if source_id == "src-b"
        )));
        assert_eq!(engine.state().source.as_ref().unwrap().source_id, "src");
        assert!(engine.state().preloaded_sources.contains_key("src-b"));
        assert_eq!(
            engine.state().preloaded_sources["src-b"].source_revision,
            Some(2)
        );
    }

    #[test]
    fn set_active_source_uses_preloaded_runtime() {
        let mut engine = fake_engine();
        engine.load_source(&source_runtime(), Some(1)).unwrap();
        let next_source = source_runtime_with_id("src-b");
        engine.preload_source(&next_source, Some(2)).unwrap();

        let mut events = engine.set_active_source(&next_source, Some(2)).unwrap();
        assert_eq!(engine.state.status, TransportStatus::Preparing);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, BroadcastEvent::SourceReady { .. }))
        );
        events.extend(engine.prepare().unwrap());

        assert!(events.iter().any(|event| matches!(
            event,
            BroadcastEvent::SourceReady { source_id } if source_id == "src-b"
        )));
        assert_eq!(engine.state().source.as_ref().unwrap().source_id, "src-b");
        assert_eq!(
            engine.state().source.as_ref().unwrap().source_revision,
            Some(2)
        );
        assert!(engine.state().preloaded_sources.is_empty());
        assert_eq!(engine.state().carrier_frame, 0);
        assert_eq!(engine.state().status, TransportStatus::Ready);
    }

    #[test]
    fn pause_interrupts_clock_until_play_resumes() {
        let mut engine = fake_engine();
        engine.load_source(&source_runtime(), None).unwrap();
        engine.play_prepared(0).unwrap();
        engine.pause().unwrap();

        let events = engine.tick(40_000_000).unwrap();

        assert!(
            !events
                .iter()
                .any(|event| matches!(event, BroadcastEvent::FramePresented { .. }))
        );
        assert!(engine.state.play_ready);
        assert_eq!(engine.state().status, TransportStatus::Paused);
    }

    #[test]
    fn stop_interrupts_clock_without_rewinding_carrier() {
        let mut engine = fake_engine();
        engine.load_source(&source_runtime(), None).unwrap();
        engine.play_prepared(0).unwrap();
        engine.tick(0).unwrap();
        engine.tick(0).unwrap();
        engine.tick(20_000_000).unwrap();

        let events = engine.stop().unwrap();

        assert_eq!(engine.state().carrier_frame, 1);
        assert!(events.iter().any(|event| matches!(
            event,
            BroadcastEvent::TransportStatusChanged {
                status: TransportStatus::Stopped
            }
        )));
        let before = engine.state.carrier_frame;
        engine.prepare().unwrap();
        assert_eq!(engine.state.carrier_frame, before);
        assert!(engine.tick(80_000_000).unwrap().is_empty());
    }

    #[test]
    fn playback_request_sync_cues_requested_start_and_presents_it() {
        let mut engine = fake_engine();
        engine.load_source(&source_runtime(), None).unwrap();

        let events = engine
            .sync_range_runtime(Some(FrameRange::new(10, 20).unwrap()), 10, true)
            .unwrap();

        assert_eq!(engine.state().carrier_frame, 10);
        assert_eq!(engine.state().status, TransportStatus::Paused);
        assert_event_frame(&events, 10);
    }

    #[test]
    fn request_rate_is_applied_when_play_starts() {
        let mut engine = fake_engine();
        engine.load_source(&source_runtime(), None).unwrap();
        engine.apply_request_rate(2, 1).unwrap();
        let first = engine.play_prepared(0).unwrap();
        let next = engine.tick(20_000_000).unwrap();

        assert_event_frame(&first, 0);
        assert_event_frame(&next, 1);
        assert_eq!(engine.state().playback_rate_num, 2);
        assert_eq!(engine.state().playback_rate_den, 1);
    }

    #[test]
    fn playback_boundary_rewinds_after_presenting_last_in_range_frame() {
        let mut engine = fake_engine();
        engine.load_source(&source_runtime(), None).unwrap();
        engine
            .sync_range_runtime(Some(FrameRange::new(10, 20).unwrap()), 18, true)
            .unwrap();
        engine.play_prepared(0).unwrap();

        let mut events = engine.tick(0).unwrap();
        events.extend(engine.tick(20_000_000).unwrap());
        events.extend(engine.tick(40_000_000).unwrap());
        events.extend(engine.tick(60_000_000).unwrap());

        let frame_presented_index = event_index(&events, |event| {
            matches!(event, BroadcastEvent::FramePresented { frame: 19 })
        });
        let at_out_index = event_index(&events, |event| {
            matches!(event, BroadcastEvent::PlaybackBoundaryReached { frame: 20 })
        });
        let preparing_index = event_index(&events, |event| {
            matches!(
                event,
                BroadcastEvent::TransportStatusChanged {
                    status: TransportStatus::Preparing
                }
            )
        });
        assert!(frame_presented_index < at_out_index);
        assert!(at_out_index < preparing_index);
        assert_eq!(engine.state().carrier_frame, 10);
        assert_eq!(engine.state().status, TransportStatus::Preparing);
    }

    #[test]
    fn delayed_tick_still_presents_last_frame_before_boundary_event() {
        let mut engine = fake_engine();
        engine.load_source(&source_runtime(), None).unwrap();
        engine
            .sync_range_runtime(Some(FrameRange::new(10, 20).unwrap()), 18, true)
            .unwrap();
        engine.play_prepared(0).unwrap();

        let mut events = engine.tick(0).unwrap();
        events.extend(engine.tick(10_000_000_000).unwrap());

        let frame_presented_index = event_index(&events, |event| {
            matches!(event, BroadcastEvent::FramePresented { frame: 19 })
        });
        let boundary_index = event_index(&events, |event| {
            matches!(event, BroadcastEvent::PlaybackBoundaryReached { frame: 20 })
        });
        assert!(frame_presented_index < boundary_index);
        assert_eq!(engine.state().carrier_frame, 10);
        assert_eq!(engine.state().status, TransportStatus::Preparing);
        assert!(events.iter().all(|event| !matches!(
            event,
            BroadcastEvent::FramePresented { frame } if *frame >= 20
        )));
    }

    #[test]
    fn load_source_open_failure_keeps_active_engine_state() {
        let mut engine = rejecting_open_engine();
        engine.load_source(&source_runtime(), Some(1)).unwrap();
        engine.prepare().unwrap();

        let err = engine
            .load_source(&source_runtime_with_id("bad"), Some(2))
            .unwrap_err();

        assert_eq!(err.kind, BroadcastEngineErrorKind::SourceOpen);
        assert_eq!(engine.state().source.as_ref().unwrap().source_id, "src");
        assert_eq!(
            engine.state().source.as_ref().unwrap().source_revision,
            Some(1)
        );
        assert_eq!(engine.state().status, TransportStatus::Ready);
        assert_eq!(
            engine.state().active_range.unwrap(),
            FrameRange::new(0, 20).unwrap()
        );
    }

    #[test]
    fn transport_engine_state_serialization_is_neutral() {
        let mut engine = fake_engine();
        engine.load_source(&source_runtime(), Some(7)).unwrap();

        let text = serde_json::to_string(engine.state())
            .unwrap()
            .to_ascii_lowercase();
        let value = serde_json::to_value(engine.state()).unwrap();
        let fields = value.as_object().expect("state object");

        assert!(text.contains("frame"));
        assert_eq!(fields.len(), 12);
        for field in [
            "source",
            "preloaded_sources",
            "carrier_frame",
            "submitted_frame",
            "presented_frame",
            "at_end",
            "status",
            "play_ready",
            "active_range",
            "playback_rate_num",
            "playback_rate_den",
            "decode_burst_frames",
        ] {
            assert!(fields.contains_key(field), "missing field: {field}");
        }
    }

    fn fake_engine() -> TransportEngine<
        FakeSourceOpen,
        FakeVideoDecode,
        SplitAvPlayoutOutput<FakeAudioOutput, FakePresenter>,
    > {
        TransportEngine::new(
            FakeSourceOpen,
            FakeVideoDecode,
            FakeAudioOutput,
            FakePresenter,
        )
    }

    fn video_only_engine() -> TransportEngine<
        FakeSourceOpen,
        FakeVideoDecode,
        SplitAvPlayoutOutput<RejectingAudioOutput, FakePresenter>,
    > {
        TransportEngine::new(
            FakeSourceOpen,
            FakeVideoDecode,
            RejectingAudioOutput,
            FakePresenter,
        )
    }

    fn audio_only_engine() -> TransportEngine<
        FakeSourceOpen,
        RejectingVideoDecode,
        SplitAvPlayoutOutput<FakeAudioOutput, RejectingPresenter>,
    > {
        TransportEngine::new(
            FakeSourceOpen,
            RejectingVideoDecode,
            FakeAudioOutput,
            RejectingPresenter,
        )
    }

    fn rejecting_open_engine() -> TransportEngine<
        RejectBadSourceOpen,
        FakeVideoDecode,
        SplitAvPlayoutOutput<FakeAudioOutput, FakePresenter>,
    > {
        TransportEngine::new(
            RejectBadSourceOpen,
            FakeVideoDecode,
            FakeAudioOutput,
            FakePresenter,
        )
    }

    fn failing_frame_decode_engine() -> TransportEngine<
        FakeSourceOpen,
        FailingFrameVideoDecode,
        SplitAvPlayoutOutput<FakeAudioOutput, FakePresenter>,
    > {
        TransportEngine::new(
            FakeSourceOpen,
            FailingFrameVideoDecode,
            FakeAudioOutput,
            FakePresenter,
        )
    }

    fn source_runtime() -> SourceRuntime {
        source_runtime_with_id("src")
    }

    fn source_runtime_with_id(source_id: &str) -> SourceRuntime {
        SourceRuntime::new(source_id, 20, Timebase::new(50, 1).unwrap())
            .unwrap()
            .with_video_format(
                VideoFormat::new(1920, 1080, FieldMode::Progressive, ColorSpace::Rec709).unwrap(),
            )
            .with_audio_format(AudioFormat::new(48_000, 2).unwrap())
    }

    fn video_only_source_runtime() -> SourceRuntime {
        SourceRuntime::new("video-only", 20, Timebase::new(50, 1).unwrap())
            .unwrap()
            .with_video_format(
                VideoFormat::new(1920, 1080, FieldMode::Progressive, ColorSpace::Rec709).unwrap(),
            )
    }

    fn audio_only_source_runtime() -> SourceRuntime {
        SourceRuntime::new("audio-only", 20, Timebase::new(50, 1).unwrap())
            .unwrap()
            .with_audio_format(AudioFormat::new(48_000, 2).unwrap())
    }

    fn assert_event_frame(events: &[BroadcastEvent], expected_frame: FrameNumber) {
        assert!(events.iter().any(|event| matches!(
            event,
            BroadcastEvent::CarrierPositionChanged { frame, .. } if *frame == expected_frame
        )));
    }

    fn event_index(
        events: &[BroadcastEvent],
        predicate: impl Fn(&BroadcastEvent) -> bool,
    ) -> usize {
        events
            .iter()
            .position(predicate)
            .expect("event should exist")
    }

    struct FakeSourceOpen;

    impl SourceOpenAdapter for FakeSourceOpen {
        fn open_source(
            &mut self,
            source: &SourceRuntime,
            source_revision: Option<u64>,
        ) -> Result<EngineSourceHandle, BroadcastEngineError> {
            Ok(EngineSourceHandle::from_source_runtime(
                source,
                source_revision,
            ))
        }

        fn close_source(&mut self, _source_id: &str) -> Result<(), BroadcastEngineError> {
            Ok(())
        }
    }

    struct RejectBadSourceOpen;

    impl SourceOpenAdapter for RejectBadSourceOpen {
        fn open_source(
            &mut self,
            source: &SourceRuntime,
            source_revision: Option<u64>,
        ) -> Result<EngineSourceHandle, BroadcastEngineError> {
            if source.source_id == "bad" {
                return Err(BroadcastEngineError::new(
                    BroadcastEngineErrorKind::SourceOpen,
                    "rejected",
                )
                .with_source_id(source.source_id.clone()));
            }
            Ok(EngineSourceHandle::from_source_runtime(
                source,
                source_revision,
            ))
        }

        fn close_source(&mut self, _source_id: &str) -> Result<(), BroadcastEngineError> {
            Ok(())
        }
    }

    struct FakeVideoDecode;

    impl VideoDecodeAdapter for FakeVideoDecode {
        type VideoFrame = FrameNumber;

        fn cue_video(&mut self, _: EngineFrameRequest) -> Result<(), BroadcastEngineError> {
            Ok(())
        }

        fn prepare_video(
            &mut self,
            _source: &EngineSourceHandle,
        ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            Ok(Vec::new())
        }

        fn decode_video_frame(
            &mut self,
            request: EngineFrameRequest,
        ) -> Result<DecodedVideoFrame<Self::VideoFrame>, BroadcastEngineError> {
            Ok(DecodedVideoFrame {
                source_id: request.source_id,
                frame: request.frame,
                video_format: None,
                payload: request.frame,
            })
        }
    }

    struct CountingVideoDecode {
        stop_count: Rc<Cell<usize>>,
    }

    impl CountingVideoDecode {
        fn new(stop_count: Rc<Cell<usize>>) -> Self {
            Self { stop_count }
        }
    }

    impl VideoDecodeAdapter for CountingVideoDecode {
        type VideoFrame = FrameNumber;

        fn prepare_video(
            &mut self,
            _source: &EngineSourceHandle,
        ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            Ok(Vec::new())
        }

        fn decode_video_frame(
            &mut self,
            request: EngineFrameRequest,
        ) -> Result<DecodedVideoFrame<Self::VideoFrame>, BroadcastEngineError> {
            Ok(DecodedVideoFrame {
                source_id: request.source_id,
                frame: request.frame,
                video_format: None,
                payload: request.frame,
            })
        }

        fn stop_video(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            self.stop_count.set(self.stop_count.get() + 1);
            Ok(Vec::new())
        }
    }

    struct RejectingVideoDecode;

    impl VideoDecodeAdapter for RejectingVideoDecode {
        type VideoFrame = FrameNumber;

        fn prepare_video(
            &mut self,
            source: &EngineSourceHandle,
        ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            Err(BroadcastEngineError::new(
                BroadcastEngineErrorKind::VideoDecode,
                "video prepare should not be called",
            )
            .with_source_id(source.source_id.clone()))
        }

        fn decode_video_frame(
            &mut self,
            request: EngineFrameRequest,
        ) -> Result<DecodedVideoFrame<Self::VideoFrame>, BroadcastEngineError> {
            Err(BroadcastEngineError::new(
                BroadcastEngineErrorKind::VideoDecode,
                "video decode should not be called",
            )
            .with_source_id(request.source_id)
            .with_frame(request.frame))
        }
    }

    struct FailingFrameVideoDecode;

    impl VideoDecodeAdapter for FailingFrameVideoDecode {
        type VideoFrame = FrameNumber;

        fn prepare_video(
            &mut self,
            _source: &EngineSourceHandle,
        ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            Ok(Vec::new())
        }

        fn decode_video_frame(
            &mut self,
            request: EngineFrameRequest,
        ) -> Result<DecodedVideoFrame<Self::VideoFrame>, BroadcastEngineError> {
            Err(BroadcastEngineError::new(
                BroadcastEngineErrorKind::VideoDecode,
                "decode failed during preroll",
            )
            .with_source_id(request.source_id)
            .with_frame(request.frame))
        }
    }

    struct FakeAudioOutput;

    impl AudioOutputAdapter for FakeAudioOutput {
        type AudioPacket = FrameNumber;
        fn cue_audio(&mut self, _: EngineFrameRequest) -> Result<(), BroadcastEngineError> {
            Ok(())
        }
        fn begin_audio_preroll(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            Ok(Vec::new())
        }
        fn commit_audio_preroll(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            Ok(Vec::new())
        }
        fn start_audio(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            Ok(vec![BroadcastEvent::AudioLevelChanged {
                track_id: "monitor".into(),
                peak_dbfs_x100: -900,
            }])
        }
        fn pause_audio(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            Ok(Vec::new())
        }

        fn prepare_audio(
            &mut self,
            _source: &EngineSourceHandle,
        ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            Ok(Vec::new())
        }

        fn render_audio_for_frame(
            &mut self,
            request: EngineFrameRequest,
        ) -> Result<AudioFramePacket<Self::AudioPacket>, BroadcastEngineError> {
            Ok(AudioFramePacket {
                source_id: request.source_id,
                start_frame: request.frame,
                frame_count: 1,
                audio_format: None,
                payload: request.frame,
            })
        }

        fn submit_audio_packet(
            &mut self,
            _packet: AudioFramePacket<Self::AudioPacket>,
        ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            Ok(vec![BroadcastEvent::AudioLevelChanged {
                track_id: "monitor".to_string(),
                peak_dbfs_x100: -900,
            }])
        }

        fn stop_audio(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            Ok(Vec::new())
        }
    }

    struct RejectingAudioOutput;

    impl AudioOutputAdapter for RejectingAudioOutput {
        type AudioPacket = FrameNumber;
        fn begin_audio_preroll(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            panic!("no audio output expected")
        }
        fn commit_audio_preroll(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            panic!("no audio output expected")
        }
        fn start_audio(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            panic!("no audio output expected")
        }
        fn pause_audio(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            panic!("no audio output expected")
        }

        fn prepare_audio(
            &mut self,
            source: &EngineSourceHandle,
        ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            Err(BroadcastEngineError::new(
                BroadcastEngineErrorKind::AudioOutput,
                "audio prepare should not be called",
            )
            .with_source_id(source.source_id.clone()))
        }

        fn render_audio_for_frame(
            &mut self,
            request: EngineFrameRequest,
        ) -> Result<AudioFramePacket<Self::AudioPacket>, BroadcastEngineError> {
            Err(BroadcastEngineError::new(
                BroadcastEngineErrorKind::AudioOutput,
                "audio render should not be called",
            )
            .with_source_id(request.source_id)
            .with_frame(request.frame))
        }

        fn submit_audio_packet(
            &mut self,
            packet: AudioFramePacket<Self::AudioPacket>,
        ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            Err(BroadcastEngineError::new(
                BroadcastEngineErrorKind::AudioOutput,
                "audio submit should not be called",
            )
            .with_source_id(packet.source_id)
            .with_frame(packet.start_frame))
        }

        fn stop_audio(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            Ok(Vec::new())
        }
    }

    struct FakePresenter;

    impl FramePresenter for FakePresenter {
        type VideoFrame = FrameNumber;
        fn prepare_start_frame(
            &mut self,
            _: &DecodedVideoFrame<FrameNumber>,
        ) -> Result<bool, BroadcastEngineError> {
            Ok(true)
        }
        fn prepare_presentation(
            &mut self,
            _: &EngineSourceHandle,
        ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            Ok(Vec::new())
        }

        fn present_frame(
            &mut self,
            frame: DecodedVideoFrame<Self::VideoFrame>,
        ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            Ok(vec![BroadcastEvent::FramePresented { frame: frame.frame }])
        }
    }

    struct RejectingPresenter;

    impl FramePresenter for RejectingPresenter {
        type VideoFrame = FrameNumber;
        fn prepare_start_frame(
            &mut self,
            _: &DecodedVideoFrame<FrameNumber>,
        ) -> Result<bool, BroadcastEngineError> {
            Err(contract_error(
                "audio-only source must not prepare a video image",
            ))
        }
        fn prepare_presentation(
            &mut self,
            _: &EngineSourceHandle,
        ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            panic!("no video output expected")
        }

        fn present_frame(
            &mut self,
            frame: DecodedVideoFrame<Self::VideoFrame>,
        ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
            Err(BroadcastEngineError::new(
                BroadcastEngineErrorKind::VideoPresent,
                "frame presenter should not be called",
            )
            .with_source_id(frame.source_id)
            .with_frame(frame.frame))
        }
    }
}
