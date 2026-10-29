//! The transport panel (user 2026-10-19: blocks build modules, modules build panels,
//! a layout places panels; a panel is written once and works wherever a layout puts it,
//! no code is written again). Its blocks are its own and the same everywhere: Play, IN,
//! OUT and the monitor's Auto | On switch (one choice for every monitor,
//! `qnc_monitor::auto`). A layout only gives it a place. A click is a catalog action that
//! the shared source panel keys carry out (`qnc-source-panel`).
//!
//! Passive: it keeps no state, plays nothing and reads no database.

use eframe::egui::{Rect, Ui};
use qnc_media_pool_head::{RowClick, RowCommand};
use qnc_monitor::{MONITOR_AUTO, MONITOR_ON};

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

/// The panel's blocks, left to right: (catalog action, label key).
const BLOCKS: [(&str, &str); 5] = [
    ("play_pause", "transport.play"),
    ("mark_in", "transport.mark_in"),
    ("mark_out", "transport.mark_out"),
    (MONITOR_AUTO, "monitor.auto"),
    (MONITOR_ON, "monitor.on"),
];

/// The buttons, with the one of Auto | On that is on.
pub fn controls() -> Vec<Control> {
    BLOCKS
        .iter()
        .map(|(action, key)| Control {
            label: qnc_ui_text::text(key).to_string(),
            action_id: action.to_string(),
            on: qnc_monitor::switch_on(action, qnc_monitor::auto()),
        })
        .collect()
}

/// The panel in `rect`; the clicked button's catalog action.
/// The panel in `rect`, in the look of the one theme (`tab_gap` is the form's measure).
pub fn show(ui: &mut Ui, rect: Rect, theme: &qnc_desktop_theme::Theme, tab_gap: f32) -> Option<String> {
    let (row, chrome) = (qnc_theme_styles::row(theme, tab_gap), qnc_theme_styles::dock(theme, 0.0));
    let controls = controls();
    let commands: Vec<_> = controls.iter().map(|control| RowCommand { label: &control.label, on: control.on }).collect();
    let mut clicked = None;
    qnc_source_dock::show_chrome_row(ui, rect, &chrome, theme.surface, true, |ui| {
        if let Some(RowClick::Command(index)) = qnc_media_pool_head::show_row(ui, &row, &[], &commands) {
            clicked = Some(controls[index].action_id.clone());
        }
    });
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_has_its_own_blocks_and_one_choice_serves_every_panel() {
        assert!(qnc_monitor::choose(MONITOR_AUTO));
        let shown: Vec<_> = controls().into_iter().map(|control| (control.action_id, control.on)).collect();
        let expected = [("play_pause", false), ("mark_in", false), ("mark_out", false), (MONITOR_AUTO, true), (MONITOR_ON, false)]
            .map(|(action, on)| (action.to_string(), on));
        assert_eq!(shown, expected);
        assert!(qnc_monitor::choose(MONITOR_ON));
        assert!(controls().last().is_some_and(|control| control.on), "On is on everywhere");
        assert!(qnc_monitor::choose(MONITOR_AUTO));
    }
}
