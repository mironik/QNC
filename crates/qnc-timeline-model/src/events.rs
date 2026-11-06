//! What the pointer did on a timeline, as the catalog action the mouse map gives it on that
//! panel (user rule 2026-10-20, locked; the map is `contracts/input/mouse.json`, read by the
//! block `qnc-mouse-map`). Core (docs/99 step 3): it reads no egui, so the logic that turns a
//! timeline's intent into its action builds without a window.

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
        // The frame clicked: in the S-Story the side of the cut says IN or OUT (user 2026-11-01).
        TimelineIntent::TakeMarker { id, frame: at } => (Pointer::RightClickMarker, Some(frame(*at)), Some(id)),
        TimelineIntent::TakeSlot { id, .. } => (Pointer::RightClickStrip, None, Some(id)),
        TimelineIntent::TakeCover { id, frame: at } => (Pointer::RightClickCover, Some(frame(*at)), Some(id)),
        TimelineIntent::TakeAudioLane(lane) => (if a2(lane) { Pointer::RightClickA2 } else { Pointer::RightClickA1 }, None, None),
        TimelineIntent::TakeMarkerSound { id } => (Pointer::RightClickMarkerA2, None, Some(id)),
        TimelineIntent::TakeMarkerPicture { id } => (Pointer::RightClickTakenMarker, None, Some(id)),
        TimelineIntent::TakeCutEdge { id, frame: at } => (Pointer::ClickCutEdge, Some(frame(*at)), Some(id)),
        TimelineIntent::TakeSound { lane, frame: at } => (if a2(lane) { Pointer::RightClickA2 } else { Pointer::RightClickA1 }, Some(frame(*at)), None),
        TimelineIntent::TakeLevelLine { lane, frame: at } => (if a2(lane) { Pointer::RightClickLevelA2 } else { Pointer::RightClickLevelA1 }, Some(frame(*at)), None),
        // IN or OUT dragged: the clip's own frame, never mapped.
        TimelineIntent::DragMark { in_mark, frame: at } => (if *in_mark { Pointer::DragIn } else { Pointer::DragOut }, Some(*at), None),
        TimelineIntent::Cut => (Pointer::MiddleClick, None, None),
        TimelineIntent::CutAt(at) => (Pointer::DoubleClickFrame, Some(frame(*at)), None),
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

