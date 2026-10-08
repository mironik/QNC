//! The program picture on an external screen (user rule 2026-10-03: QNC is a news
//! cutter, a laptop in the field with an HDMI monitor; 2026-10-08: that screen may run
//! 50p, 60p, 50i, 60i, 24p or else).
//!
//! When a screen other than the main one is connected, this block runs the program
//! output process (`qnc-program-output-host`) full screen on it and tells it which
//! shared-memory frame map the preview monitor shows; without one the process is not
//! running and there is no error, and a screen plugged in or out is followed every two
//! seconds. The pictures go from the Broadcast Player to that process directly and are
//! drawn on every refresh of that screen: the desktop's own drawing (its timer, its
//! vsync, a second window in its pass) is not on their way. Drawing them in the
//! desktop's pass gave ghosts and stutter on both screens (live 2026-10-08).
//!
//! It is a passive interface of the Broadcast Player (AGENTS 0.9): no clock, no decode,
//! no database, no knowledge of the application on screen.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{Context, Id};

mod screens;
pub use screens::{output_screen, screens, Screen};

pub const MODULE_ID: &str = "qnc.module.program-output";

/// How often the screens are looked at again (a monitor plugged in or out).
const SCREENS_EVERY: Duration = Duration::from_secs(2);

/// The program output process, next to the desktop's executable.
const HOST: &str = "qnc-program-output-host";

/// The frame map offered in a pass, with that pass.
#[derive(Clone)]
struct Offered {
    pass: u64,
    frame_map: Arc<Path>,
}

fn offered_id() -> Id {
    Id::new("qnc-program-output-frame-map")
}

/// The preview monitor gives the frame map of the picture it paints in this pass.
/// Whether the program output is on an external screen now (it was in the last pass):
/// a preview monitor may then leave the picture to it (Monitor Auto, user 2026-10-08).
pub fn showing(ctx: &Context) -> bool {
    let pass = ctx.cumulative_pass_nr();
    ctx.data(|data| data.get_temp::<u64>(showing_id())).is_some_and(|shown| shown + 1 >= pass)
}

fn showing_id() -> Id {
    Id::new("qnc-program-output-showing")
}

pub fn offer(ctx: &Context, frame_map: &Arc<Path>) {
    let offered = Offered { pass: ctx.cumulative_pass_nr(), frame_map: frame_map.clone() };
    ctx.data_mut(|data| data.insert_temp(offered_id(), offered));
}

/// The running output process: it ends when its stdin closes (dropped here, or the
/// desktop is gone), so it never outlives the desktop.
struct Host {
    child: Child,
    stdin: ChildStdin,
    screen: Screen,
    sent: Option<Option<PathBuf>>,
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.stdin.write_all(b"\n");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The program output of one desktop.
#[derive(Default)]
pub struct ProgramOutput {
    screen: Option<Screen>,
    looked: Option<Instant>,
    host: Option<Host>,
    /// Why the output is off, once (no host executable, no screen list on this OS).
    pub problem: Option<String>,
}

impl ProgramOutput {
    /// Once a pass, after the forms painted: the output follows the screen and shows the
    /// frame map the monitor offered in this pass, black when it offered none.
    pub fn show(&mut self, ctx: &Context) {
        if self.looked.is_none_or(|at| at.elapsed() >= SCREENS_EVERY) {
            self.looked = Some(Instant::now());
            match screens() {
                Ok(found) => (self.screen, self.problem) = (output_screen(&found), None),
                Err(error) => (self.screen, self.problem) = (None, Some(error)),
            }
            if self.host.as_mut().is_some_and(|host| host.child.try_wait().ok().flatten().is_some()) {
                self.host = None; // ended by itself: started again on this look
            }
        }
        let Some(screen) = self.screen else {
            self.host = None;
            return;
        };
        if self.host.as_ref().is_none_or(|host| host.screen != screen) {
            self.host = None;
            match start(screen) {
                Ok(host) => self.host = Some(host),
                Err(error) => {
                    self.problem = Some(error);
                    self.screen = None; // tried again on the next look
                    return;
                }
            }
        }
        let pass = ctx.cumulative_pass_nr();
        ctx.data_mut(|data| data.insert_temp(showing_id(), pass));
        let current = ctx
            .data(|data| data.get_temp::<Offered>(offered_id()))
            .filter(|offered| offered.pass == pass)
            .map(|offered| offered.frame_map.to_path_buf());
        let Some(host) = &mut self.host else { return };
        if host.sent.as_ref() != Some(&current) {
            let line = match &current {
                Some(path) => format!("map {}\n", path.display()),
                None => "clear\n".to_string(),
            };
            if host.stdin.write_all(line.as_bytes()).and_then(|()| host.stdin.flush()).is_ok() {
                host.sent = Some(current);
            } else {
                self.host = None;
            }
        }
    }
}

fn start(screen: Screen) -> Result<Host, String> {
    let executable = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(format!("{HOST}{}", std::env::consts::EXE_SUFFIX))))
        .filter(|path| path.is_file())
        .ok_or_else(|| format!("Program izlaz: nema programa {HOST}."))?;
    let mut command = Command::new(executable);
    command
        .args(["--x", &screen.x.to_string(), "--y", &screen.y.to_string()])
        .args(["--width", &screen.width.to_string(), "--height", &screen.height.to_string()])
        .args(["--refresh-hz", &screen.refresh_hz.unwrap_or(0).to_string()])
        .args(["--interlaced", if screen.interlaced { "1" } else { "0" }])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // no console window
    }
    let mut child = command.spawn().map_err(|error| format!("Program izlaz: {error}"))?;
    let stdin = child.stdin.take().ok_or("Program izlaz: nema ulaza procesa.")?;
    Ok(Host { child, stdin, screen, sent: None })
}
