use super::*;

#[test]
fn slots_are_stored_between_adjacent_markers_and_named_by_their_pair() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    assert_eq!(slot_rows(&path), vec![(0, 30, false)]);
    let Data::Created(m14) = marker(&mut store, 14).unwrap() else {
        panic!()
    };
    marker(&mut store, 25).unwrap();
    assert_eq!(
        slot_rows(&path),
        vec![(0, 14, false), (14, 25, false), (25, 30, false)]
    );
    let middle = slot_at_frame(&path, 20);
    assert!(middle.starts_with(&format!("{m14}|")), "{middle}");
    // Segment borders at 10 and 20 are no markers, so no slot ends there.
    assert!(slot_rows(&path)
        .iter()
        .all(|(start, end, _)| ![10, 20].contains(start) && ![10, 20].contains(end)));
}

#[test]
fn deleting_a_marker_deletes_its_slots_with_their_covers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    let Data::Created(m14) = marker(&mut store, 14).unwrap() else {
        panic!()
    };
    marker(&mut store, 25).unwrap();
    let (left, middle, right) = (
        slot_at_frame(&path, 5),
        slot_at_frame(&path, 20),
        slot_at_frame(&path, 27),
    );
    drop(store);
    put_cover(&path, "cover_left", &left);
    put_cover(&path, "cover_middle", &middle);
    put_cover(&path, "cover_right", &right);
    Connection::open(&path)
        .unwrap()
        .execute(
            "UPDATE story_state SET selected_slot_id = ?1 WHERE id = 1",
            [&middle],
        )
        .unwrap();
    let mut store = reopen(&path);
    run(
        &mut store,
        Operation::DeleteMarker {
            marker_id: m14.clone(),
        },
    )
    .unwrap();
    // M14 closed the left slot and opened the middle one: both go with their covers.
    assert_eq!(cover_count(&path), 1);
    assert_eq!(cover_row(&path, "cover_right").1, 25);
    assert_eq!(slot_rows(&path), vec![(0, 25, false), (25, 30, true)]);
    let selected: String = Connection::open(&path)
        .unwrap()
        .query_row("SELECT selected_slot_id FROM story_state", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(selected, "", "a slot that no longer exists is not selected");
}

#[test]
fn deleting_a_segment_deletes_the_slots_of_the_markers_inside_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    let ids = three_segments(&mut store);
    marker(&mut store, 5).unwrap();
    marker(&mut store, 14).unwrap();
    let (first, inside) = (slot_at_frame(&path, 2), slot_at_frame(&path, 20));
    drop(store);
    put_cover(&path, "cover_first", &first);
    put_cover(&path, "cover_inside", &inside);
    let mut store = reopen(&path);
    run(
        &mut store,
        Operation::DeleteSegment {
            segment_id: ids[1].clone(),
        },
    )
    .unwrap();
    // M14 was inside the deleted segment: its slots and their covers go.
    assert_eq!(cover_count(&path), 1);
    assert_eq!(cover_row(&path, "cover_first"), (first, 0, 5));
    run(
        &mut store,
        Operation::DeleteSegment {
            segment_id: ids[0].clone(),
        },
    )
    .unwrap();
    run(
        &mut store,
        Operation::DeleteSegment {
            segment_id: ids[2].clone(),
        },
    )
    .unwrap();
    assert_eq!(cover_count(&path), 0, "no program, no markers, no covers");
}

#[test]
fn moving_the_start_marker_of_a_covered_slot_rolls_its_cover() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    let Data::Created(m10) = marker(&mut store, 10).unwrap() else {
        panic!()
    };
    let Data::Created(m16) = marker(&mut store, 16).unwrap() else {
        panic!()
    };
    marker(&mut store, 25).unwrap();
    let slot_of = |marker_id: &str| -> String {
        Connection::open(&path)
            .unwrap()
            .query_row("SELECT slot_id FROM story_marker_slots WHERE start_marker_id = ?1", [marker_id], |row| row.get(0))
            .unwrap()
    };
    create_cover(&mut store, &slot_of(&m16), (500, 520)).unwrap();
    let source_in = |store: &mut Story| (covers(store)[0].source_in_frame, covers(store)[0].source_out_frame);
    // The marker at 16 starts the covered slot: 3 frames later, the cover starts 3 later.
    run(&mut store, Operation::MoveMarker { marker_id: m16.clone(), program_frame: 19 }).unwrap();
    assert_eq!(source_in(&mut store), (503, 523));
    run(&mut store, Operation::MoveMarker { marker_id: m16.clone(), program_frame: 14 }).unwrap();
    assert_eq!(source_in(&mut store), (498, 518), "earlier: the cover starts earlier");
    run(&mut store, Operation::UndoStory).unwrap();
    assert_eq!(source_in(&mut store), (503, 523), "undo takes the roll back");
    // The slot before moves with its markers: the covered slot after it rolls.
    run(&mut store, Operation::MoveSlot { slot_id: slot_of(&m10), start_frame: 12 }).unwrap();
    assert_eq!(markers(&mut store), vec![12, 21, 25]);
    assert_eq!(source_in(&mut store), (505, 525));
    // The covered slot itself moves: its cover keeps its pictures.
    run(&mut store, Operation::MoveSlot { slot_id: slot_of(&m16), start_frame: 20 }).unwrap();
    assert_eq!(source_in(&mut store), (505, 525));
    // A cover at the start of its source rolls as far as it can: its IN stays 0, it grows.
    let dir = tempfile::tempdir().unwrap();
    let mut store = imported_store(&dir.path().join("db"));
    three_segments(&mut store);
    let Data::Created(m16) = marker(&mut store, 16).unwrap() else {
        panic!()
    };
    let slot: String = Connection::open(dir.path().join("db"))
        .unwrap()
        .query_row("SELECT slot_id FROM story_marker_slots WHERE start_marker_id = ?1", [&m16], |row| row.get(0))
        .unwrap();
    create_cover(&mut store, &slot, (2, 20)).unwrap();
    run(&mut store, Operation::MoveMarker { marker_id: m16, program_frame: 13 }).unwrap();
    assert_eq!(markers(&mut store), vec![13], "the move is written");
    assert_eq!((covers(&mut store)[0].source_in_frame, covers(&mut store)[0].source_out_frame), (0, 18));
}

#[test]
fn a_slot_moves_with_both_its_markers_up_to_its_neighbours() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    let Data::Created(first) = marker(&mut store, 10).unwrap() else {
        panic!()
    };
    marker(&mut store, 16).unwrap();
    marker(&mut store, 25).unwrap();
    let slot_of = |marker_id: &str| -> String {
        Connection::open(&path)
            .unwrap()
            .query_row("SELECT slot_id FROM story_marker_slots WHERE start_marker_id = ?1", [marker_id], |row| row.get(0))
            .unwrap()
    };
    let slot = slot_of(&first);
    let move_to = |store: &mut Story, slot_id: &str, frame| {
        run(store, Operation::MoveSlot { slot_id: slot_id.into(), start_frame: frame })
    };
    move_to(&mut store, &slot, 12).unwrap();
    assert_eq!(markers(&mut store), vec![12, 18, 25], "both markers moved, the length stays");
    assert_eq!(slot_of(&first), slot, "the same slot: its marker pair");
    assert!(move_to(&mut store, &slot, 20).unwrap_err().contains(qnc_ui_text::text("story.slot_moves_to_neighbour")), "its end would pass 25");
    move_to(&mut store, &slot, 1).unwrap();
    assert_eq!(markers(&mut store), vec![1, 7, 25]);
    assert!(move_to(&mut store, &slot, 0).unwrap_err().contains(qnc_ui_text::text("story.slot_moves_to_neighbour")), "the locked start");
}

#[test]
fn a_slot_trimmed_at_an_edge_moves_the_covered_slots_after_it_up_to_an_empty_one() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    let Data::Created(m10) = marker(&mut store, 10).unwrap() else {
        panic!()
    };
    let Data::Created(m16) = marker(&mut store, 16).unwrap() else {
        panic!()
    };
    marker(&mut store, 22).unwrap();
    marker(&mut store, 26).unwrap();
    let slot_of = |marker_id: &str| -> String {
        Connection::open(&path)
            .unwrap()
            .query_row("SELECT slot_id FROM story_marker_slots WHERE start_marker_id = ?1", [marker_id], |row| row.get(0))
            .unwrap()
    };
    let slot = slot_of(&m10);
    create_cover(&mut store, &slot, (500, 506)).unwrap();
    create_cover(&mut store, &slot_of(&m16), (600, 606)).unwrap();
    // Slots 10..16 and 16..22 are covered, 22..26 is empty: it takes the difference.
    let trim = |store: &mut Story, start: bool, frames: i64| {
        run(store, Operation::TrimSlot { slot_id: slot.clone(), start, frames })
    };
    trim(&mut store, false, 2).unwrap();
    assert_eq!(markers(&mut store), vec![10, 18, 24, 26], "the covered slot after it moves, the empty one shrinks, 26 stays");
    assert_eq!(covers(&mut store)[0].source_in_frame, 500, "the cover plays on from its IN");
    // The start one frame earlier: the slot keeps its place, the cover starts earlier.
    trim(&mut store, true, 1).unwrap();
    assert_eq!(markers(&mut store), vec![10, 19, 25, 26]);
    let mut ins: Vec<u64> = covers(&mut store).iter().map(|c| c.source_in_frame).collect();
    ins.sort();
    assert_eq!(ins, vec![499, 600]);
    assert!(trim(&mut store, false, 1).unwrap_err().contains("empty slot"), "the empty slot keeps a frame");
    assert!(trim(&mut store, false, -9).unwrap_err().contains("one picture"), "the slot keeps a frame");
    run(&mut store, Operation::UndoStory).unwrap();
    assert_eq!(markers(&mut store), vec![10, 18, 24, 26], "one undo step per trim");
    let last = slot_of(&markers_id(&path, 26));
    assert!(run(&mut store, Operation::TrimSlot { slot_id: last, start: false, frames: 1 })
        .unwrap_err()
        .contains(qnc_ui_text::text("story.last_slot_ends_with_story")));
}

#[test]
fn without_an_empty_slot_after_it_the_last_slot_takes_the_difference() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    let Data::Created(m10) = marker(&mut store, 10).unwrap() else {
        panic!()
    };
    let Data::Created(m16) = marker(&mut store, 16).unwrap() else {
        panic!()
    };
    let Data::Created(m22) = marker(&mut store, 22).unwrap() else {
        panic!()
    };
    let slot_of = |marker_id: &str| -> String {
        Connection::open(&path)
            .unwrap()
            .query_row("SELECT slot_id FROM story_marker_slots WHERE start_marker_id = ?1", [marker_id], |row| row.get(0))
            .unwrap()
    };
    for (marker_id, from) in [(&m10, 500), (&m16, 600), (&m22, 700)] {
        create_cover(&mut store, &slot_of(marker_id), (from, from + 8)).unwrap();
    }
    let slot = slot_of(&m10);
    run(&mut store, Operation::TrimSlot { slot_id: slot, start: false, frames: 2 }).unwrap();
    assert_eq!(markers(&mut store), vec![10, 18, 24], "every covered slot after it moves, the last one is shorter");
}
