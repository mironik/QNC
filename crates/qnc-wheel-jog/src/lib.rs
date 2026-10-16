//! The mouse wheel over a panel as catalog actions (user 2026-10-16: the wheel does what
//! the left/right arrows do, by the panel under the pointer). A mouse notch is one step;
//! a touchpad's fine scrolling is added up, a step every `POINTS_PER_STEP`. A fast spin
//! gives one action a frame and the rest in the next frames, so the player gets its steps
//! in order. Over the panel the scroll is taken, so a list under it does not move.
//! No form state, no database, no player.

use eframe::egui::{self, Rect, Ui};

pub const MODULE_ID: &str = "qnc.module.wheel-jog";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The setting of this computer beside its others (`data/mouse-wheel.json`).
pub const SETTING_FILE: &str = "mouse-wheel.json";

/// Whether the wheel away from the user moves the playhead forward: the choice of this
/// computer (`{"playhead_up": "forward"}` or `"back"` in `SETTING_FILE`, user 2026-10-16:
/// editors come with either habit), forward without it. A level is not turned by it: up
/// is louder everywhere. Read once a run.
pub fn playhead_up_forward() -> bool {
    static CHOSEN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *CHOSEN.get_or_init(|| {
        let Some(path) = qnc_dev_diagnostics::locate_qnc_root().map(|root| qnc_dev_diagnostics::config_path(&root).with_file_name(SETTING_FILE)) else {
            return true;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return true; // not chosen: forward
        };
        match parse_setting(&text) {
            Ok(forward) => forward,
            Err(error) => {
                qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("wheel-setting {}: {error}; forward", path.display()));
                true
            }
        }
    })
}

/// `{"playhead_up": "forward" | "back"}`.
fn parse_setting(text: &str) -> Result<bool, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    match value.get("playhead_up").and_then(serde_json::Value::as_str) {
        Some("forward") => Ok(true),
        Some("back") => Ok(false),
        other => Err(format!("playhead_up must be \"forward\" or \"back\", not {other:?}")),
    }
}

/// Touchpad points of one step.
pub const POINTS_PER_STEP: f32 = 40.0;

/// Steps not yet given and the touchpad points not yet a step, kept by the panel's id.
#[derive(Clone, Copy, Default)]
struct Pending {
    steps: i32,
    points: f32,
    /// The steps came with Ctrl held (Ctrl+wheel, like Ctrl+arrows).
    ctrl: bool,
}

/// The wheel steps of one frame from its scroll events: notches (`Line`, `Page`) count
/// one each, points add up. Up (away from the user) and right are forward (user
/// 2026-10-16: up moves the playhead on, raises a level).
fn steps_of(events: &[egui::Event], points: &mut f32) -> i32 {
    let mut steps = 0;
    for event in events {
        let egui::Event::MouseWheel { unit, delta, .. } = event else { continue };
        let amount = delta.y + delta.x;
        match unit {
            egui::MouseWheelUnit::Point => *points += amount,
            _ => steps += amount.signum() as i32 * amount.abs().round().max(1.0) as i32,
        }
    }
    let whole = (*points / POINTS_PER_STEP).trunc();
    *points -= whole * POINTS_PER_STEP;
    steps + whole as i32
}

/// Whether this frame's wheel events came with Ctrl (Cmd on macOS) held.
fn with_ctrl(events: &[egui::Event]) -> Option<bool> {
    events.iter().rev().find_map(|event| match event {
        egui::Event::MouseWheel { modifiers, .. } => Some(modifiers.command || modifiers.ctrl),
        _ => None,
    })
}

/// The catalog actions of a panel's wheel: back and forward, then with Ctrl held.
pub type WheelActions<'a> = ((&'a str, &'a str), (&'a str, &'a str));

/// The catalog action of this frame's wheel over `rect` (back or forward, the Ctrl pair
/// when Ctrl is held), when the wheel steps there now (`enabled`); `None` otherwise.
/// Over the panel the scroll is taken. `id` names the panel's pending steps.
pub fn action(ui: &Ui, rect: Rect, enabled: bool, id: &str, ((back, forward), (ctrl_back, ctrl_forward)): WheelActions<'_>) -> Option<String> {
    let key = egui::Id::new(("qnc-wheel-jog", id));
    let ctx = ui.ctx();
    let mut pending: Pending = ctx.data(|data| data.get_temp(key)).unwrap_or_default();
    let over = enabled && ctx.pointer_hover_pos().is_some_and(|at| rect.contains(at));
    if over {
        let events = ctx.input(|input| input.events.clone());
        if let Some(ctrl) = with_ctrl(&events).filter(|ctrl| *ctrl != pending.ctrl) {
            pending = Pending { ctrl, ..Pending::default() }; // Ctrl pressed or let go: start again
        }
        pending.steps += steps_of(&events, &mut pending.points);
        ctx.input_mut(|input| {
            input.smooth_scroll_delta = egui::Vec2::ZERO;
            input.raw_scroll_delta = egui::Vec2::ZERO;
        });
    } else {
        pending = Pending::default(); // the pointer left: nothing carries over
    }
    let step = pending.steps.signum();
    pending.steps -= step;
    ctx.data_mut(|data| data.insert_temp(key, pending));
    if pending.steps != 0 {
        ctx.request_repaint(); // the rest of a fast spin, one a frame
    }
    match (step, pending.ctrl) {
        (-1, false) => Some(back.to_string()),
        (1, false) => Some(forward.to_string()),
        (-1, true) => Some(ctrl_back.to_string()),
        (1, true) => Some(ctrl_forward.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wheel(unit: egui::MouseWheelUnit, y: f32) -> egui::Event {
        egui::Event::MouseWheel { unit, delta: egui::vec2(0.0, y), modifiers: egui::Modifiers::NONE }
    }

    #[test]
    fn a_notch_is_one_step_up_is_forward() {
        let mut points = 0.0;
        assert_eq!(steps_of(&[wheel(egui::MouseWheelUnit::Line, 1.0)], &mut points), 1);
        assert_eq!(steps_of(&[wheel(egui::MouseWheelUnit::Line, -1.0), wheel(egui::MouseWheelUnit::Line, -1.0)], &mut points), -2);
    }

    #[test]
    fn the_computer_chooses_the_playhead_direction() {
        assert_eq!(parse_setting(r#"{"playhead_up": "forward"}"#), Ok(true));
        assert_eq!(parse_setting(r#"{"playhead_up": "back"}"#), Ok(false));
        assert!(parse_setting(r#"{"playhead_up": "up"}"#).is_err(), "no guess");
    }

    #[test]
    fn a_touchpad_adds_up_its_points() {
        let mut points = 0.0;
        assert_eq!(steps_of(&[wheel(egui::MouseWheelUnit::Point, 15.0)], &mut points), 0);
        assert_eq!(steps_of(&[wheel(egui::MouseWheelUnit::Point, 30.0)], &mut points), 1);
        assert!((points - 5.0).abs() < 0.01, "the rest is kept");
    }
}
