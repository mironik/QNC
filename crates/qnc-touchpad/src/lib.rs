//! The laptop touchpad as panel actions (user 2026-10-18: every input device is a block with
//! its own map, so that the keyboard, the mouse, the touchpad and later consoles all drive
//! QNC through system actions). A two-finger swipe across or along, and a pinch, each do
//! the role the touchpad map gives them (`contracts/input/touchpad.json`; roles, steps and
//! map reading: piece `qnc-gesture-steps`): `jog` is the panel's wheel (its left/right
//! arrow), `level` Ctrl+wheel (the level of a taken sound), `none` nothing. The fine
//! movement is added up, a step every `*_per_step`.
//!
//! What the system gives: on Windows a swipe arrives as fine wheel steps (fractions of a
//! notch) and a pinch as those with Ctrl; on macOS a swipe arrives in points and a pinch as
//! its own zoom gesture. A mouse wheel's whole notches are the wheel block's
//! (`qnc-wheel-jog`), never this one's. No form state, no database, no player.

use eframe::egui::{self, Rect, Ui};
use qnc_gesture_steps::{Role, Steps};
use qnc_wheel_jog::Wheel;

pub const MODULE_ID: &str = "qnc.module.touchpad";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The map's place under the QNC root.
pub const MAP_FILE: &str = "contracts/input/touchpad.json";

/// The touchpad map.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchpadMap {
    pub swipe_horizontal: Role,
    pub swipe_vertical: Role,
    pub pinch: Role,
    /// Fine wheel steps (Windows) of one step.
    pub lines_per_step: f32,
    /// Points (macOS) of one step.
    pub points_per_step: f32,
    /// The zoom factor of one pinch step (macOS).
    pub zoom_per_step: f32,
}

impl TouchpadMap {
    pub fn parse(text: &str) -> Result<Self, String> {
        let map: serde_json::Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
        Ok(Self {
            swipe_horizontal: qnc_gesture_steps::role(&map, "swipe_horizontal")?,
            swipe_vertical: qnc_gesture_steps::role(&map, "swipe_vertical")?,
            pinch: qnc_gesture_steps::role(&map, "pinch")?,
            lines_per_step: qnc_gesture_steps::amount(&map, "lines_per_step", 0.0)?,
            points_per_step: qnc_gesture_steps::amount(&map, "points_per_step", 0.0)?,
            zoom_per_step: qnc_gesture_steps::amount(&map, "zoom_per_step", 1.0)?,
        })
    }

    /// The map of this QNC, read once a run (no map: the touchpad does nothing).
    pub fn of_this_qnc() -> Option<Self> {
        static MAP: std::sync::OnceLock<Option<TouchpadMap>> = std::sync::OnceLock::new();
        *MAP.get_or_init(|| qnc_gesture_steps::read_map(MAP_FILE, "touchpad", Self::parse))
    }
}

/// Adds this frame's touchpad movement to `steps`. Up (away from the user), right and a
/// pinch out are forward.
fn add(map: &TouchpadMap, events: &[egui::Event], steps: &mut Steps) {
    for event in events {
        match event {
            egui::Event::MouseWheel { unit, delta, modifiers } if !qnc_wheel_jog::is_notch(*unit, *delta) => {
                let per_step = if *unit == egui::MouseWheelUnit::Point { map.points_per_step } else { map.lines_per_step };
                // Windows: a pinch is the fine wheel with Ctrl.
                if modifiers.ctrl || modifiers.command {
                    steps.add(map.pinch, (delta.x + delta.y) / per_step);
                } else {
                    steps.add(map.swipe_horizontal, delta.x / per_step);
                    steps.add(map.swipe_vertical, delta.y / per_step);
                }
            }
            egui::Event::Zoom(factor) if *factor > 0.0 => steps.add(map.pinch, factor.ln() / map.zoom_per_step.ln()),
            _ => {}
        }
    }
}

/// Whether an event is the touchpad's.
fn touchpad_event(event: &egui::Event) -> bool {
    match event {
        egui::Event::MouseWheel { unit, delta, .. } => !qnc_wheel_jog::is_notch(*unit, *delta),
        egui::Event::Zoom(_) => true,
        _ => false,
    }
}

/// The touchpad over `rect` this frame: one step of the panel's wheel (`ctrl`: Ctrl+wheel),
/// the rest in the next frames, in order. Over the panel the scroll is taken, so a list under
/// it does not move.
pub fn take(ui: &Ui, rect: Rect, key: egui::Id) -> Option<Wheel> {
    let map = TouchpadMap::of_this_qnc()?;
    let ctx = ui.ctx();
    let over = ctx.pointer_hover_pos().is_some_and(|at| rect.contains(at));
    let mut steps: Steps = if over { ctx.data(|data| data.get_temp(key)).unwrap_or_default() } else { Steps::default() };
    if over {
        let events = ctx.input(|input| input.events.clone());
        add(&map, &events, &mut steps);
        if events.iter().any(touchpad_event) {
            ctx.input_mut(|input| {
                input.smooth_scroll_delta = egui::Vec2::ZERO;
                input.raw_scroll_delta = egui::Vec2::ZERO;
            });
        }
    }
    let next = steps.next();
    ctx.data_mut(|data| data.insert_temp(key, steps));
    if steps.waiting() {
        ctx.request_repaint();
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> TouchpadMap {
        TouchpadMap::parse(include_str!("../../../contracts/input/touchpad.json")).expect("the map of the repository")
    }

    fn fine(x: f32, y: f32, ctrl: bool) -> egui::Event {
        let modifiers = if ctrl { egui::Modifiers::CTRL } else { egui::Modifiers::NONE };
        egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: egui::vec2(x, y), modifiers }
    }

    fn all(steps: &mut Steps) -> Vec<Wheel> {
        std::iter::from_fn(|| steps.next()).collect()
    }

    #[test]
    fn a_windows_swipe_adds_up_fractions_of_a_notch_to_steps() {
        let mut steps = Steps::default();
        add(&map(), &[fine(0.0, 0.4, false), fine(0.0, 0.4, false)], &mut steps);
        assert!(all(&mut steps).is_empty(), "less than a notch: no step yet");
        add(&map(), &[fine(0.0, 0.4, false)], &mut steps);
        assert_eq!(all(&mut steps), vec![Wheel { up: true, ctrl: false }]);
        add(&map(), &[fine(-2.5, 0.0, false)], &mut steps);
        assert_eq!(all(&mut steps).len(), 2, "across, back: two steps");
    }

    #[test]
    fn a_pinch_is_the_level_and_a_whole_notch_is_the_mouse_wheels() {
        let mut steps = Steps::default();
        add(&map(), &[fine(0.0, 0.5, true), fine(0.0, 0.6, true)], &mut steps);
        assert_eq!(all(&mut steps), vec![Wheel { up: true, ctrl: true }], "Windows: the fine wheel with Ctrl is a pinch");
        add(&map(), &[egui::Event::Zoom(1.1f32.powi(2) * 1.001)], &mut steps);
        assert_eq!(all(&mut steps).len(), 2, "macOS: two zoom steps out");
        let notch = egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: egui::vec2(0.0, 1.0), modifiers: egui::Modifiers::NONE };
        add(&map(), &[notch], &mut steps);
        assert!(all(&mut steps).is_empty(), "a mouse notch is not the touchpad's");
    }

    #[test]
    fn a_map_with_an_unknown_role_is_refused() {
        let text = include_str!("../../../contracts/input/touchpad.json").replace("\"pinch\": \"level\"", "\"pinch\": \"zoom\"");
        assert!(TouchpadMap::parse(&text).is_err());
    }
}
