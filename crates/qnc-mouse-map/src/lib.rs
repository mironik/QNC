//! The mouse map of every timeline (user rule 2026-10-20, locked): what the mouse did on a
//! panel is one catalog action of `contracts/qnc-keyboard-shortcuts.json`, read from
//! `contracts/input/mouse.json` and nowhere else. The rule, the same as the keys:
//!
//! - a left click places the playhead; on a cover's colour strip it selects the slot;
//!   while something is taken it keeps it first (as Enter), then places the playhead;
//! - a right click is Ctrl+: it takes what it points at (the nearest IN or OUT, the nearest
//!   M marker, the slot on a colour strip, A1/A2);
//! - a drag moves only what is taken; Shift + left click adds; the middle button cuts;
//! - a double click on a segment opens it on the source timeline (Edit; user 2026-10-21);
//! - the wheel is the left/right arrows, Ctrl+wheel Ctrl+left/right.
//!
//! The timeline says what the pointer did (`Pointer`); this block says which action that is
//! on the panel. The map is built in (the contract is the one source; a map that does not
//! parse fails the tests, never a run). No state, no form, no database.

pub const MODULE_ID: &str = "qnc.module.mouse-map";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The contract, built in.
const MAP: &str = include_str!("../../../contracts/input/mouse.json");

/// What the pointer did on a timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pointer {
    /// A left click on the track: the playhead goes there.
    Click,
    /// A left click while something is taken: kept, then the playhead goes there.
    ClickHolding,
    /// A left click on a cover's colour strip: its slot.
    ClickStrip,
    /// A left click on a cover with no slot.
    ClickCover,
    /// Ctrl + left click on a colour strip: the cover taken (for Delete).
    CtrlClickStrip,
    /// Shift + left click: an M marker added there.
    ShiftClick,
    /// A double click on a segment (its A1): it opens on the source timeline (user 2026-10-21).
    DoubleClick,
    /// A double click on the picture or A2 of a slot: the slot enlarged (user 2026-10-21).
    DoubleClickSlot,
    ClickVLabel,
    CtrlClickVLabel,
    ClickA1Label,
    ClickA2Label,
    ClickA1Channel,
    ClickA2Channel,
    RightClickIn,
    RightClickOut,
    RightClickMarker,
    RightClickStrip,
    RightClickCover,
    RightClickA1,
    RightClickA2,
    /// A right click over A2 of a Wrap row with markers: the nearest M marker, its sound (user 2026-10-21).
    RightClickMarkerA2,
    DragIn,
    DragOut,
    DragMarker,
    MiddleClick,
    WheelBack,
    WheelForward,
    CtrlWheelBack,
    CtrlWheelForward,
}

impl Pointer {
    pub const ALL: [Pointer; 30] = [
        Self::Click,
        Self::ClickHolding,
        Self::ClickStrip,
        Self::ClickCover,
        Self::CtrlClickStrip,
        Self::ShiftClick,
        Self::DoubleClick,
        Self::DoubleClickSlot,
        Self::ClickVLabel,
        Self::CtrlClickVLabel,
        Self::ClickA1Label,
        Self::ClickA2Label,
        Self::ClickA1Channel,
        Self::ClickA2Channel,
        Self::RightClickIn,
        Self::RightClickOut,
        Self::RightClickMarker,
        Self::RightClickStrip,
        Self::RightClickCover,
        Self::RightClickA1,
        Self::RightClickA2,
        Self::RightClickMarkerA2,
        Self::DragIn,
        Self::DragOut,
        Self::DragMarker,
        Self::MiddleClick,
        Self::WheelBack,
        Self::WheelForward,
        Self::CtrlWheelBack,
        Self::CtrlWheelForward,
    ];

    /// Its name in the contract.
    pub fn name(self) -> &'static str {
        match self {
            Self::Click => "click",
            Self::ClickHolding => "click_holding",
            Self::ClickStrip => "click_strip",
            Self::ClickCover => "click_cover",
            Self::CtrlClickStrip => "ctrl_click_strip",
            Self::ShiftClick => "shift_click",
            Self::DoubleClick => "double_click",
            Self::DoubleClickSlot => "double_click_slot",
            Self::ClickVLabel => "click_v_label",
            Self::CtrlClickVLabel => "ctrl_click_v_label",
            Self::ClickA1Label => "click_a1_label",
            Self::ClickA2Label => "click_a2_label",
            Self::ClickA1Channel => "click_a1_channel",
            Self::ClickA2Channel => "click_a2_channel",
            Self::RightClickIn => "right_click_in",
            Self::RightClickOut => "right_click_out",
            Self::RightClickMarker => "right_click_marker",
            Self::RightClickStrip => "right_click_strip",
            Self::RightClickCover => "right_click_cover",
            Self::RightClickA1 => "right_click_a1",
            Self::RightClickA2 => "right_click_a2",
            Self::RightClickMarkerA2 => "right_click_marker_a2",
            Self::DragIn => "drag_in",
            Self::DragOut => "drag_out",
            Self::DragMarker => "drag_marker",
            Self::MiddleClick => "middle_click",
            Self::WheelBack => "wheel_back",
            Self::WheelForward => "wheel_forward",
            Self::CtrlWheelBack => "ctrl_wheel_back",
            Self::CtrlWheelForward => "ctrl_wheel_forward",
        }
    }
}

/// The map: per panel, the action of each pointer it answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseMap {
    panels: Vec<(String, Vec<(Pointer, String)>)>,
}

impl MouseMap {
    /// The map of `text`; an unknown pointer name or an empty action is refused.
    pub fn parse(text: &str) -> Result<Self, String> {
        let map: serde_json::Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
        let panels = map.get("panels").and_then(serde_json::Value::as_object).ok_or("the map has no panels")?;
        let mut parsed = Vec::new();
        for (panel, pointers) in panels {
            let pointers = pointers.as_object().ok_or_else(|| format!("panel {panel} is not a table"))?;
            let mut actions = Vec::new();
            for (name, action) in pointers {
                let pointer = Pointer::ALL
                    .into_iter()
                    .find(|pointer| pointer.name() == name)
                    .ok_or_else(|| format!("{panel}: unknown pointer {name}"))?;
                let action = action
                    .as_str()
                    .filter(|action| !action.trim().is_empty())
                    .ok_or_else(|| format!("{panel}.{name}: the action must be a catalog action id"))?;
                actions.push((pointer, action.to_string()));
            }
            parsed.push((panel.clone(), actions));
        }
        Ok(Self { panels: parsed })
    }

    /// The map of this QNC (the contract, built in).
    pub fn of_this_qnc() -> &'static Self {
        static BUILT: std::sync::OnceLock<MouseMap> = std::sync::OnceLock::new();
        BUILT.get_or_init(|| Self::parse(MAP).unwrap_or_else(|error| panic!("contracts/input/mouse.json: {error}")))
    }

    /// The action of `pointer` on `panel`; `None`: the pointer does nothing there.
    pub fn action(&self, panel: &str, pointer: Pointer) -> Option<&str> {
        self.panels
            .iter()
            .find(|(name, _)| name == panel)
            .and_then(|(_, actions)| actions.iter().find(|(known, _)| *known == pointer))
            .map(|(_, action)| action.as_str())
    }

    /// Every action the map names, with its panel and pointer.
    pub fn entries(&self) -> impl Iterator<Item = (&str, Pointer, &str)> {
        self.panels
            .iter()
            .flat_map(|(panel, actions)| actions.iter().map(move |(pointer, action)| (panel.as_str(), *pointer, action.as_str())))
    }
}

/// The action of `pointer` on `panel` in this QNC's map.
pub fn action(panel: &str, pointer: Pointer) -> Option<&'static str> {
    MouseMap::of_this_qnc().action(panel, pointer)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Locked (user 2026-10-20): the map names only actions of the keyboard catalog, and
    /// where a pointer stands for a key it is that key's action.
    #[test]
    fn the_map_names_catalog_actions_and_a_right_click_is_the_ctrl_key() {
        let catalog: serde_json::Value = serde_json::from_str(include_str!("../../../contracts/qnc-keyboard-shortcuts.json")).unwrap();
        let actions = catalog["actions"].as_object().unwrap();
        let map = MouseMap::of_this_qnc();
        for (panel, pointer, action) in map.entries() {
            assert!(actions.contains_key(action), "{panel}.{}: {action} is not in the catalog", pointer.name());
        }
        let source = "source_timeline";
        assert_eq!(map.action(source, Pointer::RightClickIn), Some("select_mark_in"), "the same action as its key");
        assert_eq!(map.action(source, Pointer::RightClickA1), Some("select_audio_a1"), "the same action as its key");
        // User 2026-10-21: over the picture the nearest marker's picture (Ctrl+M then Ctrl+I), over A2 its sound (Ctrl+2).
        assert_eq!(map.action("segments", Pointer::RightClickMarker), Some("select_mark_in"), "the picture of the nearest marker's cut");
        assert_eq!(map.action("segments", Pointer::RightClickMarkerA2), Some("select_audio_a2"), "the sound of the nearest marker's cut");
        assert_eq!(map.action("segments", Pointer::RightClickStrip), Some("select_current_marker_slot"), "the same action as its key");
        assert_eq!(map.action("segments", Pointer::ShiftClick), Some("add_marker_continue"), "the same action as its key");
        assert_eq!(map.action("segments", Pointer::Click), Some("timeline_cue_frame"), "a left click is the playhead");
        assert_eq!(map.action(source, Pointer::ShiftClick), None, "nothing to add on a source timeline");
        assert_eq!(map.action("segments", Pointer::DoubleClick), Some("segment_edit"), "a double click is Edit");
    }

    #[test]
    fn an_unknown_pointer_or_an_empty_action_is_refused() {
        assert!(MouseMap::parse(r#"{"panels": {"p": {"triple_click": "x"}}}"#).is_err());
        assert!(MouseMap::parse(r#"{"panels": {"p": {"click": ""}}}"#).is_err());
    }
}
