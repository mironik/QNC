//! Explicit read-only real-media integration. Window hosts pixels only; one worker owns player.
use qnc_broadcast_engine::{InputPlan, Runtime};
use qnc_media_stream::{LocalSource, MediaStream, SourceReference};
use qnc_player_input::InputReader;
use qnc_work_settings::SettingsReader;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Seek},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[path = "support/seek_check.rs"]
mod seek_check;

fn hash(stream: &mut MediaStream) -> Result<String> {
    stream.rewind()?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let n = stream.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn exercise(
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    size: (u32, u32),
    args: Vec<std::ffi::OsString>,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    let reader = SettingsReader::from_root(&PathBuf::from(&args[0]))?;
    let settings = reader.read()?;
    let clip_id = args[2].to_str().ok_or("invalid clip id")?;
    let input = InputReader::new(reader.clone()).load(&settings.workspace_db_uri, clip_id)?;
    let plan = InputPlan::new(&input, &settings.workspace_db_uri, clip_id)?;
    println!(
        "Native audio inventory: {:?}; project source routing: {:?}",
        input.layout.audio_channels,
        plan.audio_channels()
    );
    let frames = plan.source().duration_frames;
    let reference = SourceReference::from_uri(&input.media()?.media_uri)?;
    let source = LocalSource::new(reference.source_uri(), PathBuf::from(&args[1]))?;
    let mut verify = MediaStream::local(&source, &input.media()?.media_uri)?;
    let before = hash(&mut verify)?;
    println!(
        "Saved project {}, representation {:?}, {} frames; read-only source",
        settings.project_name, input.representation, frames
    );
    let prepared_at = Instant::now();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: Some(&surface),
        ..Default::default()
    }))
    .ok_or("no native GPU adapter")?;
    let config = plan.output_config("saved-media-diagnostic", 1)?;
    let output = pollster::block_on(qnc_video_output::VideoOutput::open(
        &adapter,
        Some(surface),
        config.clone(),
        size,
    ))?;
    let mut runtime = Runtime::open(
        plan,
        output,
        config,
        qnc_media_decode::DecoderConfig::new(qnc_ffmpeg_decode::FfmpegAdapter::new("ffmpeg")),
        None,
        move |uri| MediaStream::local(&source, uri),
    )?;
    if args.get(3).is_some_and(|arg| arg == "seek") {
        seek_check::exercise(&mut runtime, frames, &stop)?;
        drop(runtime);
        if hash(&mut verify)? != before || reader.read()? != settings {
            return Err("source or saved settings changed".into());
        }
        println!(
            "PASS: native seek/play/end/replay; source/settings unchanged; no DB writes or probe. Not physical A/V sync or network certification."
        );
        return Ok(());
    }
    let mut phase = 0;
    let mut phase_at = Instant::now();
    let mut reported_ready = false;
    let mut callback_reported = false;
    let mut ended = false;
    let mut last = 0;
    let mut changes = 0;
    let mut max_tick_us = 0;
    let mut previous_tick = Instant::now();
    let mut max_gap_us = 0;
    while prepared_at.elapsed() < Duration::from_secs(60) && !stop.load(Ordering::Acquire) {
        {
            let before_audio = runtime.audio_telemetry();
            let before_frame = runtime.state().carrier_frame;
            let now = Instant::now();
            max_gap_us = max_gap_us.max(now.duration_since(previous_tick).as_micros());
            previous_tick = now;
            let result = runtime.tick();
            max_tick_us = max_tick_us.max(now.elapsed().as_micros());
            if let Err(error) = result {
                eprintln!(
                    "Tick failure at {before_frame}; max tick {max_tick_us} us, max gap {max_gap_us} us; before audio {before_audio:?}"
                );
                return Err(error.into());
            }
        }
        if runtime.state().carrier_frame != last {
            changes += 1;
            last = runtime.state().carrier_frame;
        }
        if runtime.state().presented_frame.is_some() {
            return Err("submission relabeled as physical presentation".into());
        }
        if !reported_ready && runtime.state().play_ready {
            println!(
                "Ready after {} ms; initial AV buffered, no new probe",
                prepared_at.elapsed().as_millis()
            );
            reported_ready = true;
            phase_at = Instant::now();
        }
        if ((phase == 0 && reported_ready) || phase == 2)
            && runtime.state().play_ready
            && phase_at.elapsed() > Duration::from_secs(2)
        {
            let start = Instant::now();
            runtime.play()?;
            println!(
                "Play phase {}: {} us; surface submission {} us",
                phase,
                start.elapsed().as_micros(),
                runtime.last_submission_us()
            );
            phase += 1;
            phase_at = Instant::now();
            callback_reported = false;
        } else if phase == 1 && phase_at.elapsed() > Duration::from_secs(1) {
            runtime.pause()?;
            println!(
                "Pause at source frame {}: decoder/devices retained",
                runtime.state().carrier_frame
            );
            phase = 2;
            phase_at = Instant::now();
        }
        if matches!(phase, 1 | 3)
            && !callback_reported
            && let Some(telemetry) = runtime.audio_telemetry()
            && let Some(ns) = telemetry.start_to_first_callback_ns
        {
            println!(
                "Audio first callback {ns} ns; driver delay {:?} ns",
                telemetry.first_driver_delay_ns
            );
            callback_reported = true;
        }
        if !ended && runtime.state().at_end {
            if phase != 3 || runtime.state().submitted_frame != Some(frames - 1) || changes < 10 {
                return Err("range ended without verified pause/resume and final frame".into());
            }
            println!(
                "Exclusive end reached; last submitted frame {}; {} position changes",
                frames - 1,
                changes
            );
            println!("Maximum tick {max_tick_us} us; maximum scheduling gap {max_gap_us} us");
            ended = true;
            phase_at = Instant::now();
        }
        if ended && phase_at.elapsed() > Duration::from_secs(15) {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    drop(runtime);
    if hash(&mut verify)? != before || reader.read()? != settings {
        return Err("source or saved settings changed".into());
    }
    if stop.load(Ordering::Acquire) {
        return Err("diagnostic closed before completion".into());
    }
    if !ended {
        return Err("diagnostic deadline reached".into());
    }
    println!(
        "PASS: actual AV, Play/pause/resume/end, source/settings unchanged, no DB writes. Not a physical A/V sync or network certification."
    );
    Ok(())
}
struct Diagnostic {
    args: Vec<std::ffi::OsString>,
    window: Option<Arc<Window>>,
    worker: Option<JoinHandle<std::result::Result<(), String>>>,
    stop: Arc<AtomicBool>,
    result: Option<std::result::Result<(), String>>,
}
impl ApplicationHandler for Diagnostic {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = match event_loop.create_window(
            Window::default_attributes()
                .with_title("QNC Saved Media Playback")
                .with_resizable(false)
                .with_inner_size(winit::dpi::PhysicalSize::new(960, 640)),
        ) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.result = Some(Err(error.to_string()));
                event_loop.exit();
                return;
            }
        };
        let instance = wgpu::Instance::default();
        let surface = match instance.create_surface(window.clone()) {
            Ok(surface) => surface,
            Err(error) => {
                self.result = Some(Err(error.to_string()));
                event_loop.exit();
                return;
            }
        };
        let size = window.inner_size();
        let args = self.args.clone();
        let stop = self.stop.clone();
        self.worker = Some(thread::spawn(move || {
            exercise(instance, surface, (size.width, size.height), args, stop)
                .map_err(|e| e.to_string())
        }));
        self.window = Some(window);
    }
    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::CloseRequested) {
            self.stop.store(true, Ordering::Release);
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.worker.as_ref().is_some_and(JoinHandle::is_finished) {
            self.result = Some(
                self.worker
                    .take()
                    .unwrap()
                    .join()
                    .unwrap_or_else(|_| Err("player worker panicked".into())),
            );
            event_loop.exit();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(20),
        ));
    }
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 && !(args.len() == 4 && args[3] == "seek") {
        return Err("Usage: live_saved ROOT OWNER_SOURCE_DIRECTORY CLIP_ID [seek]".into());
    }
    let mut diagnostic = Diagnostic {
        args,
        window: None,
        worker: None,
        stop: Arc::new(AtomicBool::new(false)),
        result: None,
    };
    EventLoop::new()?.run_app(&mut diagnostic)?;
    diagnostic.stop.store(true, Ordering::Release);
    if let Some(worker) = diagnostic.worker.take() {
        let _ = worker.join();
    }
    diagnostic.result.ok_or("no diagnostic result")??;
    Ok(())
}
