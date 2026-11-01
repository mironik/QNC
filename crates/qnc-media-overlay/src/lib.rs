//! The media overlay (user 2026-11-01, form G): a form whose whole board is one panel (the
//! script) has the media pool, the monitor and the source timeline in a window over it. This
//! block draws the bar with the button that opens the window, and the window itself; what is
//! in the window is a board layout the form draws with its usual places. Whether the window
//! is open is a view choice of this process (egui memory), never written anywhere. Passive:
//! no database, no workflow, no player.

use eframe::egui::{self, Align, Id, Rect, Sense, Ui, UiBuilder, Vec2};
use qnc_desktop_theme::Theme;

pub const MODULE_ID: &str = "qnc.module.media-overlay";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The share of the screen the window takes.
const SCREEN_SHARE: f32 = 0.92;
/// The minimum width of the open button.
const BUTTON_WIDTH: f32 = 180.0;

fn open_id() -> Id {
    Id::new("qnc-media-overlay-open")
}

/// Whether the window is open now.
pub fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|data| data.get_temp::<bool>(open_id())).unwrap_or(false)
}

/// Opens or closes the window.
pub fn set_open(ctx: &egui::Context, open: bool) {
    ctx.data_mut(|data| data.insert_temp(open_id(), open));
}

/// The bar in `rect`: the button `label` on its left opens the window (and closes it while it
/// is open, shown as pressed). It gives no answer of its own: the window is its own.
pub fn bar<R>(ui: &mut Ui, rect: Rect, theme: &Theme, pad: f32, label: Option<&str>) -> Option<R> {
    let label = label?;
    ui.painter().rect_filled(rect, 0.0, theme.bg);
    let open = is_open(ui.ctx());
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        ui.allocate_rect(rect, Sense::hover());
        let inner = UiBuilder::new().max_rect(rect.shrink(pad)).layout(egui::Layout::left_to_right(Align::Center));
        ui.scope_builder(inner, |ui| {
            ui.spacing_mut().interact_size = Vec2::new(BUTTON_WIDTH, theme.chrome_control_height);
            let color = if open { theme.focus } else { theme.accent };
            if qnc_ui_kit::strong_button(ui, label, true, color, theme.chrome_control_height).clicked() {
                set_open(ui.ctx(), !open);
            }
        });
    });
    None
}

/// The window with the board `layout` (faces by name through `face`, blocks through `block`)
/// while it is open; nothing without a title and a layout (a form without the overlay).
pub fn show_board<R>(
    ctx: &egui::Context,
    theme: &Theme,
    title: Option<&str>,
    layout: Option<qnc_board::Layout>,
    face: impl Fn(&str) -> Option<egui::Color32>,
    block: impl FnMut(&mut Ui, &str, Rect) -> Option<R>,
) -> Option<R> {
    let (title, layout) = (title?, layout?);
    show(ctx, theme, title, |ui| qnc_board::show_layout(ui, &layout, face, block))
}

/// The window over the form while it is open: `title` on top, closed by its cross; `draw`
/// draws the layout in its body and its answer is the window's.
pub fn show<R>(ctx: &egui::Context, theme: &Theme, title: &str, draw: impl FnOnce(&mut Ui) -> Option<R>) -> Option<R> {
    let mut open = is_open(ctx);
    if !open {
        return None;
    }
    let screen = ctx.screen_rect();
    let size = window_size(screen.size());
    let frame = egui::Frame::window(&ctx.style()).fill(theme.bg).stroke(egui::Stroke::new(1.0, theme.border_soft));
    let shown = egui::Window::new(title)
        .id(Id::new("qnc-media-overlay-window"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .movable(true)
        .frame(frame)
        .fixed_size(size)
        .default_pos(screen.center() - size / 2.0)
        .show(ctx, |ui| {
            ui.set_min_size(size);
            draw(ui)
        });
    set_open(ctx, open);
    shown.and_then(|shown| shown.inner).flatten()
}

/// The body of the window: a share of the screen.
fn window_size(screen: Vec2) -> Vec2 {
    screen * SCREEN_SHARE - Vec2::new(0.0, 40.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_is_a_share_of_the_screen_less_its_title() {
        assert_eq!(window_size(Vec2::new(1000.0, 800.0)), Vec2::new(920.0, 696.0));
    }

    #[test]
    fn the_window_is_closed_until_it_is_opened() {
        let ctx = egui::Context::default();
        assert!(!is_open(&ctx));
        set_open(&ctx, true);
        assert!(is_open(&ctx));
    }
}
