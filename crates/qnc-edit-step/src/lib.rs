//! One step by one catalog action, the same on every timeline and its zoom (user 2026-11-05:
//! "every timeline and its zoom use the same blocks"; AGENTS 0.12). The left/right arrows and
//! the wheel over a source timeline or the Wrap rows are a frame of the playhead or of what is
//! taken, the wheel's direction this computer's choice; Ctrl+wheel moves a taken mark the same
//! way; Ctrl+left/right and Ctrl+wheel are a level, up louder everywhere. No state, no owner's
//! rules: an owner asks here what a key is and does it on its own axis.

pub const MODULE_ID: &str = "qnc.module.edit-step";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The setting of this computer beside its others (`data/mouse-wheel.json`).
pub const SETTING_FILE: &str = "mouse-wheel.json";

pub const BACK: &str = "step_back_frame";
pub const FORWARD: &str = "step_forward_frame";
pub const LEVEL_DOWN: &str = "lane_level_down";
pub const LEVEL_UP: &str = "lane_level_up";

/// The wheel actions of every timeline: (back, forward) without and with Ctrl.
const WHEELS: [(&str, &str); 2] = [("wheel_source_back", "wheel_source_forward"), ("wheel_segments_back", "wheel_segments_forward")];
const CTRL_WHEELS: [(&str, &str); 2] = [("wheel_source_ctrl_back", "wheel_source_ctrl_forward"), ("wheel_segments_ctrl_back", "wheel_segments_ctrl_forward")];

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
pub fn parse_setting(text: &str) -> Result<bool, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    match value.get("playhead_up").and_then(serde_json::Value::as_str) {
        Some("forward") => Ok(true),
        Some("back") => Ok(false),
        other => Err(format!("playhead_up must be \"forward\" or \"back\", not {other:?}")),
    }
}

/// The wheel's step after this computer's direction: up (`forward` id) is +1 when up is
/// forward.
fn turned(forward_id: bool) -> i64 {
    if forward_id == playhead_up_forward() {
        1
    } else {
        -1
    }
}

/// A frame of the playhead or of what is taken: the left/right arrows and the wheel over any
/// timeline (without Ctrl). `None` for any other action.
pub fn frames(action_id: &str) -> Option<i64> {
    match action_id {
        BACK => Some(-1),
        FORWARD => Some(1),
        id => WHEELS.iter().find_map(|(back, forward)| (id == *back || id == *forward).then(|| turned(id == *forward))),
    }
}

/// Ctrl+wheel moving a taken mark (user 2026-10-17: the right button and the wheel cut the
/// clip): the same direction as the wheel. `None` for any other action.
pub fn ctrl_frames(action_id: &str) -> Option<i64> {
    CTRL_WHEELS.iter().find_map(|(back, forward)| (action_id == *back || action_id == *forward).then(|| turned(action_id == *forward)))
}

/// A level step of a taken sound: Ctrl+left/right and Ctrl+wheel, up (away from the user) and
/// right louder everywhere. `None` for any other action.
pub fn level(action_id: &str) -> Option<i64> {
    match action_id {
        LEVEL_DOWN => Some(-1),
        LEVEL_UP => Some(1),
        id => CTRL_WHEELS.iter().find_map(|(back, forward)| (id == *back || id == *forward).then_some(if id == *forward { 1 } else { -1 })),
    }
}

/// Whether the action is a wheel of a timeline (with or without Ctrl).
pub fn is_wheel(action_id: &str) -> bool {
    WHEELS.iter().any(|(back, forward)| action_id == *back || action_id == *forward) || ctrl_frames(action_id).is_some()
}

/// Alt+arrows, the same on every timeline (user 2026-11-09): the next or previous stop from
/// `at`, the playhead only, nothing is taken; the owner says what its stops are. `None`: no stop
/// on that side.
pub fn next_stop(stops: &[u64], at: u64, back: bool) -> Option<u64> {
    if back {
        stops.iter().copied().filter(|stop| *stop < at).max()
    } else {
        stops.iter().copied().filter(|stop| *stop > at).min()
    }
}

/// What the owner of a timeline holds when the wheel turns.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WheelHeld {
    /// A sound whose left/right arrows turn it off and on (a taken A1/A2).
    pub sound: bool,
    /// A level curve: Ctrl+wheel is its level, the arrows and the wheel the playhead.
    pub level: bool,
    /// Something the arrows move a frame: IN/OUT, an edge, a cut, a slot.
    pub mark: bool,
}

/// What one wheel step does, the same on every timeline and its zoom (user 2026-11-09: "all
/// timelines must be based on one code"); the owner does it on its own axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WheelDoes {
    /// The playhead that many frames, where the arrows mean something else.
    Playhead(i64),
    /// The left/right arrow of that step.
    Arrow(i64),
    /// The level step of the taken sound.
    Level(i64),
    /// Ctrl+wheel with nothing it could change.
    Nothing,
}

/// The one rule of the wheel (user 2026-10-16, 2026-10-17): alone it is the arrow, except on a
/// taken sound, where the arrows turn it off and on and the wheel moves the playhead; with Ctrl
/// it is the level of a taken sound or curve, else it moves what is taken. `None`: not a wheel.
pub fn wheel(action_id: &str, held: WheelHeld) -> Option<WheelDoes> {
    if !is_wheel(action_id) {
        return None;
    }
    Some(match ctrl_frames(action_id) {
        Some(_) if held.sound || held.level => WheelDoes::Level(level(action_id)?),
        Some(frames) if held.mark => WheelDoes::Arrow(frames),
        Some(_) => WheelDoes::Nothing,
        None if held.sound => WheelDoes::Playhead(frames(action_id)?),
        None => WheelDoes::Arrow(frames(action_id)?),
    })
}

/// The arrow of a step: forward for a positive one.
pub fn arrow(step: i64) -> &'static str {
    if step > 0 {
        FORWARD
    } else {
        BACK
    }
}

/// The level action of a level step.
pub fn level_action(step: i64) -> &'static str {
    if step > 0 {
        LEVEL_UP
    } else {
        LEVEL_DOWN
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arrows_and_every_timelines_wheel_are_one_step() {
        assert_eq!(frames(BACK), Some(-1));
        assert_eq!(frames(FORWARD), Some(1));
        let up = if playhead_up_forward() { 1 } else { -1 };
        for (back, forward) in [("wheel_source_back", "wheel_source_forward"), ("wheel_segments_back", "wheel_segments_forward")] {
            assert_eq!(frames(forward), Some(up));
            assert_eq!(frames(back), Some(-up));
        }
        assert_eq!(frames("wheel_source_ctrl_forward"), None, "the wheel with Ctrl is not a plain step");
        assert_eq!(ctrl_frames("wheel_segments_ctrl_forward"), Some(up));
        assert_eq!(frames("mark_in"), None);
    }

    #[test]
    fn a_level_is_up_louder_everywhere() {
        assert_eq!(level(LEVEL_UP), Some(1));
        assert_eq!(level("wheel_source_ctrl_forward"), Some(1));
        assert_eq!(level("wheel_segments_ctrl_back"), Some(-1));
        assert_eq!(level("wheel_source_forward"), None);
        assert_eq!(level_action(1), LEVEL_UP);
        assert!(is_wheel("wheel_segments_ctrl_back") && !is_wheel(FORWARD));
    }

    #[test]
    fn the_stops_are_in_order_whatever_order_they_come_in() {
        let stops = [40, 0, 10, 10];
        assert_eq!(next_stop(&stops, 20, false), Some(40));
        assert_eq!(next_stop(&stops, 20, true), Some(10));
        assert_eq!(next_stop(&stops, 10, true), Some(0), "not the stop it stands on");
        assert_eq!(next_stop(&stops, 40, false), None);
    }

    #[test]
    fn the_wheel_has_one_rule_whatever_is_held_on_every_timeline() {
        for (plain, ctrl) in [("wheel_source_forward", "wheel_source_ctrl_forward"), ("wheel_segments_forward", "wheel_segments_ctrl_forward")] {
            let up = frames(plain).unwrap();
            let held = |sound, level, mark| WheelHeld { sound, level, mark };
            assert_eq!(wheel(plain, held(false, false, false)), Some(WheelDoes::Arrow(up)));
            assert_eq!(wheel(plain, held(false, true, false)), Some(WheelDoes::Arrow(up)), "a curve: the arrows are the playhead");
            assert_eq!(wheel(plain, held(false, false, true)), Some(WheelDoes::Arrow(up)));
            assert_eq!(wheel(plain, held(true, false, false)), Some(WheelDoes::Playhead(up)), "the arrows turn it off and on");
            assert_eq!(wheel(ctrl, held(true, false, false)), Some(WheelDoes::Level(1)));
            assert_eq!(wheel(ctrl, held(false, true, false)), Some(WheelDoes::Level(1)));
            assert_eq!(wheel(ctrl, held(false, false, true)), Some(WheelDoes::Arrow(up)));
            assert_eq!(wheel(ctrl, held(false, false, false)), Some(WheelDoes::Nothing));
        }
        assert_eq!(wheel(FORWARD, WheelHeld::default()), None);
    }

    #[test]
    fn the_computer_chooses_the_playhead_direction() {
        assert_eq!(parse_setting(r#"{"playhead_up": "forward"}"#), Ok(true));
        assert_eq!(parse_setting(r#"{"playhead_up": "back"}"#), Ok(false));
        assert!(parse_setting(r#"{"playhead_up": "up"}"#).is_err(), "no guess");
    }
}

/// The catalog actions of a panel's wheel: back and forward, then with Ctrl held.
pub type WheelActions<'a> = ((&'a str, &'a str), (&'a str, &'a str));

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
