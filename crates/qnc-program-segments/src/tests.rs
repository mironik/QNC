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
        active: true,
    }
}

fn marker(id: &str, program: u64, role: &str) -> ProgramMarker {
    ProgramMarker {
        marker_id: id.into(),
        program_frame: program,
        system_role: role.into(),
    }
}

fn slot(start: (&str, u64), end: (&str, u64), has_cover: bool) -> ProgramSlot {
    ProgramSlot {
        slot_id: format!("{}|{}", start.0, end.0),
        start_frame: start.1,
        end_frame: end.1,
        start_marker_id: start.0.into(),
        end_marker_id: end.0.into(),
        has_cover,
    }
}

/// a: 100..110 (program 0..10), b: 0..20 (program 10..30), c: 500..510 (program 30..40);
/// markers S at 0 (locked), m1 at 15, m2 at 33, E at 40 (locked), as the store gives them.
fn component() -> ProgramSegments {
    let mut segments = ProgramSegments::new();
    segments.stored = vec![
        stored("a", "tonovi", (100, 110)),
        stored("b", "offovi", (0, 20)),
        stored("c", "tonovi", (500, 510)),
    ];
    segments.stored_markers = vec![
        marker("S", 0, "program_start"),
        marker("m1", 15, ""),
        marker("m2", 33, ""),
        marker("E", 40, "program_end"),
    ];
    segments.stored_slots = vec![
        slot(("S", 0), ("m1", 15), true),
        slot(("m1", 15), ("m2", 33), false),
        slot(("m2", 33), ("E", 40), false),
    ];
    segments.refresh_view(String::new());
    segments
}

#[test]
fn segments_follow_each_other_on_one_program_axis() {
    let view = program(
        &[
            stored("a", "tonovi", (100, 350)),
            stored("b", "offovi", (0, 60)),
        ],
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
fn deleted_segments_stay_in_the_segment_tab_but_not_in_the_program() {
    let mut deleted = stored("b", "offovi", (0, 60));
    deleted.active = false;
    let view = program(
        &[
            stored("a", "tonovi", (0, 10)),
            deleted,
            stored("c", "tonovi", (0, 5)),
        ],
        Some("b"),
    );
    assert_eq!(view.total_frames, 15);
    assert_eq!(view.rows.len(), 2);
    assert_eq!(
        view.parts
            .iter()
            .map(|part| (part.segment_id.as_str(), part.active))
            .collect::<Vec<_>>(),
        vec![("a", true), ("b", false), ("c", true)]
    );
    assert!(!view.parts[1].selected, "a deleted segment is not selected");
}

#[test]
fn the_segment_at_a_frame_and_at_the_end_is_found() {
    let view = program(
        &[
            stored("a", "tonovi", (0, 10)),
            stored("b", "tonovi", (0, 5)),
        ],
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
fn markers_and_slots_come_from_the_store_with_the_locked_ends() {
    let segments = component();
    let view = segments.view();
    assert_eq!(
        view.markers
            .iter()
            .map(|pin| (pin.marker_id.as_str(), pin.frame, pin.locked))
            .collect::<Vec<_>>(),
        vec![
            ("S", 0, true),
            ("m1", 15, false),
            ("m2", 33, false),
            ("E", 40, true)
        ]
    );
    assert_eq!(
        view.slots
            .iter()
            .map(|slot| (slot.slot_id.as_str(), slot.has_cover))
            .collect::<Vec<_>>(),
        vec![("S|m1", true), ("m1|m2", false), ("m2|E", false)]
    );
    assert_eq!(slot_at(&view.slots, 40).unwrap().slot_id, "m2|E");
    assert_eq!(slot_at(&view.slots, 15).unwrap().slot_id, "m1|m2");
    assert_eq!(first_empty_slot(&view.slots).unwrap().slot_id, "m1|m2");
}

#[test]
fn a_selected_marker_moves_only_between_its_neighbours_and_the_ends_are_locked() {
    let segments = component();
    let pins = &segments.view().markers;
    assert!(check_move(pins, "m1", 33).is_err());
    assert!(check_move(pins, "m1", 32).is_ok());
    assert!(check_move(pins, "m2", 14).is_err());
    assert!(check_move(pins, "m2", 39).is_ok());
    assert!(check_move(pins, "E", 38).is_err());
    assert!(check_move(pins, "S", 1).is_err());
}

#[test]
fn slot_steps_start_from_the_selection_else_the_playhead_else_the_first_empty_slot() {
    let segments = component();
    let slots = &segments.view().slots;
    assert_eq!(neighbour_slot(slots, 20, false).unwrap().slot_id, "m2|E");
    assert_eq!(neighbour_slot(slots, 20, true).unwrap().slot_id, "S|m1");
    assert!(
        neighbour_slot(slots, 5, true).is_none(),
        "no wrapping around"
    );
}

#[test]
fn the_locked_start_marker_is_not_selected() {
    let mut segments = component();
    segments.apply(SegmentCommand::SelectMarker("S".into()));
    assert!(segments.view().selected_marker().is_none());
    assert!(segments.view().message.contains("zaključan"));
    segments.apply(SegmentCommand::SelectMarker("E".into()));
    assert_eq!(segments.view().selected_marker().unwrap().marker_id, "E");
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
