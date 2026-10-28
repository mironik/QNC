use super::*;

#[test]
fn a_pointer_event_belongs_to_the_panel_it_was_on() {
    let mut focus = PanelFocus::new();
    let (mut preview, mut wrap, mut segments) = (SourcePreview::default(), WrapSession::new(), ProgramSegments::new());
    let mut timeline = TimelineProjection { duration_frames: 100, playhead_frame: Some(0), ..Default::default() };
    focus.set(Panel::Segments);
    // A right click on the source timeline is Ctrl+I there, not the slot edge of the segment panel in focus.
    focus.route(ActionEvent::key("select_mark_in").at(qnc_source_timeline::PANEL), (&mut preview, &mut wrap, &mut segments, &mut timeline));
    assert_eq!(focus.panel(), Panel::SourceTimeline);
    focus.route(ActionEvent::key("timeline_cue_frame").with_value(5u64).at("segments"), (&mut preview, &mut wrap, &mut segments, &mut timeline));
    assert_eq!(focus.panel(), Panel::Segments, "a click on a Wrap row focuses the segment panel");
    focus.route(ActionEvent::key("timeline_cue_frame").with_value(9u64).at(qnc_source_timeline::PANEL), (&mut preview, &mut wrap, &mut segments, &mut timeline));
    assert_eq!(focus.panel(), Panel::SourceTimeline, "and one on the source timeline the source timeline");
}

#[test]
fn tab_walks_the_panels_of_the_form_and_keys_on_tabs_and_buttons_become_their_actions() {
    let layout = FocusLayout::new((
        vec!["pool_tabs", "pool", "source_timeline", "source_buttons", "segments"],
        vec!["editorial_tab_all", "editorial_tab_virtual"],
        vec!["save_virtual_shot", "add_ton_segment"],
    ));
    let mut focus = PanelFocus::with_layout(layout);
    let (mut preview, mut wrap, mut segments) = (SourcePreview::default(), WrapSession::new(), ProgramSegments::new());
    let mut timeline = TimelineProjection::default();
    assert_eq!(focus.panel(), Panel::PoolTabs, "the walk starts on its first panel");
    for expected in [Panel::Pool, Panel::SourceTimeline, Panel::SourceButtons, Panel::Segments, Panel::PoolTabs] {
        focus.route("focus_next", (&mut preview, &mut wrap, &mut segments, &mut timeline));
        assert_eq!(focus.panel(), expected);
    }
    // Enter goes on round the walk: the pool tabs to the pool, where it opens a card.
    focus.route("activate_focused_item", (&mut preview, &mut wrap, &mut segments, &mut timeline));
    assert_eq!(focus.panel(), Panel::Pool);
    focus.route("step_next_part", (&mut preview, &mut wrap, &mut segments, &mut timeline));
    assert_eq!(focus.poll((&mut preview, &mut wrap, &mut segments, &mut timeline, &mut [])).pool_step, Some((PoolStep::Down, 1)));
    focus.route("step_prev_segment", (&mut preview, &mut wrap, &mut segments, &mut timeline));
    assert_eq!(focus.poll((&mut preview, &mut wrap, &mut segments, &mut timeline, &mut [])).pool_step, Some((PoolStep::Down, 1)), "the second action of the same arrow keeps it");
    focus.set(Panel::SourceTimeline);
    focus.route("activate_focused_item", (&mut preview, &mut wrap, &mut segments, &mut timeline));
    assert_eq!(focus.panel(), Panel::SourceButtons, "nothing to confirm on the source: on to its buttons");
    assert_eq!(focus.route("mark_in", (&mut preview, &mut wrap, &mut segments, &mut timeline)), Some(true), "I still marks: the buttons are part of the source timeline panel");
    focus.set(Panel::PoolTabs);
    assert_eq!(focus.route("step_forward_frame", (&mut preview, &mut wrap, &mut segments, &mut timeline)), None);
    assert_eq!(focus.take_pick().as_deref(), Some("editorial_tab_virtual"), "the right arrow is the next tab");
    focus.set(Panel::SourceButtons);
    focus.route("step_forward_frame", (&mut preview, &mut wrap, &mut segments, &mut timeline));
    assert_eq!(focus.poll((&mut preview, &mut wrap, &mut segments, &mut timeline, &mut [])).button.as_deref(), Some("add_ton_segment"));
    assert_eq!(cycle(Panel::SourceTimeline, false, &[Panel::Pool, Panel::SourceTimeline]), Panel::Pool);
}

/// Locked (user rule 2026-10-20): Ctrl+wheel is Ctrl+arrows: a taken IN moves with it.
#[test]
fn ctrl_wheel_moves_a_taken_in_as_on_every_source_timeline() {
    let mut focus = PanelFocus::new();
    let mut preview = SourcePreview::default();
    let mut wrap = WrapSession::new();
    let mut segments = ProgramSegments::new();
    let mut timeline = TimelineProjection { duration_frames: 100, playhead_frame: Some(20), source_in_frame: Some(10), source_out_frame: Some(50), ..Default::default() };
    focus.set(Panel::SourceTimeline);
    assert_eq!(focus.route("wheel_source_ctrl_forward", (&mut preview, &mut wrap, &mut segments, &mut timeline)), Some(false), "nothing taken: nothing moves");
    focus.route("select_mark_in", (&mut preview, &mut wrap, &mut segments, &mut timeline));
    focus.route("wheel_source_ctrl_forward", (&mut preview, &mut wrap, &mut segments, &mut timeline));
    assert_ne!(timeline.source_in_frame, Some(10), "the taken IN moved a frame");
}

#[test]
fn the_source_timeline_takes_i_o_and_the_segments_take_markers() {
    let mut focus = PanelFocus::new();
    let mut preview = SourcePreview::default();
    let mut wrap = WrapSession::new();
    let mut segments = ProgramSegments::new();
    focus.set(Panel::Segments);
    assert_eq!(
        focus.route("mark_in", (&mut preview, &mut wrap, &mut segments, &mut TimelineProjection::default())),
        Some(false),
        "I does nothing on the segment panel"
    );
    focus.set(Panel::SourceTimeline);
    assert_eq!(
        focus.route("mark_in", (&mut preview, &mut wrap, &mut segments, &mut TimelineProjection::default())),
        Some(true),
        "the source timeline marks IN (one owner)"
    );
    focus.set(Panel::Pool);
    assert_eq!(
        focus.route("mark_out", (&mut preview, &mut wrap, &mut segments, &mut TimelineProjection::default())),
        Some(true),
        "a clip chosen in the pool is marked from the pool"
    );
    assert_eq!(
        focus.route("add_marker", (&mut preview, &mut wrap, &mut segments, &mut TimelineProjection::default())),
        Some(false),
        "M belongs to the segment panel"
    );
    assert_eq!(
        focus.route(
            "editorial_tab_all",
            (&mut preview, &mut wrap, &mut segments, &mut TimelineProjection::default())
        ),
        None
    );
}

#[test]
fn keys_act_on_the_panel_in_focus_only() {
    let mut focus = PanelFocus::new();
    let (mut preview, mut wrap, mut segments) = (SourcePreview::default(), WrapSession::new(), ProgramSegments::new());
    let mut timeline = TimelineProjection::default();
    focus.set(Panel::Pool);
    assert_eq!(focus.route("add_marker", (&mut preview, &mut wrap, &mut segments, &mut timeline)), Some(false), "M is the segment panel's");
}

#[test]
fn ctrl_2_takes_a2_on_the_source_timeline_from_any_panel() {
    let mut focus = PanelFocus::new();
    let (mut preview, mut wrap, mut segments) = (SourcePreview::default(), WrapSession::new(), ProgramSegments::new());
    let mut timeline = TimelineProjection::default();
    focus.set(Panel::Pool);
    // A clip without a known channel count: the lane is not taken, the keyboard is on the source timeline.
    assert_eq!(focus.route("select_audio_a2", (&mut preview, &mut wrap, &mut segments, &mut timeline)), Some(false));
    assert_eq!(focus.panel(), Panel::SourceTimeline);
    assert!(!focus.source_timeline().lane_taken());
    assert!(!focus.source_timeline().view().message.is_empty(), "it says why");
}

#[test]
fn the_arrows_move_the_playhead_until_ctrl_i_or_ctrl_o_takes_a_mark() {
    let mut focus = PanelFocus::new();
    let (mut preview, mut wrap, mut segments) = (SourcePreview::default(), WrapSession::new(), ProgramSegments::new());
    focus.set(Panel::SourceTimeline);
    // I marked IN: the timeline paints its focus on IN, the arrows stay the playhead's.
    let mut timeline = TimelineProjection { duration_frames: 100, playhead_frame: Some(0), source_in_frame: Some(10), ..Default::default() }.focus_source_in();
    focus.route("step_forward_frame", (&mut preview, &mut wrap, &mut segments, &mut timeline));
    assert_eq!(timeline.source_in_frame, Some(10), "the arrow stepped the playhead");
    focus.route("select_mark_in", (&mut preview, &mut wrap, &mut segments, &mut timeline));
    focus.route("step_forward_frame", (&mut preview, &mut wrap, &mut segments, &mut timeline));
    assert_eq!(timeline.source_in_frame, Some(11), "select_mark_in took IN");
    focus.route("clear_focus", (&mut preview, &mut wrap, &mut segments, &mut timeline));
    focus.route("step_forward_frame", (&mut preview, &mut wrap, &mut segments, &mut timeline));
    assert_eq!(timeline.source_in_frame, Some(11), "Escape gave the arrows back");
}
