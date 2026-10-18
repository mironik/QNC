//! The mouse wheel over a panel as catalog actions (user 2026-10-16: the wheel does what
//! the left/right arrows do, by the panel under the pointer). A mouse notch is one step;
//! a touchpad's fine scrolling is the touchpad block's (`qnc-touchpad`, user 2026-10-18:
//! every device its own block). A fast spin
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

/// Whether a scroll is a mouse wheel's whole notch (or page); a fine one (points, or a
/// fraction of a notch as Windows gives a touchpad) is the touchpad's.
pub fn is_notch(unit: egui::MouseWheelUnit, delta: egui::Vec2) -> bool {
    match unit {
        egui::MouseWheelUnit::Point => false,
        egui::MouseWheelUnit::Page => true,
        egui::MouseWheelUnit::Line => delta.x.fract() == 0.0 && delta.y.fract() == 0.0 && delta != egui::Vec2::ZERO,
    }
}

/// Steps not yet given, kept by the panel's id.
#[derive(Clone, Copy, Default)]
struct Pending {
    steps: i32,
    /// The steps came with Ctrl held (Ctrl+wheel, like Ctrl+arrows).
    ctrl: bool,
}

/// The wheel steps of one frame from its scroll events: a notch counts one step. Up (away
/// from the user) and right are forward (user 2026-10-16: up moves the playhead on, raises a
/// level). Fine scrolling is the touchpad block's.
fn steps_of(events: &[egui::Event]) -> i32 {
    let mut steps = 0;
    for event in events {
        let egui::Event::MouseWheel { unit, delta, .. } = event else { continue };
        if is_notch(*unit, *delta) {
            let amount = delta.y + delta.x;
            steps += amount.signum() as i32 * amount.abs().round().max(1.0) as i32;
        }
    }
    steps
}

/// Whether this frame's wheel notches came with Ctrl (Cmd on macOS) held.
fn with_ctrl(events: &[egui::Event]) -> Option<bool> {
    events.iter().rev().find_map(|event| match event {
        egui::Event::MouseWheel { unit, delta, modifiers } if is_notch(*unit, *delta) => Some(modifiers.command || modifiers.ctrl),
        _ => None,
    })
}

fn right_wheel_key() -> egui::Id {
    egui::Id::new("qnc_wheel_jog_right_wheel")
}

/// The wheel was turned while the right mouse button was held: letting that button go is
/// not a right click (it was Ctrl for the wheel). Asked when a right click comes; the next
/// press of the right button starts over.
pub fn wheeled_with_right(ctx: &egui::Context) -> bool {
    ctx.data(|data| data.get_temp::<bool>(right_wheel_key())).unwrap_or(false)
}

/// The catalog actions of a panel's wheel: back and forward, then with Ctrl held.
pub type WheelActions<'a> = ((&'a str, &'a str), (&'a str, &'a str));

/// The wheel over a source timeline or its monitor, in every form (user 2026-10-16: the
/// source timeline behaves the same everywhere).
pub const SOURCE_ACTIONS: WheelActions<'static> =
    (("wheel_source_back", "wheel_source_forward"), ("wheel_source_ctrl_back", "wheel_source_ctrl_forward"));

/// The wheel over the segment panel's Wrap rows, as over the source timeline.
pub const SEGMENTS_ACTIONS: WheelActions<'static> =
    (("wheel_segments_back", "wheel_segments_forward"), ("wheel_segments_ctrl_back", "wheel_segments_ctrl_forward"));

/// Whether a wheel action of `SOURCE_ACTIONS` moves the playhead forward, after this
/// computer's choice of direction; `None` for the Ctrl pair or another action.
pub fn source_step_forward(action_id: &str) -> Option<bool> {
    let ((back, forward), _) = SOURCE_ACTIONS;
    match action_id {
        id if id == back => Some(!playhead_up_forward()),
        id if id == forward => Some(playhead_up_forward()),
        _ => None,
    }
}

/// One wheel step: away from the user (`up`) or toward, with Ctrl (Cmd) held or not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wheel {
    pub up: bool,
    pub ctrl: bool,
}

impl Wheel {
    /// Its catalog action: forward for up, the Ctrl pair with Ctrl.
    pub fn action<'a>(self, ((back, forward), (ctrl_back, ctrl_forward)): WheelActions<'a>) -> &'a str {
        match (self.ctrl, self.up) {
            (false, false) => back,
            (false, true) => forward,
            (true, false) => ctrl_back,
            (true, true) => ctrl_forward,
        }
    }
}

/// This frame's wheel step over `rect` (one a frame; the rest of a fast spin follows in
/// the next frames); the scroll over it is taken. `key` names its pending steps.
pub fn take(ui: &Ui, rect: Rect, key: egui::Id) -> Option<Wheel> {
    let ctx = ui.ctx();
    if ctx.input(|input| input.pointer.secondary_pressed()) {
        ctx.data_mut(|data| data.remove::<bool>(right_wheel_key()));
    }
    let mut pending: Pending = ctx.data(|data| data.get_temp(key)).unwrap_or_default();
    let over = ctx.pointer_hover_pos().is_some_and(|at| rect.contains(at));
    if over {
        let events = ctx.input(|input| input.events.clone());
        // The right mouse button is Ctrl (user 2026-10-17): the wheel turned with it held is
        // Ctrl+wheel, the level of a taken sound.
        let right = ctx.input(|input| input.pointer.secondary_down());
        let ctrl = with_ctrl(&events).map(|ctrl| ctrl || right);
        if right && ctrl.is_some() {
            ctx.data_mut(|data| data.insert_temp(right_wheel_key(), true));
        }
        if let Some(ctrl) = ctrl.filter(|ctrl| *ctrl != pending.ctrl) {
            pending = Pending { ctrl, ..Pending::default() }; // Ctrl pressed or let go: start again
        }
        pending.steps += steps_of(&events);
        if ctrl.is_some() {
            ctx.input_mut(|input| {
                input.smooth_scroll_delta = egui::Vec2::ZERO;
                input.raw_scroll_delta = egui::Vec2::ZERO;
            });
        }
    } else {
        pending = Pending::default(); // the pointer left: nothing carries over
    }
    let step = pending.steps.signum();
    pending.steps -= step;
    ctx.data_mut(|data| data.insert_temp(key, pending));
    if pending.steps != 0 {
        ctx.request_repaint(); // the rest of a fast spin, one a frame
    }
    (step != 0).then_some(Wheel { up: step > 0, ctrl: pending.ctrl })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wheel(unit: egui::MouseWheelUnit, y: f32) -> egui::Event {
        egui::Event::MouseWheel { unit, delta: egui::vec2(0.0, y), modifiers: egui::Modifiers::NONE }
    }

    #[test]
    fn a_notch_is_one_step_up_is_forward() {
        assert_eq!(steps_of(&[wheel(egui::MouseWheelUnit::Line, 1.0)]), 1);
        assert_eq!(steps_of(&[wheel(egui::MouseWheelUnit::Line, -1.0), wheel(egui::MouseWheelUnit::Line, -1.0)]), -2);
    }

    #[test]
    fn the_computer_chooses_the_playhead_direction() {
        assert_eq!(parse_setting(r#"{"playhead_up": "forward"}"#), Ok(true));
        assert_eq!(parse_setting(r#"{"playhead_up": "back"}"#), Ok(false));
        assert!(parse_setting(r#"{"playhead_up": "up"}"#).is_err(), "no guess");
    }

    #[test]
    fn fine_scrolling_is_the_touchpads_not_the_wheels() {
        assert_eq!(steps_of(&[wheel(egui::MouseWheelUnit::Point, 45.0)]), 0, "points: a touchpad");
        assert_eq!(steps_of(&[wheel(egui::MouseWheelUnit::Line, 0.4)]), 0, "a fraction of a notch: a Windows touchpad");
        assert!(is_notch(egui::MouseWheelUnit::Line, egui::vec2(0.0, -2.0)));
    }
}
