use super::*;
use std::cell::RefCell;

#[derive(Clone, Default)]
struct Trace {
    calls: Rc<RefCell<Vec<&'static str>>>,
    forbid_work: Rc<Cell<bool>>,
    fail_decode: Rc<Cell<bool>>,
    pending_decode: Rc<Cell<bool>>,
    pending_audio: Rc<Cell<bool>>,
}
impl Trace {
    fn work(&self, name: &'static str) {
        assert!(
            !self.forbid_work.get(),
            "unexpected preparation during Play: {name}"
        );
        self.call(name);
    }
    fn call(&self, name: &'static str) {
        self.calls.borrow_mut().push(name);
    }
    fn take(&self) -> Vec<&'static str> {
        std::mem::take(&mut *self.calls.borrow_mut())
    }
}
struct Source(Trace);
impl SourceOpenAdapter for Source {
    fn open_source(
        &mut self,
        source: &SourceRuntime,
        revision: Option<u64>,
    ) -> Result<EngineSourceHandle, BroadcastEngineError> {
        self.0.work("open");
        Ok(EngineSourceHandle::from_source_runtime(source, revision))
    }
    fn close_source(&mut self, _: &str) -> Result<(), BroadcastEngineError> {
        self.0.call("close");
        Ok(())
    }
}
struct Video(Trace);
impl VideoDecodeAdapter for Video {
    type VideoFrame = FrameNumber;
    fn cue_video(&mut self, _: EngineFrameRequest) -> Result<(), BroadcastEngineError> {
        self.0.call("cue_video");
        Ok(())
    }
    fn prepare_video(
        &mut self,
        _: &EngineSourceHandle,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        self.0.work("prepare_video");
        Ok(Vec::new())
    }
    fn decode_video_frame(
        &mut self,
        request: EngineFrameRequest,
    ) -> Result<DecodedVideoFrame<FrameNumber>, BroadcastEngineError> {
        self.0.work("decode");
        if self.0.pending_decode.get() {
            return Err(BroadcastEngineError::new(
                BroadcastEngineErrorKind::NotReady,
                "pending video",
            ));
        }
        if self.0.fail_decode.get() {
            return Err(contract_error("decode failed"));
        }
        Ok(DecodedVideoFrame {
            source_id: request.source_id,
            frame: request.frame,
            video_format: None,
            payload: request.frame,
        })
    }
    fn stop_video(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        self.0.call("stop_video");
        Ok(Vec::new())
    }
}
struct Output {
    trace: Trace,
    opened: bool,
    committed: bool,
    running: bool,
    fail_prepare: bool,
    fail_commit: bool,
    fail_start: bool,
    pending_video: bool,
    fail_video: bool,
    submitted_only: bool,
    wrong_ack: bool,
    fail_append: bool,
    queued: Vec<FrameNumber>,
}
impl PlayoutOutput for Output {
    type VideoFrame = FrameNumber;
    type AudioPacket = FrameNumber;
    fn cue_output(&mut self, _: EngineFrameRequest) -> Result<(), BroadcastEngineError> {
        self.trace.call("cue_audio");
        Ok(())
    }
    fn prepare_start_frame(
        &mut self,
        frame: &DecodedVideoFrame<FrameNumber>,
    ) -> Result<bool, BroadcastEngineError> {
        assert!(self.opened && !self.running);
        assert_eq!(frame.frame, frame.payload);
        self.trace.work("prepare_image");
        if self.fail_video {
            return Err(contract_error("image preparation failed"));
        }
        Ok(!self.pending_video)
    }
    fn append_playout_audio(
        &mut self,
        packet: AudioFramePacket<FrameNumber>,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        assert!(self.opened && self.committed && self.running);
        self.trace.work("append");
        if self.fail_append {
            return Err(contract_error("audio append failed"));
        }
        assert_eq!(self.queued.last().copied().unwrap() + 1, packet.start_frame);
        self.queued.push(packet.start_frame);
        Ok(Vec::new())
    }
    fn prepare_output(
        &mut self,
        _: &EngineSourceHandle,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        self.trace.work("prepare_output");
        self.opened = true;
        if self.fail_prepare {
            return Err(contract_error("output prepare failed"));
        }
        Ok(Vec::new())
    }
    fn render_audio_for_frame(
        &mut self,
        request: EngineFrameRequest,
    ) -> Result<AudioFramePacket<FrameNumber>, BroadcastEngineError> {
        self.trace.work("render_audio");
        if self.trace.pending_audio.get() {
            return Err(BroadcastEngineError::new(
                BroadcastEngineErrorKind::NotReady,
                "pending audio",
            ));
        }
        Ok(AudioFramePacket {
            source_id: request.source_id,
            start_frame: request.frame,
            frame_count: 1,
            audio_format: None,
            payload: request.frame,
        })
    }
    fn submit_playout_frame(
        &mut self,
        frame: PlayoutFrame<FrameNumber, FrameNumber>,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        assert!(self.opened);
        self.trace.call("present");
        if self.pending_video {
            return Err(BroadcastEngineError::new(
                BroadcastEngineErrorKind::NotReady,
                "native output still pending at presentation deadline",
            )
            .with_frame(frame.frame));
        }
        assert!(
            frame.audio.is_none(),
            "prepared audio must not be submitted twice"
        );
        assert_eq!(frame.video.unwrap().frame, frame.frame);
        let frame = frame.frame + u64::from(self.wrong_ack);
        Ok(vec![if self.submitted_only {
            BroadcastEvent::VideoFrameSubmitted { frame }
        } else {
            BroadcastEvent::FramePresented { frame }
        }])
    }
    fn begin_playout_preroll(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        assert!(self.opened);
        assert!(!self.running, "refill must not restart preroll");
        self.trace.work("begin");
        self.queued.clear();
        Ok(Vec::new())
    }
    fn submit_preroll_audio(
        &mut self,
        packet: AudioFramePacket<FrameNumber>,
    ) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        self.trace.work("queue");
        self.queued.push(packet.start_frame);
        Ok(Vec::new())
    }
    fn commit_playout_preroll(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        self.trace.work("commit");
        assert!(!self.running, "refill must not recommit preroll");
        if self.fail_commit {
            return Err(contract_error("output commit failed"));
        }
        self.committed = true;
        Ok(Vec::new())
    }
    fn start_playout(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        assert!(self.opened && self.committed);
        self.trace.call("start");
        if self.fail_start {
            return Err(contract_error("output start failed"));
        }
        self.running = true;
        Ok(Vec::new())
    }
    fn pause_playout(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        self.trace.call("pause");
        self.running = false;
        self.committed = false;
        Ok(Vec::new())
    }
    fn stop_playout(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastEngineError> {
        self.trace.call("stop");
        self.opened = false;
        self.running = false;
        self.committed = false;
        Ok(Vec::new())
    }
}
fn engine() -> (TransportEngine<Source, Video, Output>, Trace) {
    let trace = Trace::default();
    let engine = TransportEngine::with_playout_output(
        Source(trace.clone()),
        Video(trace.clone()),
        Output {
            trace: trace.clone(),
            opened: false,
            committed: false,
            running: false,
            fail_prepare: false,
            fail_commit: false,
            fail_start: false,
            pending_video: false,
            fail_video: false,
            submitted_only: false,
            wrong_ack: false,
            fail_append: false,
            queued: Vec::new(),
        },
    );
    (engine, trace)
}

#[test]
fn staged_cue_keeps_confirmed_position_and_only_latest_request_becomes_ready() {
    let (mut engine, trace) = engine();
    engine.load_source(&source_runtime(), None).unwrap();
    engine.prepare().unwrap();
    engine.play(0).unwrap();
    trace.take();
    engine.cue_frame(5, true).unwrap();
    assert_eq!(engine.state().carrier_frame, 0);
    assert!(!engine.state().play_ready);
    assert!(
        trace
            .take()
            .iter()
            .all(|c| !matches!(*c, "decode" | "render_audio" | "open"))
    );
    trace.pending_decode.set(true);
    engine.tick(1).unwrap();
    engine.cue_frame(12, true).unwrap();
    for _ in 0..3 {
        engine.tick(2).unwrap();
    }
    assert_eq!(engine.state().carrier_frame, 0);
    assert!(engine.play(3).is_err());
    trace.pending_decode.set(false);
    engine.prepare().unwrap();
    assert_eq!(engine.state().carrier_frame, 12);
    assert!(
        engine
            .playout_output
            .queued
            .iter()
            .all(|frame| *frame >= 12)
    );
    assert!(!engine.playout_output.running);
    trace.take();
    trace.forbid_work.set(true);
    engine.play(4).unwrap();
    assert_eq!(trace.take(), ["present", "start"]);
}

#[test]
fn invalid_cue_preserves_playback_and_failed_cue_does_not_confirm_or_retry() {
    let (mut engine, trace) = engine();
    engine.load_source(&source_runtime(), None).unwrap();
    engine.prepare().unwrap();
    engine.play(0).unwrap();
    let before = engine.state().clone();
    assert!(engine.cue_frame(20, true).is_err());
    assert_eq!(engine.state(), &before);
    engine.cue_frame(10, true).unwrap();
    trace.fail_decode.set(true);
    engine.tick(1).unwrap();
    assert_eq!(engine.state().carrier_frame, 0);
    assert!(!engine.state().play_ready);
    assert!(engine.idle_prebuffer_failed);
    trace.take();
    engine.tick(2).unwrap();
    assert!(trace.take().is_empty());
    trace.fail_decode.set(false);
    engine.cue_frame(19, true).unwrap();
    engine.prepare().unwrap();
    assert_eq!(engine.state().carrier_frame, 19);
}

#[test]
fn play_only_presents_and_starts_an_already_committed_output() {
    let (mut engine, trace) = engine();
    engine.load_source(&source_runtime(), None).unwrap();
    engine.prepare().unwrap();
    assert!(!engine.playout_output.running);
    assert!(engine.playout_output.committed);
    let calls = trace.take();
    assert!(
        calls.iter().position(|v| *v == "stop_video")
            < calls.iter().position(|v| *v == "prepare_video")
    );
    trace.forbid_work.set(true);
    let events = engine.play(0).unwrap();
    assert_eq!(trace.take(), ["present", "start"]);
    assert_event_frame(&events, 0);
    assert!(engine.playout_output.running);
    assert!(engine.clock.is_some());
    assert!(engine.play(0).unwrap().is_empty());
    assert!(trace.take().is_empty());
}

#[test]
fn pending_audio_keeps_video_and_does_not_redecode_or_publish_ready() {
    let (mut engine, trace) = engine();
    engine.load_source(&source_runtime(), None).unwrap();
    trace.pending_audio.set(true);
    trace.take();
    for tick in 0..20 {
        assert!(engine.tick(0).unwrap().is_empty());
        assert!(!engine.state().play_ready);
        assert!(!engine.idle_prebuffer_failed);
        assert_eq!(
            engine.playout.video.len(),
            ((tick + 1) * 4).min(engine.healthy_buffer_frames())
        );
        assert!(engine.playout.audio.is_empty());
    }
    let calls = trace.take();
    assert_eq!(
        calls.iter().filter(|call| **call == "decode").count(),
        engine.healthy_buffer_frames()
    );
    assert!(!calls.contains(&"begin"));
    trace.pending_audio.set(false);
    engine.prepare().unwrap();
    trace.take();
    trace.forbid_work.set(true);
    engine.play(0).unwrap();
    assert_eq!(trace.take(), ["present", "start"]);
}

#[test]
fn pending_video_can_be_paused_and_never_claims_ready() {
    let (mut engine, trace) = engine();
    engine.load_source(&source_runtime(), None).unwrap();
    trace.pending_decode.set(true);
    for _ in 0..20 {
        assert!(engine.tick(0).unwrap().is_empty());
    }
    assert!(!engine.state().play_ready);
    assert!(engine.playout.video.is_empty());
    engine.pause().unwrap();
    trace.pending_decode.set(false);
    engine.prepare().unwrap();
    assert!(engine.state().play_ready);
}

#[test]
fn absent_due_packet_stops_without_advancing_to_it() {
    let (mut engine, trace) = engine();
    engine.load_source(&source_runtime(), None).unwrap();
    engine.prepare().unwrap();
    engine.play(0).unwrap();
    trace.pending_decode.set(true);
    let error = (1..100)
        .find_map(|n| engine.tick(n * 40_000_000).err())
        .expect("bounded buffer must run out");
    assert_eq!(error.kind, BroadcastEngineErrorKind::NotReady);
    assert_eq!(engine.state().status, TransportStatus::Paused);
    assert!(!engine.state().play_ready);
    assert!(engine.state().carrier_frame < error.frame.unwrap());
    assert!(!engine.playout_output.running);
}

#[test]
fn every_preparation_tick_is_bounded_and_no_early_play_is_queued() {
    let (mut engine, trace) = engine();
    engine.load_source(&source_runtime(), None).unwrap();
    trace.take();
    for step in 0..4 {
        assert_eq!(
            engine.play(0).unwrap_err().kind,
            BroadcastEngineErrorKind::NotReady
        );
        assert!(trace.take().is_empty());
        let events = engine.tick(step).unwrap();
        let calls = trace.take();
        assert_eq!(calls.iter().filter(|v| **v == "decode").count(), 4);
        assert!(!engine.playout_output.running);
        assert!(engine.clock.is_none());
        assert_eq!(engine.state.play_ready, step == 3);
        assert_eq!(
            events
                .iter()
                .any(|e| matches!(e, BroadcastEvent::SourceReady { .. })),
            step == 3
        );
    }
    trace.forbid_work.set(true);
    for tick in 4..1000 {
        assert!(engine.tick(tick).unwrap().is_empty());
    }
    assert!(trace.take().is_empty());
}

#[test]
fn pause_and_stop_keep_resources_and_rearm_cached_packets_before_resume() {
    let (mut engine, trace) = engine();
    engine.load_source(&source_runtime(), None).unwrap();
    engine.prepare().unwrap();
    engine.play(0).unwrap();
    for stop in [false, true] {
        trace.take();
        trace.forbid_work.set(true);
        if stop {
            engine.stop().unwrap();
        } else {
            engine.pause().unwrap();
        }
        assert_eq!(trace.take(), ["pause"]);
        assert!(engine.playout_output.opened);
        assert_eq!(
            engine.play(0).unwrap_err().kind,
            BroadcastEngineErrorKind::NotReady
        );
        trace.forbid_work.set(false);
        engine.prepare().unwrap();
        let calls = trace.take();
        assert!(
            calls
                .iter()
                .all(|v| matches!(*v, "prepare_image" | "begin" | "queue" | "commit"))
        );
        trace.forbid_work.set(true);
        engine.play(0).unwrap();
        assert_eq!(trace.take(), ["present", "start"]);
    }
}

#[test]
fn seek_and_rate_invalidate_readiness_without_play_fallback() {
    let (mut engine, trace) = engine();
    engine.load_source(&source_runtime(), None).unwrap();
    engine.prepare().unwrap();
    engine
        .sync_range_runtime(Some(FrameRange::new(5, 20).unwrap()), 5, true)
        .unwrap();
    assert!(!engine.state.play_ready);
    trace.take();
    assert_eq!(
        engine.play(0).unwrap_err().kind,
        BroadcastEngineErrorKind::NotReady
    );
    assert!(trace.take().is_empty());
    engine.prepare().unwrap();
    engine.apply_request_rate(2, 1).unwrap();
    assert_eq!(engine.state.status, TransportStatus::Preparing);
    assert!(!engine.state.play_ready);
    assert_eq!(
        engine.play(0).unwrap_err().kind,
        BroadcastEngineErrorKind::NotReady
    );
    engine.prepare().unwrap();
    trace.forbid_work.set(true);
    assert_event_frame(&engine.play(0).unwrap(), 5);
}

#[test]
fn output_commit_or_start_failure_never_creates_a_playing_clock() {
    for fail_commit in [true, false] {
        let (mut engine, trace) = engine();
        engine.load_source(&source_runtime(), None).unwrap();
        if fail_commit {
            engine.playout_output.fail_commit = true;
            assert!(engine.prepare().is_err());
            assert!(!engine.state.play_ready);
        } else {
            engine.prepare().unwrap();
            engine.playout_output.fail_start = true;
            trace.forbid_work.set(true);
        }
        assert!(engine.play(0).is_err());
        assert!(engine.clock.is_none());
        assert!(!engine.playout_output.running);
        assert!(!engine.state.play_ready);
        assert_eq!(engine.state.status, TransportStatus::Paused);
        trace.forbid_work.set(true);
        assert!(engine.tick(0).unwrap().is_empty());
    }
}

#[test]
fn preload_does_not_reconfigure_the_active_prepared_output() {
    let (mut engine, trace) = engine();
    engine.load_source(&source_runtime(), None).unwrap();
    engine.prepare().unwrap();
    trace.take();
    engine
        .preload_source(&source_runtime_with_id("second"), Some(1))
        .unwrap();
    assert_eq!(trace.take(), ["open"]);
    assert!(engine.state.play_ready);
    trace.forbid_work.set(true);
    engine.play(0).unwrap();
    assert_eq!(trace.take(), ["present", "start"]);
}

#[test]
fn failed_output_preparation_closes_the_handle_and_cannot_be_resumed() {
    let (mut engine, trace) = engine();
    engine.playout_output.fail_prepare = true;
    assert!(engine.load_source(&source_runtime(), None).is_err());
    assert_eq!(engine.state.status, TransportStatus::Empty);
    assert!(engine.state.source.is_none());
    assert!(engine.state.active_range.is_none());
    assert!(!engine.state.play_ready);
    assert!(!engine.playout_output.opened);
    assert_eq!(
        trace.take().iter().filter(|call| **call == "close").count(),
        1
    );
    trace.forbid_work.set(true);
    assert!(engine.pause().is_err());
    assert!(engine.play(0).is_err());
    assert!(engine.tick(0).unwrap().is_empty());
    assert!(trace.take().is_empty());
}

#[test]
fn refill_failure_stops_output_and_clock_without_idle_retry() {
    let (mut engine, trace) = engine();
    engine.load_source(&source_runtime(), None).unwrap();
    engine.prepare().unwrap();
    engine.play(0).unwrap();
    trace.fail_decode.set(true);
    assert!(engine.tick(0).is_err());
    assert_eq!(engine.state.status, TransportStatus::Paused);
    assert!(engine.clock.is_none());
    assert!(!engine.playout_output.running);
    assert!(!engine.state.play_ready);
    trace.take();
    trace.forbid_work.set(true);
    assert!(engine.tick(1).unwrap().is_empty());
    assert!(trace.take().is_empty());
}

#[test]
fn same_frame_range_change_does_not_leave_an_unprepared_ready_status() {
    let (mut engine, _) = engine();
    engine.load_source(&source_runtime(), None).unwrap();
    engine.prepare().unwrap();
    engine
        .sync_range_runtime(Some(FrameRange::new(0, 10).unwrap()), 0, false)
        .unwrap();
    assert_eq!(engine.state.status, TransportStatus::Preparing);
    assert!(!engine.state.play_ready);
    assert_eq!(
        engine.play(0).unwrap_err().kind,
        BroadcastEngineErrorKind::NotReady
    );
    engine.prepare().unwrap();
    assert!(engine.playout.video.keys().all(|frame| *frame < 10));
}

#[test]
fn caller_cannot_request_unbounded_initial_decoding() {
    for requested in [0, 1, usize::MAX] {
        let (engine, trace) = engine();
        let mut engine = engine.with_decode_burst_frames(requested);
        engine.load_source(&source_runtime(), None).unwrap();
        trace.take();
        engine.tick(0).unwrap();
        let burst = requested.clamp(1, MAX_DECODE_BURST_FRAMES);
        assert_eq!(
            trace
                .take()
                .iter()
                .filter(|call| **call == "decode")
                .count(),
            burst
        );
        engine.prepare().unwrap();
        assert!(engine.playout.video.len() <= MAX_DECODE_BURST_FRAMES * 4);
        assert!(engine.playout.audio.len() <= MAX_DECODE_BURST_FRAMES * 4);
    }
}

#[test]
fn pending_image_blocks_ready_without_redecode_audio_reset_or_unbounded_cache() {
    let (mut engine, trace) = engine();
    engine.playout_output.pending_video = true;
    engine.load_source(&source_runtime(), None).unwrap();
    for tick in 0..4 {
        engine.tick(tick).unwrap();
    }
    assert!(!engine.state.play_ready);
    assert_eq!(engine.state.status, TransportStatus::Preparing);
    assert!(!engine.playout_output.committed);
    let decoded = engine.playout.video.len();
    trace.take();
    for tick in 4..100 {
        assert!(engine.tick(tick).unwrap().is_empty());
        assert_eq!(
            engine.play(0).unwrap_err().kind,
            BroadcastEngineErrorKind::NotReady
        );
    }
    assert!(trace.take().iter().all(|call| *call == "prepare_image"));
    assert_eq!(engine.playout.video.len(), decoded);
    assert!(engine.playout_output.queued.is_empty());
    engine.playout_output.pending_video = false;
    engine.prepare().unwrap();
    assert!(engine.state.play_ready);
    trace.take();
    trace.forbid_work.set(true);
    engine.play(0).unwrap();
    assert_eq!(trace.take(), ["present", "start"]);
}

#[test]
fn image_failure_never_claims_readiness_or_schedules_idle_retry() {
    let (mut engine, trace) = engine();
    engine.load_source(&source_runtime(), None).unwrap();
    engine.playout_output.fail_video = true;
    assert!(engine.prepare().is_err());
    assert!(!engine.state.play_ready);
    assert!(!engine.playout_output.running);
    assert!(!engine.playout_output.committed);
    trace.take();
    trace.forbid_work.set(true);
    assert!(engine.tick(0).unwrap().is_empty());
    assert_eq!(
        engine.play(0).unwrap_err().kind,
        BroadcastEngineErrorKind::NotReady
    );
    assert!(trace.take().is_empty());
}

#[test]
fn playing_refill_appends_contiguous_audio_without_begin_commit_or_restart() {
    let (mut engine, trace) = engine();
    engine.load_source(&source_runtime(), None).unwrap();
    engine.prepare().unwrap();
    let initial = engine.playout_output.queued.len();
    engine.play(0).unwrap();
    trace.take();
    for tick in 0..4 {
        engine.tick(tick * 20_000_000).unwrap();
    }
    let calls = trace.take();
    assert!(calls.contains(&"append"));
    assert!(
        !calls
            .iter()
            .any(|call| matches!(*call, "begin" | "commit" | "start" | "pause"))
    );
    assert!(engine.playout_output.queued.len() > initial);
    assert!(
        engine
            .playout_output
            .queued
            .windows(2)
            .all(|pair| pair[1] == pair[0] + 1)
    );
    assert!(engine.playout_output.running);
}

#[test]
fn append_failure_stops_playback_without_reset_retry_or_false_ready() {
    let (mut engine, trace) = engine();
    engine.load_source(&source_runtime(), None).unwrap();
    engine.prepare().unwrap();
    engine.play(0).unwrap();
    engine.playout_output.fail_append = true;
    trace.take();
    assert!(engine.tick(0).is_err());
    assert!(!engine.playout_output.running);
    assert!(!engine.state.play_ready);
    assert!(engine.clock.is_none());
    assert!(!trace.take().contains(&"begin"));
    trace.forbid_work.set(true);
    assert!(engine.tick(1).unwrap().is_empty());
    assert!(trace.take().is_empty());
}

#[test]
fn submission_is_not_presentation_and_wrong_frame_is_rejected() {
    for wrong in [false, true] {
        let (mut engine, _) = engine();
        engine.playout_output.submitted_only = true;
        engine.playout_output.wrong_ack = wrong;
        engine.load_source(&source_runtime(), None).unwrap();
        engine.prepare().unwrap();
        let result = engine.play(0);
        assert!(engine.state.presented_frame.is_none());
        if wrong {
            assert!(result.is_err());
            assert!(engine.state.submitted_frame.is_none());
            assert!(!engine.playout_output.running);
        } else {
            let events = result.unwrap();
            assert_eq!(engine.state.submitted_frame, Some(0));
            assert!(events.contains(&BroadcastEvent::VideoFrameSubmitted { frame: 0 }));
            assert!(
                !events
                    .iter()
                    .any(|event| matches!(event, BroadcastEvent::FramePresented { .. }))
            );
            engine.unload_source().unwrap();
            assert!(engine.state.submitted_frame.is_none());
        }
    }
}

#[test]
fn delayed_video_does_not_block_contiguous_audio_refill_but_cannot_miss_due_frame() {
    let (mut engine, trace) = engine();
    let mut source = source_runtime();
    source.duration_frames = 100_000;
    engine.load_source(&source, None).unwrap();
    engine.prepare().unwrap();
    engine.play(0).unwrap();
    let horizon = engine.healthy_buffer_frames() as u64;
    trace.pending_decode.set(true);
    trace.take();
    for frame in 0..horizon {
        engine.tick(u128::from(frame) * 20_000_000).unwrap();
        assert_eq!(engine.state.carrier_frame, frame);
        assert_eq!(engine.playout.next_video_frame, Some(horizon));
        assert_eq!(
            engine.playout.next_audio_frame,
            Some(frame.saturating_sub(1) + horizon + 1)
        );
        assert!(engine.playout.audio.len() <= horizon as usize + 2);
        assert!(engine.playout.video.len() <= horizon as usize);
    }
    assert!(engine.playout_output.queued.last().unwrap() > &horizon);
    assert!(trace.take().contains(&"append"));
    assert!(
        engine
            .playout_output
            .queued
            .windows(2)
            .all(|p| p[1] == p[0] + 1)
    );
    let error = engine.tick(u128::from(horizon) * 20_000_000).unwrap_err();
    assert_eq!(error.kind, BroadcastEngineErrorKind::NotReady);
    assert_eq!(error.frame, Some(horizon));
    assert_eq!(engine.state.carrier_frame, horizon - 1);
    assert!(engine.clock.is_none());
    assert!(!engine.playout_output.running);
    trace.forbid_work.set(true);
    engine.tick(1_000_000_000).unwrap();
}

#[test]
fn either_pending_lane_is_bounded_even_when_paused_for_a_long_time() {
    for video_pending in [true, false] {
        let (mut engine, trace) = engine();
        let mut source = source_runtime();
        source.duration_frames = 100_000;
        engine.load_source(&source, None).unwrap();
        engine.pause().unwrap();
        trace.pending_decode.set(video_pending);
        trace.pending_audio.set(!video_pending);
        let horizon = engine.healthy_buffer_frames();
        for _ in 0..1000 {
            trace.take();
            engine.tick(0).unwrap();
            let calls = trace.take();
            assert!(calls.iter().filter(|c| **c == "decode").count() <= 4);
            assert!(calls.iter().filter(|c| **c == "render_audio").count() <= 4);
            assert!(engine.playout.video.len() <= horizon);
            assert!(engine.playout.audio.len() <= horizon);
            assert!(!engine.state.play_ready);
            assert!(engine.clock.is_none());
        }
        assert_eq!(
            engine.playout.video.len(),
            if video_pending { 0 } else { horizon }
        );
        assert_eq!(
            engine.playout.audio.len(),
            if video_pending { horizon } else { 0 }
        );
        assert!(engine.playout_output.queued.is_empty());
    }
}

#[test]
fn pending_audio_does_not_block_video_and_does_not_invent_missing_samples() {
    let (mut engine, trace) = engine();
    let mut source = source_runtime();
    source.duration_frames = 100_000;
    engine.load_source(&source, None).unwrap();
    engine.prepare().unwrap();
    engine.play(0).unwrap();
    trace.pending_audio.set(true);
    let horizon = engine.healthy_buffer_frames() as u64;
    for frame in 0..horizon {
        engine.tick(u128::from(frame) * 20_000_000).unwrap();
        assert_eq!(engine.playout.next_audio_frame, Some(horizon));
        assert_eq!(engine.playout_output.queued.len(), horizon as usize);
        assert!(engine.playout.video.len() <= horizon as usize + 2);
    }
    assert!(engine.playout.next_video_frame.unwrap() > horizon);
    let error = engine.tick(u128::from(horizon) * 20_000_000).unwrap_err();
    assert_eq!(error.frame, Some(horizon));
    assert_eq!(engine.state.carrier_frame, horizon - 1);
    assert!(!engine.playout_output.running);
}

#[test]
fn seek_and_source_change_reset_both_independent_preparation_cursors() {
    let (mut engine, trace) = engine();
    let mut source = source_runtime();
    source.duration_frames = 100_000;
    engine.load_source(&source, None).unwrap();
    trace.pending_decode.set(true);
    for _ in 0..4 {
        engine.tick(0).unwrap();
    }
    assert_eq!(engine.playout.next_video_frame, Some(0));
    assert_eq!(engine.playout.next_audio_frame, Some(16));
    engine.cue_frame(70, false).unwrap();
    assert_eq!(engine.playout.next_video_frame, Some(70));
    assert_eq!(engine.playout.next_audio_frame, Some(70));
    assert!(engine.playout.audio.is_empty());
    trace.pending_decode.set(false);
    engine.prepare().unwrap();
    assert_eq!(engine.playout_output.queued, (70..86).collect::<Vec<_>>());
    engine.play(0).unwrap();
    source.source_id = "next-source".into();
    engine.load_source(&source, None).unwrap();
    assert!(!engine.playout_output.running);
    assert!(engine.clock.is_none());
    assert_eq!(engine.playout.next_video_frame, Some(0));
    assert_eq!(engine.playout.next_audio_frame, Some(0));
    assert!(engine.playout.video.is_empty());
    assert!(engine.playout.audio.is_empty());
    assert!(engine.playout.primed_audio.is_empty());
}

#[test]
fn pending_native_video_does_not_suspend_preparation_or_hide_a_missed_output() {
    let (mut engine, trace) = engine();
    engine.load_source(&source_runtime(), None).unwrap();
    engine.prepare().unwrap();
    engine.play(0).unwrap();
    engine.playout_output.pending_video = true;
    trace.take();
    engine.tick(0).unwrap();
    assert!(trace.take().contains(&"append"));
    assert!(engine.playout_output.running);
    let error = engine.tick(20_000_000).unwrap_err();
    assert_eq!(error.frame, Some(1));
    assert!(!engine.playout_output.running);
    assert_eq!(engine.state.carrier_frame, 0);
    assert!(!engine.state.play_ready);
}

#[test]
fn replay_outside_retained_window_cues_both_adapters_before_preparation() {
    let (mut engine, trace) = engine();
    engine.load_source(&source_runtime(), None).unwrap();
    engine.cue_frame(19, true).unwrap();
    engine.prepare().unwrap();
    engine.play(0).unwrap();
    trace.take();
    engine.tick(20_000_000).unwrap();
    assert!(!engine.state.at_end);
    assert_eq!(engine.state.carrier_frame, 0);
    assert_eq!(engine.state.status, TransportStatus::Preparing);
    let boundary_calls = trace.take();
    assert_eq!(&boundary_calls[..3], &["pause", "cue_video", "cue_audio"]);
    engine.prepare().unwrap();
    let calls = trace.take();
    assert_eq!(calls.iter().filter(|c| **c == "cue_video").count(), 0);
    assert_eq!(calls.iter().filter(|c| **c == "cue_audio").count(), 0);
    trace.forbid_work.set(true);
    engine.play(30_000_000).unwrap();
    assert_eq!(engine.state.carrier_frame, 0);
    assert_eq!(trace.take(), ["present", "start"]);
}
