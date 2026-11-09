//! What becomes of every pointer action of the mouse map on every timeline (user 2026-11-09:
//! "all timelines and their zooms must be based on one code, the difference is only in the
//! display"; docs/97). A click on V was lost twice because each owner of an axis keeps its own
//! list of what it takes. Every line is one timeline, one panel the keyboard was on and one
//! action: `reached` an owner answered, `LOST` nobody took it. The golden file is what is lost
//! today and may only lose lines; `QNC_WRITE_GOLDEN=1` writes it again.
//!
//! The zoom sends the same actions on the panel of its owner (`qnc-edit-verbs`), so the Story
//! lines stand for its zoom too. The keys of an open SubSegment need its script and are not here.

use qnc_action_event::ActionEvent;
use qnc_focus_walk::{FocusWalk, Walk};
use qnc_panel_focus::{FocusLayout, Panel, PanelFocus};
use qnc_program_segments::ProgramSegments;
use qnc_source_preview::SourcePreview;
use qnc_source_timeline::SourceTimeline;
use qnc_timeline_model::TimelineProjection;
use qnc_wrap_session::WrapSession;

const GOLDEN: &str = "tests/pointer_matrix.golden";

fn layout() -> FocusLayout {
    FocusLayout::new((
        vec!["pool_tabs", "pool", "source_timeline", "source_buttons", "segments"],
        vec!["editorial_tab_all", "editorial_tab_virtual", "editorial_tab_broll"],
        vec!["save_virtual_shot", "add_ton_segment", "add_off_segment"],
        vec!["segment_panel"],
    ))
}

fn timeline() -> TimelineProjection {
    TimelineProjection { duration_frames: 100, playhead_frame: Some(20), source_in_frame: Some(10), source_out_frame: Some(50), ..Default::default() }
}

/// The action as the timeline sends it: the frame or channel it names and the object under the
/// pointer, on its panel.
fn event(panel: &str, action: &str) -> ActionEvent {
    let value = if action.contains("channel") { 1u64 } else { 60 };
    let target = match action {
        "timeline_select_slots" => Some("part-0"),
        "timeline_drag_marker" | "select_marker" | "select_mark_in" | "select_audio_a2" if panel == "segments" => Some("edge-1-out"),
        "timeline_take_cut_edge" | "timeline_drag_cut_edge" => Some("edge-1-out:lead"),
        "timeline_select_slot" | "timeline_select_cover" | "timeline_take_cover" | "select_current_marker_slot" | "segment_edit" | "wrap_zoom" => Some("part-1"),
        _ => None,
    };
    let event = ActionEvent::key(action).with_value(value).at(panel);
    match target {
        Some(target) => event.with_target(target),
        None => event,
    }
}

fn story(panel_before: Panel, panel: &str, action: &str) -> &'static str {
    let mut focus = PanelFocus::with_layout(layout());
    focus.set(panel_before);
    let (mut preview, mut wrap, mut segments, mut shown) = (SourcePreview::default(), WrapSession::new(), ProgramSegments::new(), timeline());
    match focus.route(event(panel, action), (&mut preview, &mut wrap, &mut segments, &mut shown)) {
        Some(_) => "reached",
        None => "LOST",
    }
}

fn ingest(walk: &mut FocusWalk, on: &str, action: &str) -> &'static str {
    walk.go_to(on);
    match walk.route(action) {
        Walk::Done(false) => "LOST",
        Walk::Pass => {
            let (mut source, mut preview, mut shown) = (SourceTimeline::default(), SourcePreview::default(), timeline());
            let answered = source.route_input(action, Some(60), &mut preview, &mut shown).or_else(|| source.route(action, &mut preview, &mut shown));
            // The click that places the playhead has its own arm in Ingest's preview source.
            if answered.is_some() || action == "timeline_cue_frame" { "reached" } else { "LOST" }
        }
        _ => "reached",
    }
}

/// Actions an open SubSegment answers by its keys, with what is taken, not by its pointer map.
const SUBSEGMENT_KEYS: [&str; 3] = ["source_choose_a1_channel", "timeline_take_cut_edge", "timeline_drag_cut_edge"];

/// The one table says what has no meaning on the SubSegment's timeline (it is not lost); the
/// rest must be answered by its own map of a pointer or its keys.
fn subsegment(action: &str) -> &'static str {
    let part = |start, end| qnc_script::SubsegmentPart { start, end, clip_name: String::new(), length: String::new(), gap_before: 0, cut: false, a1_channel: None, a1_level_db: 0, a1_off: false, channels: None, a1_lead: 0, a1_tail: 0, smooth_frames: 0, smooth_ready: None };
    let view = qnc_script::SubsegmentView { parts: vec![part(0, 50), part(50, 90), part(95, 145)], total_frames: 145, ..Default::default() };
    let sent = event("segments", action);
    let no_meaning = !qnc_mouse_map::takes("subsegment", "segments", action);
    if no_meaning || SUBSEGMENT_KEYS.contains(&action) || qnc_script::own_action(&view, action, sent.value, sent.target.as_deref(), false).is_some() { "reached" } else { "LOST" }
}

/// Actions that answer nothing here only because this test has no clip shown or nothing taken:
/// they are not lost.
const NEED_A_CLIP_OR_WHAT_IS_TAKEN: [&str; 4] = ["select_level_a1", "select_level_a2", "wheel_segments_ctrl_back", "wheel_segments_ctrl_forward"];

#[test]
fn what_is_lost_of_the_mouse_map_only_shrinks() {
    let map = qnc_mouse_map::MouseMap::of_this_qnc();
    let contracts = qnc_ingest_layout::IngestContracts::load().expect("ingest layout");
    let mut walk = FocusWalk::new(contracts.focus_names());
    let mut lines = Vec::new();
    for (panel, pointer, action) in map.entries() {
        let name = pointer.name();
        match panel {
            "source_timeline" => {
                for (on, before) in [("pool", Panel::Pool), ("source_timeline", Panel::SourceTimeline), ("segments", Panel::Segments)] {
                    lines.push(format!("{} story-source on={on} {name} {action}", story(before, panel, action)));
                }
                for on in ["clip-cards", "source_timeline"] {
                    lines.push(format!("{} ingest-source on={on} {name} {action}", ingest(&mut walk, on, action)));
                }
            }
            _ => {
                for (on, before) in [("pool", Panel::Pool), ("segments", Panel::Segments)] {
                    lines.push(format!("{} story-wrap on={on} {name} {action}", story(before, panel, action)));
                }
                lines.push(format!("{} subsegment {name} {action}", subsegment(action)));
            }
        }
    }
    let lost: Vec<&String> = {
        lines.sort();
        lines.iter().filter(|line| line.starts_with("LOST") && (line.contains(" subsegment ") || !NEED_A_CLIP_OR_WHAT_IS_TAKEN.iter().any(|action| line.ends_with(action)))).collect()
    };
    let got = lost.iter().map(|line| format!("{line}\n")).collect::<String>();
    if std::env::var_os("QNC_WRITE_GOLDEN").is_some() {
        std::fs::write(GOLDEN, &got).expect("write golden");
        return;
    }
    let allowed = std::fs::read_to_string(GOLDEN).expect("golden");
    let new: Vec<&&String> = lost.iter().filter(|line| !allowed.lines().any(|known| known == line.as_str())).collect();
    assert!(new.is_empty(), "newly lost: {new:#?}");
    assert_eq!(allowed, got, "fewer are lost: write the golden file again");
}
