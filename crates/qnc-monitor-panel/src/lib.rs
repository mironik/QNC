//! The monitor panel (user 2026-10-19: blocks build modules, modules build panels, a
//! layout places panels; a panel is written once and works wherever a layout puts it):
//! the preview monitor, the same in every place. It draws the picture of the universal
//! source preview, else the poster of the shown clip, else the message or the label
//! (`qnc-monitor`), and offers the picture to the program output on an external screen;
//! with Auto chosen (`qnc_monitor::auto`) it leaves the picture to that screen. Its
//! buttons are a panel of their own (`qnc-transport-panel`), so a layout may place them
//! anywhere.
//!
//! Passive: no clock, no decode, no database; the state comes from the preview.

use eframe::egui::{self, Rect, Ui};
use qnc_clip_row::PoolItem;
use qnc_monitor::{MonitorPicture, MonitorPoster, MonitorSurface};
pub use qnc_monitor::MonitorState;

pub const MODULE_ID: &str = "qnc.module.monitor-panel";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The monitor in `rect`, in the look of the one theme: `id` names the surface (one per
/// place), `shown` is the clip it shows (its poster until the player has a picture), `label`
/// is shown when there is nothing else.
pub fn show(ui: &mut Ui, rect: Rect, id: egui::Id, theme: &qnc_desktop_theme::Theme, monitor: &MonitorState, shown: Option<&dyn PoolItem>, label: &str) {
    let look = qnc_theme_styles::monitor(theme);
    let poster = shown.and_then(|item| {
        let (uri, image) = item.card_poster();
        let (uri, image) = (uri?, image?);
        Some(MonitorPoster { uri, content_key: image.content_key, size: image.size, rgba: &image.pixels })
    });
    let picture = monitor.frame.as_ref().filter(|_| monitor.video_visible).map(|frame| MonitorPicture {
        session_id: &frame.session_id,
        generation: frame.generation,
        sequence: frame.sequence,
        size: [frame.width, frame.height],
        rgba: &frame.rgba,
        frame_map: frame.frame_map.as_ref(),
    });
    // Auto leaves the picture to the program output while an external screen shows it.
    let yield_to_external = qnc_monitor::auto() && qnc_program_output::showing(ui.ctx());
    let surface = MonitorSurface { id, chrome: look, picture, message: monitor.message.as_deref(), yield_to_external };
    qnc_monitor::paint_source_monitor(ui, rect, surface, poster, label);
    // The same pictures go to the program output on an external screen, when one is there.
    if let Some(frame_map) = monitor.frame.as_ref().filter(|_| monitor.video_visible).and_then(|frame| frame.frame_map.as_ref()) {
        qnc_program_output::offer(ui.ctx(), frame_map);
    }
}
