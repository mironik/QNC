use super::*;

#[test]
fn segments_are_appended_in_order_and_keep_their_source_range() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = imported_store(&dir.path().join("db"));
    let Data::Created(ton) = create_segment(&mut store, "tonovi", (10, 60), (50, 1)).unwrap()
    else {
        panic!()
    };
    create_segment(&mut store, "offovi", (100, 125), (100, 2)).unwrap();
    let rows = segments(&mut store);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].segment_id, ton);
    assert_eq!(
        (rows[0].kind.as_str(), rows[0].in_frame, rows[0].out_frame),
        ("tonovi", 10, 60)
    );
    assert_eq!((rows[1].kind.as_str(), rows[1].sort_index), ("offovi", 1));
}

#[test]
fn a_segment_needs_a_real_range_a_known_kind_an_imported_clip_and_the_story_rate() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut claimed = claimed_store(&path);
    assert!(create_segment(&mut claimed, "tonovi", (0, 10), (50, 1))
        .unwrap_err()
        .contains("is not imported"));
    drop(claimed);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    assert!(create_segment(&mut store, "tonovi", (10, 10), (50, 1)).is_err());
    assert!(create_segment(&mut store, "voice", (0, 10), (50, 1)).is_err());
    assert!(create_segment(&mut store, "tonovi", (0, 10), (0, 1)).is_err());
    // The writer checks what the client sends against the clip's stored record.
    let sent = create_segment(&mut store, "tonovi", (0, 10), (25, 1)).unwrap_err();
    assert!(sent.contains("not the clip's stored rate"), "{sent}");
    let past = create_segment(&mut store, "tonovi", (0, 100_001), (50, 1)).unwrap_err();
    assert!(past.contains("past the end of the clip"), "{past}");
    create_segment(&mut store, "tonovi", (0, 10), (50, 1)).unwrap();
    set_clip_timing(&path, 100_000, (30, 1));
    let mixed = create_segment(&mut store, "offovi", (0, 10), (30, 1)).unwrap_err();
    assert!(mixed.contains("double or half"), "{mixed}");
    assert_eq!(segments(&mut store).len(), 1);
}

#[test]
fn deleting_closes_the_gap_and_moving_swaps_neighbours_only() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = imported_store(&dir.path().join("db"));
    for start in [0, 100, 200] {
        create_segment(&mut store, "tonovi", (start, start + 10), (50, 1)).unwrap();
    }
    let ids = segments(&mut store)
        .into_iter()
        .map(|row| row.segment_id)
        .collect::<Vec<_>>();
    let move_segment = |store: &mut Story, id: &str, up: bool| {
        run(
            store,
            Operation::MoveSegment {
                segment_id: id.into(),
                up,
            },
        )
        .unwrap()
    };
    move_segment(&mut store, &ids[0], true);
    move_segment(&mut store, &ids[2], true);
    let order = |store: &mut Story| {
        segments(store)
            .into_iter()
            .map(|row| (row.segment_id, row.sort_index))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        order(&mut store),
        vec![
            (ids[0].clone(), 0),
            (ids[2].clone(), 1),
            (ids[1].clone(), 2)
        ]
    );
    run(
        &mut store,
        Operation::DeleteSegment {
            segment_id: ids[2].clone(),
        },
    )
    .unwrap();
    // The excluded one keeps its place (1) between the active ones.
    assert_eq!(
        order(&mut store),
        vec![(ids[0].clone(), 0), (ids[1].clone(), 2)]
    );
    assert!(run(
        &mut store,
        Operation::DeleteSegment {
            segment_id: ids[2].clone(),
        },
    )
    .is_err());
}

#[test]
fn an_excluded_segment_comes_back_where_it_was_and_only_it_can_be_purged() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    let ids = three_segments(&mut store);
    marker(&mut store, 25).unwrap();
    let segment_op = |store: &mut Story, op: fn(String) -> Operation, id: &str| {
        run(store, op(id.to_string()))
    };
    let exclude = |segment_id| Operation::DeleteSegment { segment_id };
    let include = |segment_id| Operation::IncludeSegment { segment_id };
    let purge = |segment_id| Operation::PurgeSegment { segment_id };
    assert!(
        segment_op(&mut store, purge, &ids[1]).is_err(),
        "an active one is not purged"
    );
    segment_op(&mut store, exclude, &ids[1]).unwrap();
    let frames = |store: &mut Story| -> Vec<u64> { markers(store) };
    assert_eq!(frames(&mut store), vec![15], "25 moved left by 10");
    segment_op(&mut store, include, &ids[1]).unwrap();
    let order: Vec<String> = segments(&mut store)
        .into_iter()
        .map(|row| row.segment_id)
        .collect();
    assert_eq!(order, ids, "back where it was");
    assert_eq!(
        frames(&mut store),
        vec![25],
        "the markers after it move back"
    );
    assert!(
        segment_op(&mut store, include, &ids[1]).is_err(),
        "already active"
    );
    segment_op(&mut store, exclude, &ids[2]).unwrap();
    segment_op(&mut store, purge, &ids[2]).unwrap();
    let Data::Segments(all) = run(&mut store, Operation::ListSegments).unwrap() else {
        panic!()
    };
    assert_eq!(all.len(), 2, "gone for good");
}

#[test]
fn deleting_a_segment_deactivates_it_shifts_markers_and_moves_the_selection() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    let ids = three_segments(&mut store);
    for frame in [5, 13, 24] {
        marker(&mut store, frame).unwrap();
    }
    drop(store);
    Connection::open(&path)
        .unwrap()
        .execute(
            "UPDATE story_state SET selected_part_id = ?1 WHERE id = 1",
            [&ids[1]],
        )
        .unwrap();
    let mut store = open_story(&path);
    run(
        &mut store,
        Operation::DeleteSegment {
            segment_id: ids[1].clone(),
        },
    )
    .unwrap();
    // v5: 13 was inside the window 10..20; 24 moves left by ten frames.
    assert_eq!(markers(&mut store), vec![5, 14]);
    let conn = Connection::open(&path).unwrap();
    let (active, selected): (i64, String) = conn
        .query_row(
            "SELECT p.active, s.selected_part_id FROM story_parts p, story_state s
             WHERE p.part_id = ?1",
            [&ids[1]],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(active, 0, "the row stays, inactive");
    assert_ne!(selected, ids[1]);
    assert!(!selected.is_empty());
    assert_eq!(segments(&mut store).len(), 2);
}

#[test]
fn replace_takes_the_source_length_and_the_program_around_follows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    let ids = three_segments(&mut store); // 0-10, 10-20, 20-30
    marker(&mut store, 15).unwrap();
    marker(&mut store, 25).unwrap();
    let slot = slot_at_frame(&path, 17); // 15..25
    create_cover(&mut store, &slot, (0, 10)).unwrap();
    let replace = |store: &mut Story, range: (u64, u64)| {
        run(
            store,
            Operation::ReplaceSegment {
                segment_id: ids[1].clone(),
                clip_id: "c1".into(),
                in_frame: range.0,
                out_frame: range.1,
                fps_num: 50,
                fps_den: 1,
            },
        )
    };
    // Same length: nothing moves.
    replace(&mut store, (300, 310)).unwrap();
    assert_eq!(markers(&mut store), vec![15, 25]);
    let row = segments(&mut store).remove(1);
    assert_eq!(
        (row.in_frame, row.out_frame, row.kind.as_str()),
        (300, 310, "tonovi")
    );
    // Longer by 10: the marker after it moves right, the cover stays as it is.
    replace(&mut store, (300, 320)).unwrap();
    assert_eq!(markers(&mut store), vec![15, 35]);
    let covers = |store: &mut Story| {
        let Data::Covers(rows) = run(store, Operation::ListCovers).unwrap() else {
            panic!()
        };
        rows
    };
    let kept = covers(&mut store);
    assert_eq!(kept.len(), 1);
    assert_eq!((kept[0].source_in_frame, kept[0].source_out_frame), (0, 10));
    // Shorter (3 frames): the marker in the cut part goes with its slot and cover.
    replace(&mut store, (300, 303)).unwrap();
    assert_eq!(
        markers(&mut store),
        vec![18],
        "35 moved left by 17, 15 was cut"
    );
    assert!(
        covers(&mut store).is_empty(),
        "the cover of the cut slot is gone"
    );
    assert!(
        run(
            &mut store,
            Operation::ReplaceSegment {
                segment_id: ids[1].clone(),
                clip_id: "c1".into(),
                in_frame: 0,
                out_frame: 10,
                fps_num: 30,
                fps_den: 1,
            },
        )
        .is_err(),
        "a rate not 2:1 is refused"
    );
}

#[test]
fn an_edited_segment_takes_a_new_range_and_its_markers_follow_the_picture() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = imported_store(&dir.path().join("db"));
    let ids = three_segments(&mut store);
    for frame in [3, 14, 17, 25] {
        marker(&mut store, frame).unwrap();
    }
    // The middle segment (source 100..110 at program 10..20): 5 frames off its
    // start, 10 more at its end.
    run(
        &mut store,
        Operation::TrimSegment { segment_id: ids[1].clone(), in_frame: 105, out_frame: 120 },
    )
    .unwrap();
    let rows = segments(&mut store);
    assert_eq!((rows[1].in_frame, rows[1].out_frame), (105, 120));
    assert_eq!((rows[0].in_frame, rows[2].in_frame), (0, 200), "the others keep their range");
    assert_eq!(
        markers(&mut store),
        vec![3, 12, 30],
        "14 was on source 104 (cut off), 17 on source 107 stays on it, 25 moves by +5"
    );
    run(&mut store, Operation::UndoStory).unwrap();
    assert_eq!((segments(&mut store)[1].in_frame, markers(&mut store)), (100, vec![3, 14, 17, 25]));
    assert!(
        run(&mut store, Operation::TrimSegment { segment_id: ids[1].clone(), in_frame: 9, out_frame: 9 }).is_err(),
        "OUT after IN"
    );
}

#[test]
fn a_source_of_half_or_twice_the_story_rate_joins_with_its_length_in_story_frames() {
    // A 50p story: a 1080i50 source (25 frames) is two story frames per frame.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    create_segment(&mut store, "tonovi", (0, 10), (50, 1)).unwrap();
    set_clip_timing(&path, 100_000, (25, 1));
    create_segment(&mut store, "offovi", (5, 15), (25, 1)).unwrap();
    let rows = segments(&mut store);
    assert_eq!((rows[1].in_frame, rows[1].out_frame, rows[1].duration_frames), (5, 15, 20));
    assert_eq!(program_length(&mut store), 30);
    // A marker inside it counts its story frames, not its source frames.
    let second = rows[1].segment_id.clone();
    run(&mut store, Operation::CreateMarker { part_id: second, local_frame: 15 }).unwrap();
    let Data::Markers(markers) = run(&mut store, Operation::ListMarkers).unwrap() else {
        panic!("markers")
    };
    assert!(markers.iter().any(|marker| marker.program_frame == 25), "{markers:?}");
    // The rate stays the story's when its first segment goes.
    let first = rows[0].segment_id.clone();
    run(&mut store, Operation::DeleteSegment { segment_id: first }).unwrap();
    set_clip_timing(&path, 100_000, (50, 1));
    create_segment(&mut store, "tonovi", (0, 4), (50, 1)).unwrap();
    assert_eq!(program_length(&mut store), 24);
    // A 25 story: a 50p source is one story frame per two, its range ends on a pair.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    set_clip_timing(&path, 100_000, (25, 1));
    create_segment(&mut store, "tonovi", (0, 10), (25, 1)).unwrap();
    set_clip_timing(&path, 100_000, (50, 1));
    create_segment(&mut store, "offovi", (0, 11), (50, 1)).unwrap();
    let rows = segments(&mut store);
    assert_eq!((rows[1].out_frame, rows[1].duration_frames), (10, 5), "11 ends on the pair at 10");
    assert_eq!(program_length(&mut store), 15);
    assert!(create_segment(&mut store, "offovi", (0, 1), (50, 1)).is_err(), "shorter than a story frame");
}
