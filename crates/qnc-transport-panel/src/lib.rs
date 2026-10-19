//! The transport panel (user 2026-10-19: blocks build modules, modules build panels,
//! a layout places panels; a panel is written once and works wherever a layout puts it).
//! It holds the transport buttons the layout lists (Play, IN, OUT ...) and, always, the
//! monitor's Auto | On switch (one choice for every monitor, `qnc_monitor::auto`), in its
//! own place of the board, so a layout may put it anywhere. A click is a catalog action
//! that the shared source panel keys carry out (`qnc-source-panel`).
//!
//! Passive: it keeps no state, plays nothing and reads no database.

use eframe::egui::{Color32, Rect, Ui};
use qnc_media_pool_head::{RowClick, RowCommand, RowStyle};
use qnc_monitor::{MONITOR_AUTO, MONITOR_ON};
use qnc_source_dock::TimelineDockStyle;

pub const MODULE_ID: &str = "qnc.module.transport-panel";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// One button of the panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control {
    pub label: String,
    pub action_id: String,
    pub on: bool,
}

/// The look of the panel, from the form's theme.
#[derive(Clone, Copy)]
pub struct TransportLook<'a> {
    pub row: &'a RowStyle,
    pub chrome: &'a TimelineDockStyle,
    pub fill: Color32,
}

/// The buttons: the ones the layout lists (label, action), then the Auto | On switch,
/// with the one that is on.
pub fn controls<'a>(listed: impl IntoIterator<Item = (&'a str, &'a str)>) -> Vec<Control> {
    let mut controls: Vec<Control> = listed
        .into_iter()
        .filter(|(_, action)| !matches!(*action, MONITOR_AUTO | MONITOR_ON))
        .map(|(label, action)| Control { label: label.to_string(), action_id: action.to_string(), on: false })
        .collect();
    for (action, key) in [(MONITOR_AUTO, "monitor.auto"), (MONITOR_ON, "monitor.on")] {
        let on = qnc_monitor::switch_on(action, qnc_monitor::auto());
        controls.push(Control { label: qnc_ui_text::text(key).to_string(), action_id: action.to_string(), on });
    }
    controls
}

/// The panel in `rect`; the clicked button's catalog action.
pub fn show<'a>(ui: &mut Ui, rect: Rect, look: TransportLook<'_>, listed: impl IntoIterator<Item = (&'a str, &'a str)>) -> Option<String> {
    let controls = controls(listed);
    let commands: Vec<_> = controls.iter().map(|control| RowCommand { label: &control.label, on: control.on }).collect();
    let mut clicked = None;
    qnc_source_dock::show_chrome_row(ui, rect, look.chrome, look.fill, true, |ui| {
        if let Some(RowClick::Command(index)) = qnc_media_pool_head::show_row(ui, look.row, &[], &commands) {
            clicked = Some(controls[index].action_id.clone());
        }
    });
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_switch_follows_the_listed_buttons_once_and_one_choice_serves_every_panel() {
        let listed = [(">", "play_pause"), ("Auto", MONITOR_AUTO)];
        assert!(qnc_monitor::choose(MONITOR_AUTO));
        let shown: Vec<_> = controls(listed).into_iter().map(|control| (control.action_id, control.on)).collect();
        let expected = [("play_pause", false), (MONITOR_AUTO, true), (MONITOR_ON, false)].map(|(action, on)| (action.to_string(), on));
        assert_eq!(shown, expected);
        assert!(qnc_monitor::choose(MONITOR_ON));
        assert!(controls(listed).last().is_some_and(|control| control.on), "On is on everywhere");
        assert!(qnc_monitor::choose(MONITOR_AUTO));
    }
}
