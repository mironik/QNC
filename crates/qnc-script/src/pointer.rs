//! The SubSegment's timeline on the one mouse map (AGENTS 0.12, docs/97 step 2, user
//! 2026-11-04): its panel sends what the mouse map gives a row of a program (`segments`,
//! contracts/input/mouse.json), the same as a Wrap row; the script, the owner of the
//! SubSegment's axis, says what each of those actions does to its parts. No gesture is mapped
//! by hand in the panel any more.

use crate::{SubsegmentView, DRAG_EDGE, SUBSEGMENT_CUE, SUBSEGMENT_SELECT, TAKE_EDGE};

/// The SubSegment's own action of a mouse-map action on its row (`value` the frame of the
/// SubSegment's axis, `target` what the pointer points at). `holding`: an edge, a cut, a part's
/// sound or a part is taken (a left click then keeps it, as Enter). `None`: not a pointer action,
/// or one the SubSegment does nothing with.
pub fn own_action(view: &SubsegmentView, action_id: &str, value: Option<i64>, target: Option<&str>, holding: bool) -> Option<(String, Option<String>)> {
    let frame = value.and_then(|value| u64::try_from(value).ok());
    let part_at = |frame: u64| view.parts.iter().position(|part| frame >= part.start && frame < part.end);
    let own = |action: &str, target: Option<String>| Some((action.to_string(), target));
    match action_id {
        // While something is held a left click keeps it, as Enter (the mouse rule).
        "segments_keep_edit" => own("activate_focused_item", None),
        "timeline_cue_frame" | "timeline_select_slot" | "timeline_select_cover" if holding => own("activate_focused_item", None),
        "timeline_cue_frame" => own(SUBSEGMENT_CUE, Some(frame?.to_string())),
        // A click on a part's length strip chooses the part, as a cover's strip chooses its slot.
        "timeline_select_slot" | "timeline_select_cover" => own(SUBSEGMENT_SELECT, Some(part_at(frame?)?.to_string())),
        // A right click (or Ctrl + click) on the strip opens the slot's zoom (user 2026-11-05: "a
        // right click on the selected slot in its strip opens the zoom in slot mode, the cuts are
        // the slot's, longer or shorter"): the slot is chosen and its IN or OUT nearer the click
        // is taken, its own edge (never the roll); the other slots keep their lengths.
        "select_current_marker_slot" | "timeline_take_cover" => {
            let index = target.and_then(|id| id.strip_prefix("part-")).and_then(|index| index.parse::<usize>().ok()).or_else(|| frame.and_then(part_at))?;
            let part = view.parts.get(index)?;
            let out = frame.is_some_and(|frame| frame.abs_diff(part.end) < frame.abs_diff(part.start));
            own(TAKE_EDGE, Some(format!("{index}:{}:slot", if out { "out" } else { "in" })))
        }
        // A right click on A1 (or a click on its label): the sound of the part there, as Ctrl+1.
        // (Only a pointer gives a frame: Ctrl+1 has none and stays the part in hand.)
        "select_audio_a1" if target.is_none_or(|target| target.parse::<usize>().is_err()) => own("select_audio_a1", Some(part_at(frame?)?.to_string())),
        // A right click near an edge takes the IN or OUT nearest to it; at a join the whole cut.
        "select_marker" | "select_mark_in" if target.is_some_and(|id| id.starts_with("edge-")) => {
            let at = frame.or_else(|| edge_frame(view, target?))?;
            let (index, side) = nearest_edge(view, at)?;
            own(TAKE_EDGE, Some(format!("{index}:{side}")))
        }
        // A drag of the taken edge (only what is taken moves): the edge follows the pointer.
        "timeline_drag_marker" => own(DRAG_EDGE, Some(drag_target(target?, frame?)?)),
        // The wheel: the arrows, Ctrl+wheel the Ctrl+arrows of the source lane (the part's level).
        "wheel_segments_back" | "wheel_segments_forward" => own(qnc_edit_step::arrow(qnc_edit_step::frames(action_id)?), None),
        "wheel_segments_ctrl_back" => own("wheel_source_ctrl_back", None),
        "wheel_segments_ctrl_forward" => own("wheel_source_ctrl_forward", None),
        _ => None,
    }
}

/// The mouse map's row actions the SubSegment takes; any other a row would send (an M marker,
/// a Wrap zoom, a segment edit) is not the SubSegment's and goes nowhere.
pub const ROW_ACTIONS: [&str; 16] = [
    "timeline_cue_frame",
    "segments_keep_edit",
    "timeline_select_slot",
    "timeline_select_cover",
    "select_current_marker_slot",
    "timeline_take_cover",
    "select_audio_a1",
    "select_marker",
    "select_mark_in",
    "timeline_drag_marker",
    "timeline_take_cut_edge",
    "timeline_drag_cut_edge",
    "wheel_segments_back",
    "wheel_segments_forward",
    "wheel_segments_ctrl_back",
    "wheel_segments_ctrl_forward",
];

/// The frame an edge marker stands on.
fn edge_frame(view: &SubsegmentView, id: &str) -> Option<u64> {
    let mut parts = id.strip_prefix("edge-")?.split('-');
    let part = view.parts.get(parts.next()?.parse::<usize>().ok()?)?;
    match parts.next()? {
        "in" => Some(part.start),
        "out" => Some(part.end),
        _ => None,
    }
}

/// The target of a drag of edge marker `id` ("edge-{part}-in|out") to `frame`: `part:in|out:frame`.
pub fn drag_target(id: &str, frame: u64) -> Option<String> {
    let mut parts = id.strip_prefix("edge-")?.split('-');
    let (index, side) = (parts.next()?.parse::<usize>().ok()?, parts.next()?);
    matches!(side, "in" | "out").then(|| format!("{index}:{side}:{frame}"))
}

/// The edge nearest to `frame`: its part and its side, `in`, `out` or `cut`. A right click takes
/// the whole of what it is on, as on a cover's row (user 2026-11-03): where an OUT and the next
/// IN meet (a join without a pause) that is the cut of both (`cut`, on the part before it, a
/// roll); an IN after a pause, an OUT before one, the first IN and the last OUT are their own.
/// Only one side of a cut is taken in its zoom (the picture left of the cut its OUT, the one
/// right of it the next IN) or with Ctrl+I / Ctrl+O.
pub fn nearest_edge(view: &SubsegmentView, frame: u64) -> Option<(usize, &'static str)> {
    let (index, out, at) = view
        .parts
        .iter()
        .enumerate()
        .flat_map(|(index, part)| [(index, false, part.start), (index, true, part.end)])
        .min_by_key(|(_, out, at)| (frame.abs_diff(*at), (*out && frame >= *at) || (!*out && frame < *at)))?;
    let joined = |before: usize| view.parts.get(before + 1).is_some_and(|next| next.start == view.parts[before].end);
    Some(match (out, index) {
        (true, index) if joined(index) => (index, "cut"),
        (false, index) if index > 0 && joined(index - 1) && view.parts[index - 1].end == at => (index - 1, "cut"),
        (true, index) => (index, "out"),
        (false, index) => (index, "in"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SubsegmentPart;

    fn part(start: u64, end: u64) -> SubsegmentPart {
        SubsegmentPart { start, end, clip_name: String::new(), length: String::new(), gap_before: 0, cut: false, a1_channel: None, a1_level_db: 0, a1_off: false, channels: None, a1_lead: 0, a1_tail: 0 }
    }

    fn view() -> SubsegmentView {
        SubsegmentView { parts: vec![part(0, 50), part(50, 90), part(95, 145)], total_frames: 145, ..Default::default() }
    }

    #[test]
    fn a_right_click_takes_every_in_and_out_of_every_part() {
        let view = view();
        assert_eq!(nearest_edge(&view, 2), Some((0, "in")), "the first part's IN");
        assert_eq!(nearest_edge(&view, 48), Some((0, "cut")), "a join without a pause is one cut, before it");
        assert_eq!(nearest_edge(&view, 52), Some((0, "cut")), "and after it");
        assert_eq!(nearest_edge(&view, 91), Some((1, "out")), "an OUT before a pause");
        assert_eq!(nearest_edge(&view, 94), Some((2, "in")), "an IN after a pause");
        assert_eq!(nearest_edge(&view, 140), Some((2, "out")), "the last part's OUT");
    }

    #[test]
    fn a_drag_of_an_edge_names_its_part_side_and_frame() {
        assert_eq!(drag_target("edge-2-out", 120).as_deref(), Some("2:out:120"));
        assert_eq!(drag_target("edge-0-in", 3).as_deref(), Some("0:in:3"));
        assert_eq!(drag_target("part-1", 3), None);
    }

    /// The mouse map's actions of a program row are the SubSegment's own (AGENTS 0.12): the same
    /// gesture on a Wrap row and on the SubSegment's timeline.
    #[test]
    fn the_mouse_maps_row_actions_are_the_subsegments_own() {
        let view = view();
        let own = |action: &str, value: Option<i64>, target: Option<&str>, holding: bool| own_action(&view, action, value, target, holding).map(|(action, target)| (action, target));
        let some = |action: &str, target: Option<&str>| Some((action.to_string(), target.map(str::to_string)));
        assert_eq!(own("timeline_cue_frame", Some(60), None, false), some(SUBSEGMENT_CUE, Some("60")), "a click: the playhead");
        assert_eq!(own("timeline_cue_frame", Some(60), None, true), some("activate_focused_item", None), "a click while holding keeps");
        assert_eq!(own("segments_keep_edit", Some(60), None, true), some("activate_focused_item", None));
        assert_eq!(own("timeline_select_cover", Some(60), Some("part-1"), false), some(SUBSEGMENT_SELECT, Some("1")), "a click on the strip chooses the part");
        assert_eq!(own("timeline_take_cover", Some(100), Some("part-2"), false), some(TAKE_EDGE, Some("2:in:slot")), "a right click on the strip: the slot and its nearer IN");
        assert_eq!(own("timeline_take_cover", Some(140), Some("part-2"), false), some(TAKE_EDGE, Some("2:out:slot")), "or its nearer OUT");
        assert_eq!(own("select_audio_a1", Some(100), None, false), some("select_audio_a1", Some("2")), "a right click on A1: that part's sound");
        assert_eq!(own("select_marker", Some(48), Some("edge-0-out"), false), some(TAKE_EDGE, Some("0:cut")), "a right click near a join: the cut");
        assert_eq!(own("select_marker", Some(140), Some("edge-2-out"), false), some(TAKE_EDGE, Some("2:out")));
        assert_eq!(own("timeline_drag_marker", Some(120), Some("edge-2-out"), true), some(DRAG_EDGE, Some("2:out:120")), "a drag of the taken edge");
        assert_eq!(own("export_open", None, None, false), None, "not a pointer action");
    }
}
