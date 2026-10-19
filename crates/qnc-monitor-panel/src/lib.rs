//! The monitor panel (user 2026-10-19: "a block written once works everywhere; forms are
//! only the layout"): the preview monitor with its own controls, the same wherever a
//! layout places it. It draws the picture of the universal source preview, else the
//! poster of the shown clip, else the message or the label (`qnc-monitor`), and offers the
//! picture to the program output on an external screen. Its controls are the transport
//! buttons a layout lists for the row under the monitor and, always, the monitor's own
//! Auto | On switch; which button is on comes from here, a click is a catalog action that
//! the shared source panel keys carry out (`qnc-source-panel`).
//!
//! Passive: no clock, no decode, no database; the state comes from the preview.

use eframe::egui::{self, Rect, Ui};
use qnc_monitor::{MonitorChrome, MonitorPicture, MonitorPoster, MonitorSurface, MONITOR_AUTO, MONITOR_ON};
pub use qnc_source_preview::MonitorState;

pub const MODULE_ID: &str = "qnc.module.monitor-panel";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// One button of the row under the monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control {
    pub label: String,
    pub action_id: String,
    pub on: bool,
}

/// The monitor's controls: the transport buttons the layout lists (label, action), then
/// the monitor's own Auto | On switch, with the one that is on.
pub fn controls<'a>(listed: impl IntoIterator<Item = (&'a str, &'a str)>, monitor: &MonitorState) -> Vec<Control> {
    let mut controls: Vec<Control> = listed
        .into_iter()
        .filter(|(_, action)| !matches!(*action, MONITOR_AUTO | MONITOR_ON))
        .map(|(label, action)| Control { label: label.to_string(), action_id: action.to_string(), on: false })
        .collect();
    for (action, key) in [(MONITOR_AUTO, "monitor.auto"), (MONITOR_ON, "monitor.on")] {
        let on = qnc_monitor::switch_on(action, monitor.auto);
        controls.push(Control { label: qnc_ui_text::text(key).to_string(), action_id: action.to_string(), on });
    }
    controls
}

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
    let surface = MonitorSurface { id, chrome: look, picture, message: monitor.message.as_deref(), yield_to_external: monitor.auto };
    qnc_monitor::paint_source_monitor(ui, rect, surface, poster, label);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_switch_follows_the_listed_buttons_once_with_the_chosen_one_on() {
        let auto = MonitorState { auto: true, ..MonitorState::default() };
        let listed = [(">", "play_pause"), ("Auto", MONITOR_AUTO)];
        let shown: Vec<_> = controls(listed, &auto).into_iter().map(|control| (control.action_id, control.on)).collect();
        let expected = [("play_pause", false), (MONITOR_AUTO, true), (MONITOR_ON, false)].map(|(action, on)| (action.to_string(), on));
        assert_eq!(shown, expected);
        let on = controls(listed, &MonitorState { auto: false, ..MonitorState::default() });
        assert!(on.last().is_some_and(|control| control.on), "On is on");
    }
}
