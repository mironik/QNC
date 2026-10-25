use super::*;

#[test]
fn start_and_end_are_stored_locked_markers_that_follow_the_program() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    let ids = three_segments(&mut store);
    assert_eq!(
        boundaries(&path),
        vec![
            ("program_start".into(), 0, ids[0].clone()),
            ("program_end".into(), 30, ids[2].clone())
        ]
    );
    let delete = |store: &mut Story, id: &str| {
        run(
            store,
            Operation::DeleteSegment {
                segment_id: id.into(),
            },
        )
    };
    delete(&mut store, &ids[2]).unwrap();
    assert_eq!(
        boundaries(&path)[1],
        ("program_end".into(), 20, ids[1].clone())
    );
    delete(&mut store, &ids[0]).unwrap();
    delete(&mut store, &ids[1]).unwrap();
    assert!(boundaries(&path).is_empty(), "no segments, no markers");
}

#[test]
fn m_keeps_its_segment_and_a_marker_on_the_same_frame_is_refreshed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    assert!(marker(&mut store, 5).is_err(), "no story fps, no marker");
    let ids = three_segments(&mut store);
    let Data::Created(id) = run(
        &mut store,
        Operation::CreateMarker {
            part_id: ids[1].clone(),
            local_frame: 4,
        },
    )
    .unwrap() else {
        panic!()
    };
    let Data::Created(again) = marker(&mut store, 14).unwrap() else {
        panic!()
    };
    assert_eq!(again, id, "v5 refreshes the marker already on that frame");
    let Data::Created(start) = marker(&mut store, 0).unwrap() else {
        panic!()
    };
    assert_ne!(start, id);
    assert_eq!(markers(&mut store), vec![14]);
    let conn = Connection::open(&path).unwrap();
    let (origin, tc, local): (String, String, i64) = conn
        .query_row(
            "SELECT origin_part_id, tc, origin_local_frame FROM story_markers WHERE marker_id = ?1",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(local, 4, "placed on the segment at its own frame 4");
    assert_eq!(
        (origin.as_str(), tc.as_str()),
        (ids[1].as_str(), "00:00:00:14")
    );
}

#[test]
fn moving_and_deleting_markers_keep_the_start_and_the_end_locked() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    let Data::Created(id) = marker(&mut store, 14).unwrap() else {
        panic!()
    };
    marker(&mut store, 25).unwrap();
    let move_to = |store: &mut Story, marker_id: &str, frame| {
        run(
            store,
            Operation::MoveMarker {
                marker_id: marker_id.into(),
                program_frame: frame,
            },
        )
    };
    let between = qnc_ui_text::text("story.marker_between_neighbours");
    assert_eq!(move_to(&mut store, &id, 25).unwrap_err(), between, "onto the neighbour");
    assert_eq!(move_to(&mut store, &id, 28).unwrap_err(), between, "past the neighbour (audit 2026-10-25)");
    assert!(move_to(&mut store, &id, 31)
        .unwrap_err()
        .contains("length of the story"));
    move_to(&mut store, &id, 21).unwrap();
    assert_eq!(markers(&mut store), vec![21, 25]);
    let Data::Created(start) = marker(&mut store, 0).unwrap() else {
        panic!()
    };
    assert!(move_to(&mut store, &start, 5)
        .unwrap_err()
        .contains("start M marker"));
    let delete = |store: &mut Story, marker_id: &str| {
        run(
            store,
            Operation::DeleteMarker {
                marker_id: marker_id.into(),
            },
        )
    };
    assert!(delete(&mut store, &start).unwrap_err().contains("start M marker"));
    delete(&mut store, &id).unwrap();
    assert_eq!(markers(&mut store), vec![25]);
    assert!(delete(&mut store, &id).unwrap_err().contains("not found"));
}

#[test]
fn markers_cross_segment_borders_and_stay_on_their_frame_when_segments_move() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = imported_store(&dir.path().join("db"));
    let ids = three_segments(&mut store);
    marker(&mut store, 10).unwrap();
    marker(&mut store, 16).unwrap();
    run(
        &mut store,
        Operation::MoveSegment {
            segment_id: ids[2].clone(),
            up: true,
        },
    )
    .unwrap();
    assert_eq!(markers(&mut store), vec![10, 16]);
}

#[test]
fn excluding_a_segment_with_markers_on_both_its_edges_leaves_one_marker() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    let ids = three_segments(&mut store);
    let Data::Created(m5) = marker(&mut store, 5).unwrap() else {
        panic!()
    };
    let Data::Created(m10) = marker(&mut store, 10).unwrap() else {
        panic!()
    };
    let Data::Created(m20) = marker(&mut store, 20).unwrap() else {
        panic!()
    };
    let Data::Created(m25) = marker(&mut store, 25).unwrap() else {
        panic!()
    };
    let slot = |start: &str, end: &str| format!("{start}|{end}");
    create_cover(&mut store, &slot(&m5, &m10), (500, 505)).unwrap();
    create_cover(&mut store, &slot(&m10, &m20), (600, 610)).unwrap();
    create_cover(&mut store, &slot(&m20, &m25), (700, 705)).unwrap();
    run(&mut store, Operation::DeleteSegment { segment_id: ids[1].clone() }).unwrap();
    assert_eq!(markers(&mut store), vec![5, 10, 15], "one marker where the segment was");
    let left: Vec<(u64, u64)> = covers(&mut store).iter().map(|c| (c.source_in_frame, c.program_start_frame)).collect();
    assert_eq!(left, vec![(500, 5), (700, 10)], "the covers around it stay, the one inside goes");
}
