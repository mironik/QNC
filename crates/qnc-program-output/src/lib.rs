//! The program picture on an external screen (user rule 2026-10-03: QNC is a news
//! cutter, a laptop in the field with an HDMI monitor). When a second screen is
//! connected, it shows full screen the picture the preview monitor shows, black when
//! there is none; without one there is no window and no error, and a screen plugged in
//! or out while working is followed.
//!
//! It is a passive interface of the Broadcast Player (AGENTS 0.9): the preview monitor
//! offers the picture it paints (`offer`), the desktop host shows the output once a
//! frame (`show`). It owns no clock, decodes nothing, reads no database and does not
//! know which application is on screen. It follows the confirmed pictures the monitor
//! gets, at the pace the desktop paints them; an output driven by the player clock
//! (SDI/NDI, genlock) is a later adapter of the same contract.

use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Context, Id, ViewportBuilder, ViewportId};

mod screens;
pub use screens::{output_screen, screens, Screen};

pub const MODULE_ID: &str = "qnc.module.program-output";

/// How often the screens are looked at again (a monitor plugged in or out).
const SCREENS_EVERY: Duration = Duration::from_secs(2);

/// The picture the preview monitor painted, as it was given to it.
#[derive(Clone)]
pub struct ProgramPicture {
    pub session_id: String,
    pub generation: u64,
    pub sequence: u64,
    pub size: [usize; 2],
    pub rgba: Arc<[u8]>,
}

/// The picture offered in a pass, with that pass.
#[derive(Clone)]
struct Offered {
    pass: u64,
    picture: ProgramPicture,
}

/// Which screen the output uses, looked at every two seconds.
#[derive(Clone, Default)]
struct Output {
    screen: Option<Screen>,
    looked: Option<Instant>,
    error: Option<String>,
}

fn offered_id() -> Id {
    Id::new("qnc-program-output-picture")
}

fn output_id() -> Id {
    Id::new("qnc-program-output-screen")
}

/// Whether an external screen takes the program now: the monitor offers its picture
/// only then (a copy of the picture is not made for nothing).
pub fn active(ctx: &Context) -> bool {
    ctx.data(|data| data.get_temp::<Output>(output_id())).is_some_and(|output| output.screen.is_some())
}

/// The preview monitor gives the picture it paints in this pass.
pub fn offer(ctx: &Context, picture: impl FnOnce() -> ProgramPicture) {
    if !active(ctx) {
        return;
    }
    let offered = Offered { pass: ctx.cumulative_pass_nr(), picture: picture() };
    ctx.data_mut(|data| data.insert_temp(offered_id(), offered));
}

/// Shows the output on the external screen, once a pass, after the forms painted: the
/// picture the monitor offered in this pass, else black. No external screen, no window.
pub fn show(ctx: &Context) {
    let mut output = ctx.data(|data| data.get_temp::<Output>(output_id())).unwrap_or_default();
    if output.looked.is_none_or(|at| at.elapsed() >= SCREENS_EVERY) {
        output.looked = Some(Instant::now());
        match screens() {
            Ok(found) => (output.screen, output.error) = (output_screen(&found), None),
            Err(error) => (output.screen, output.error) = (None, Some(error)),
        }
    }
    ctx.data_mut(|data| data.insert_temp(output_id(), output.clone()));
    if let Some(screen) = output.screen {
        present(ctx, screen);
    }
}

/// The output window full screen on `screen`, with the picture offered in this pass.
pub fn present(ctx: &Context, screen: Screen) {
    let pass = ctx.cumulative_pass_nr();
    let picture = ctx
        .data(|data| data.get_temp::<Offered>(offered_id()))
        .filter(|offered| offered.pass == pass)
        .map(|offered| offered.picture);
    present_picture(ctx, screen, picture);
}

/// The output window full screen on `screen` with `picture` (black without one).
pub fn present_picture(ctx: &Context, screen: Screen, picture: Option<ProgramPicture>) {
    // Opened inside the external screen, then full screen on that screen: the OS gives it
    // the whole screen whatever its scale (an HDMI screen at 100 % beside a laptop at
    // 125 % got a window sized by the laptop's scale, live 2026-10-08). The desktop gives
    // screens in pixels; egui turns points into pixels by the main screen's scale.
    let scale = ctx.pixels_per_point().max(0.1);
    let builder = ViewportBuilder::default()
        .with_title("QNC Program")
        .with_decorations(false)
        .with_position([
            (screen.x as f32 + screen.width as f32 / 4.0) / scale,
            (screen.y as f32 + screen.height as f32 / 4.0) / scale,
        ])
        .with_inner_size([screen.width as f32 / scale / 2.0, screen.height as f32 / scale / 2.0])
        .with_taskbar(false)
        .with_active(false)
        .with_mouse_passthrough(true);
    ctx.show_viewport_immediate(ViewportId::from_hash_of("qnc-program-output"), builder, |ctx, _| {
        if ctx.input(|input| input.viewport().fullscreen) != Some(true) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::BLACK))
            .show(ctx, |ui| {
                if let Some(picture) = &picture {
                    qnc_ui_kit::paint_stream_frame(
                        ui,
                        ui.max_rect(),
                        Id::new("qnc-program-output-surface"),
                        (&picture.session_id, picture.generation, picture.sequence),
                        picture.size,
                        &picture.rgba,
                    );
                }
            });
    });
}
