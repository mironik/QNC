use super::*;

#[test]
fn a_segment_hears_source_channel_one_on_a1_and_covers_are_read_with_their_a2_channel() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    marker(&mut store, 14).unwrap();
    assert!(
        segments(&mut store)
            .iter()
            .all(|row| row.a1_source_channel == 0),
        "v5: channel 1 of the source until the user picks another one"
    );
    drop(store);
    put_cover(&path, "cover-1", &slot_at_frame(&path, 20));
    Connection::open(&path)
        .unwrap()
        .execute(
            "UPDATE story_items SET a2_source_channel = 1, source_fps_num = 50,
                virtual_shot_id = 'shot' WHERE role = 'cover' AND item_id = 'cover-1'",
            [],
        )
        .unwrap();
    let mut store = reopen(&path);
    let Data::Covers(covers) = run(&mut store, Operation::ListCovers).unwrap() else {
        panic!()
    };
    assert_eq!(covers.len(), 1);
    let cover = &covers[0];
    assert_eq!(
        (
            cover.cover_id.as_str(),
            cover.clip_id.as_str(),
            cover.virtual_shot_id.as_str()
        ),
        ("cover-1", "c1", "shot")
    );
    assert_eq!(
        (cover.program_start_frame, cover.program_end_frame),
        (14, 30)
    );
    assert_eq!((cover.source_in_frame, cover.source_out_frame), (0, 100));
    assert_eq!(
        (cover.fps_num, cover.fps_den, cover.a2_source_channel),
        (50, 1, 1)
    );
}

#[test]
fn a_new_segment_keeps_the_source_channel_chosen_for_a1() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    run(
        &mut store,
        Operation::CreateSegment {
            project_id: "p1".into(),
            kind: "tonovi".into(),
            clip_id: "c1".into(),
            in_frame: 0,
            out_frame: 50,
            fps_num: 50,
            fps_den: 1,
            a1_source_channel: 1,
            a1_level_db: -6,
            a1_off: false,
            a2_source_channel: 1,
            a2_level_db: 0,
            a2_off: false,
        },
    )
    .unwrap();
    let segment = segments(&mut store)[0].clone();
    assert_eq!(segment.a1_source_channel, 1, "channel 2 of the source on A1");
    assert_eq!((segment.a1_level_db, segment.a1_off), (-6, false), "its level as set on the source timeline");
    // Changed later on the Wrap row: one UNDO step.
    run(&mut store, Operation::SetSegmentSound { segment_id: segment.segment_id.clone(), channel: 0, level_db: 3, off: true }).unwrap();
    let changed = segments(&mut store)[0].clone();
    assert_eq!((changed.a1_source_channel, changed.a1_level_db, changed.a1_off), (0, 3, true));
    run(&mut store, Operation::UndoStory).unwrap();
    let undone = segments(&mut store)[0].clone();
    assert_eq!((undone.a1_source_channel, undone.a1_level_db, undone.a1_off), (1, -6, false));
    assert!(run(&mut store, Operation::SetSegmentSound { segment_id: segment.segment_id, channel: 0, level_db: 40, off: false }).is_err(), "above +12 dB");
}

#[test]
fn a_new_cover_keeps_the_source_channel_chosen_for_a2() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    marker(&mut store, 14).unwrap();
    let slot = slot_at_frame(&path, 20);
    let qnc_virtual_shots::Data::Created(shot_id) = shots(
        &path,
        qnc_virtual_shots::Operation::CreateCoverShot {
            project_id: "p1".into(),
            clip_id: "c1".into(),
            clip_name: "Clip".into(),
            in_frame: 40,
            out_frame: 90,
        },
    )
    .unwrap()
    else {
        panic!()
    };
    run(
        &mut store,
        Operation::CreateCover {
            project_id: "p1".into(),
            slot_id: slot,
            clip_id: "c1".into(),
            virtual_shot_id: shot_id,
            in_frame: 40,
            out_frame: 90,
            fps_num: 50,
            fps_den: 1,
            a2_source_channel: 2,
            a2_level_db: 4,
            a2_off: true,
        },
    )
    .unwrap();
    let cover = covers(&mut store)[0].clone();
    assert_eq!(cover.a2_source_channel, 2, "channel 3 of the source on A2");
    assert_eq!((cover.a2_level_db, cover.a2_off), (4, true));
    run(&mut store, Operation::SetCoverSound { cover_id: cover.cover_id, channel: 1, level_db: -2, off: false }).unwrap();
    let changed = covers(&mut store)[0].clone();
    assert_eq!((changed.a2_source_channel, changed.a2_level_db, changed.a2_off), (1, -2, false));
    // The J and L of the cover (user 2026-10-21): stored, an Undo step, bounded.
    assert_eq!((changed.a2_lead_frames, changed.a2_tail_frames), (0, 0));
    run(&mut store, Operation::SetCoverOverlap { cover_id: changed.cover_id.clone(), lead_frames: 12, tail_frames: 5 }).unwrap();
    let overlapped = covers(&mut store)[0].clone();
    assert_eq!((overlapped.a2_lead_frames, overlapped.a2_tail_frames), (12, 5));
    assert!(run(&mut store, Operation::SetCoverOverlap { cover_id: changed.cover_id, lead_frames: 360_001, tail_frames: 0 }).is_err());
    run(&mut store, Operation::UndoStory).unwrap();
    assert_eq!((covers(&mut store)[0].a2_lead_frames, covers(&mut store)[0].a2_tail_frames), (0, 0));
}

/// User 2026-10-30: "the A1 of the Ton cannot go into the Off, only its A2 as a J or an L".
#[test]
fn only_the_a2_of_a_ton_goes_into_an_off_its_a1_does_not() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    for (kind, start) in [("offovi", 0), ("tonovi", 100), ("offovi", 200)] {
        create_segment(&mut store, kind, (start, start + 10), (50, 1)).unwrap();
    }
    let users: Vec<String> = segments(&mut store).into_iter().filter(|row| row.lead.is_empty()).map(|row| row.segment_id).collect();
    assert_eq!(segments(&mut store).iter().find(|row| row.segment_id == users[1]).unwrap().a2_source_channel, 1, "A2 as kept");
    // Off -> Ton: the Ton's A1 J into the Off is refused, its A2 J is taken; the Off's A1 L stays.
    let refused = run(&mut store, Operation::SetJoinSound { segment_id: users[1].clone(), tail_frames: 0, lead_frames: 4 });
    assert_eq!(refused.err().as_deref(), Some(qnc_ui_text::text("story.ton_a1_not_into_off")));
    run(&mut store, Operation::SetJoinSound { segment_id: users[1].clone(), tail_frames: 3, lead_frames: 0 }).unwrap();
    run(&mut store, Operation::SetJoinA2 { segment_id: users[1].clone(), tail_frames: 0, lead_frames: 6 }).unwrap();
    assert!(run(&mut store, Operation::SetJoinA2 { segment_id: users[1].clone(), tail_frames: 2, lead_frames: 0 }).is_err(), "an Off has no A2 to give");
    // Ton -> Off: the Ton's A1 L is refused, its A2 L is taken.
    assert!(run(&mut store, Operation::SetJoinSound { segment_id: users[2].clone(), tail_frames: 2, lead_frames: 0 }).is_err());
    run(&mut store, Operation::SetJoinA2 { segment_id: users[2].clone(), tail_frames: 5, lead_frames: 0 }).unwrap();
    let ton = segments(&mut store).into_iter().find(|row| row.segment_id == users[1]).unwrap();
    assert_eq!((ton.a2_lead_frames, ton.a2_tail_frames, ton.a1_lead_frames), (6, 5, 0));
    run(&mut store, Operation::UndoStory).unwrap();
    let ton = segments(&mut store).into_iter().find(|row| row.segment_id == users[1]).unwrap();
    assert_eq!((ton.a2_lead_frames, ton.a2_tail_frames), (6, 0), "one Undo step");
}

/// User 2026-11-05 ("nowhere a hole in the picture, only a hole in the sound"): a negative J or
/// L of the A1 at a join and of a cover's A2 is stored as a hole; a story made when they could
/// not be negative is built again with its rows.
#[test]
fn a_negative_j_or_l_is_a_hole_in_the_sound_and_an_older_story_keeps_its_rows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    for start in [0, 100] {
        create_segment(&mut store, "tonovi", (start, start + 10), (50, 1)).unwrap();
    }
    let users: Vec<String> = segments(&mut store).into_iter().filter(|row| row.lead.is_empty()).map(|row| row.segment_id).collect();
    run(&mut store, Operation::SetJoinSound { segment_id: users[1].clone(), tail_frames: -3, lead_frames: -2 }).unwrap();
    let after = segments(&mut store).into_iter().find(|row| row.segment_id == users[1]).unwrap();
    let before = segments(&mut store).into_iter().find(|row| row.segment_id == users[0]).unwrap();
    assert_eq!((before.a1_tail_frames, after.a1_lead_frames), (-3, -2));
    drop(store);
    // A story of the table with the old checks: built again, the rows stay.
    let conn = Connection::open(&path).unwrap();
    let sql: String = conn.query_row("SELECT sql FROM sqlite_master WHERE name = 'story_items'", [], |row| row.get(0)).unwrap();
    assert!(!sql.contains("a1_lead_frames >= 0"), "no check on a sound's J");
    let old = sql.replace("a1_lead_frames INTEGER NOT NULL DEFAULT 0,", "a1_lead_frames INTEGER NOT NULL DEFAULT 0 CHECK (a1_lead_frames >= 0),");
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM story_items", [], |row| row.get(0)).unwrap();
    conn.execute_batch(&format!(
        "UPDATE story_items SET a1_lead_frames = 0, a1_tail_frames = 0;
         DROP VIEW IF EXISTS public_story_parts; DROP VIEW IF EXISTS public_story_covers; DROP VIEW IF EXISTS public_story_items; DROP VIEW IF EXISTS public_story_markers; DROP VIEW IF EXISTS public_story_marker_slots; DROP VIEW IF EXISTS public_story_state;
         ALTER TABLE story_items RENAME TO story_items_x; {old}; INSERT INTO story_items SELECT * FROM story_items_x; DROP TABLE story_items_x;"
    ))
    .unwrap();
    drop(conn);
    let mut store = reopen(&path);
    assert_eq!(segments(&mut store).len() as i64 + covers(&mut store).len() as i64, count, "the rows stay");
    run(&mut store, Operation::SetJoinSound { segment_id: users[1].clone(), tail_frames: -1, lead_frames: 0 }).unwrap();
}

/// docs/98 2.1: the level curve of a segment (A1) and of a cover (A2) is kept on the frames of
/// its clip, sorted; a step of Undo; a lead has none; a story made before it keeps its rows.
#[test]
fn a_level_curve_is_kept_sorted_on_its_clip_and_undone_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    let segment = segments(&mut store)[0].segment_id.clone();
    assert!(segments(&mut store)[0].level_keys.is_empty());
    let keys = vec![LevelKey { frame: 20, db10: -60 }, LevelKey { frame: 5, db10: 0 }];
    run(&mut store, Operation::SetLevelKeys { item_id: segment.clone(), keys }).unwrap();
    assert_eq!(
        segments(&mut store)[0].level_keys,
        vec![LevelKey { frame: 5, db10: 0 }, LevelKey { frame: 20, db10: -60 }],
        "sorted by the frame of the clip"
    );
    let same = vec![LevelKey { frame: 5, db10: 0 }, LevelKey { frame: 5, db10: -10 }];
    assert!(run(&mut store, Operation::SetLevelKeys { item_id: segment.clone(), keys: same }).is_err());
    let loud = vec![LevelKey { frame: 5, db10: 121 }];
    assert!(run(&mut store, Operation::SetLevelKeys { item_id: segment.clone(), keys: loud }).is_err());
    run(&mut store, Operation::UndoStory).unwrap();
    assert!(segments(&mut store)[0].level_keys.is_empty(), "one Undo step");
    // The curve is the middle of its points in dB, theirs before and after them.
    let curve = [LevelKey { frame: 10, db10: 0 }, LevelKey { frame: 20, db10: -100 }];
    assert_eq!(LevelKey::level_at(&curve, 0.0), 0.0);
    assert_eq!(LevelKey::level_at(&curve, 15.0), -50.0);
    assert_eq!(LevelKey::level_at(&curve, 30.0), -100.0);
    drop(store);
    // A story made before the column: the owner adds it, the rows stay.
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(
        "DROP VIEW public_story_parts; DROP VIEW public_story_covers; ALTER TABLE story_items DROP COLUMN level_keys;",
    )
    .unwrap();
    drop(conn);
    let mut store = reopen(&path);
    assert_eq!(segments(&mut store).len(), 3, "its rows stay");
    assert!(segments(&mut store)[0].level_keys.is_empty());
}

/// User 2026-11-06: the sound assistant is on or off for the whole story, a step of Undo.
#[test]
fn the_sound_assistant_is_on_or_off_in_the_story_and_undone_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    let assist = |store: &mut _| match run(store, Operation::ReadProgramSnapshot).unwrap() {
        Data::Program(snapshot) => snapshot.sound_assist,
        _ => panic!(),
    };
    assert!(!assist(&mut store), "off until the user turns it on");
    run(&mut store, Operation::SetSoundAssist { on: true }).unwrap();
    assert!(assist(&mut store));
    run(&mut store, Operation::UndoStory).unwrap();
    assert!(!assist(&mut store), "one Undo step");
}

/// User 2026-11-06: the sound assistant's proposal kept as the user's curves, all at once, one
/// Undo step; one item that cannot take its curve leaves nothing written.
#[test]
fn several_curves_are_written_at_once_and_undone_together() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    let ids: Vec<String> = segments(&mut store).iter().map(|row| row.segment_id.clone()).collect();
    let curve = |db10| vec![LevelKey { frame: 2, db10 }];
    let curves = vec![(ids[0].clone(), curve(-30)), (ids[1].clone(), curve(-60))];
    run(&mut store, Operation::SetLevelKeysMany { curves }).unwrap();
    assert_eq!(segments(&mut store)[1].level_keys, curve(-60));
    let wrong = vec![(ids[2].clone(), curve(-10)), ("missing".to_string(), curve(-10))];
    assert!(run(&mut store, Operation::SetLevelKeysMany { curves: wrong }).is_err());
    assert!(segments(&mut store)[2].level_keys.is_empty(), "nothing of a refused write");
    run(&mut store, Operation::UndoStory).unwrap();
    assert!(segments(&mut store).iter().all(|row| row.level_keys.is_empty()), "one Undo step");
}
