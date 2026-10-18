//! The mouse map of every timeline (user 2026-10-18: every action under a system code, any
//! input device; the timeline is one for all, written once): what the pointer did on a
//! timeline becomes a catalog action with its value and target. A click is the same action
//! as its key where one exists (a right click is Ctrl+: on a marker Ctrl+M, in a slot
//! Ctrl+S, near a slot edge Ctrl+I/O, on A1/A2 Ctrl+1/Ctrl+2; the wheel is the arrows);
//! the pointer adds what the key takes from the playhead (the marker, slot or frame).
//!
//! Frames are the row's own; `frame` maps them where the row stands (a Wrap row's program
//! frames). `wheel` is the wheel's actions on this panel. The host panel says which panel
//! the event is on.

use qnc_action_event::ActionEvent;

use crate::{AudioLane, TimelineIntent};

/// The catalog action of a pointer intent on a timeline; `None` for nothing (or an intent
/// the host keeps for itself, as the A1/A2 expand of a Wrap row).
pub fn event(intent: &TimelineIntent, wheel: qnc_wheel_jog::WheelActions<'_>, frame: &dyn Fn(u64) -> u64) -> Option<ActionEvent> {
    let key = ActionEvent::key;
    let lane = |lane: &AudioLane| if *lane == AudioLane::A2 { "select_audio_a2" } else { "select_audio_a1" };
    Some(match intent {
        TimelineIntent::None | TimelineIntent::ToggleAudioExpand(_) => return None,
        TimelineIntent::CueFrame(at) | TimelineIntent::SelectVirtual { frame: at, .. } => key("timeline_cue_frame").with_value(frame(*at)),
        // V on a timeline is Home: the start.
        TimelineIntent::GoHome => key("playlist_input_start"),
        TimelineIntent::ToggleVideoMode => key("toggle_wrap_video"),
        TimelineIntent::Wheel(turn) => key(turn.action(wheel)),
        // A left click keeps a taken IN/OUT or lane (never Enter, which walks the panels).
        TimelineIntent::KeepMark => key("source_keep_mark"),
        TimelineIntent::TakeMark(in_mark) => key(if *in_mark { "select_mark_in" } else { "select_mark_out" }),
        TimelineIntent::DragMark { in_mark, frame: at } => key(if *in_mark { "source_drag_in" } else { "source_drag_out" }).with_value(*at),
        TimelineIntent::ChooseA1Channel(channel) => key("source_choose_a1_channel").with_value(*channel),
        TimelineIntent::ChooseA2Channel(channel) => key("source_choose_a2_channel").with_value(*channel),
        TimelineIntent::TakeAudioLane(taken) => key(lane(taken)),
        TimelineIntent::TakeSound { lane: taken, frame: at } => key(lane(taken)).with_value(frame(*at)),
        TimelineIntent::SelectMarker { id, .. } => key("timeline_select_marker").with_target(id),
        TimelineIntent::TakeMarker { id, .. } => key("select_marker").with_target(id),
        TimelineIntent::TakeSlot { id, .. } => key("select_current_marker_slot").with_target(id),
        TimelineIntent::TakeSlotEdge { id, start } => key(if *start { "select_mark_in" } else { "select_mark_out" }).with_target(id),
        TimelineIntent::TakeCover { id, frame: at } => key("timeline_take_cover").with_target(id).with_value(frame(*at)),
        TimelineIntent::SelectCover { id, frame: at } => key("timeline_select_cover").with_target(id).with_value(frame(*at)),
        TimelineIntent::SelectMarkerSlot { id, frame: at } => key("timeline_select_slot").with_target(id).with_value(frame(*at)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_is_the_action_of_its_key_with_what_the_pointer_points_at() {
        let row = |frame: u64| 100 + frame;
        let take = event(&TimelineIntent::TakeMarker { id: "m1".into(), frame: 5 }, qnc_wheel_jog::SOURCE_ACTIONS, &row).unwrap();
        assert_eq!((take.action_id.as_str(), take.target.as_deref()), ("select_marker", Some("m1")), "a right click takes the marker it is on");
        let cue = event(&TimelineIntent::CueFrame(7), qnc_wheel_jog::SOURCE_ACTIONS, &row).unwrap();
        assert_eq!((cue.action_id.as_str(), cue.value), ("timeline_cue_frame", Some(107)), "the row's frame where the row stands");
        let channel = event(&TimelineIntent::ChooseA2Channel(3), qnc_wheel_jog::SOURCE_ACTIONS, &row).unwrap();
        assert_eq!((channel.action_id.as_str(), channel.value), ("source_choose_a2_channel", Some(3)));
        assert_eq!(event(&TimelineIntent::None, qnc_wheel_jog::SOURCE_ACTIONS, &row), None);
    }
}
