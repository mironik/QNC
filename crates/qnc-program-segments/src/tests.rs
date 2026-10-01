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
        a1_source_channel: 0,
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
fn navigation_asks_only_for_a_program_frame_never_for_a_source_clip() {
    let mut segments = component();
    segments.apply(SegmentCommand::Select("b".into()));
    assert_eq!(
        segments.take_seek(),
        Some(10),
        "the start of b on the program axis"
    );
    assert_eq!(segments.take_seek(), None);
    assert_eq!(
        segments.view().playhead,
        Some(10),
        "v5: the playhead goes to the frame at once"
    );
    segments.apply(SegmentCommand::Cue(33));
    assert_eq!(segments.take_seek(), Some(33));
    assert_eq!(
        segments.view().selected().unwrap().segment_id,
        "c",
        "the segment under the playhead becomes the selected one"
    );
    segments.set_playhead(Some(20));
    segments.apply(SegmentCommand::StepMarker { up: false });
    assert_eq!(segments.take_seek(), Some(33));
    assert!(segments.view().selected_marker().is_some());
    segments.apply(SegmentCommand::SelectSlot {
        slot_id: "m1|m2".into(),
        frame: 20,
    });
    assert!(
        segments.view().selected_marker().is_none(),
        "a slot clears the marker"
    );
    assert_eq!(
        segments.take_seek(),
        Some(20),
        "the clicked frame, not the slot start"
    );
    segments.apply(SegmentCommand::ProgramStart);
    assert_eq!(segments.take_seek(), Some(0));
    segments.apply(SegmentCommand::Cue(999));
    assert_eq!(segments.take_seek(), Some(40), "never past the program end");
    assert!(ProgramSegments::new().take_seek().is_none());
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

fn cover(id: &str, slot_id: &str, frames: (u64, u64)) -> ProgramCover {
    ProgramCover {
        cover_id: id.into(),
        slot_id: slot_id.into(),
        clip_id: "clip-x".into(),
        virtual_shot_id: "shot".into(),
        program_start_frame: frames.0,
        program_end_frame: frames.1,
        source_in_frame: 0,
        source_out_frame: 50,
        fps_num: 50,
        fps_den: 1,
        a2_source_channel: 0,
    }
}

#[test]
fn a_cover_is_drawn_over_its_slot_and_targets_follow_v5() {
    let mut segments = component();
    segments.stored_covers = vec![cover("k", "S|m1", (0, 15))];
    segments.selected_slot = Some("S|m1".into());
    segments.refresh_view(String::new());
    let view = segments.view();
    assert_eq!(
        (view.covers[0].start_frame, view.covers[0].end_frame),
        (0, 15)
    );
    assert_eq!(
        view.quick_cover_slot().unwrap_err(),
        "Odabrani marker slot već ima pokrivalicu",
        "B needs an empty slot"
    );
    assert_eq!(view.overwrite_cover_slot(), Ok("S|m1"));
    segments.selected_slot = None;
    segments.selected_cover = Some("k".into());
    segments.refresh_view(String::new());
    assert_eq!(
        segments.view().overwrite_cover_slot(),
        Ok("S|m1"),
        "Overwrite takes the selected cover's slot"
    );
    assert!(segments.view().quick_cover_slot().is_err());
}

#[test]
fn a_cover_needs_the_marked_source_and_a_segment_click_clears_the_cover() {
    let mut segments = component();
    segments.stored_covers = vec![cover("k", "S|m1", (0, 15))];
    segments.selected_slot = Some("m1|m2".into());
    segments.refresh_view(String::new());
    segments.apply(SegmentCommand::Cover { overwrite: false });
    assert_eq!(
        segments.view().message,
        "Odaberi klip i potvrdi IN i OUT na playeru."
    );
    assert!(SegmentCommand::from_action("quick_overwrite_cover").is_some());
    assert_eq!(
        SegmentCommand::from_action("overwrite_cover"),
        Some(SegmentCommand::Cover { overwrite: true })
    );
    segments.apply(SegmentCommand::SelectCover {
        cover_id: "k".into(),
        frame: 4,
    });
    assert_eq!(
        segments
            .view()
            .selected_cover()
            .map(|c| c.cover_id.as_str()),
        Some("k")
    );
    assert_eq!(segments.view().playhead, Some(4));
    segments.select("c");
    assert!(
        segments.view().selected_cover().is_none(),
        "v5: choosing a segment clears the cover"
    );
    segments.selected_cover = Some("k".into());
    segments.apply(SegmentCommand::DeleteSelected);
    assert!(
        segments.selected_cover.is_none(),
        "Delete takes the cover first"
    );
    assert_eq!(segments.selected.as_deref(), Some("c"), "the segment stays");
}

fn marked(in_mark: u64) -> SourcePick {
    SourcePick::new(
        Some("clip-x"),
        Some("X"),
        Some((in_mark, 60)),
        (Some(in_mark), 60),
        Some((50, 1)),
    )
}

#[test]
fn sync_starts_from_the_marker_before_the_playhead_after_a_new_in() {
    let mut segments = component();
    segments.set_playhead(Some(20));
    segments.set_source(marked(10), None);
    assert_eq!(segments.sync_space(false), SyncSpace::Play, "Sync is off");
    segments.apply(SegmentCommand::ToggleSync);
    assert!(segments.view().sync_enabled);
    assert_eq!(
        segments.sync_space(false),
        SyncSpace::Play,
        "IN before Sync does not arm"
    );
    segments.set_source(marked(12), None);
    assert_eq!(
        segments.sync_space(true),
        SyncSpace::Play,
        "Space in Wrap plays"
    );
    segments.arm_sync();
    segments.set_source(marked(12), None);
    assert!(
        matches!(segments.sync_space(false), SyncSpace::Start(_)),
        "IN again on the same frame arms it again (v5)"
    );
    segments.finish_sync();
    segments.set_source(marked(13), None);
    let SyncSpace::Start(preview) = segments.sync_space(false) else {
        panic!("Sync starts in the Source view")
    };
    assert_eq!(
        preview.window,
        (15, 33),
        "from m1 at 15 to the next marker m2 at 33 (before the source OUT)"
    );
    assert_eq!(preview.source_in, 13);
    assert_eq!(segments.sync_frame(Some(0)), Some(15));
    assert_eq!(segments.sync_frame(Some(8)), Some(23));
    segments.set_playhead(Some(23));
    assert!(segments.finish_sync(), "O closes the slot");
    assert!(
        segments.take_program_changed(),
        "the whole program opens again"
    );
    assert!(
        segments.sync_holds_enter(),
        "the slot waits for its marker and Enter"
    );
    assert!(!segments.finish_sync(), "no Sync play any more");
    segments.resolve_sync();
    assert_eq!(
        segments.view().message,
        "Sync slot još nije materijaliziran",
        "v5: no stored slot 15..23 while its end marker is missing"
    );
}

#[test]
fn the_panel_header_shows_program_time_as_timecode() {
    let mut view = SegmentsView::default();
    assert_eq!(view.timecode(10), "--:--:--:--", "no story rate yet");
    view.timebase = Some((50, 1));
    assert_eq!(view.timecode(0), "00:00:00:00");
    assert_eq!(view.timecode(1837), "00:00:36:37");
}

#[test]
fn a1_is_chosen_from_the_real_channels_of_the_marked_clip_and_starts_on_channel_one() {
    let mut segments = component();
    segments.set_source(marked(10), None);
    assert_eq!(segments.view().a1_choice, None, "no count, no choice");
    segments.set_source_channels(Some(4));
    assert_eq!(segments.view().a1_choice, Some((0, 4)), "v5: channel 1 first");
    assert!(segments.choose_a1_channel(1));
    assert_eq!(segments.view().a1_choice, Some((1, 4)));
    assert!(!segments.choose_a1_channel(4), "a channel the clip does not have");
    let mut other = marked(10);
    other.clip_id = Some("clip-y".into());
    segments.set_source(other, None);
    segments.set_source_channels(Some(4));
    assert_eq!(segments.view().a1_choice, Some((0, 4)), "another clip starts on channel 1");
    assert!(segments.choose_a1_channel(3));
    segments.set_source_channels(Some(2));
    assert_eq!(segments.view().a1_choice, Some((0, 2)), "back to 1 when the clip has fewer");
}
