//! Gestures of two or more fingers on a touch screen as panel actions (user 2026-10-18:
//! gestures are a block with their own map). A drag across or along and a pinch each do the
//! role the touch map gives them (`contracts/input/touch.json`; roles and steps: piece
//! `qnc-gesture-steps`): `jog` the panel's wheel (its left/right arrow), `level` Ctrl+wheel
//! (the level of a taken sound), `none` nothing. One finger is the pointer: a tap is a click
//! and a drag a drag, as with the mouse. No form state, no database, no player.

use eframe::egui::{self, Rect, Ui};
use qnc_gesture_steps::{Gesture, Role, Steps};
#[cfg(test)]
use qnc_wheel_jog::Wheel;

pub const MODULE_ID: &str = "qnc.module.touch-gestures";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The map's place under the QNC root.
pub const MAP_FILE: &str = "contracts/input/touch.json";

/// The touch map.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchMap {
    pub drag_horizontal: Role,
    pub drag_vertical: Role,
    pub pinch: Role,
    /// Whether a quick tap of two fingers in place is a cut (scissors) at the playhead
    /// (`"cut"`) or nothing (`"none"`).
    pub two_finger_tap_cuts: bool,
    /// Points of a drag of one step.
    pub points_per_step: f32,
    /// The zoom factor of one pinch step.
    pub zoom_per_step: f32,
}

impl TouchMap {
    pub fn parse(text: &str) -> Result<Self, String> {
        let map: serde_json::Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
        Ok(Self {
            drag_horizontal: qnc_gesture_steps::role(&map, "drag_horizontal")?,
            drag_vertical: qnc_gesture_steps::role(&map, "drag_vertical")?,
            pinch: qnc_gesture_steps::role(&map, "pinch")?,
            two_finger_tap_cuts: match map.get("two_finger_tap").and_then(serde_json::Value::as_str) {
                Some("cut") => true,
                Some("none") => false,
                other => return Err(format!("two_finger_tap must be \"cut\" or \"none\", not {other:?}")),
            },
            points_per_step: qnc_gesture_steps::amount(&map, "points_per_step", 0.0)?,
            zoom_per_step: qnc_gesture_steps::amount(&map, "zoom_per_step", 1.0)?,
        })
    }

    /// The map of this QNC, read once a run; a missing or invalid map is said in the player
    /// log and the gestures do nothing (no invented map).
    pub fn of_this_qnc() -> Option<Self> {
        static MAP: std::sync::OnceLock<Option<TouchMap>> = std::sync::OnceLock::new();
        *MAP.get_or_init(|| qnc_gesture_steps::read_map(MAP_FILE, "touch", Self::parse))
    }
}

/// Adds one frame of a gesture: the move of the fingers' centre and the pinch factor. Right,
/// up and a spread are forward.
fn add(map: &TouchMap, translation: egui::Vec2, zoom: f32, steps: &mut Steps) {
    steps.add(map.drag_horizontal, translation.x / map.points_per_step);
    steps.add(map.drag_vertical, -translation.y / map.points_per_step); // the screen's y grows down
    if zoom > 0.0 {
        steps.add(map.pinch, zoom.ln() / map.zoom_per_step.ln());
    }
}

/// A gesture of two or more fingers over `rect` this frame: one step of the panel's wheel
/// (`ctrl`: Ctrl+wheel), the rest in the next frames, in order.
pub fn take(ui: &Ui, rect: Rect, key: egui::Id) -> Option<Gesture> {
    let map = TouchMap::of_this_qnc()?;
    let ctx = ui.ctx();
    let gesture = ctx.multi_touch().filter(|touch| rect.contains(touch.center_pos));
    let mut steps: Steps = ctx.data(|data| data.get_temp(key)).unwrap_or_default();
    match gesture {
        Some(touch) => add(&map, touch.translation_delta, touch.zoom_delta, &mut steps),
        None if !steps.waiting() => steps = Steps::default(), // the fingers left: no rest carries over
        None => {}
    }
    let next = steps.next();
    ctx.data_mut(|data| data.insert_temp(key, steps));
    if steps.waiting() {
        ctx.request_repaint();
    }
    next
}

/// How long and how far two fingers may stay for a tap.
const TAP_SECONDS: f64 = 0.35;
const TAP_POINTS: f32 = 10.0;

/// The two fingers of a possible tap: when they came and how far they moved.
#[derive(Debug, Clone, Copy, Default)]
struct Tap {
    start: f64,
    moved: f32,
    zoom: f32,
    on: bool,
}

/// Whether two fingers were lifted this frame after a quick tap in place over `rect`, when
/// the map makes that a cut (user 2026-10-18: scissors by gesture).
pub fn tapped(ui: &Ui, rect: Rect, key: egui::Id) -> bool {
    if !TouchMap::of_this_qnc().is_some_and(|map| map.two_finger_tap_cuts) {
        return false;
    }
    let ctx = ui.ctx();
    let now = ctx.input(|input| input.time);
    let mut tap: Tap = ctx.data(|data| data.get_temp(key)).unwrap_or_default();
    let ended = match ctx.multi_touch().filter(|touch| touch.num_touches == 2 && rect.contains(touch.center_pos)) {
        Some(touch) => {
            if !tap.on {
                tap = Tap { start: now, on: true, zoom: 1.0, ..Tap::default() };
            }
            tap.moved += touch.translation_delta.length();
            tap.zoom *= touch.zoom_delta;
            false
        }
        None => std::mem::take(&mut tap.on),
    };
    let cut = ended && is_tap(now - tap.start, tap.moved, tap.zoom);
    ctx.data_mut(|data| data.insert_temp(key, tap));
    cut
}

/// A tap: quick, in place, no pinch.
fn is_tap(seconds: f64, moved: f32, zoom: f32) -> bool {
    seconds <= TAP_SECONDS && moved <= TAP_POINTS && (zoom - 1.0).abs() < 0.05
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> TouchMap {
        TouchMap::parse(include_str!("../../../contracts/input/touch.json")).expect("the map of the repository")
    }

    #[test]
    fn a_two_finger_drag_across_jogs_and_a_spread_raises_the_level() {
        let (map, mut steps) = (map(), Steps::default());
        add(&map, egui::vec2(30.0, 0.0), 1.0, &mut steps);
        assert_eq!(std::iter::from_fn(|| steps.next()).count(), 2, "30 points across at 12 a step");
        add(&map, egui::vec2(0.0, -100.0), 1.0, &mut steps);
        assert_eq!(steps.next(), None, "along does nothing in this map");
        add(&map, egui::Vec2::ZERO, 1.1 * 1.001, &mut steps);
        assert_eq!(steps.next(), Some(Gesture::Wheel(Wheel { up: true, ctrl: true })));
        assert!(map.two_finger_tap_cuts);
        assert!(is_tap(0.2, 3.0, 1.0), "quick and in place: a cut");
        assert!(!is_tap(0.8, 3.0, 1.0), "held: no cut");
        assert!(!is_tap(0.2, 40.0, 1.0), "a drag: no cut");
    }
}
