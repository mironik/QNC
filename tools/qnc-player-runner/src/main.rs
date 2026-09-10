//! One native-output Broadcast Player process. No application or project workflow.
mod config;
mod control;
mod session;
use config::Boot;
use qnc_broadcast_engine::Runtime;
use qnc_json_transport::Credentials;
use qnc_player_frame_transport::LatestFrameWriter;
use std::{
    io::{self, Write},
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

fn run(
    boot: Boot,
    native: Option<(wgpu::Instance, wgpu::Surface<'static>)>,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    let mut frame_writer = boot
        .monitor_frame_map
        .as_ref()
        .map(LatestFrameWriter::open)
        .transpose()?;
    let output_scope = if native.is_some() {
        "native_host_window_and_device"
    } else if frame_writer.is_some() {
        "local_frame_map_monitor_and_host_audio_device"
    } else {
        "socket_monitor_and_host_audio_device"
    };
    let plan = boot.plan()?;
    if std::env::var_os("QNC_PLAYER_DIAGNOSTICS").is_some() {
        eprintln!(
            "player-audio decoded_channels={} project_channels={} project_rate={} source_channels={:?}",
            plan.source()
                .audio_format
                .as_ref()
                .map_or(0, |f| f.channel_count),
            boot.input.project_audio.channels,
            boot.input.project_audio.sample_rate_hz,
            plan.audio_channels().map(|m| m.output_channels())
        );
    }
    let media_uri = boot.input.media()?.media_uri.clone();
    let opener = boot.media_binding.opener(&media_uri)?;
    let output_config = plan.output_config(&boot.session_id, boot.source_generation)?;
    let decoder = qnc_decoder_catalog::installed_config()?;
    let player = if let Some((instance, surface)) = native {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .ok_or("no native GPU adapter")?;
        let output = pollster::block_on(qnc_video_output::VideoOutput::open(
            &adapter,
            Some(surface),
            output_config.clone(),
            (960, 640),
        ))?;
        Runtime::open(plan, output, output_config, decoder, None, opener)?
    } else {
        Runtime::open_monitor(plan, output_config, decoder, None, opener)?
    };
    let control = control::Control::open(
        boot.listen_port,
        Credentials::new(&boot.read_token, &boot.command_token)?,
    )?;
    let mut session =
        session::Session::new(player, boot.session_id.clone(), boot.source_generation);
    println!(
        "{}",
        serde_json::json!({
            "contract_version": qnc_player_contract::VERSION,
            "session_id": boot.session_id, "source_generation": boot.source_generation,
            "wire_url": control.address,
            "monitor_transport": if frame_writer.is_some() { "qnc-player-frame-map" } else { "qnc-player+tcp" },
            "output": output_scope
        })
    );
    io::stdout().flush()?;
    let mut last_contact = Instant::now();
    let diagnostics = std::env::var_os("QNC_PLAYER_DIAGNOSTICS").is_some();
    let mut last_report = Instant::now();
    let mut last_av_report = Instant::now();
    while !stop.load(Ordering::Acquire) && !session.closed {
        if last_contact.elapsed() >= Duration::from_millis(boot.idle_timeout_ms) {
            break;
        }
        if let Ok(pending) = control.requests.try_recv() {
            let reply = if Instant::now() > pending.expires {
                Err("request expired before execution".into())
            } else {
                session.handle(pending.request)
            };
            if reply.is_ok() {
                last_contact = Instant::now();
            }
            let _ = pending.response.send(reply);
        }
        if !session.closed {
            session.tick();
            let source_timebase = session
                .player
                .state()
                .source
                .as_ref()
                .map(|source| source.timebase);
            let (clear_monitor, frames) = session.player.take_monitor_frames();
            if let Some(writer) = frame_writer.as_mut()
                && let Err(error) = publish_frame_map(
                    writer,
                    &boot.session_id,
                    boot.source_generation,
                    source_timebase,
                    clear_monitor,
                    &frames,
                )
            {
                if diagnostics {
                    eprintln!("AV_F frame_map_error={error}");
                }
                frame_writer = None;
            }
            if frame_writer.is_some() {
                if clear_monitor {
                    control.publish_frames(
                        &boot.session_id,
                        boot.source_generation,
                        source_timebase,
                        true,
                        Vec::new(),
                    );
                }
            } else {
                control.publish_frames(
                    &boot.session_id,
                    boot.source_generation,
                    source_timebase,
                    clear_monitor,
                    frames,
                );
            }
            if diagnostics && last_av_report.elapsed() >= Duration::from_millis(250) {
                if let Some(timing) = session.player.audio_driver_timing() {
                    eprintln!(
                        "AV_A session={} generation={} sample={} rate={} unix_ns={}",
                        boot.session_id,
                        timing.generation,
                        timing.first_sample_frame,
                        timing.sample_rate_hz,
                        timing.playback_unix_ns
                    );
                }
                last_av_report = Instant::now();
            }
            if diagnostics && last_report.elapsed() >= Duration::from_secs(2) {
                let state = session.player.state();
                eprintln!(
                    "player-output status={:?} ready={} carrier={} submitted={:?} presented={:?} audio={:?}",
                    state.status,
                    state.play_ready,
                    state.carrier_frame,
                    state.submitted_frame,
                    state.presented_frame,
                    session.player.audio_telemetry()
                );
                last_report = Instant::now();
            }
        }
        thread::sleep(Duration::from_millis(1));
    }
    drop(session);
    drop(control);
    Ok(())
}

fn publish_frame_map(
    writer: &mut LatestFrameWriter,
    session: &str,
    source_generation: u64,
    source_timebase: Option<qnc_player_contract::Timebase>,
    clear: bool,
    frames: &[(qnc_video_output::FrameHeader, Arc<[u8]>)],
) -> Result<()> {
    if clear {
        writer.clear()?;
    }
    for (header, rgba) in frames {
        let timebase = source_timebase.ok_or("monitor frame missing source timebase")?;
        writer.publish(
            &qnc_player_contract::session::MonitorHeader {
                contract_version: qnc_player_contract::VERSION.into(),
                session_id: session.into(),
                source_generation,
                output_generation: header.generation,
                sequence: header.sequence,
                source_id: header.source_id.clone(),
                frame: header.frame_number,
                timebase,
                width: header.width,
                height: header.height,
            },
            rgba,
        )?;
    }
    Ok(())
}

struct NativeHost {
    boot: Option<Boot>,
    window: Option<Arc<Window>>,
    worker: Option<JoinHandle<std::result::Result<(), String>>>,
    stop: Arc<AtomicBool>,
    result: Option<std::result::Result<(), String>>,
}
impl ApplicationHandler for NativeHost {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Some(boot) = self.boot.take() else {
            return;
        };
        let setup = (|| -> Result<_> {
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("QNC Broadcast Player")
                        .with_resizable(false)
                        .with_inner_size(winit::dpi::PhysicalSize::new(960, 640)),
                )?,
            );
            let instance = wgpu::Instance::default();
            let surface = instance.create_surface(window.clone())?;
            Ok((window, instance, surface))
        })();
        match setup {
            Ok((window, instance, surface)) => {
                let stop = self.stop.clone();
                self.worker = Some(thread::spawn(move || {
                    run(boot, Some((instance, surface)), stop).map_err(|e| e.to_string())
                }));
                self.window = Some(window);
            }
            Err(error) => {
                self.result = Some(Err(error.to_string()));
                event_loop.exit();
            }
        }
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
                    .unwrap_or_else(|_| Err("player thread panicked".into())),
            );
            event_loop.exit();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(20),
        ));
    }
}
fn main() -> Result<()> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args != ["--native-output"] && args != ["--monitor-output"] {
        return Err(
            "Usage: qnc-broadcast-player --native-output; bounded bootstrap JSON on stdin".into(),
        );
    }
    let boot = Boot::read(io::stdin().lock())?;
    if args == ["--monitor-output"] {
        return run(boot, None, Arc::new(AtomicBool::new(false)));
    }
    let mut host = NativeHost {
        boot: Some(boot),
        window: None,
        worker: None,
        stop: Arc::new(AtomicBool::new(false)),
        result: None,
    };
    let result = EventLoop::new()?.run_app(&mut host);
    host.stop.store(true, Ordering::Release);
    if let Some(worker) = host.worker.take() {
        let _ = worker.join();
    }
    result?;
    host.result.ok_or("no player result")??;
    Ok(())
}
