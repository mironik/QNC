//! What a gesture does over a panel and how its fine movement becomes steps, the same for
//! every gesture device (the touchpad, the touch screen; user 2026-10-18: one block, no
//! copies). A role comes from a device's map: `jog` is the panel's wheel (its left/right
//! arrow), `level` Ctrl+wheel (the level of a taken A1/A2 sound), `none` nothing. The
//! movement is added up by role, a whole step at a time, one step a frame in order.

use qnc_wheel_jog::Wheel;

pub const MODULE_ID: &str = "qnc.module.gesture-steps";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What a gesture does over a panel. `Undo`: undo (forward) or redo (back) of the story, a step
/// at a time (user 2026-10-29: "left-right for the markers, up-down for undo-redo").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Jog,
    Level,
    Undo,
    None,
}

/// One step of a gesture: the panel's wheel (or Ctrl+wheel), or undo (`redo`: redo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    Wheel(Wheel),
    Undo { redo: bool },
}

impl Gesture {
    /// The wheel step, `None` for an undo.
    pub fn wheel(self) -> Option<Wheel> {
        match self {
            Self::Wheel(wheel) => Some(wheel),
            Self::Undo { .. } => None,
        }
    }

    /// The undo step (`true`: redo), `None` for a wheel.
    pub fn undo(self) -> Option<bool> {
        match self {
            Self::Undo { redo } => Some(redo),
            Self::Wheel(_) => None,
        }
    }
}

/// The role `key` of a device map.
pub fn role(map: &serde_json::Value, key: &str) -> Result<Role, String> {
    match map.get(key).and_then(serde_json::Value::as_str) {
        Some("jog") => Ok(Role::Jog),
        Some("level") => Ok(Role::Level),
        Some("undo") => Ok(Role::Undo),
        Some("none") => Ok(Role::None),
        other => Err(format!("{key} must be \"jog\", \"level\", \"undo\" or \"none\", not {other:?}")),
    }
}

/// The amount `key` of a device map, above `above`.
pub fn amount(map: &serde_json::Value, key: &str, above: f32) -> Result<f32, String> {
    match map.get(key).and_then(serde_json::Value::as_f64) {
        Some(amount) if amount as f32 > above => Ok(amount as f32),
        other => Err(format!("{key} must be a number above {above}, not {other:?}")),
    }
}

/// A device map under the QNC root (`file`), parsed; a missing or invalid map is said in the
/// player log (`device` names it) and the device does nothing (no invented map).
pub fn read_map<T>(file: &str, device: &str, parse: impl FnOnce(&str) -> Result<T, String>) -> Option<T> {
    let read = qnc_dev_diagnostics::locate_qnc_root()
        .ok_or_else(|| "no QNC root".to_string())
        .and_then(|root| std::fs::read_to_string(root.join(file)).map_err(|error| format!("{file}: {error}")))
        .and_then(|text| parse(&text));
    read.map_err(|error| qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("{device} map: {error}; the {device} does nothing")))
        .ok()
}

/// The movement of one panel not yet given, by role.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Steps {
    jog: f32,
    level: f32,
    undo: f32,
    whole: [i32; 3],
}

impl Steps {
    /// Movement of `steps` (fractions kept) in `role`; up, right and spread are forward.
    pub fn add(&mut self, role: Role, steps: f32) {
        let (rest, whole) = match role {
            Role::Jog => (&mut self.jog, &mut self.whole[0]),
            Role::Level => (&mut self.level, &mut self.whole[1]),
            Role::Undo => (&mut self.undo, &mut self.whole[2]),
            Role::None => return,
        };
        *rest += steps;
        let done = rest.trunc();
        *rest -= done;
        *whole += done as i32;
    }

    /// The next step to give, the jog first, then the level, then undo: one a frame.
    pub fn next(&mut self) -> Option<Gesture> {
        let index = (0..3).find(|index| self.whole[*index] != 0)?;
        let step = self.whole[index].signum();
        self.whole[index] -= step;
        Some(match index {
            2 => Gesture::Undo { redo: step < 0 },
            index => Gesture::Wheel(Wheel { up: step > 0, ctrl: index == 1 }),
        })
    }

    /// Whether whole steps wait for the next frames.
    pub fn waiting(&self) -> bool {
        self.whole != [0, 0, 0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fine_movement_becomes_whole_steps_given_one_at_a_time_the_jog_first() {
        let mut steps = Steps::default();
        steps.add(Role::Jog, 0.6);
        assert_eq!(steps.next(), None, "not a step yet");
        steps.add(Role::Jog, 1.5);
        steps.add(Role::Level, -1.0);
        steps.add(Role::None, 9.0);
        assert_eq!(steps.next(), Some(Gesture::Wheel(Wheel { up: true, ctrl: false })));
        assert_eq!(steps.next(), Some(Gesture::Wheel(Wheel { up: true, ctrl: false })));
        steps.add(Role::Undo, -1.2);
        assert_eq!(steps.next(), Some(Gesture::Wheel(Wheel { up: false, ctrl: true })), "then the level");
        assert_eq!(steps.next(), Some(Gesture::Undo { redo: true }), "then undo: back is redo");
        assert!(!steps.waiting());
        let map = serde_json::json!({ "pinch": "zoom" });
        assert!(role(&map, "pinch").is_err());
    }
}
