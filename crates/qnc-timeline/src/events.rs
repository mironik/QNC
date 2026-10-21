//! What the pointer did on a timeline, as the catalog action the mouse map gives it on that
//! panel (user rule 2026-10-20, locked; the map is `contracts/input/mouse.json`, read by the
//! block `qnc-mouse-map`): the timeline says what happened (`TimelineIntent`), the map says
//! which action that is, with the value (a frame, a channel) and the target (a marker, slot,
//! cover) the pointer gives. Frames are the row's own; `frame` maps them where the row stands
//! (a Wrap row's program frames).

use qnc_action_event::ActionEvent;
use qnc_mouse_map::Pointer;

use crate::{AudioLane, TimelineIntent};

/// The catalog action of a pointer intent on `panel`; `None` for nothing (or a pointer the
/// map leaves out there).
pub fn event(intent: &TimelineIntent, panel: &str, frame: &dyn Fn(u64) -> u64) -> Option<ActionEvent> {
    let a2 = |lane: &AudioLane| *lane == AudioLane::A2;
    let (pointer, value, target): (Pointer, Option<u64>, Option<&str>) = match intent {
        TimelineIntent::None | TimelineIntent::ToggleAudioExpand(_) => return None,
        TimelineIntent::CueFrame(at) | TimelineIntent::SelectVirtual { frame: at, .. } => (Pointer::Click, Some(frame(*at)), None),
        TimelineIntent::Keep(at) => (Pointer::ClickHolding, at.map(frame), None),
        TimelineIntent::SelectMarkerSlot { id, frame: at } => (Pointer::ClickStrip, Some(frame(*at)), Some(id)),
        TimelineIntent::SelectCover { id, frame: at } => (Pointer::ClickCover, Some(frame(*at)), Some(id)),
        TimelineIntent::TakeCoverOnStrip { id, frame: at } => (Pointer::CtrlClickStrip, Some(frame(*at)), Some(id)),
        TimelineIntent::AddMarkerAt(at) => (Pointer::ShiftClick, Some(frame(*at)), None),
        TimelineIntent::OpenVirtual { id, frame: at } => (Pointer::DoubleClick, Some(frame(*at)), Some(id)),
        TimelineIntent::OpenSlot { id, frame: at } => (Pointer::DoubleClickSlot, Some(frame(*at)), Some(id)),
        TimelineIntent::GoHome => (Pointer::ClickVLabel, None, None),
        TimelineIntent::ToggleVideoMode => (Pointer::CtrlClickVLabel, None, None),
        TimelineIntent::LaneLabel { lane, frame: at } => (if a2(lane) { Pointer::ClickA2Label } else { Pointer::ClickA1Label }, at.map(frame), None),
        TimelineIntent::ChooseA1Channel(channel) => (Pointer::ClickA1Channel, Some(u64::from(*channel)), None),
        TimelineIntent::ChooseA2Channel(channel) => (Pointer::ClickA2Channel, Some(u64::from(*channel)), None),
        TimelineIntent::TakeMark(in_mark) => (if *in_mark { Pointer::RightClickIn } else { Pointer::RightClickOut }, None, None),
        TimelineIntent::TakeMarker { id, .. } => (Pointer::RightClickMarker, None, Some(id)),
        TimelineIntent::TakeSlot { id, .. } => (Pointer::RightClickStrip, None, Some(id)),
        TimelineIntent::TakeCover { id, frame: at } => (Pointer::RightClickCover, Some(frame(*at)), Some(id)),
        TimelineIntent::TakeAudioLane(lane) => (if a2(lane) { Pointer::RightClickA2 } else { Pointer::RightClickA1 }, None, None),
        TimelineIntent::TakeMarkerSound { id } => (Pointer::RightClickMarkerA2, None, Some(id)),
        TimelineIntent::TakeSound { lane, frame: at } => (if a2(lane) { Pointer::RightClickA2 } else { Pointer::RightClickA1 }, Some(frame(*at)), None),
        // IN or OUT dragged: the clip's own frame, never mapped.
        TimelineIntent::DragMark { in_mark, frame: at } => (if *in_mark { Pointer::DragIn } else { Pointer::DragOut }, Some(*at), None),
        TimelineIntent::Cut => (Pointer::MiddleClick, None, None),
        TimelineIntent::Wheel(turn) => (
            match (turn.ctrl, turn.up) {
                (false, false) => Pointer::WheelBack,
                (false, true) => Pointer::WheelForward,
                (true, false) => Pointer::CtrlWheelBack,
                (true, true) => Pointer::CtrlWheelForward,
            },
            None,
            None,
        ),
    };
    let mut event = ActionEvent::key(qnc_mouse_map::action(panel, pointer)?);
    if let Some(value) = value {
        event = event.with_value(value);
    }
    if let Some(target) = target {
        event = event.with_target(target);
    }
    Some(event.at(panel))
}

/// The M marker dragged on a row this frame, as the action of the mouse map (`drag_marker`)
/// with the marker and the frame it is dragged to (`frame` maps the row's frame).
pub fn take_marker_drag_event(ctx: &eframe::egui::Context, panel: &str, frame: &dyn Fn(u64) -> u64) -> Option<ActionEvent> {
    let (marker_id, at) = crate::take_marker_drag(ctx)?;
    let action = qnc_mouse_map::action(panel, Pointer::DragMarker)?;
    Some(ActionEvent::key(action).with_target(marker_id).with_value(frame(at)).at(panel))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Locked (user rule 2026-10-20): the mouse on a timeline is the mouse map's action, with
    /// what the pointer points at.
    #[test]
    fn a_pointer_is_the_mouse_maps_action_with_what_it_points_at() {
        let row = |frame: u64| 100 + frame;
        let on = |intent: TimelineIntent, panel: &str| event(&intent, panel, &row).map(|event| (event.action_id, event.value, event.target));
        let id = |text: &str| Some(text.to_string());
        assert_eq!(on(TimelineIntent::TakeMarker { id: "m1".into(), frame: 5 }, "segments"), Some(("select_mark_in".into(), None, id("m1"))), "a right click over the picture takes the nearest marker's picture (user 2026-10-21)");
        assert_eq!(on(TimelineIntent::TakeSlot { id: "s1".into(), frame: 5 }, "segments"), Some(("select_current_marker_slot".into(), None, id("s1"))), "a right click on the strip takes the slot");
        assert_eq!(on(TimelineIntent::CueFrame(7), "segments"), Some(("timeline_cue_frame".into(), Some(107), None)), "left click = the playhead");
        assert_eq!(on(TimelineIntent::Keep(Some(7)), "segments"), Some(("segments_keep_edit".into(), Some(107), None)), "kept, then the playhead");
        assert_eq!(on(TimelineIntent::Keep(Some(7)), "source_timeline"), Some(("source_keep_mark".into(), Some(107), None)));
        assert_eq!(on(TimelineIntent::SelectMarkerSlot { id: "s1".into(), frame: 3 }, "segments"), Some(("timeline_select_slot".into(), Some(103), id("s1"))));
        assert_eq!(on(TimelineIntent::AddMarkerAt(9), "segments"), Some(("add_marker_continue".into(), Some(109), None)), "a click that adds is the adding key");
        assert_eq!(on(TimelineIntent::AddMarkerAt(9), "source_timeline"), None, "nothing to add on a source timeline");
        assert_eq!(on(TimelineIntent::TakeMark(true), "source_timeline"), Some(("select_mark_in".into(), None, None)), "a right click is the taking key");
        assert_eq!(on(TimelineIntent::Cut, "segments"), Some(("add_marker".into(), None, None)), "the middle button cuts: an M marker");
        assert_eq!(on(TimelineIntent::OpenVirtual { id: "s1".into(), frame: 4 }, "segments"), Some(("segment_edit".into(), Some(104), id("s1"))), "a double click is Edit");
        assert_eq!(on(TimelineIntent::OpenVirtual { id: "s1".into(), frame: 4 }, "source_timeline"), None);
        assert_eq!(on(TimelineIntent::OpenSlot { id: "m1|m2".into(), frame: 4 }, "segments"), Some(("wrap_zoom".into(), Some(104), id("m1|m2"))), "a double click on the picture enlarges the slot");
        assert_eq!(on(TimelineIntent::Cut, "source_timeline"), Some(("source_cut".into(), None, None)));
        assert_eq!(on(TimelineIntent::ChooseA2Channel(3), "source_timeline"), Some(("source_choose_a2_channel".into(), Some(3), None)));
        assert_eq!(on(TimelineIntent::None, "segments"), None);
    }
}
