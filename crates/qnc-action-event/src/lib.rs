//! One system action, whatever device it came from (user 2026-10-18: every action under a
//! system code, so that the keyboard, the mouse, a touchpad, a programmable keyboard, an edit
//! console or gestures can all drive QNC through their own maps). An input device block turns
//! its input into action events; the panel router takes only action events.
//!
//! An event is a catalog `action_id` (`contracts/qnc-keyboard-shortcuts.json`), an optional
//! value (a frame, a channel), an optional target (the marker, slot or cover it acts on) and
//! the panel it happened on (a click or a gesture is where the pointer is; a key is where the
//! keyboard focus is, so it has none). It knows no device, form or application.

pub const MODULE_ID: &str = "qnc.module.action-event";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActionEvent {
    pub action_id: String,
    pub value: Option<i64>,
    pub target: Option<String>,
    /// The panel the pointer was on (a layout contract's panel name), none for a key.
    pub at: Option<String>,
}

impl ActionEvent {
    /// A key: the action alone, on the panel with the keyboard focus.
    pub fn key(action_id: impl Into<String>) -> Self {
        Self { action_id: action_id.into(), ..Self::default() }
    }

    pub fn with_value(mut self, value: impl TryInto<i64>) -> Self {
        self.value = value.try_into().ok();
        self
    }

    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// Where the pointer was: the event belongs to that panel.
    pub fn at(mut self, panel: impl Into<String>) -> Self {
        self.at = Some(panel.into());
        self
    }

    /// The value as a frame or a channel (none when it is negative or missing).
    pub fn unsigned(&self) -> Option<u64> {
        self.value.and_then(|value| u64::try_from(value).ok())
    }

    pub fn is_at(&self, panel: &str) -> bool {
        self.at.as_deref() == Some(panel)
    }
}

impl From<&str> for ActionEvent {
    fn from(action_id: &str) -> Self {
        Self::key(action_id)
    }
}

impl From<&String> for ActionEvent {
    fn from(action_id: &String) -> Self {
        Self::key(action_id.as_str())
    }
}

impl From<String> for ActionEvent {
    fn from(action_id: String) -> Self {
        Self::key(action_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_has_no_value_target_or_panel_and_a_click_has_them() {
        assert_eq!(ActionEvent::key("play_pause"), ActionEvent { action_id: "play_pause".into(), ..Default::default() });
        let click = ActionEvent::key("timeline_select_slot").with_value(120u64).with_target("a|b").at("segments");
        assert_eq!((click.unsigned(), click.target.as_deref(), click.is_at("segments")), (Some(120), Some("a|b"), true));
        assert_eq!(ActionEvent::key("x").with_value(u64::MAX).value, None, "a value out of range is none");
    }
}
