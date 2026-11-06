//! What the pointer did on a timeline, as the catalog action the mouse map gives it on that
//! panel (user rule 2026-10-20, locked; the map is `contracts/input/mouse.json`, read by the
//! block `qnc-mouse-map`): the timeline says what happened (`TimelineIntent`), the map says
//! which action that is, with the value (a frame, a channel) and the target (a marker, slot,
//! cover) the pointer gives. Frames are the row's own; `frame` maps them where the row stands
//! (a Wrap row's program frames).

use qnc_action_event::ActionEvent;
use qnc_mouse_map::Pointer;

#[allow(unused_imports)]
use crate::{event, AudioLane, TimelineIntent};

/// The M marker dragged on a row this frame, as the action of the mouse map (`drag_marker`)
/// with the marker and the frame it is dragged to (`frame` maps the row's frame).
pub fn take_marker_drag_event(ctx: &eframe::egui::Context, panel: &str, frame: &dyn Fn(u64) -> u64) -> Option<ActionEvent> {
    // A side of the taken cut's sound dragged (user 2026-10-29).
    if let Some((marker_id, at)) = crate::take_cut_edge_drag(ctx) {
        let action = qnc_mouse_map::action(panel, Pointer::DragCutEdge)?;
        return Some(ActionEvent::key(action).with_target(marker_id).with_value(frame(at)).at(panel));
    }
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
        assert_eq!(on(TimelineIntent::TakeMarker { id: "m1".into(), frame: 5 }, "segments"), Some(("select_marker".into(), Some(105), id("m1"))), "a right click takes the nearest marker whole (user 2026-10-22)");
        assert_eq!(on(TimelineIntent::TakeMarkerPicture { id: "m1".into() }, "segments"), Some(("select_mark_in".into(), None, id("m1"))), "a second right click on the taken marker: its picture");
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
        assert_eq!(on(TimelineIntent::CutAt(40), "source_timeline"), Some(("source_cut".into(), Some(140), None)), "a double click is a cut where it clicked");
        assert_eq!(on(TimelineIntent::ChooseA2Channel(3), "source_timeline"), Some(("source_choose_a2_channel".into(), Some(3), None)));
        assert_eq!(on(TimelineIntent::None, "segments"), None);
    }

    /// AGENTS 0.12 (user 2026-11-04, locked): a timeline and its zoom have the same tools. The
    /// golden table of the gestures they share: what the timeline sends through the mouse map is
    /// the very verb the zoom builds (`qnc-edit-verbs`, used by `qnc-trim-overlay`) on the same
    /// owner's panel. A change of any row needs the user's decision.
    #[test]
    fn the_timeline_and_its_zoom_send_the_same_verbs() {
        use qnc_edit_verbs::{Lane, Verb};
        let row = |frame: u64| 100 + frame;
        let cut = || "m1".to_string();
        let golden: [(TimelineIntent, &str, Verb, &str); 11] = [
            (TimelineIntent::LaneLabel { lane: AudioLane::A1, frame: Some(5) }, "segments", Verb::TakeLane { lane: Lane::A1, frame: Some(105) }, "the A1 label on a program row: the sound at the playhead (zoom: its A1 label)"),
            (TimelineIntent::LaneLabel { lane: AudioLane::A2, frame: None }, "source_timeline", Verb::TakeLane { lane: Lane::A2, frame: None }, "the A2 label on a source timeline: its lane (zoom: its A2 label)"),
            (TimelineIntent::ChooseA1Channel(2), "source_timeline", Verb::PickChannel { lane: Lane::A1, channel: 2 }, "a channel of the taken lane's column (zoom: the same column)"),
            (TimelineIntent::ChooseA2Channel(3), "segments", Verb::PickChannel { lane: Lane::A2, channel: 3 }, "the same on a program row"),
            (TimelineIntent::CueFrame(7), "segments", Verb::Cue(107), "a left click: the playhead (zoom: a click on the axis)"),
            (TimelineIntent::CueFrame(7), "source_timeline", Verb::Cue(107), "the same on the source timeline"),
            (TimelineIntent::Keep(Some(7)), "segments", Verb::Keep, "a left click while holding keeps (zoom: Save)"),
            (TimelineIntent::Keep(Some(7)), "source_timeline", Verb::Keep, "the same on the source timeline"),
            (TimelineIntent::TakeMarkerPicture { id: cut() }, "segments", Verb::TakePicture { cut: cut() }, "a second right click on the taken cut's picture (zoom: V)"),
            (TimelineIntent::TakeMarkerSound { id: cut() }, "segments", Verb::TakeSound { cut: cut(), lane: Lane::A2 }, "a right click on the taken cut's A2 (zoom: A2, a right click on its lane)"),
            (TimelineIntent::TakeCutEdge { id: cut(), frame: 3 }, "segments", Verb::TakeSoundSide { cut: cut(), frame: 103 }, "a click on a J/L line (zoom: the same line)"),
        ];
        for (intent, panel, verb, why) in golden {
            let sent = event(&intent, panel, &row).unwrap_or_else(|| panic!("{why}: nothing sent"));
            assert_eq!(qnc_edit_verbs::Verb::of(&sent), Some(verb.clone()), "{why}");
            assert_eq!(sent.at.as_deref(), Some(panel), "{why}: on the owner's panel");
        }
        // The drag of a J/L line is the zoom's DragSoundSide (the drag goes through egui memory
        // on the timeline, so its action is checked against the map directly).
        let drag = qnc_mouse_map::action("segments", qnc_mouse_map::Pointer::DragCutEdge);
        assert_eq!(drag.as_deref(), Some(qnc_edit_verbs::DRAG_SOUND_SIDE), "dragging a J/L line");
    }
}
