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
        a1_level_db: 0,
        a1_off: false,
        duration_frames: range.1 - range.0,
        v_lead_frames: 0,
        a1_lead_frames: 0,
        a1_tail_frames: 0,
        v_tail_frames: 0,
        lead: String::new(),
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
        None,
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
        None,
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
        None,
    );
    assert_eq!(view.segment_at(9).unwrap().segment_id, "a");
    assert_eq!(view.segment_at(10).unwrap().segment_id, "b");
    assert_eq!(view.segment_at(99).unwrap().segment_id, "b");
    assert!(program(&[], None, None).segment_at(0).is_none());
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
    let view = program(&[stored("a", "voice", (0, 10))], None, None);
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
    assert!(segments.view().message.contains("locked"));
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
    assert_eq!(segments.view().message, qnc_ui_text::text("story.msg.pick_segment"));
    segments.apply(SegmentCommand::Move { up: true });
    assert_eq!(segments.view().message, qnc_ui_text::text("story.msg.pick_segment"));
    segments.apply(SegmentCommand::Select("a".into()));
    segments.apply(SegmentCommand::DeleteSelected);
    assert_eq!(segments.view().message, qnc_ui_text::text("story.msg.db_unavailable"));
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
        a2_level_db: 0,
        a2_off: false,
        a2_lead_frames: 0,
        a2_tail_frames: 0,
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
        qnc_ui_text::text("story.msg.slot_has_broll"),
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
        qnc_ui_text::text("story.msg.pick_clip_and_marks")
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
    segments.set_source(marked(10));
    assert_eq!(segments.sync_space(false), SyncSpace::Play, "Sync is off");
    segments.apply(SegmentCommand::ToggleSync);
    assert!(segments.view().sync_enabled);
    assert_eq!(
        segments.sync_space(false),
        SyncSpace::Play,
        "IN before Sync does not arm"
    );
    segments.set_source(marked(12));
    assert_eq!(
        segments.sync_space(true),
        SyncSpace::Play,
        "Space in Wrap plays"
    );
    segments.arm_sync(false);
    segments.set_source(marked(12));
    assert!(
        matches!(segments.sync_space(false), SyncSpace::Start(_)),
        "IN again on the same frame arms it again (v5)"
    );
    segments.finish_sync();
    segments.set_source(marked(13));
    let SyncSpace::Start(preview) = segments.sync_space(false) else {
        panic!("Sync starts in the Source view")
    };
    assert_eq!(
        preview.window,
        (15, 33),
        "from m1 at 15 to the next marker m2 at 33 (before the source OUT)"
    );
    assert_eq!(preview.source_in, 13);
    assert_eq!(segments.sync_frame(None, Some(13)), Some(15), "the source at IN");
    assert_eq!(segments.sync_frame(None, Some(21)), Some(23));
    assert_eq!(segments.view().sync_draft, Some((15, 24)), "the cover being written grows with the play");
    segments.set_playhead(Some(23));
    assert!(segments.sync_holds_enter(), "Enter belongs to the running Sync play");
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
        qnc_ui_text::text("story.msg.sync_slot_not_ready"),
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
fn edit_opens_the_segment_on_its_clip_and_escape_drops_it() {
    let mut segments = component();
    let id = segments.view().rows[0].segment_id.clone();
    let clip = segments.view().rows[0].clip_id.clone();
    assert!(segments.apply(SegmentCommand::Edit(id.clone())));
    assert_eq!(segments.view().editing.as_deref(), Some(id.as_str()));
    let (asked_clip, ..) = segments.take_source_request().expect("the source view opens the clip");
    assert_eq!(asked_clip, clip);
    assert!(segments.take_source_request().is_none(), "asked once");
    assert!(segments.sync_holds_enter(), "Enter writes the edit");
    assert!(segments.handles("clear_focus"));
    segments.apply_action("clear_focus");
    assert_eq!(segments.view().editing, None, "Escape drops it");
}

#[test]
fn ctrl_s_takes_the_slot_under_the_playhead_and_shift_s_the_first_empty_one() {
    let mut segments = component();
    let Some(first) = segments.view().slots.first().cloned() else {
        return; // the test program has no slots
    };
    segments.set_playhead(Some(first.start_frame));
    segments.apply(SegmentCommand::SelectSlotAtPlayhead);
    assert_eq!(segments.view().selected_slot().map(|slot| slot.slot_id.clone()), Some(first.slot_id.clone()));
    segments.apply(SegmentCommand::FocusEmptySlot);
    let empty = segments.view().slots.iter().find(|slot| !slot.has_cover).map(|slot| slot.slot_id.clone());
    assert_eq!(segments.view().selected_slot().map(|slot| slot.slot_id.clone()), empty);
    assert!(segments.fit_slot().is_some(), "mark_in_fit_duration takes the length of the selected slot");
}

#[test]
fn a_segment_of_half_the_story_rate_takes_its_length_in_story_frames() {
    let mut xdcam = stored("x", "offovi", (10, 20));
    (xdcam.fps_num, xdcam.duration_frames) = (25, 20);
    let view = program(&[stored("a", "tonovi", (0, 10)), xdcam], None, Some((50, 1)));
    assert_eq!(view.timebase, Some((50, 1)));
    assert_eq!((view.rows[1].start_frame, view.rows[1].end_frame), (10, 30));
    assert_eq!(view.total_frames, 30);
}

#[test]
fn ctrl_o_takes_the_end_of_the_segment_under_the_playhead_and_escape_goes_back_to_wrap() {
    let mut segments = component();
    let row = segments.view().rows[1].clone();
    segments.set_playhead(Some(row.start_frame + 3));
    assert!(segments.take_edge(false));
    assert!(segments.trimming_edge());
    let (clip, in_frame, out_frame, take) = segments.take_source_request().expect("the clip opens");
    assert_eq!((clip, in_frame, out_frame, take), (row.clip_id.clone(), row.source_in_frame, row.source_out_frame, Some(false)));
    assert_eq!(segments.take_wrap_return(), None, "nothing to go back to while the edge is taken");
    segments.apply_action("clear_focus");
    assert!(!segments.trimming_edge());
    assert_eq!(segments.take_wrap_return(), Some(row.start_frame + 3), "Escape goes back where Wrap was");
}

#[test]
fn covers_and_sync_wait_for_segments_and_neither_is_chosen_at_first() {
    let mut empty = ProgramSegments::new();
    empty.apply(SegmentCommand::CoverMode { sync: true });
    empty.apply(SegmentCommand::CoverMode { sync: false });
    let view = empty.view();
    assert!(!view.action_enabled("cover_mode_covers") && !view.action_enabled("cover_mode_sync"), "no segments");
    assert!(!view.action_selected("cover_mode_covers") && !view.action_selected("cover_mode_sync"));
    let mut segments = component();
    assert!(segments.view().action_enabled("cover_mode_sync"));
    assert!(!segments.view().action_selected("cover_mode_covers") && !segments.view().action_selected("cover_mode_sync"), "neither at first");
    segments.apply(SegmentCommand::CoverMode { sync: false });
    assert!(segments.view().action_selected("cover_mode_covers"));
    segments.apply(SegmentCommand::CoverMode { sync: true });
    assert!(segments.view().action_selected("cover_mode_sync") && !segments.view().action_selected("cover_mode_covers"));
}

#[test]
fn ctrl_s_takes_the_slot_and_the_arrows_move_it_with_both_markers_up_to_the_next_marker() {
    let mut segments = component();
    segments.set_playhead(Some(20));
    segments.apply(SegmentCommand::SelectSlotAtPlayhead);
    let frames = |segments: &ProgramSegments| segments.view().markers.iter().map(|pin| pin.frame).collect::<Vec<_>>();
    assert!(segments.handles("step_forward_frame"), "the arrows belong to the taken slot");
    segments.apply_action("step_forward_frame");
    assert_eq!(frames(&segments), vec![0, 16, 34, 40], "both markers move, the slot keeps its length");
    for _ in 0..10 {
        segments.apply_action("step_forward_frame");
    }
    assert_eq!(frames(&segments), vec![0, 21, 39, 40], "it stops before the end marker");
    let neighbour = segments.view().slots.iter().find(|slot| slot.start_frame == 0).map(|slot| slot.end_frame);
    assert_eq!(neighbour, Some(21), "the slot before grows");
    segments.apply_action("clear_focus");
    assert_eq!(frames(&segments), vec![0, 15, 33, 40], "Escape drops the move");
    assert!(!segments.handles("step_forward_frame"), "the arrows go back to the playhead");
}

#[test]
fn a_taken_slot_shows_the_four_edit_pictures_and_a_taken_marker_two() {
    let mut segments = component();
    assert!(segments.view().edit_stills.is_empty(), "nothing taken, no pictures");
    segments.set_playhead(Some(20));
    segments.apply(SegmentCommand::SelectSlotAtPlayhead);
    segments.apply_action("step_forward_frame");
    let marks: Vec<_> = segments.view().edit_stills.iter().map(|still| (still.marker_frame, still.after)).collect();
    assert_eq!(marks, vec![(16, false), (16, true), (34, false), (34, true)]);
    segments.apply_action("clear_focus");
    assert!(segments.view().edit_stills.is_empty());
}

#[test]
fn a_taken_segment_edge_shows_its_pictures_on_the_source_timeline() {
    let mut segments = component();
    segments.set_playhead(Some(32));
    assert!(segments.take_edge(true), "the start of segment c (program 30..40, source 500..510)");
    let pick = |marks| SourcePick::new(Some("clip-c"), Some("C"), Some(marks), (Some(marks.0), 600), Some((50, 1)));
    segments.set_source(pick((498, 510)));
    let stills: Vec<_> = segments
        .view()
        .source_edit_stills
        .iter()
        .map(|still| (still.marker_frame, still.after, still.picture.as_ref().map(|p| (p.clip_id.clone(), p.source_frame))))
        .collect();
    assert_eq!(
        stills,
        vec![(498, false, None), (498, true, Some(("clip-c".into(), 498)))],
        "the Off segment before is black, the new first picture is source frame 498"
    );
    segments.apply_action("clear_focus");
    segments.set_source(pick((498, 510)));
    assert!(segments.view().source_edit_stills.is_empty(), "Escape drops the edge");
}

#[test]
fn a_moved_slot_takes_its_cover_with_it_in_the_edit_pictures() {
    let mut segments = component();
    let cover = |id: &str, slot: &str, (start, end): (u64, u64), clip: &str| qnc_program_db::ProgramCover {
        cover_id: id.into(),
        slot_id: slot.into(),
        clip_id: clip.into(),
        virtual_shot_id: format!("{clip}_broll"),
        program_start_frame: start,
        program_end_frame: end,
        source_in_frame: 1000,
        source_out_frame: 1000 + (end - start),
        fps_num: 50,
        fps_den: 1,
        a2_source_channel: 1,
        a2_level_db: 0,
        a2_off: false,
        a2_lead_frames: 0,
        a2_tail_frames: 0,
    };
    segments.stored_covers = vec![
        cover("c0", "S|m1", (0, 15), "prev"),
        cover("c1", "m1|m2", (15, 33), "own"),
        cover("c2", "m2|E", (33, 40), "next"),
    ];
    segments.refresh_view(String::new());
    segments.set_playhead(Some(20));
    segments.apply(SegmentCommand::SelectSlotAtPlayhead);
    segments.apply_action("step_forward_frame");
    segments.apply_action("step_forward_frame");
    let pictures: Vec<_> = segments
        .view()
        .edit_stills
        .iter()
        .map(|still| still.picture.as_ref().map(|p| (p.clip_id.clone(), p.source_frame)))
        .collect();
    // The slot is 17..35 now: the cover of the slot before fills it to 17 (its source plays on),
    // the slot after starts 2 later and so does its cover (roll edit).
    assert_eq!(
        pictures,
        vec![Some(("prev".into(), 1016)), Some(("own".into(), 1000)), Some(("own".into(), 1017)), Some(("next".into(), 1002))]
    );
}

#[test]
fn a_taken_slot_has_trim_playheads_on_its_moved_markers_and_the_playhead_stays() {
    let mut segments = component();
    segments.set_playhead(Some(20));
    segments.apply(SegmentCommand::SelectSlotAtPlayhead);
    assert_eq!(segments.view().trim_frames, vec![15, 33]);
    segments.apply_action("step_forward_frame");
    assert_eq!(segments.view().trim_frames, vec![16, 34], "the trim playheads follow the draft");
    assert_eq!(segments.take_seek(), None, "the player's playhead is not moved");
    segments.apply_action("clear_focus");
    assert!(segments.view().trim_frames.is_empty());
}

#[test]
fn a_new_take_is_refused_with_a_warning_while_another_waits_for_enter() {
    // Locked (user 2026-10-21): one thing taken at a time; taking something else is refused
    // with a warning the user cannot miss, and what is taken stays as it was.
    let mut segments = component();
    segments.set_playhead(Some(20));
    segments.apply(SegmentCommand::SelectSlotAtPlayhead);
    assert!(segments.slot_move.is_some());
    let marker = segments.view().markers.iter().find(|pin| pin.frame == 33).expect("marker at 33").marker_id.clone();
    segments.apply(SegmentCommand::TakeMarker(marker.clone()));
    assert!(segments.slot_move.is_some(), "the slot stays taken");
    assert!(segments.marker_edit.is_none(), "the marker is not taken");
    assert!(segments.view().alert.is_some(), "the warning is shown");
    segments.apply_action("step_forward_frame");
    assert!(segments.view().trim_frames.len() == 2, "the arrows go on moving the slot");
    segments.apply(SegmentCommand::SelectSlotAtPlayhead);
    assert!(segments.slot_move.is_some(), "taking again what is taken goes");
    // Enter or Escape lets it go; then the marker is taken.
    segments.apply_action("clear_focus");
    segments.apply(SegmentCommand::CancelMarkerEdit);
    segments.slot_move = None;
    segments.apply(SegmentCommand::TakeMarker(marker));
    assert!(segments.marker_edit.is_some(), "with nothing taken the marker is taken");
}

#[test]
fn a_marker_is_not_taken_while_a_sync_waits() {
    let mut segments = component();
    segments.set_playhead(Some(20));
    segments.apply(SegmentCommand::ToggleSync);
    segments.arm_sync(false);
    segments.set_source(marked(13));
    assert!(matches!(segments.sync_space(false), SyncSpace::Start(_)), "Sync plays");
    let marker = segments.view().markers.iter().find(|pin| pin.frame == 33).expect("marker at 33").marker_id.clone();
    segments.apply(SegmentCommand::TakeMarker(marker));
    assert!(segments.marker_edit.is_none(), "the Sync waits, the marker is not taken");
    assert!(segments.view().alert.is_some());
}

#[test]
fn a_click_that_only_places_the_playhead_lets_go_of_a_chosen_cover() {
    let mut segments = component();
    segments.selected_cover = Some("k".into());
    segments.apply(SegmentCommand::Cue(3));
    assert!(segments.selected_cover.is_none());
}

#[test]
fn ctrl_delete_on_a_chosen_slot_without_b_roll_says_so() {
    let mut segments = component();
    segments.set_playhead(Some(20));
    segments.apply(SegmentCommand::SelectSlotAtPlayhead);
    segments.slot_move = None;
    let slot = segments.selected_slot.clone().expect("a chosen slot");
    segments.stored_covers.retain(|cover| cover.slot_id != slot);
    segments.apply(SegmentCommand::DeleteFocused);
    assert_eq!(segments.view().message, qnc_ui_text::text("story.msg.slot_has_no_broll"));
}

#[test]
fn a1_of_a_segment_takes_its_channel_count_from_the_media_record_not_the_wave() {
    let mut segments = component();
    let row = segments.view().rows[0].clone();
    // No stored record counted: the sound is not taken (there is no wave either).
    assert!(!segments.take_part_sound(0, row.start_frame));
    assert_eq!(segments.view().message, qnc_ui_text::text("story.msg.no_audio_channels"));
    // The media record has four channels: taken, without any wave read.
    segments.clip_channels.insert(row.clip_id.clone(), Some(4));
    assert!(segments.take_part_sound(0, row.start_frame));
    assert!(segments.part_sound_taken());
}

#[test]
fn a_clip_whose_channel_count_was_not_read_is_read_again() {
    let mut channels = ClipChannels::new();
    channels.insert("read".into(), Some(4));
    channels.insert("missing".into(), None);
    let known = counted(&channels);
    assert!(known.contains("read"));
    assert!(!known.contains("missing"), "a count not read is asked again");
}

#[test]
fn an_in_pressed_while_the_source_plays_starts_sync_once_it_is_armed() {
    let mut segments = component();
    segments.set_playhead(Some(20));
    segments.apply(SegmentCommand::ToggleSync);
    assert!(!segments.take_sync_flight(), "nothing armed yet");
    segments.arm_sync(true);
    segments.set_source(marked(13));
    assert!(segments.take_sync_flight(), "IN in flight starts Sync");
    assert!(!segments.take_sync_flight(), "once");
    segments.arm_sync(false);
    segments.set_source(marked(14));
    assert!(!segments.take_sync_flight(), "IN while stopped waits for Space");
}

#[test]
fn an_in_on_the_fly_while_a_sync_plays_starts_it_again_from_that_in() {
    let mut segments = component();
    segments.set_playhead(Some(20));
    segments.apply(SegmentCommand::ToggleSync);
    segments.arm_sync(false);
    segments.set_source(marked(13));
    let SyncSpace::Start(first) = segments.sync_space(false) else { panic!("Sync starts") };
    assert_eq!(first.source_in, 13);
    // Live 2026-10-24: I while the Sync plays reached it as Space and paused it after one frame.
    segments.arm_sync(true);
    segments.set_source(marked(17));
    assert!(segments.take_sync_flight(), "the IN on the fly starts the Sync");
    let SyncSpace::Start(again) = segments.sync_space(false) else { panic!("it starts again, not a pause") };
    assert_eq!(again.source_in, 17, "from the new IN");
    assert_eq!(again.window.0, first.window.0, "from the same marker");
}

#[test]
fn replace_exclude_or_a_new_segment_end_an_open_edit() {
    let mut segments = component();
    let id = segments.view().rows[0].segment_id.clone();
    for command in [SegmentCommand::Replace(id.clone()), SegmentCommand::Exclude(id.clone()), SegmentCommand::Undo] {
        assert!(segments.apply(SegmentCommand::Edit(id.clone())));
        assert!(segments.view().editing.is_some());
        segments.apply(command);
        assert_eq!(segments.view().editing, None, "the story changed under the edit: Enter is free again");
    }
}

#[test]
fn ctrl_e_enlarges_the_slot_under_the_playhead_and_escape_or_the_same_slot_closes_it() {
    let mut segments = component();
    segments.set_playhead(Some(20));
    assert!(segments.apply_action("wrap_zoom"));
    let (start, end) = segments.view().zoom.expect("the slot under the playhead is enlarged");
    assert!(start <= 20 && 20 < end);
    assert!(segments.apply_action("clear_focus"));
    assert_eq!(segments.view().zoom, None, "Escape with nothing taken closes it");
    segments.apply_action("wrap_zoom");
    segments.apply_action("wrap_zoom");
    assert_eq!(segments.view().zoom, None, "the same slot again closes it");
}

#[test]
fn one_escape_lets_a_taken_marker_go_and_keeps_the_zoom() {
    let mut segments = component();
    segments.set_playhead(Some(20));
    segments.apply_action("wrap_zoom");
    let marker = segments.view().markers.iter().find(|pin| pin.frame == 33).expect("marker at 33").marker_id.clone();
    segments.apply(SegmentCommand::TakeMarker(marker));
    // Escape gives both of its catalog actions, one after the other.
    segments.apply_action("clear_focus");
    segments.apply_action("close_player");
    assert!(segments.marker_edit.is_none(), "the marker is let go");
    assert!(segments.view().zoom.is_some(), "the zoom stays for the next Escape");
}

#[test]
fn delete_leaves_a_sync_draft_alone_escape_drops_it() {
    // Locked (user 2026-10-21): a Sync draft is not saved, so it is not an object; Delete says
    // so and keeps it, Escape (`clear_focus`, routed by the panel focus) drops it.
    let mut segments = component();
    segments.set_playhead(Some(20));
    segments.apply(SegmentCommand::ToggleSync);
    segments.arm_sync(false);
    segments.set_source(marked(13));
    assert!(matches!(segments.sync_space(false), SyncSpace::Start(_)));
    segments.apply_action("delete_marker");
    assert!(segments.sync_drafting(), "Delete does not drop the draft");
    assert_eq!(segments.view().message, qnc_ui_text::text("story.msg.sync_draft_is_escape"));
    assert!(segments.discard_sync(), "Escape drops it");
}

/// Locked (user 2026-10-21, 2026-10-22): an M marker taken (Ctrl+M) moves whole, on a Wrap row
/// and in the enlarged slot; only in the enlarged slot, with it taken, Ctrl+2 moves only its A2
/// sound (an L of the cover before, a J of the cover after; a trim playhead shows it), Ctrl+I
/// only its picture, Ctrl+M the whole cut again; Ctrl+1 has no cut there.
#[test]
fn a_taken_marker_moves_whole_or_only_its_sound_or_only_its_picture() {
    let mut segments = component();
    segments.stored_covers = vec![cover("c0", "S|m1", (0, 15)), cover("c1", "m1|m2", (15, 33))];
    segments.refresh_view(String::new());
    segments.apply(SegmentCommand::TakeMarker("m1".into()));
    assert!(segments.editing_marker());
    assert!(segments.take_cut_part(None, false, 1), "answered on a Wrap row");
    assert!(!segments.cut_sound_taken(), "on a Wrap row the marker moves whole only");
    segments.set_playhead(Some(20));
    segments.apply_action("wrap_zoom");
    assert!(segments.view().zoom.is_some());
    assert!(segments.take_cut_part(None, false, 1));
    assert!(segments.cut_sound_taken());
    segments.apply_action("step_back_frame");
    segments.apply_action("step_back_frame");
    assert!(segments.view().trim_frames.contains(&13), "the sound 2 frames before the cut: a J");
    let draft = segments.cut_sound_draft().unwrap();
    assert!(draft.contains(&("c1".to_string(), (2, 0))) && draft.contains(&("c0".to_string(), (0, 0))), "{draft:?}");
    assert!(segments.apply(SegmentCommand::CutPart { marker_id: "m1".into(), picture: false, lane: 1 }));
    assert_eq!(segments.cut_sound_draft(), Some(draft.clone()), "the sound taken again keeps its draft");
    assert!(segments.take_cut_part(None, false, 0), "the A1 part is answered");
    assert!(segments.cut_sound_taken(), "A1 has no cut at a marker: the sound stays taken");
    assert!(segments.take_cut_part(None, true, 0));
    assert!(!segments.cut_sound_taken(), "the picture part: only the picture");
    segments.apply_action("clear_focus");
    assert!(!segments.editing_marker());
    // A right click over A2 near the marker takes it with its sound at once.
    assert!(segments.apply(SegmentCommand::CutPart { marker_id: "m1".into(), picture: false, lane: 1 }));
    assert!(segments.cut_sound_taken());
}

/// User 2026-10-29: the A2 sound of a cut has two sides, the J of the cover after and the L of
/// the cover before; with the sound taken (Ctrl+2) a side is taken alone (a click on its line,
/// as Ctrl+I/O), moved by the arrows or a drag, and both are written at once.
#[test]
fn the_j_and_the_l_of_a_cut_move_apart_and_are_written_together() {
    let mut segments = component();
    segments.stored_covers = vec![cover("c0", "S|m1", (0, 15)), cover("c1", "m1|m2", (15, 33))];
    segments.refresh_view(String::new());
    segments.apply(SegmentCommand::TakeMarker("m1".into()));
    segments.set_playhead(Some(20));
    segments.apply_action("wrap_zoom");
    assert!(segments.take_cut_part(None, false, 1));
    // The L alone: the sound before ends 3 frames after the cut.
    assert!(segments.take_sound_edge(false));
    for _ in 0..3 {
        segments.apply_action("step_forward_frame");
    }
    // A click near the J's line (at the cut) takes the J; a drag puts its start 4 frames before.
    assert!(segments.apply(SegmentCommand::TakeCutEdge { marker_id: "m1".into(), frame: 15 }));
    assert!(segments.apply(SegmentCommand::DragCutEdge { marker_id: "m1".into(), frame: 11 }));
    let draft = segments.cut_sound_draft().unwrap();
    assert!(draft.contains(&("c0".to_string(), (0, 3))) && draft.contains(&("c1".to_string(), (4, 0))), "both sides: {draft:?}");
    let marker = segments.view().markers.iter().find(|pin| pin.marker_id == "m1").cloned().unwrap();
    assert_eq!((marker.sound_lead, marker.sound_tail), (Some(11), Some(18)), "both drawn where they are going");
    assert_eq!(marker.taken_part, qnc_program_view::TakenPart::SoundLead);
    // The trim zoom shows both sides too, the moving one marked (user 2026-10-29).
    if let Some(sound) = &segments.view().zoom_sound {
        let cut = sound.cut_sound.as_ref().expect("the cut's sound in the zoom");
        assert_eq!((cut.lead, cut.tail, cut.moving.clone()), (Some(11), Some(18), vec![11]));
        assert!(sound.from <= 11 && sound.to >= 18, "the window holds both sides");
    }
    // Ctrl+2 again: both sides move together, the overlap keeps its length.
    assert!(segments.take_cut_part(None, false, 1));
    segments.apply_action("step_forward_frame");
    let draft = segments.cut_sound_draft().unwrap();
    assert!(draft.contains(&("c0".to_string(), (0, 4))) && draft.contains(&("c1".to_string(), (3, 0))), "{draft:?}");
}

/// Log 2026-10-29: the A1/A2 buttons of the trim zoom name the taken cut and take its sound on a
/// Wrap row too (the trim zoom is open there); a keyboard Ctrl+2 there still moves it whole.
#[test]
fn the_lane_buttons_of_the_trim_zoom_take_the_sound_of_the_taken_cut() {
    let mut segments = component();
    segments.stored_covers = vec![cover("c0", "S|m1", (0, 15)), cover("c1", "m1|m2", (15, 33))];
    segments.refresh_view(String::new());
    segments.apply(SegmentCommand::TakeMarker("m1".into()));
    assert!(segments.take_cut_part(None, false, 1));
    assert!(!segments.cut_sound_taken(), "a key on a Wrap row: the marker moves whole");
    assert!(segments.apply(SegmentCommand::CutPart { marker_id: "m1".into(), picture: false, lane: 1 }));
    assert!(segments.cut_sound_taken(), "the A2 button of the trim zoom takes its sound");
}

/// User 2026-10-29: every cut is a marker, the join of two clips too (the OUT of the clip
/// before and the IN of the clip after). With it taken in the zoom Ctrl+1 takes its A1 sound:
/// the J of the clip after and the L of the clip before move as on A2 and are drafted to their
/// clips.
#[test]
fn the_a1_sound_of_a_cut_between_two_clips_has_a_j_and_an_l() {
    let mut segments = component();
    segments.stored = vec![stored("a", "tonovi", (100, 140)), stored("b", "tonovi", (100, 160))];
    segments.stored_markers.clear();
    segments.refresh_view(String::new());
    let join = segments.view().joins.iter().find(|pin| pin.frame == 40).expect("the cut between the two clips").marker_id.clone();
    assert!(segments.view().joins.iter().all(|pin| pin.join));
    segments.apply(SegmentCommand::TakeMarker(join.clone()));
    assert!(segments.editing_marker(), "the cut is taken");
    segments.apply_action("step_forward_frame");
    assert_eq!(segments.view().joins.iter().find(|pin| pin.marker_id == join).map(|pin| pin.frame), Some(41), "the whole cut rolls");
    segments.set_playhead(Some(45));
    segments.apply_action("wrap_zoom");
    assert!(segments.take_cut_part(None, false, 0), "the A1 sound taken in the zoom");
    assert_eq!(segments.taken_sound().map(|(lane, _)| lane), Some(0));
    segments.apply_action("step_back_frame");
    segments.apply_action("step_back_frame");
    let draft = segments.join_sound_draft().expect("the A1 draft");
    assert!(draft.contains(&("b".to_string(), (2, 0))) && draft.contains(&("a".to_string(), (0, 0))), "{draft:?}");
    assert!(segments.take_sound_edge(false), "the L alone");
    segments.apply_action("step_forward_frame");
    let draft = segments.join_sound_draft().unwrap();
    assert!(draft.contains(&("a".to_string(), (0, 1))), "{draft:?}");
    if let Some(sound) = &segments.view().zoom_sound {
        assert_eq!(sound.cut_sound.as_ref().map(|cut| cut.lane), Some(0));
        assert_eq!(sound.taken_cut.as_deref(), Some(join.as_str()));
    }
}


#[test]
fn a_double_click_enlarges_a_cover_with_its_neighbours_else_the_segment_and_the_program_row_the_program() {
    // User 2026-10-24: a cover with part of the slots before and after, a segment alone, the program whole.
    let mut segments = component();
    segments.apply(SegmentCommand::Zoom { slot: Some("S|m1".into()), frame: Some(5) });
    assert_eq!(segments.view().zoom, Some((0, 18)), "the covered slot S-m1 and 15 % beside it");
    segments.apply(SegmentCommand::Zoom { slot: Some("m1|m2".into()), frame: Some(20) });
    assert_eq!(segments.view().zoom, Some((10, 30)), "no cover there: segment b");
    segments.apply(SegmentCommand::ProgramZoom);
    assert_eq!(segments.view().zoom, Some((0, 40)));
    segments.apply(SegmentCommand::ProgramZoom);
    assert_eq!(segments.view().zoom, None, "the same again closes it");
}

#[test]
fn n_turns_normalising_the_loudness_on_and_off() {
    // User 2026-10-24: a key turns the preview's loudness normalisation on and off.
    // On when the application starts (2026-10-25: as the export and the prerender).
    let mut segments = component();
    assert!(segments.normalization().is_some());
    assert!(segments.apply_action("toggle_loudness_normalize"));
    assert_eq!(segments.normalization(), None);
    assert!(segments.apply_action("toggle_loudness_normalize"));
    assert!(segments.normalization().is_some());
}

#[test]
fn the_export_box_opens_with_the_project_setting_chosen_and_says_what_is_exported() {
    // User 2026-10-24: Export opens a box of the project's export settings, its own as default.
    let mut segments = component();
    segments.open_export(&serde_json::json!({ "preset": "h264_1080p50" }), None);
    assert!(segments.export_open());
    segments.export_step(true);
    segments.apply(SegmentCommand::ExportChoose(0));
    assert!(segments.apply(SegmentCommand::ExportConfirm));
    assert!(!segments.export_open());
    let running = qnc_ui_text::fill("export.running", &[("format", qnc_ui_text::text("export.to_xml"))]);
    assert_eq!(segments.view().message, running, "XML is exported in the background, its format named");
    assert!(segments.view().export_alert, "an export running is shown red");
}

#[test]
fn a_taken_in_or_out_shows_the_pictures_around_its_cut_and_a_second_of_sound_each_side() {
    let mut segments = component();
    let pick = |taken| SourcePick::new(Some("clip-c"), Some("C"), Some((300, 450)), (Some(300), 600), Some((50, 1))).with_taken(taken);
    segments.set_source(pick(None));
    assert!(segments.view().source_edit_stills.is_empty() && segments.view().zoom_sound.is_none(), "nothing taken");
    segments.set_source(pick(Some(true)));
    let frames = |segments: &ProgramSegments| -> Vec<_> {
        segments.view().source_edit_stills.iter().map(|still| (still.picture.as_ref().map(|p| p.source_frame), still.cut)).collect()
    };
    assert_eq!(frames(&segments), vec![(Some(299), false), (Some(300), true)], "before IN, IN first");
    let sound = segments.view().zoom_sound.clone().expect("the sound of the IN");
    assert_eq!((sound.from, sound.cut, sound.to), (250, 300, 350), "a second each side on the clip's frames");
    assert_eq!((sound.lanes[0][0].channel, sound.lanes[1][0].channel), (0, 1), "A1 and A2 as the source timeline hears them");
    segments.set_source(pick(Some(false)));
    assert_eq!(frames(&segments), vec![(Some(449), true), (Some(450), false)], "OUT last, after OUT");
}

#[test]
fn undo_lets_go_of_a_taken_slot_whose_frames_belong_to_the_story_before_it() {
    let mut segments = component();
    segments.set_playhead(Some(20));
    segments.apply(SegmentCommand::SelectSlotAtPlayhead);
    segments.apply_action("step_forward_frame");
    assert!(segments.handles("step_forward_frame"), "the slot is taken");
    segments.apply(SegmentCommand::Undo);
    assert!(!segments.handles("step_forward_frame"), "Undo let it go: the arrows go back to the playhead");
}

/// User 2026-10-29 ("a marker is a timecode: clip X's sound starts at Y, its picture at Z"):
/// the join of two clips is a cut as an M marker is. Whole it rolls between the cuts beside it;
/// in the zoom Ctrl+I/O take its picture alone: the picture J of the clip after (earlier) or the
/// picture L of the clip before (later), drafted to their clips; Ctrl+M takes it whole again.
#[test]
fn the_picture_of_a_join_moves_alone_as_a_j_or_an_l() {
    let mut segments = component();
    segments.stored = vec![stored("a", "offovi", (100, 140)), stored("b", "tonovi", (100, 160))];
    segments.stored_markers = vec![marker("S", 0, "program_start"), marker("m1", 30, ""), marker("E", 100, "program_end")];
    segments.refresh_view(String::new());
    let join = segments.view().joins.iter().find(|pin| pin.frame == 40).unwrap().marker_id.clone();
    segments.apply(SegmentCommand::TakeMarker(join.clone()));
    for _ in 0..20 {
        segments.apply_action("step_back_frame");
    }
    assert_eq!(segments.view().joins.iter().find(|pin| pin.marker_id == join).map(|pin| pin.frame), Some(31), "the roll stops before the marker at 30");
    segments.apply_action("clear_focus");
    segments.apply(SegmentCommand::TakeMarker(join.clone()));
    segments.set_playhead(Some(45));
    segments.apply_action("wrap_zoom");
    assert!(segments.take_cut_part(None, true, 0), "the picture alone");
    assert!(segments.cut_picture_taken());
    for _ in 0..4 {
        segments.apply_action("step_back_frame");
    }
    let draft = segments.join_picture_draft().expect("the picture drafted");
    assert!(draft.contains(&("b".to_string(), (4, 0))), "the picture J of the clip after: {draft:?}");
    assert_eq!(segments.view().a1_splits, vec![(36, 40)], "its picture at 36, its sound at 40");
    for _ in 0..7 {
        segments.apply_action("step_forward_frame");
    }
    let draft = segments.join_picture_draft().unwrap();
    assert!(draft.contains(&("a".to_string(), (0, 3))), "the picture L of the clip before: {draft:?}");
    assert!(segments.take_cut_part(Some(join.clone()), false, 0), "the A1 sound of the same cut");
    assert!(segments.join_picture_draft().is_none());
}

/// User 2026-10-29: a cover is no layer above the clips, it overwrites the picture and A2 of its
/// slot. A cut of the picture is only where the picture changes: the join of two clips under a
/// cover cuts only A1; an M marker between two empty slots cuts neither picture nor A2; at the
/// edge of a cover the picture is cut.
#[test]
fn a_picture_cut_is_only_where_the_picture_changes() {
    let mut segments = component();
    segments.stored = vec![stored("a", "tonovi", (100, 140)), stored("b", "tonovi", (100, 160))];
    segments.stored_markers = vec![marker("S", 0, "program_start"), marker("m1", 30, ""), marker("m2", 50, ""), marker("m3", 80, ""), marker("E", 100, "program_end")];
    segments.stored_slots = vec![slot(("S", 0), ("m1", 30), false), slot(("m1", 30), ("m2", 50), true), slot(("m2", 50), ("m3", 80), false), slot(("m3", 80), ("E", 100), false)];
    segments.stored_covers = vec![cover("c1", "m1|m2", (30, 50))];
    segments.refresh_view(String::new());
    let join = segments.view().joins.iter().find(|pin| pin.frame == 40).unwrap().marker_id.clone();
    assert_eq!(segments.no_picture_cut(&join), Some("story.msg.join_under_cover"));
    assert_eq!(segments.no_picture_cut("m3"), Some("story.msg.marker_no_picture_cut"), "two empty slots");
    assert_eq!(segments.no_picture_cut("m2"), None, "the end of the cover");
    segments.apply(SegmentCommand::TakeMarker(join.clone()));
    segments.set_playhead(Some(40));
    segments.apply_action("wrap_zoom");
    assert!(segments.take_cut_part(None, true, 0));
    assert!(!segments.cut_picture_taken(), "the join under the cover has no picture cut");
    assert!(segments.take_cut_part(None, false, 0), "its A1 cut is there");
    assert!(segments.cut_sound_taken());
}

/// User 2026-10-29 (undo selected): the selected object is the taken cut, else the selected
/// cover, marker, the cover of the selected slot, else the selected segment; the catalog's
/// Ctrl+Backspace action is the program's.
#[test]
fn undo_selected_takes_the_selected_object() {
    let mut segments = component();
    segments.stored_covers = vec![cover("c0", "S|m1", (0, 15))];
    segments.refresh_view(String::new());
    assert!(segments.handles("undo_selected") && segments.handles("redo_selected"));
    segments.selected = Some("b".into());
    assert_eq!(segments.selected_object(), Some(("segment", "b".to_string())));
    segments.selected_slot = Some("S|m1".into());
    assert_eq!(segments.selected_object(), Some(("cover", "c0".to_string())), "the cover of the slot");
    segments.apply(SegmentCommand::TakeMarker("m2".into()));
    assert_eq!(segments.selected_object(), Some(("marker", "m2".to_string())), "the taken cut first");
}
