use super::*;

#[test]
fn development_program_tables_are_removed_on_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    drop(store);
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TABLE program_segments (segment_id TEXT PRIMARY KEY, marker_mode TEXT);
             CREATE TABLE program_markers (marker_id TEXT PRIMARY KEY, segment_id TEXT);",
        )
        .unwrap();
    let mut store = open_story(&path);
    assert_eq!(segments(&mut store).len(), 3);
    drop(store);
    let legacy: i64 = Connection::open(&path)
        .unwrap()
        .query_row(
            "SELECT count(*) FROM sqlite_master
             WHERE name IN ('program_segments', 'program_markers')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(legacy, 0);
}

#[test]
fn reads_list_deleted_segments_all_markers_slots_and_the_selection() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    let ids = three_segments(&mut store);
    marker(&mut store, 14).unwrap();
    run(
        &mut store,
        Operation::DeleteSegment {
            segment_id: ids[2].clone(),
        },
    )
    .unwrap();
    let Data::Segments(all) = run(&mut store, Operation::ListSegments).unwrap() else {
        panic!()
    };
    assert_eq!(all.len(), 3, "a deleted segment stays listed");
    assert_eq!(all.iter().filter(|row| !row.active).count(), 1);
    let Data::Markers(markers) = run(&mut store, Operation::ListMarkers).unwrap() else {
        panic!()
    };
    let roles: Vec<(&str, u64)> = markers
        .iter()
        .map(|row| (row.system_role.as_str(), row.program_frame))
        .collect();
    assert_eq!(
        roles,
        vec![("program_start", 0), ("", 14), ("program_end", 20)]
    );
    let Data::Slots(slots) = run(&mut store, Operation::ListSlots).unwrap() else {
        panic!()
    };
    let spans: Vec<(u64, u64)> = slots
        .iter()
        .map(|slot| (slot.start_frame, slot.end_frame))
        .collect();
    assert_eq!(spans, vec![(0, 14), (14, 20)]);
    assert_eq!(
        slots[1].slot_id,
        format!("{}|{}", markers[1].marker_id, markers[2].marker_id)
    );
    run(
        &mut store,
        Operation::SelectSlot {
            slot_id: slots[1].slot_id.clone(),
        },
    )
    .unwrap();
    run(
        &mut store,
        Operation::SelectPart {
            part_id: ids[0].clone(),
        },
    )
    .unwrap();
    assert!(run(
        &mut store,
        Operation::SelectSlot {
            slot_id: "missing|slot".into(),
        },
    )
    .is_err());
    let Data::StorySelection(selection) = run(&mut store, Operation::ReadStorySelection).unwrap()
    else {
        panic!()
    };
    assert_eq!(selection.selected_part_id, ids[0]);
    assert_eq!(selection.selected_slot_id, slots[1].slot_id);
}

#[test]
fn a_development_story_without_the_audio_channel_columns_is_removed_on_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    marker(&mut store, 14).unwrap();
    drop(store);
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "DROP VIEW public_story_parts; ALTER TABLE story_parts DROP COLUMN a1_source_channel;",
        )
        .unwrap();
    let mut store = reopen(&path);
    assert!(segments(&mut store).is_empty(), "removed, not converted");
    assert!(markers(&mut store).is_empty());
    three_segments(&mut store);
    assert_eq!(segments(&mut store).len(), 3, "a new story starts cleanly");
}

#[test]
fn undo_puts_the_story_back_step_by_step_and_redo_replays_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    let ids = three_segments(&mut store);
    marker(&mut store, 25).unwrap();
    let depth = |store: &mut Story| {
        let Data::StorySelection(selection) = run(store, Operation::ReadStorySelection).unwrap()
        else {
            panic!()
        };
        (selection.undo_depth, selection.redo_depth)
    };
    assert_eq!(depth(&mut store), (4, 0), "three segments and a marker");
    run(&mut store, Operation::DeleteSegment { segment_id: ids[1].clone() }).unwrap();
    assert_eq!(markers(&mut store), vec![15]);
    run(&mut store, Operation::UndoStory).unwrap();
    assert_eq!(markers(&mut store), vec![25], "the marker is back where it was");
    assert!(segments(&mut store).iter().all(|row| row.active));
    assert_eq!(depth(&mut store), (4, 1));
    run(&mut store, Operation::RedoStory).unwrap();
    assert_eq!(markers(&mut store), vec![15], "redo excludes it again");
    run(&mut store, Operation::UndoStory).unwrap();
    run(
        &mut store,
        Operation::SelectPart {
            part_id: ids[0].clone(),
        },
    )
    .unwrap();
    assert_eq!(depth(&mut store), (4, 1), "a selection is no step");
    run(&mut store, Operation::MoveSegment { segment_id: ids[0].clone(), up: true }).unwrap();
    assert_eq!(depth(&mut store), (4, 1), "a move that changes nothing is no step");
    run(&mut store, Operation::MoveSegment { segment_id: ids[0].clone(), up: false }).unwrap();
    assert_eq!(depth(&mut store), (5, 0), "a new edit clears redo");
    for _ in 0..5 {
        run(&mut store, Operation::UndoStory).unwrap();
    }
    assert!(segments(&mut store).is_empty(), "back to an empty story");
    assert!(run(&mut store, Operation::UndoStory).is_err());
}


#[test]
fn a_story_of_an_older_schema_is_not_read_until_the_owner_brings_it_up() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    drop(store);
    // A story from before the A1 level (2026-10-16).
    Connection::open(&path)
        .unwrap()
        .execute_batch("DROP VIEW public_story_parts; ALTER TABLE story_parts DROP COLUMN a1_level_db; ALTER TABLE story_parts DROP COLUMN a1_off;")
        .unwrap();
    let read_only = |path: &Path| store::Store::attach(Connection::open(path).unwrap(), Access::ReadOnly);
    let refused = read_only(&path).err().expect("an older story is not read");
    assert_eq!(refused, qnc_ui_text::text("story.schema_outdated"));
    drop(reopen(&path)); // the owner's write connection brings it up
    let mut reader = Story { store: read_only(&path).expect("read after the owner"), path: path.clone() };
    let Data::Segments(rows) = run(&mut reader, Operation::ListSegments).unwrap() else {
        panic!()
    };
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|row| row.a1_level_db == 0), "as recorded");
}

#[test]
fn a_refused_edit_writes_nothing_not_even_an_undo_step() {
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
    Connection::open(&path).unwrap().execute("UPDATE clips SET duration_frames = 512 WHERE clip_id = 'c1'", []).unwrap();
    let mut store = reopen(&path);
    let undo_steps = |path: &Path| -> i64 {
        Connection::open(path).unwrap().query_row("SELECT COUNT(*) FROM story_undo", [], |row| row.get(0)).unwrap()
    };
    let steps = undo_steps(&path);
    run(&mut store, Operation::MoveMarker { marker_id: m25, program_frame: 29 }).unwrap_err();
    assert_eq!(undo_steps(&path), steps, "no undo step for a refused edit");
    assert_eq!(markers(&mut store), vec![16, 25]);
    let open: bool = Connection::open(&path).unwrap().is_autocommit();
    assert!(open, "no transaction is left open");
}

#[test]
fn the_program_snapshot_has_a_revision_that_grows_with_every_change_and_never_goes_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    let snapshot = |store: &mut Story| match run(store, Operation::ReadProgramSnapshot).unwrap() {
        Data::Program(snapshot) => snapshot,
        _ => panic!(),
    };
    assert_eq!(snapshot(&mut store).revision, 0);
    three_segments(&mut store);
    let after_edits = snapshot(&mut store);
    assert_eq!(after_edits.segments.len(), 3);
    assert!(after_edits.revision >= 3);
    assert!(after_edits.story_fps.is_some());
    run(&mut store, Operation::UndoStory).unwrap();
    let undone = snapshot(&mut store);
    assert_eq!(undone.segments.iter().filter(|segment| segment.active).count(), 2);
    assert!(undone.revision > after_edits.revision, "Undo is a new revision, never an old one");
    run(&mut store, Operation::SelectSlot { slot_id: String::new() }).ok();
    assert_eq!(snapshot(&mut store).revision, undone.revision, "a selection is not a change of the program");
}

#[test]
fn the_story_reader_brings_an_older_story_up_through_its_owner_and_reads_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    drop(store);
    // A story from before its revision (2026-10-17): the reader saw nothing until a write.
    Connection::open(&path).unwrap().execute_batch("DROP TABLE story_revision;").unwrap();
    let target = qnc_db_broker::ProjectDbTarget::from_owner_binding(&path, "qnc://local/db/project_db/p1").unwrap();
    let snapshot = crate::StoryReader::open(&target).unwrap().program_snapshot().expect("read after the owner brought it up");
    assert_eq!(snapshot.segments.len(), 3);
}
