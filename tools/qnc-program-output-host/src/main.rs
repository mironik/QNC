//! The program output process (user rule 2026-10-08: QNC is a news cutter, a laptop in
//! the field with an HDMI monitor that may run 50p, 60p, 50i, 60i, 24p or else).
//!
//! It owns one borderless full-screen window on the external screen. A thread of its own
//! waits for each refresh of that screen and then draws the newest picture the Broadcast
//! Player wrote to its shared-memory frame map, so the picture follows the
//! player clock at the screen's own cadence (50p on 50 Hz one picture per refresh, other
//! pairs repeat or drop as the screen requires). It never goes through the desktop's
//! drawing; the desktop only says which frame map is current, one line on stdin:
//! `map <path>` or `clear`. Stdin closing (the desktop is gone) ends the process.
//!
//! No clock of its own, no decode, no database, no knowledge of the application: it is
//! a passive output interface of the Broadcast Player (AGENTS 0.9).

mod render;
mod vblank;

use std::{
    io::BufRead,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use qnc_player_frame_transport::{LatestFrameReader, LatestFrameUpdate};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Fullscreen, Window, WindowId},
};

/// What the desktop asked last, from stdin.
#[derive(Default)]
struct Asked {
    map: Option<PathBuf>,
    changed: bool,
    quit: bool,
}

/// The external screen, in desktop pixels.
#[derive(Clone, Copy, Debug)]
struct Screen {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    /// The screen's refresh and interlacing as the desktop read them from the OS.
    refresh_hz: Option<u32>,
    interlaced: bool,
}

fn args() -> Result<Screen, String> {
    let mut screen = Screen { x: 0, y: 0, width: 0, height: 0, refresh_hz: None, interlaced: false };
    let mut list = std::env::args().skip(1);
    while let Some(arg) = list.next() {
        let value = list.next().ok_or(format!("{arg} needs a value"))?;
        let number = |value: &str| value.parse::<i64>().map_err(|_| format!("{arg}: not a number"));
        match arg.as_str() {
            "--x" => screen.x = number(&value)? as i32,
            "--y" => screen.y = number(&value)? as i32,
            "--width" => screen.width = number(&value)? as u32,
            "--height" => screen.height = number(&value)? as u32,
            "--refresh-hz" => screen.refresh_hz = Some(number(&value)? as u32).filter(|hz| *hz > 1),
            "--interlaced" => screen.interlaced = value == "1",
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if screen.width == 0 || screen.height == 0 {
        return Err("usage: qnc-program-output-host --x X --y Y --width W --height H (map on stdin)".into());
    }
    Ok(screen)
}

fn log(message: impl AsRef<str>) {
    if qnc_dev_diagnostics::player_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(
            qnc_dev_diagnostics::DiagnosticsStream::Player,
            format!("program-output {}", message.as_ref()),
        );
    }
}

/// What the output showed, reported every five seconds (diagnostics): refreshes drawn,
/// new pictures among them, pictures the player made that no refresh showed, and draws
/// that came early or late against the screen's refresh.
#[derive(Default)]
struct Counts {
    refreshes: u64,
    new_pictures: u64,
    skipped: u64,
    short: u64,
    long: u64,
}

/// The drawing, on a thread of its own: wait for the screen's refresh, take the newest
/// picture, draw it. The window's event loop (winit) never paces it: its redraw
/// requests come late and missed refreshes (live 2026-10-08: 217 draws in 5 s at 50 Hz).
struct Output {
    screen: Screen,
    asked: Arc<Mutex<Asked>>,
    resized: Arc<Mutex<Option<(u32, u32)>>>,
    renderer: render::Renderer,
    vblank: Option<vblank::VBlank>,
    reader: Option<LatestFrameReader>,
    last: Option<(u64, u64)>,
    source_rate: Option<(i64, i64)>,
    refresh_mhz: Option<u32>,
    counts: Counts,
}

impl Output {
    fn run(mut self) {
        // The output must not miss a refresh because other work took the processor.
        #[cfg(windows)]
        unsafe {
            use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL};
            let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
        }
        let mut reported = Instant::now();
        let mut last_draw: Option<Instant> = None;
        loop {
            // Right after this screen's refresh: the newest picture is the one due now.
            if let Some(vblank) = &self.vblank {
                vblank.wait();
            }
            if self.asked.lock().unwrap().quit {
                return;
            }
            if let Some((width, height)) = self.resized.lock().unwrap().take() {
                self.renderer.resize(width, height);
            }
            self.follow_desktop();
            self.take_picture();
            if let Err(error) = self.renderer.draw() {
                log(format!("draw error={error}"));
                std::thread::sleep(Duration::from_millis(20));
            }
            self.counts.refreshes += 1;
            let now = Instant::now();
            if let (Some(last), Some(mhz)) = (last_draw, self.refresh_mhz) {
                let period = 1_000_000.0 / mhz as f64;
                let gap = now.duration_since(last).as_secs_f64() * 1000.0;
                self.counts.short += u64::from(gap < period * 0.75);
                self.counts.long += u64::from(gap > period * 1.25);
            }
            last_draw = Some(now);
            if reported.elapsed() >= Duration::from_secs(5) {
                let Counts { refreshes, new_pictures, skipped, short, long } = std::mem::take(&mut self.counts);
                if new_pictures > 0 {
                    log(format!(
                        "refreshes={refreshes} new_pictures={new_pictures} skipped={skipped} early_draws={short} late_draws={long}"
                    ));
                }
                reported = Instant::now();
            }
        }
    }

    /// Takes what the desktop asked: another map opens a reader (black until it has a
    /// picture), `clear` shows black.
    fn follow_desktop(&mut self) {
        let (changed, map) = {
            let mut asked = self.asked.lock().unwrap();
            (std::mem::take(&mut asked.changed), asked.map.clone())
        };
        if changed {
            self.reader = None;
            self.last = None;
            self.renderer.clear_picture();
            log(format!("map {}", map.as_ref().map_or("none".into(), |path| path.display().to_string())));
        }
        if self.reader.is_none()
            && let Some(path) = map
        {
            // The player may not have written its map yet: asked again next refresh.
            self.reader = LatestFrameReader::open(&path).ok();
        }
    }

    /// The next picture of the map in order, one per refresh: two pictures the player
    /// handed in one refresh interval are shown on two refreshes instead of the first
    /// being dropped (a pan showed every drop, live 2026-10-08). Only when the output
    /// falls more than a few pictures behind does it jump to the newest.
    fn take_picture(&mut self) {
        const MAX_BEHIND: u64 = 3;
        let Some(reader) = &mut self.reader else { return };
        let behind = reader.backlog();
        let next = if behind > MAX_BEHIND { reader.read_newest() } else { reader.read_latest() };
        let update = match next {
            Ok(update) => update,
            Err(error) => {
                log(format!("read error={error}"));
                self.reader = None;
                return;
            }
        };
        match update {
            Some(LatestFrameUpdate::Picture(frame)) => {
                let header = &frame.header;
                let key = (header.output_generation, header.sequence);
                if let Some((generation, sequence)) = self.last
                    && generation == key.0
                    && key.1 > sequence + 1
                {
                    self.counts.skipped += key.1 - sequence - 1;
                }
                self.last = Some(key);
                self.counts.new_pictures += 1;
                let rate = (header.timebase.fps_num, header.timebase.fps_den);
                if self.source_rate != Some(rate) {
                    self.source_rate = Some(rate);
                    self.report_mode();
                }
                self.renderer.set_picture(header.width, header.height, &frame.rgba);
            }
            Some(LatestFrameUpdate::Clear) => self.renderer.clear_picture(),
            None => {}
        }
    }

    /// The screen's refresh against the source rate: a pair that does not divide evenly
    /// cannot be shown smoothly (the OS mode is the user's to set, never changed here).
    fn report_mode(&self) {
        let (Some(mhz), Some((num, den))) = (self.refresh_mhz, self.source_rate) else { return };
        let refresh = mhz as f64 / 1000.0;
        let source = num as f64 / den as f64;
        let ratio = refresh / source;
        let even = (ratio - ratio.round()).abs() < 0.01 && ratio.round() >= 1.0;
        log(format!(
            "mode screen_hz={refresh:.3}{} source_fps={source:.3} {}",
            if self.screen.interlaced { "i" } else { "p" },
            if even { "even" } else { "uneven: set the screen to a multiple of the source rate" }
        ));
    }
}

/// The window on the main thread (winit needs it there); it only follows window events.
struct Host {
    screen: Screen,
    asked: Arc<Mutex<Asked>>,
    resized: Arc<Mutex<Option<(u32, u32)>>>,
    window: Option<Arc<Window>>,
    drawing: Option<std::thread::JoinHandle<()>>,
    result: Result<(), String>,
}

impl Host {
    /// The monitor at the screen's corner; else the one holding its middle; else the
    /// largest that is not the main one (a caller unaware of the screens' scales gives
    /// other numbers).
    fn monitor(&self, event_loop: &ActiveEventLoop) -> Option<winit::monitor::MonitorHandle> {
        let monitors: Vec<_> = event_loop.available_monitors().collect();
        let primary = event_loop.primary_monitor();
        let (middle_x, middle_y) = (
            self.screen.x + self.screen.width as i32 / 2,
            self.screen.y + self.screen.height as i32 / 2,
        );
        monitors
            .iter()
            .find(|monitor| monitor.position().x == self.screen.x && monitor.position().y == self.screen.y)
            .or_else(|| {
                monitors.iter().find(|monitor| {
                    let (position, size) = (monitor.position(), monitor.size());
                    (position.x..position.x + size.width as i32).contains(&middle_x)
                        && (position.y..position.y + size.height as i32).contains(&middle_y)
                        && Some(*monitor) != primary.as_ref()
                })
            })
            .or_else(|| {
                monitors
                    .iter()
                    .filter(|monitor| Some(*monitor) != primary.as_ref())
                    .max_by_key(|monitor| u64::from(monitor.size().width) * u64::from(monitor.size().height))
            })
            .cloned()
    }

    fn open(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let monitor = self.monitor(event_loop);
        let refresh_mhz = self
            .screen
            .refresh_hz
            .map(|hz| hz * 1000)
            .or_else(|| monitor.as_ref().and_then(|monitor| monitor.refresh_rate_millihertz()));
        // Full screen on that monitor whatever its scale.
        let attributes = Window::default_attributes()
            .with_title("QNC Program")
            .with_decorations(false)
            .with_active(false)
            .with_position(winit::dpi::PhysicalPosition::new(self.screen.x, self.screen.y))
            .with_inner_size(winit::dpi::PhysicalSize::new(self.screen.width, self.screen.height))
            .with_fullscreen(Some(Fullscreen::Borderless(monitor)));
        let window = Arc::new(event_loop.create_window(attributes).map_err(|error| error.to_string())?);
        window.set_cursor_visible(false);
        let (position, size) = (window.outer_position().unwrap_or_default(), window.outer_size());
        let vblank = vblank::VBlank::find((position.x, position.y), (size.width, size.height));
        let renderer = render::Renderer::open(window.clone(), vblank.is_some())?;
        log(format!("open screen={:?} refresh_mhz={refresh_mhz:?}", self.screen));
        log(format!(
            "vblank {}",
            if vblank.is_some() { "screen, mailbox present, own thread" } else { "fifo only, own thread" }
        ));
        let output = Output {
            screen: self.screen,
            asked: self.asked.clone(),
            resized: self.resized.clone(),
            renderer,
            vblank,
            reader: None,
            last: None,
            source_rate: None,
            refresh_mhz,
            counts: Counts::default(),
        };
        self.drawing = Some(
            std::thread::Builder::new()
                .name("program-output-draw".into())
                .spawn(move || output.run())
                .map_err(|error| error.to_string())?,
        );
        self.window = Some(window);
        Ok(())
    }
}

impl ApplicationHandler for Host {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        if let Err(error) = self.open(event_loop) {
            log(format!("open error={error}"));
            self.result = Err(error);
            event_loop.exit();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => *self.resized.lock().unwrap() = Some((size.width, size.height)),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let quit = self.asked.lock().unwrap().quit;
        if quit || self.drawing.as_ref().is_some_and(std::thread::JoinHandle::is_finished) {
            event_loop.exit();
        }
        // The stdin thread sets quit; looked at a few times a second.
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(200)));
    }
}

fn main() -> Result<(), String> {
    let screen = args()?;
    let asked = Arc::new(Mutex::new(Asked::default()));
    // The desktop's lines; the end of stdin is the end of the desktop.
    let from_desktop = asked.clone();
    std::thread::Builder::new()
        .name("program-output-stdin".into())
        .spawn(move || {
            for line in std::io::stdin().lock().lines() {
                let Ok(line) = line else { break };
                let mut asked = from_desktop.lock().unwrap();
                match line.split_once(' ') {
                    Some(("map", path)) => asked.map = Some(PathBuf::from(path)),
                    _ if line == "clear" => asked.map = None,
                    _ => continue,
                }
                asked.changed = true;
            }
            from_desktop.lock().unwrap().quit = true;
        })
        .map_err(|error| error.to_string())?;
    let mut host = Host {
        screen,
        asked: asked.clone(),
        resized: Arc::default(),
        window: None,
        drawing: None,
        result: Ok(()),
    };
    let event_loop = EventLoop::new().map_err(|error| error.to_string())?;
    event_loop.run_app(&mut host).map_err(|error| error.to_string())?;
    asked.lock().unwrap().quit = true;
    if let Some(drawing) = host.drawing.take() {
        let _ = drawing.join();
    }
    host.result
}
