use super::*;

fn stored(id: &str, kind: &str, range: (u64, u64)) -> ProgramSegment {
    ProgramSegment {
        segment_id: id.into(),
        kind: kind.into(),
        sort_index: 0,
        clip_id: format!("clip-{id}"),
        in_frame: range.0,
        out_frame: range.1,
        fps_num: 50,
        fps_den: 1,
    }
}

fn marker(id: &str, program: u64) -> ProgramMarker {
    ProgramMarker {
        marker_id: id.into(),
        program_frame: program,
    }
}

/// a: 100..110 (program 0..10), b: 0..20 (program 10..30), c: 500..510 (program 30..40).
fn component() -> ProgramSegments {
    let mut segments = ProgramSegments::new();
    segments.stored = vec![
        stored("a", "ton", (100, 110)),
        stored("b", "off", (0, 20)),
        stored("c", "ton", (500, 510)),
    ];
    segments.stored_markers = vec![marker("m1", 15), marker("m2", 33)];
    segments.refresh_view(String::new());
    segments
}

#[test]
fn segments_follow_each_other_on_one_program_axis() {
    let view = program(
        &[stored("a", "ton", (100, 350)), stored("b", "off", (0, 60))],
        Some("b"),
    );
    assert_eq!(view.total_frames, 310);
    assert_eq!(view.timebase, Some((50, 1)));
    assert_eq!((view.rows[0].start_frame, view.rows[0].end_frame), (0, 250));
    assert_eq!(
        (view.rows[1].start_frame, view.rows[1].end_frame),
        (250, 310)
    );
    assert_eq!(view.rows[0].duration_label, "5:00");
    assert_eq!(view.rows[1].duration_label, "1:10");
    assert_eq!(
        view.selected().map(|row| row.segment_id.as_str()),
        Some("b")
    );
    assert!(view.rows[0].kind.has_base_video() && !view.rows[1].kind.has_base_video());
}

#[test]
fn the_segment_at_a_frame_and_at_the_end_is_found() {
    let view = program(
        &[stored("a", "ton", (0, 10)), stored("b", "ton", (0, 5))],
        None,
    );
    assert_eq!(view.segment_at(9).unwrap().segment_id, "a");
    assert_eq!(view.segment_at(10).unwrap().segment_id, "b");
    assert_eq!(view.segment_at(99).unwrap().segment_id, "b");
    assert!(program(&[], None).segment_at(0).is_none());
}

#[test]
fn duration_marks_are_three_five_and_seven_seconds() {
    assert_eq!(duration_color_key(149, 50, 1), "under_3");
    assert_eq!(duration_color_key(150, 50, 1), "under_5");
    assert_eq!(duration_color_key(300, 60000, 1001), "under_7");
    assert_eq!(duration_color_key(350, 50, 1), "over_7");
    assert_eq!(duration_label(3000, 30000, 1001), "100:00");
    assert_eq!(duration_label(10, 0, 1), "0:00");
}

#[test]
fn unknown_kinds_in_the_database_are_not_shown() {
    let view = program(&[stored("a", "voice", (0, 10))], None);
    assert!(view.is_empty());
    assert_eq!(view.label(10), "--");
}

#[test]
fn markers_stay_on_their_program_frame_and_only_live_inside_the_program() {
    let mut segments = component();
    let frames = |segments: &ProgramSegments| {
        segments
            .view()
            .markers
            .iter()
            .map(|pin| pin.frame)
            .collect::<Vec<_>>()
    };
    assert_eq!(frames(&segments), vec![15, 33]);
    // v5: moving segments keeps markers on their program frame.
    segments.stored.swap(1, 2);
    segments.refresh_view(String::new());
    assert_eq!(frames(&segments), vec![15, 33]);
    // A shorter program hides a marker at or after its end.
    segments.stored.pop();
    segments.refresh_view(String::new());
    assert_eq!(frames(&segments), vec![15]);
}

#[test]
fn slots_run_between_the_locked_ends_and_the_markers_with_stable_names() {
    let segments = component();
    let slots = &segments.view().slots;
    assert_eq!(
        slots
            .iter()
            .map(|slot| (slot.slot_id.as_str(), slot.start_frame, slot.end_frame))
            .collect::<Vec<_>>(),
        vec![
            ("program_start|m1", 0, 15),
            ("m1|m2", 15, 33),
            ("m2|program_end", 33, 40)
        ]
    );
    assert_eq!(slot_at(slots, 40).unwrap().slot_id, "m2|program_end");
    assert_eq!(slot_at(slots, 15).unwrap().slot_id, "m1|m2");
}

#[test]
fn new_and_moved_markers_respect_the_locked_ends_free_frames_and_neighbours() {
    let segments = component();
    let view = segments.view();
    assert!(check_new(view, &view.markers, 0).is_err());
    assert!(check_new(view, &view.markers, 40).is_err());
    assert!(check_new(view, &view.markers, 15).is_err());
    assert!(check_new(view, &view.markers, 20).is_ok());
    assert!(check_move(view, &view.markers, "m1", 33).is_err());
    assert!(check_move(view, &view.markers, "m1", 32).is_ok());
    assert!(check_move(view, &view.markers, "m2", 14).is_err());
}

#[test]
fn navigation_only_asks_the_player_and_the_playhead_comes_from_its_picture() {
    let mut segments = component();
    segments.apply(SegmentCommand::Select("b".into()));
    let cue = segments.take_cue().unwrap();
    assert_eq!((cue.clip_id.as_str(), cue.source_frame), ("clip-b", 0));
    assert!(segments.take_cue().is_none());
    assert_eq!(
        segments.view().playhead,
        None,
        "no playhead before the player confirms"
    );
    segments.follow_player(Some("clip-b"), Some(7));
    assert_eq!(segments.view().playhead, Some(17));
    segments.follow_player(Some("clip-b"), Some(99));
    assert_eq!(
        segments.view().playhead,
        Some(17),
        "a picture outside the segment is ignored"
    );
    segments.apply(SegmentCommand::StepMarker { up: false });
    let cue = segments.take_cue().unwrap();
    assert_eq!((cue.clip_id.as_str(), cue.source_frame), ("clip-c", 503));
    assert!(segments.view().selected_marker().is_some());
    segments.apply(SegmentCommand::SelectSlot("m1|m2".into()));
    assert!(
        segments.view().selected_marker().is_none(),
        "a slot clears the marker"
    );
    assert_eq!(segments.take_cue().unwrap().source_frame, 5);
    segments.apply(SegmentCommand::ProgramStart);
    assert_eq!(segments.take_cue().unwrap().clip_id, "clip-a");
}

#[test]
fn a_marker_needs_the_program_playhead() {
    let mut segments = component();
    segments.apply(SegmentCommand::Marker);
    assert!(segments.view().message.contains("playhead"));
}

#[test]
fn arrow_steps_select_neighbours_and_stop_at_the_ends() {
    let mut segments = component();
    let selected =
        |segments: &ProgramSegments| segments.view().selected().map(|row| row.segment_id.clone());
    segments.apply(SegmentCommand::Step { up: false });
    assert_eq!(selected(&segments).as_deref(), Some("a"));
    segments.apply(SegmentCommand::Step { up: true });
    assert_eq!(selected(&segments).as_deref(), Some("a"));
    segments.apply(SegmentCommand::Select("c".into()));
    segments.apply(SegmentCommand::Step { up: false });
    assert_eq!(selected(&segments).as_deref(), Some("c"));
    segments.apply(SegmentCommand::Step { up: true });
    assert_eq!(selected(&segments).as_deref(), Some("b"));
    let mut empty = ProgramSegments::new();
    empty.apply(SegmentCommand::Step { up: true });
    assert!(empty.view().selected().is_none());
}

#[test]
fn move_and_delete_need_a_selection_and_a_database() {
    let mut segments = component();
    segments.apply(SegmentCommand::DeleteSelected);
    assert_eq!(segments.view().message, "Odaberi segment.");
    segments.apply(SegmentCommand::Move { up: true });
    assert_eq!(segments.view().message, "Odaberi segment.");
    segments.apply(SegmentCommand::Select("a".into()));
    segments.apply(SegmentCommand::DeleteSelected);
    assert_eq!(segments.view().message, "Projektna baza nije dostupna.");
}

#[test]
fn keyboard_actions_map_to_commands() {
    assert_eq!(
        SegmentCommand::from_action("add_marker"),
        Some(SegmentCommand::Marker)
    );
    assert_eq!(
        SegmentCommand::from_action("playlist_input_start"),
        Some(SegmentCommand::ProgramStart)
    );
    assert_eq!(SegmentCommand::from_action("mark_in"), None);
    assert_eq!(SegmentCommand::from_action("play_pause"), None);
}

#[test]
fn a_wrap_cue_opens_the_clip_once_and_cues_after_its_first_picture() {
    let mut segments = component();
    segments.apply(SegmentCommand::Select("c".into()));
    assert_eq!(
        segments.drive_player(Some("clip-a"), Some(3)),
        Some(CueStep::Open("clip-c".into()))
    );
    assert_eq!(segments.drive_player(Some("clip-a"), Some(3)), None);
    assert_eq!(segments.drive_player(Some("clip-c"), None), None);
    assert_eq!(
        segments.drive_player(Some("clip-c"), Some(0)),
        Some(CueStep::Cue(500))
    );
    assert_eq!(segments.drive_player(Some("clip-c"), Some(504)), None);
    assert_eq!(segments.view().playhead, Some(34));
    segments.leave_wrap();
    assert!(!segments.in_wrap() && segments.view().playhead.is_none());
}
