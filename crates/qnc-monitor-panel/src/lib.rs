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
use qnc_monitor::{MonitorChrome, MonitorPicture, MonitorPoster, MonitorSurface};
pub use qnc_source_preview::MonitorState;

pub const MODULE_ID: &str = "qnc.module.monitor-panel";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The poster of the shown clip, when it has one loaded.
pub fn poster<'a>(uri: Option<&'a String>, image: Option<&'a qnc_image_assets::RgbaImage>) -> Option<MonitorPoster<'a>> {
    let (uri, image) = (uri?, image?);
    Some(MonitorPoster { uri, content_key: image.content_key, size: image.size, rgba: &image.pixels })
}

/// The monitor in `rect`: `id` names the surface (one per place), `label` is shown when
/// there is nothing else.
pub fn show(ui: &mut Ui, rect: Rect, id: egui::Id, look: MonitorChrome, monitor: &MonitorState, poster: Option<MonitorPoster<'_>>, label: &str) {
    let picture = monitor.frame.as_ref().filter(|_| monitor.video_visible).map(|frame| MonitorPicture {
        session_id: &frame.session_id,
        generation: frame.generation,
        sequence: frame.sequence,
        size: [frame.width, frame.height],
        rgba: &frame.rgba,
        frame_map: frame.frame_map.as_ref(),
    });
    let surface = MonitorSurface { id, chrome: look, picture, message: monitor.message.as_deref(), yield_to_external: qnc_monitor::auto() };
    qnc_monitor::paint_source_monitor(ui, rect, surface, poster, label);
}
