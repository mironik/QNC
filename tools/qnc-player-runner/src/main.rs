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
        Arc, Condvar, Mutex,
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
    let monitor_output = native.is_none();
    if monitor_output && boot.monitor_frame_map.is_none() {
        return Err("monitor output requires qnc-player-frame-map transport".into());
    }
    let mut monitor_post = boot
        .monitor_frame_map
        .as_ref()
        .map(|path| -> std::result::Result<MonitorPost, String> {
            let writer = LatestFrameWriter::open(path)?;
            MonitorPost::start(writer).map_err(|e| e.to_string())
        })
        .transpose()?;
    let output_scope = output_scope(native.is_some(), monitor_post.is_some())?;
    let plan = boot.plan()?;
    if qnc_dev_diagnostics::player_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(
            qnc_dev_diagnostics::DiagnosticsStream::Player,
            format!(
                "player-audio decoded_channels={} project_channels={} project_rate={} source_channels={:?}",
                plan.source()
                    .audio_format
                    .as_ref()
                    .map_or(0, |f| f.channel_count),
                boot.input.project_audio.channels,
                boot.input.project_audio.sample_rate_hz,
                plan.audio_channels().map(|m| m.output_channels())
            ),
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
        Runtime::open_access(plan, output, output_config, decoder, None, opener)?
    } else {
        Runtime::open_monitor_access(plan, output_config, decoder, None, opener)?
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
            "monitor_transport": if monitor_post.is_some() { "qnc-player-frame-map" } else { "qnc-player+tcp" },
            "output": output_scope
        })
    );
    io::stdout().flush()?;
    let mut last_contact = Instant::now();
    let mut last_report = Instant::now();
    let mut last_av_report = Instant::now();
    while !stop.load(Ordering::Acquire) && !session.closed {
        if !session.keeps_process_alive()
            && last_contact.elapsed() >= Duration::from_millis(boot.idle_timeout_ms)
        {
            break;
        }
        while let Ok(pending) = control.requests.try_recv() {
            last_contact = Instant::now();
            let reply = if Instant::now() > pending.expires {
                Err("request expired before execution".into())
            } else {
                session.handle(pending.request)
            };
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
            if let Some(poster) = monitor_post.as_mut() {
                if let Some(error) = poster.take_error() {
                    if qnc_dev_diagnostics::player_diagnostics_enabled() {
                        qnc_dev_diagnostics::log_line(
                            qnc_dev_diagnostics::DiagnosticsStream::Player,
                            format!("AV_F frame_map_error={error}"),
                        );
                    }
                    return Err(format!("monitor frame-map transport failed: {error}").into());
                }
                if let Err(error) = queue_monitor_mail(
                    poster,
                    &boot.session_id,
                    boot.source_generation,
                    source_timebase,
                    clear_monitor,
                    &frames,
                ) {
                    if qnc_dev_diagnostics::player_diagnostics_enabled() {
                        qnc_dev_diagnostics::log_line(
                            qnc_dev_diagnostics::DiagnosticsStream::Player,
                            format!("AV_F frame_map_error={error}"),
                        );
                    }
                    return Err(format!("monitor frame-map transport failed: {error}").into());
                }
            }
            if monitor_post.is_some() {
                // Monitor-output uses the local frame map for pixels. The
                // control socket remains command/state only, so a slow monitor
                // cannot force large RGBA writes through the command channel.
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
            if qnc_dev_diagnostics::player_diagnostics_enabled()
                && last_av_report.elapsed() >= Duration::from_millis(250)
            {
                if let Some(timing) = session.player.audio_driver_timing() {
                    qnc_dev_diagnostics::log_line(
                        qnc_dev_diagnostics::DiagnosticsStream::Player,
                        format!(
                            "AV_A session={} generation={} sample={} rate={} unix_ns={}",
                            boot.session_id,
                            timing.generation,
                            timing.first_sample_frame,
                            timing.sample_rate_hz,
                            timing.playback_unix_ns
                        ),
                    );
                }
                last_av_report = Instant::now();
            }
            if qnc_dev_diagnostics::player_diagnostics_enabled()
                && last_report.elapsed() >= Duration::from_secs(2)
            {
                let state = session.player.state();
                qnc_dev_diagnostics::log_line(
                    qnc_dev_diagnostics::DiagnosticsStream::Player,
                    format!(
                        "player-output status={:?} ready={} carrier={} submitted={:?} presented={:?} audio={:?}",
                        state.status,
                        state.play_ready,
                        state.carrier_frame,
                        state.submitted_frame,
                        state.presented_frame,
                        session.player.audio_telemetry()
                    ),
                );
                last_report = Instant::now();
            }
        }
        thread::sleep(Duration::from_millis(1));
    }
    drop(session);
    drop(monitor_post);
    drop(control);
    Ok(())
}

enum MonitorMail {
    Clear,
    Picture {
        header: qnc_player_contract::session::MonitorHeader,
        rgba: Arc<[u8]>,
    },
}

struct MonitorPost {
    slot: Arc<Mutex<Option<MonitorMail>>>,
    wake: Arc<Condvar>,
    stop: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    worker: Option<JoinHandle<()>>,
}

impl MonitorPost {
    fn start(mut writer: LatestFrameWriter) -> Result<Self> {
        let slot = Arc::new(Mutex::new(None));
        let wake = Arc::new(Condvar::new());
        let stop = Arc::new(AtomicBool::new(false));
        let error = Arc::new(Mutex::new(None));
        let worker = thread::Builder::new()
            .name("player-monitor-post".into())
            .spawn({
                let slot = slot.clone();
                let wake = wake.clone();
                let stop = stop.clone();
                let error = error.clone();
                move || {
                    while !stop.load(Ordering::Acquire) {
                        let mail = {
                            let mut guard = slot.lock().unwrap();
                            if guard.is_none() && !stop.load(Ordering::Acquire) {
                                let (next, _) = wake
                                    .wait_timeout(guard, Duration::from_millis(50))
                                    .expect("monitor post");
                                guard = next;
                            }
                            guard.take()
                        };
                        let Some(mail) = mail else {
                            continue;
                        };
                        let result = match mail {
                            MonitorMail::Clear => writer.clear().map(|_| ()),
                            MonitorMail::Picture { header, rgba } => {
                                writer.publish(&header, &rgba).map(|_| ())
                            }
                        };
                        if let Err(failed) = result {
                            *error.lock().unwrap() = Some(failed);
                            break;
                        }
                    }
                }
            })?;
        Ok(Self {
            slot,
            wake,
            stop,
            error,
            worker: Some(worker),
        })
    }

    fn post(&self, mail: MonitorMail) {
        *self.slot.lock().unwrap() = Some(mail);
        self.wake.notify_one();
    }

    fn take_error(&self) -> Option<String> {
        self.error.lock().unwrap().take()
    }
}

impl Drop for MonitorPost {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.wake.notify_one();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn queue_monitor_mail(
    poster: &MonitorPost,
    session: &str,
    source_generation: u64,
    source_timebase: Option<qnc_player_contract::Timebase>,
    clear: bool,
    frames: &[(qnc_video_output::FrameHeader, Arc<[u8]>)],
) -> Result<()> {
    if clear {
        poster.post(MonitorMail::Clear);
        return Ok(());
    }
    // Clock thread only hands off the current picture. The post thread writes
    // the mailbox so mmap copy cannot stall Play.
    let Some((header, rgba)) = frames.last() else {
        return Ok(());
    };
    let timebase = source_timebase.ok_or("monitor frame missing source timebase")?;
    poster.post(MonitorMail::Picture {
        header: qnc_player_contract::session::MonitorHeader {
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
        rgba: Arc::clone(rgba),
    });
    Ok(())
}

fn output_scope(native_output: bool, has_frame_map: bool) -> Result<&'static str> {
    match (native_output, has_frame_map) {
        (true, _) => Ok("native_host_window_and_device"),
        (false, true) => Ok("local_frame_map_monitor_and_host_audio_device"),
        (false, false) => Err("monitor output requires qnc-player-frame-map transport".into()),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_output_has_no_socket_frame_fallback() {
        assert_eq!(
            output_scope(false, true).unwrap(),
            "local_frame_map_monitor_and_host_audio_device"
        );
        assert!(output_scope(false, false).is_err());
        assert_eq!(
            output_scope(true, false).unwrap(),
            "native_host_window_and_device"
        );
    }
}
