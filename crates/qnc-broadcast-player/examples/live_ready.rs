//! Explicit synthetic device diagnostic. Not a worker, application UI or media fallback.
use qnc_audio_output::{AudioOutput, Config, Format};
use qnc_broadcast_player::*;
use qnc_video_output::{FrameHeader, OutputConfig, PixelFormat, PreparedFrame, VideoOutput};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const FRAMES: u64 = 100;
const SAMPLES: usize = 960;
type E = BroadcastEngineError;
type R<T> = Result<T, E>;
type Picture = Rc<PreparedFrame>;
type Engine = TransportEngine<Source, Video, SplitAvPlayoutOutput<Audio, Presenter>>;
fn error(e: impl std::fmt::Display) -> E {
    E::new(BroadcastEngineErrorKind::Contract, e.to_string())
}

#[derive(Clone, Default)]
struct Guard(Rc<Cell<bool>>);
impl Guard {
    fn cold(&self) {
        assert!(!self.0.get(), "cold operation on ready Play");
    }
}
struct Source(Guard);
impl SourceOpenAdapter for Source {
    fn open_source(
        &mut self,
        source: &SourceRuntime,
        revision: Option<u64>,
    ) -> R<EngineSourceHandle> {
        self.0.cold();
        Ok(EngineSourceHandle::from_source_runtime(source, revision))
    }
    fn close_source(&mut self, _: &str) -> R<()> {
        self.0.cold();
        Ok(())
    }
}

struct Gpu {
    output: VideoOutput,
    images: BTreeMap<u64, Picture>,
    sequence: u64,
    submit_us: u128,
    inflight: bool,
}
impl Gpu {
    fn poll_and_collect(&mut self) -> R<()> {
        if self.output.poll().map_err(error)?.is_some() {
            self.inflight = false;
        }
        if self.inflight {
            return Ok(());
        }
        let expired: Vec<_> = self
            .images
            .iter()
            .filter(|(_, token)| Rc::strong_count(token) == 1)
            .map(|(frame, _)| *frame)
            .collect();
        for frame in expired {
            let token = self.images.get(&frame).unwrap().clone();
            self.output.release(&token).map_err(error)?;
            self.images.remove(&frame);
        }
        Ok(())
    }
}
type SharedGpu = Rc<RefCell<Gpu>>;
struct Video(SharedGpu, Guard);
impl VideoDecodeAdapter for Video {
    type VideoFrame = Picture;
    fn prepare_video(&mut self, _: &EngineSourceHandle) -> R<Vec<BroadcastEvent>> {
        self.1.cold();
        Ok(Vec::new())
    }
    fn decode_video_frame(&mut self, request: EngineFrameRequest) -> R<DecodedVideoFrame<Picture>> {
        self.1.cold();
        let mut gpu = self.0.borrow_mut();
        let mut pixels = vec![0; (WIDTH * HEIGHT * 4) as usize];
        for (i, p) in pixels.chunks_exact_mut(4).enumerate() {
            let x = i as u32 % WIDTH;
            let y = i as u32 / WIDTH;
            let colors = [
                [220, 30, 40, 255],
                [20, 180, 70, 255],
                [20, 80, 220, 255],
                [220, 200, 30, 255],
            ];
            p.copy_from_slice(
                &colors[(usize::from(y >= HEIGHT / 2) * 2
                    + usize::from(x >= WIDTH / 2)
                    + request.frame as usize / 10)
                    % 4],
            );
            if x.abs_diff((request.frame as u32 * 12) % WIDTH) < 5 {
                p.fill(255);
            }
        }
        let sequence = gpu.sequence;
        gpu.sequence += 1;
        let token = gpu
            .output
            .prepare(
                FrameHeader {
                    version: qnc_video_output::VERSION.into(),
                    session_id: "core-diagnostic".into(),
                    generation: 1,
                    sequence,
                    source_id: request.source_id.clone(),
                    frame_number: request.frame,
                    width: WIDTH,
                    height: HEIGHT,
                    pixel_format: PixelFormat::Rgba8Srgb,
                },
                &pixels,
            )
            .map_err(error)?;
        let token = Rc::new(token);
        gpu.images.insert(request.frame, token.clone());
        Ok(DecodedVideoFrame {
            source_id: request.source_id,
            frame: request.frame,
            video_format: None,
            payload: token,
        })
    }
}
struct Presenter(SharedGpu, Guard);
impl FramePresenter for Presenter {
    type VideoFrame = Picture;
    fn prepare_presentation(&mut self, _: &EngineSourceHandle) -> R<Vec<BroadcastEvent>> {
        self.1.cold();
        Ok(Vec::new())
    }
    fn prepare_start_frame(&mut self, frame: &DecodedVideoFrame<Picture>) -> R<bool> {
        self.1.cold();
        self.0
            .borrow()
            .output
            .is_ready(&frame.payload)
            .map_err(error)
    }
    fn present_frame(&mut self, frame: DecodedVideoFrame<Picture>) -> R<Vec<BroadcastEvent>> {
        let mut gpu = self.0.borrow_mut();
        let begin = Instant::now();
        let submission = gpu.output.submit(&frame.payload).map_err(error)?;
        gpu.inflight = true;
        gpu.submit_us = begin.elapsed().as_micros();
        assert_eq!(
            submission.target,
            qnc_video_output::SubmissionTarget::Surface
        );
        assert_eq!(submission.frame.frame_number, frame.frame);
        Ok(vec![BroadcastEvent::VideoFrameSubmitted {
            frame: frame.frame,
        }])
    }
}

struct Audio {
    device: Rc<RefCell<AudioOutput>>,
    guard: Guard,
    generation: Option<u64>,
    append_generation: Rc<Cell<u64>>,
}
impl AudioOutputAdapter for Audio {
    type AudioPacket = Arc<[f32]>;
    fn prepare_audio(&mut self, _: &EngineSourceHandle) -> R<Vec<BroadcastEvent>> {
        self.guard.cold();
        Ok(Vec::new())
    }
    fn render_audio_for_frame(
        &mut self,
        request: EngineFrameRequest,
    ) -> R<AudioFramePacket<Arc<[f32]>>> {
        self.guard.cold();
        let mut samples = Vec::with_capacity(SAMPLES * 2);
        for i in 0..SAMPLES {
            let phase =
                (request.frame as usize * SAMPLES + i) as f32 * 440.0 * std::f32::consts::TAU
                    / 48_000.0;
            samples.extend([phase.sin() * 0.005; 2]);
        }
        Ok(AudioFramePacket {
            source_id: request.source_id,
            start_frame: request.frame,
            frame_count: 1,
            audio_format: None,
            payload: samples.into(),
        })
    }
    fn submit_audio_packet(
        &mut self,
        packet: AudioFramePacket<Arc<[f32]>>,
    ) -> R<Vec<BroadcastEvent>> {
        self.guard.cold();
        let mut output = self.device.borrow_mut();
        let generation = match self.generation {
            Some(value) => value,
            None => {
                let value = output
                    .begin(packet.start_frame * SAMPLES as u64)
                    .map_err(error)?;
                self.generation = Some(value);
                self.append_generation.set(value);
                value
            }
        };
        assert_eq!(generation, self.append_generation.get());
        output
            .queue(
                generation,
                packet.start_frame * SAMPLES as u64,
                &packet.payload,
            )
            .map_err(error)?;
        if packet.start_frame + 1 == FRAMES {
            output.finish(generation).map_err(error)?;
        }
        Ok(Vec::new())
    }
    fn begin_audio_preroll(&mut self) -> R<Vec<BroadcastEvent>> {
        self.guard.cold();
        assert_ne!(
            self.device.borrow().telemetry().status,
            qnc_audio_output::Status::Playing
        );
        self.generation = None;
        Ok(Vec::new())
    }
    fn commit_audio_preroll(&mut self) -> R<Vec<BroadcastEvent>> {
        self.guard.cold();
        self.device
            .borrow_mut()
            .commit(self.generation.ok_or_else(|| error("no queued audio"))?)
            .map_err(error)?;
        Ok(Vec::new())
    }
    fn start_audio(&mut self) -> R<Vec<BroadcastEvent>> {
        self.device
            .borrow_mut()
            .start(self.generation.ok_or_else(|| error("not prepared"))?)
            .map_err(error)?;
        Ok(Vec::new())
    }
    fn pause_audio(&mut self) -> R<Vec<BroadcastEvent>> {
        self.device.borrow_mut().pause().map_err(error)?;
        self.generation = None;
        Ok(Vec::new())
    }
    fn stop_audio(&mut self) -> R<Vec<BroadcastEvent>> {
        self.pause_audio()
    }
}

struct Diagnostic {
    window: Option<Arc<Window>>,
    engine: Option<Engine>,
    gpu: Option<SharedGpu>,
    audio: Option<Rc<RefCell<AudioOutput>>>,
    guard: Guard,
    epoch: Instant,
    ready_at: Option<Instant>,
    played_at: Option<Instant>,
    stage: u8,
    reported_callback: bool,
    completed: bool,
}
impl Diagnostic {
    fn play(&mut self) {
        self.guard.0.set(true);
        let begin = Instant::now();
        let events = self
            .engine
            .as_mut()
            .unwrap()
            .play(self.epoch.elapsed().as_nanos())
            .unwrap();
        let elapsed = begin.elapsed();
        self.guard.0.set(false);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, BroadcastEvent::FramePresented { .. }))
        );
        assert!(
            self.engine
                .as_ref()
                .unwrap()
                .state()
                .presented_frame
                .is_none()
        );
        println!(
            "Ready Play {}: command {:?}, surface submit {} us, no cold calls, no physical presentation claim",
            self.stage,
            elapsed,
            self.gpu.as_ref().unwrap().borrow().submit_us
        );
        self.played_at = Some(Instant::now());
        self.reported_callback = false;
    }
}
impl ApplicationHandler for Diagnostic {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("QNC Core Ready Diagnostic")
                        .with_resizable(false)
                        .with_inner_size(winit::dpi::PhysicalSize::new(960, 640)),
                )
                .unwrap(),
        );
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .unwrap();
        let size = window.inner_size();
        let output = pollster::block_on(VideoOutput::open(
            &adapter,
            Some(surface),
            OutputConfig {
                version: qnc_video_output::VERSION.into(),
                session_id: "core-diagnostic".into(),
                generation: 1,
                width: WIDTH,
                height: HEIGHT,
                pixel_format: PixelFormat::Rgba8Srgb,
                slots: 8,
                pool_budget_bytes: u64::from(WIDTH) * u64::from(HEIGHT) * 4 * 8,
            },
            (size.width, size.height),
        ))
        .unwrap();
        let gpu = Rc::new(RefCell::new(Gpu {
            output,
            images: BTreeMap::new(),
            sequence: 0,
            submit_us: 0,
            inflight: false,
        }));
        let audio = Rc::new(RefCell::new(
            AudioOutput::open(Config {
                version: qnc_audio_output::VERSION.into(),
                format: Format {
                    sample_rate_hz: 48_000,
                    channels: 2,
                },
                device_id: None,
                capacity_frames: 48_000,
                ready_frames: (SAMPLES * 4) as u32,
            })
            .unwrap(),
        ));
        let mut engine = TransportEngine::new(
            Source(self.guard.clone()),
            Video(gpu.clone(), self.guard.clone()),
            Audio {
                device: audio.clone(),
                guard: self.guard.clone(),
                generation: None,
                append_generation: Rc::new(Cell::new(0)),
            },
            Presenter(gpu.clone(), self.guard.clone()),
        )
        .with_decode_burst_frames(1);
        let source = SourceRuntime::new(
            "synthetic-device-fixture",
            FRAMES,
            Timebase::new(50, 1).unwrap(),
        )
        .unwrap()
        .with_video_format(
            VideoFormat::new(WIDTH, HEIGHT, FieldMode::Progressive, ColorSpace::Srgb).unwrap(),
        )
        .with_audio_format(AudioFormat::new(48_000, 2).unwrap());
        engine.load_source(&source, None).unwrap();
        self.engine = Some(engine);
        self.gpu = Some(gpu);
        self.audio = Some(audio);
        self.window = Some(window);
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit();
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.epoch.elapsed() > Duration::from_secs(40) {
            assert!(
                self.completed,
                "diagnostic did not finish pause/resume playback"
            );
            event_loop.exit();
            return;
        }
        let Some(gpu) = &self.gpu else {
            return;
        };
        gpu.borrow_mut().poll_and_collect().unwrap();
        if gpu.borrow().inflight {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(1),
            ));
            return;
        }
        if !self.completed {
            self.engine
                .as_mut()
                .unwrap()
                .tick(self.epoch.elapsed().as_nanos())
                .unwrap();
            let state = self.engine.as_ref().unwrap().state();
            if state.at_end {
                assert_eq!(state.submitted_frame, Some(FRAMES - 1));
                assert!(state.presented_frame.is_none());
                self.completed = true;
                println!(
                    "Synthetic range ended: last surface frame {}, pause/resume completed; no DB/source access",
                    FRAMES - 1
                );
            } else if self.stage == 0 && state.play_ready {
                let ready_at = self.ready_at.get_or_insert_with(|| {
                    println!(
                        "Ready after {:?}; device queue silent",
                        self.epoch.elapsed()
                    );
                    Instant::now()
                });
                assert_eq!(
                    self.audio
                        .as_ref()
                        .unwrap()
                        .borrow()
                        .telemetry()
                        .submitted_frames,
                    0
                );
                if ready_at.elapsed() >= Duration::from_secs(6) {
                    self.play();
                    self.stage = 1;
                }
            } else if self.stage == 1
                && self.played_at.unwrap().elapsed() >= Duration::from_millis(120)
            {
                self.engine.as_mut().unwrap().pause().unwrap();
                self.ready_at = Some(Instant::now());
                self.stage = 2;
                println!("Paused; retained output devices");
            } else if self.stage == 2
                && state.play_ready
                && self.ready_at.unwrap().elapsed() >= Duration::from_secs(2)
            {
                self.play();
                self.stage = 3;
            }
            if matches!(self.stage, 1 | 3) && !self.reported_callback {
                let telemetry = self.audio.as_ref().unwrap().borrow().telemetry();
                if let Some(ns) = telemetry.start_to_first_callback_ns {
                    println!(
                        "Audio first callback {} ns, driver delay {:?} ns (not acoustic evidence)",
                        ns, telemetry.first_driver_delay_ns
                    );
                    self.reported_callback = true;
                }
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(1),
        ));
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Opt-in synthetic output test: quiet 440 Hz tone, real output, no media/DB writes.");
    EventLoop::new()?.run_app(&mut Diagnostic {
        window: None,
        engine: None,
        gpu: None,
        audio: None,
        guard: Guard::default(),
        epoch: Instant::now(),
        ready_at: None,
        played_at: None,
        stage: 0,
        reported_callback: false,
        completed: false,
    })?;
    println!("Native core diagnostic closed.");
    Ok(())
}
