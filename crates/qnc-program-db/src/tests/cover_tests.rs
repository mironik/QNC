use super::*;

#[test]
fn a_cover_follows_its_slot_when_markers_move_or_split_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    let Data::Created(m14) = marker(&mut store, 14).unwrap() else {
        panic!()
    };
    let Data::Created(m25) = marker(&mut store, 25).unwrap() else {
        panic!()
    };
    let slot = slot_at_frame(&path, 20);
    drop(store);
    put_cover(&path, "cover_a", &slot);
    let mut store = reopen(&path);
    run(
        &mut store,
        Operation::MoveMarker {
            marker_id: m25.clone(),
            program_frame: 27,
        },
    )
    .unwrap();
    // The same pair moves: the cover takes the new frames.
    assert_eq!(cover_row(&path, "cover_a"), (slot.clone(), 14, 27));
    // A new marker splits the slot: the cover is trimmed to the part that keeps its start.
    let Data::Created(m20) = marker(&mut store, 20).unwrap() else {
        panic!()
    };
    assert_eq!(
        cover_row(&path, "cover_a"),
        (format!("{m14}|{m20}"), 14, 20)
    );
    assert!(slot_rows(&path)[1].2, "the slot shows its cover");
}

#[test]
fn a_cover_is_a_b_roll_virtual_shot_in_its_slot_and_replaces_the_one_there() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    marker(&mut store, 14).unwrap();
    let slot = slot_at_frame(&path, 20);
    let Data::Created(first) = create_cover(&mut store, &slot, (40, 90)).unwrap() else {
        panic!()
    };
    let Data::Created(second) = create_cover(&mut store, &slot, (5, 25)).unwrap() else {
        panic!()
    };
    let rows = covers(&mut store);
    assert_eq!(
        rows.len(),
        1,
        "v5: a new cover replaces the one in its slot"
    );
    let cover = &rows[0];
    assert_eq!(cover.cover_id, second);
    assert_ne!(first, second);
    assert_eq!(
        (cover.program_start_frame, cover.program_end_frame),
        (14, 30)
    );
    assert_eq!(
        (cover.source_in_frame, cover.source_out_frame),
        (5, 25),
        "the source is not cut to the slot"
    );
    assert_eq!(
        (cover.fps_num, cover.fps_den, cover.a2_source_channel),
        (50, 1, 0)
    );
    let classes: Vec<(String, String, i64, i64)> = Connection::open(&path)
        .unwrap()
        .prepare("SELECT shot_id, class, in_frame, out_frame FROM virtual_shots ORDER BY shot_id")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let b_roll: Vec<_> = classes.iter().filter(|row| row.1 == "b_roll").collect();
    assert_eq!(b_roll.len(), 2, "each cover writes its B-roll virtual shot");
    assert_eq!(cover.virtual_shot_id, b_roll[1].0);
    assert!(classes.iter().all(|row| row.1 != "short"), "never a short");
    let qnc_virtual_shots::Data::ShortClips(shorts) =
        shots(&path, qnc_virtual_shots::Operation::ListShorts).unwrap()
    else {
        panic!()
    };
    assert!(shorts.is_empty());
    let qnc_virtual_shots::Data::ShortClips(b_roll) =
        shots(&path, qnc_virtual_shots::Operation::ListBroll).unwrap()
    else {
        panic!()
    };
    assert_eq!(b_roll.len(), 2, "the B-roll tab lists the cover shots");
    assert!(b_roll
        .iter()
        .all(|shot| shot.b_roll && shot.name.starts_with("Clip B")));
    let Data::StorySelection(selection) = run(&mut store, Operation::ReadStorySelection).unwrap()
    else {
        panic!()
    };
    assert_eq!(
        selection.selected_cover_id, second,
        "v5: the new cover is selected"
    );
}

#[test]
fn a_cover_needs_a_slot_the_story_rate_and_one_frame() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    assert!(
        create_cover(&mut store, "a|b", (0, 10)).is_err(),
        "no story"
    );
    three_segments(&mut store);
    let slot = slot_at_frame(&path, 5);
    assert!(create_cover(&mut store, "missing|slot", (0, 10)).is_err());
    assert!(create_cover(&mut store, &slot, (10, 10)).is_err());
    let mixed = run(
        &mut store,
        Operation::CreateCover {
            project_id: "p1".into(),
            slot_id: slot.clone(),
            clip_id: "c1".into(),
            virtual_shot_id: "c1_broll_001".into(),
            in_frame: 0,
            out_frame: 10,
            fps_num: 30,
            fps_den: 1,
            a2_source_channel: 0,
            a2_level_db: 0,
            a2_off: false,
        },
    );
    assert!(mixed.is_err(), "a rate not 2:1 is refused");
    assert!(covers(&mut store).is_empty());
}

#[test]
fn a_cover_is_selected_and_deleted_and_its_selection_goes_with_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    marker(&mut store, 14).unwrap();
    let Data::Created(early) = create_cover(&mut store, &slot_at_frame(&path, 5), (0, 10)).unwrap()
    else {
        panic!()
    };
    let Data::Created(late) = create_cover(&mut store, &slot_at_frame(&path, 20), (0, 10)).unwrap()
    else {
        panic!()
    };
    run(
        &mut store,
        Operation::SelectCover {
            cover_id: early.clone(),
        },
    )
    .unwrap();
    assert!(run(
        &mut store,
        Operation::SelectCover {
            cover_id: "none".into()
        }
    )
    .is_err());
    run(
        &mut store,
        Operation::DeleteCover {
            cover_id: late.clone(),
        },
    )
    .unwrap();
    let Data::StorySelection(selection) = run(&mut store, Operation::ReadStorySelection).unwrap()
    else {
        panic!()
    };
    assert_eq!(
        selection.selected_cover_id, early,
        "another cover's selection stays"
    );
    run(&mut store, Operation::DeleteCover { cover_id: early }).unwrap();
    let Data::StorySelection(selection) = run(&mut store, Operation::ReadStorySelection).unwrap()
    else {
        panic!()
    };
    assert!(selection.selected_cover_id.is_empty());
    assert!(covers(&mut store).is_empty());
    assert!(run(&mut store, Operation::DeleteCover { cover_id: late }).is_err());
    let b_roll: i64 = Connection::open(&path)
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM virtual_shots WHERE class = 'b_roll'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(b_roll, 2, "the B-roll shots stay in the B-roll tab");
}

#[test]
fn a_marker_inside_a_cover_splits_it_and_the_picture_plays_on() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    let Data::Created(m10) = marker(&mut store, 10).unwrap() else {
        panic!()
    };
    marker(&mut store, 20).unwrap();
    let slot: String = Connection::open(&path)
        .unwrap()
        .query_row("SELECT slot_id FROM story_marker_slots WHERE start_marker_id = ?1", [&m10], |row| row.get(0))
        .unwrap();
    create_cover(&mut store, &slot, (500, 510)).unwrap();
    marker(&mut store, 14).unwrap();
    let mut parts: Vec<(u64, u64, u64)> =
        covers(&mut store).iter().map(|c| (c.program_start_frame, c.program_end_frame, c.source_in_frame)).collect();
    parts.sort();
    assert_eq!(parts, vec![(10, 14, 500), (14, 20, 504)], "two covers, the second plays on from the marker");
    run(&mut store, Operation::UndoStory).unwrap();
    assert_eq!(covers(&mut store).len(), 1, "undo brings the one cover back");
}

#[test]
fn no_edit_leaves_a_cover_shorter_than_its_slot() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    let Data::Created(m16) = marker(&mut store, 16).unwrap() else {
        panic!()
    };
    let Data::Created(m25) = marker(&mut store, 25).unwrap() else {
        panic!()
    };
    let slot: String = Connection::open(&path)
        .unwrap()
        .query_row("SELECT slot_id FROM story_marker_slots WHERE start_marker_id = ?1", [&m16], |row| row.get(0))
        .unwrap();
    create_cover(&mut store, &slot, (500, 509)).unwrap();
    drop(store);
    // The clip has 512 frames: the cover 500.. can fill a slot of 12 at most.
    Connection::open(&path).unwrap().execute("UPDATE clips SET duration_frames = 512 WHERE clip_id = 'c1'", []).unwrap();
    let mut store = reopen(&path);
    run(&mut store, Operation::MoveMarker { marker_id: m25.clone(), program_frame: 27 }).unwrap();
    let error = run(&mut store, Operation::MoveMarker { marker_id: m25, program_frame: 29 }).unwrap_err();
    assert!(error.contains("its clip has 512"), "{error}");
    assert_eq!(markers(&mut store), vec![16, 27], "nothing was written: the M marker stays the cover's OUT");
    let _ = m16;
}

#[test]
fn a_b_roll_shorter_than_its_slot_splits_it_with_an_m_marker() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store); // 30 frames, one slot 0..30
    let slot: String = Connection::open(&path)
        .unwrap()
        .query_row("SELECT slot_id FROM story_marker_slots", [], |row| row.get(0))
        .unwrap();
    // Live 2026-10-18: a B-roll of a few frames in the one slot of a new story was refused.
    create_cover(&mut store, &slot, (500, 512)).expect("the cover is made");
    assert_eq!(markers(&mut store), vec![12], "an M marker where the B-roll ends");
    let covers: Vec<(i64, i64)> = Connection::open(&path)
        .unwrap()
        .prepare("SELECT timeline_start_frame, timeline_end_frame FROM story_covers")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(covers, vec![(0, 12)], "the cover fills the first part, the rest is an empty slot");
}

#[test]
fn a_new_segment_keeps_the_last_slot_of_a_cover_and_gets_an_empty_slot() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store); // 30 frames
    let Data::Created(m25) = marker(&mut store, 25).unwrap() else {
        panic!()
    };
    let slot: String = Connection::open(&path)
        .unwrap()
        .query_row("SELECT slot_id FROM story_marker_slots WHERE start_marker_id = ?1", [&m25], |row| row.get(0))
        .unwrap();
    create_cover(&mut store, &slot, (500, 505)).unwrap(); // fills the last slot (25..30)
    drop(store);
    // The cover's clip ends soon after it: a longer last slot would leave a hole.
    Connection::open(&path).unwrap().execute("UPDATE clips SET duration_frames = 506 WHERE clip_id = 'c1'", []).unwrap();
    let mut store = reopen(&path);
    // Live 2026-10-18: a new segment was refused because the end marker moved.
    create_segment(&mut store, "tonovi", (300, 310), (50, 1)).expect("a new segment is made");
    assert_eq!(markers(&mut store), vec![25, 30], "the old end stays as an M marker");
    let cover_slot: (i64, i64) = Connection::open(&path)
        .unwrap()
        .query_row("SELECT timeline_start_frame, timeline_end_frame FROM story_covers", [], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap();
    assert_eq!(cover_slot, (25, 30), "the cover keeps its slot");
}
