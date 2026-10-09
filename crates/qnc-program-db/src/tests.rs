//! Story requests on a project database that also holds the project content (an
//! imported clip, B-roll virtual shots), as a real project database does.

use super::*;
use qnc_content_store::{
    CatalogClip, ContentStore, Data as ContentData, Operation as ContentOperation, Request,
};
use qnc_media_metadata as m;
use qnc_media_records::{Binding, Completeness, Phase, Snapshot};
use rusqlite::Connection;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const URI: &str = "qnc://local/db/ingest_content/p1";

fn database(path: &Path) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        "CREATE TABLE project_settings(project_id TEXT,settings_json TEXT);
        INSERT INTO project_settings VALUES('p1','{\"storage\":{\"ingest_media\":\"link\"}}');
        CREATE VIEW public_project_settings AS SELECT * FROM project_settings;",
    )
    .unwrap();
}

fn fact<T>(value: T) -> Option<m::Fact<T>> {
    Some(m::Fact {
        value,
        evidence_id: "camera".into(),
        locator: "/clip".into(),
    })
}

fn clip(id: &str) -> CatalogClip {
    let original_uri = format!("qnc://local/source/card/media/{id}.wav");
    let metadata = m::ClipMetadata {
        contract_id: m::CONTRACT_ID.into(),
        contract_version: m::CONTRACT_VERSION.into(),
        clip_id: id.into(),
        evidence: vec![m::Evidence {
            id: "camera".into(),
            kind: m::EvidenceKind::CameraMetadata,
            document_uri: "qnc://local/source/card/index.xml".into(),
            media_uri: original_uri.clone(),
        }],
        original: m::MediaRepresentation {
            media_uri: original_uri.clone(),
            container: fact("wav".into()),
            duration_seconds: fact(m::Rational {
                numerator: 10,
                denominator: 1,
            }),
            streams_complete: fact(true),
            streams: vec![m::MediaStream {
                index: fact(0),
                codec: fact(m::Signal::Known("pcm_s24le".into())),
                profile: None,
                time_base: fact(m::Rational {
                    numerator: 1,
                    denominator: 48000,
                }),
                start_pts: fact(0),
                duration_ts: fact(480000),
                details: m::StreamDetails::Audio(Box::new(m::AudioMetadata {
                    sample_rate_hz: fact(48000),
                    channels: fact(2),
                    sample_format: fact(m::Signal::Known("s32".into())),
                    channel_layout: fact(m::Signal::Known("stereo".into())),
                    bits_per_sample: fact(24),
                })),
            }],
            tags: BTreeMap::from([(
                "creation_time".into(),
                fact("2026-09-08T10:00:00Z".into()).unwrap(),
            )]),
        },
        proxy: None,
    };
    let report = m::inspect(&metadata);
    assert!(report.is_complete(), "{report:?}");
    CatalogClip {
        name: id.into(),
        source_uri: "qnc://local/source/card".into(),
        source_name: "Card".into(),
        serial_number: "serial-123".into(),
        volume_name: "CAMERA".into(),
        thumbnail_uri: None,
        media_records_uri: "qnc://local/db/media_records".into(),
        snapshot: Snapshot {
            binding: Binding {
                source_index_uri: "qnc://local/db/source_index".into(),
                source_record_id: format!("record-{id}"),
                original_uri,
                proxy_uri: None,
            },
            revision: 1,
            phase: Phase::Final,
            completeness: Completeness::Complete,
            metadata,
            report,
            recorded_at_unix_ms: 1234,
        },
    }
}

/// A request of the project content in the same database.
fn content(path: &Path, operation: ContentOperation) -> Result<ContentData> {
    let mut store =
        ContentStore::open_owner_binding(path, URI, Access::ReadWrite).expect("content store");
    store.execute(&Request {
        version: qnc_content_store::VERSION.into(),
        db_uri: URI.into(),
        operation,
    })
}

/// The Story of the database at `path`, and the path for the content beside it.
struct Story {
    store: store::Store,
    path: PathBuf,
}

fn open_story(path: &Path) -> Story {
    let conn = Connection::open(path).unwrap();
    conn.busy_timeout(std::time::Duration::from_secs(5)).unwrap();
    conn.pragma_update(None, "journal_mode", "PERSIST").unwrap();
    Story {
        store: store::Store::attach(conn, Access::ReadWrite).unwrap(),
        path: path.to_path_buf(),
    }
}

fn reopen(path: &Path) -> Story {
    open_story(path)
}

fn run(story: &mut Story, operation: Operation) -> Result<Data> {
    story.store.execute(&operation)
}

/// Clip c1 taken by the import queue (not yet imported).
fn claimed_store(path: &Path) -> Story {
    database(path);
    content(path, ContentOperation::Publish(Box::new(clip("c1")))).unwrap();
    content(
        path,
        ContentOperation::Select {
            clip_ids: vec!["c1".into()],
            selected: true,
        },
    )
    .unwrap();
    content(path, ContentOperation::QueueSelected).unwrap();
    assert!(matches!(
        content(path, ContentOperation::ClaimNext).unwrap(),
        ContentData::Claimed(Some(_))
    ));
    open_story(path)
}

/// Clip c1 imported.
fn imported_store(path: &Path) -> Story {
    let story = claimed_store(path);
    content(
        path,
        ContentOperation::FinishImport {
            clip_id: "c1".into(),
            media_uri: Some(clip("c1").snapshot.binding.original_uri),
            thumbnail_uri: None,
            copy_of: None,
            optimized: None,
            error: None,
        },
    )
    .unwrap();
    story
}

/// The virtual shots of the same project database, through their own module.
fn shots(path: &Path, operation: qnc_virtual_shots::Operation) -> Result<qnc_virtual_shots::Data> {
    let target =
        qnc_db_broker::ProjectDbTarget::from_owner_binding(path, "qnc://local/db/project_db/p1")?;
    qnc_virtual_shots::VirtualShotsWriter::start(target)?.call(&operation)
}

/// v5 cover from source frames: the B-roll virtual shot first, then the cover.
fn create_cover(store: &mut Story, slot_id: &str, range: (u64, u64)) -> Result<Data> {
    let qnc_virtual_shots::Data::Created(shot_id) = shots(
        &store.path.clone(),
        qnc_virtual_shots::Operation::CreateCoverShot {
            project_id: "p1".into(),
            clip_id: "c1".into(),
            clip_name: "Clip".into(),
            in_frame: range.0,
            out_frame: range.1,
        },
    )?
    else {
        panic!()
    };
    run(
        store,
        Operation::CreateCover {
            project_id: "p1".into(),
            slot_id: slot_id.into(),
            clip_id: "c1".into(),
            virtual_shot_id: shot_id,
            in_frame: range.0,
            out_frame: range.1,
            fps_num: 50,
            fps_den: 1,
            a2_source_channel: 0,
        },
    )
}

fn create_segment(
    store: &mut Story,
    kind: &str,
    range: (u64, u64),
    fps: (u32, u32),
) -> Result<Data> {
    run(
        store,
        Operation::CreateSegment {
            project_id: "p1".into(),
            kind: kind.into(),
            clip_id: "c1".into(),
            in_frame: range.0,
            out_frame: range.1,
            fps_num: fps.0,
            fps_den: fps.1,
            a1_source_channel: 0,
        },
    )
}

/// The program: active segments in order.
fn segments(store: &mut Story) -> Vec<ProgramSegment> {
    let Data::Segments(rows) = run(store, Operation::ListSegments).unwrap() else {
        panic!()
    };
    rows.into_iter().filter(|row| row.active).collect()
}

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
        .contains("nije uvezen"));
    drop(claimed);
    let dir = tempfile::tempdir().unwrap();
    let mut store = imported_store(&dir.path().join("db"));
    assert!(create_segment(&mut store, "tonovi", (10, 10), (50, 1)).is_err());
    assert!(create_segment(&mut store, "voice", (0, 10), (50, 1)).is_err());
    assert!(create_segment(&mut store, "tonovi", (0, 10), (0, 1)).is_err());
    create_segment(&mut store, "tonovi", (0, 10), (50, 1)).unwrap();
    let mixed = create_segment(&mut store, "offovi", (0, 10), (25, 1)).unwrap_err();
    assert!(mixed.contains("mijesani fps"), "{mixed}");
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

/// M on the Wrap segment that holds a program frame (the end belongs to the last one).
fn marker(store: &mut Story, program_frame: u64) -> Result<Data> {
    let mut start = 0;
    let mut target = (String::new(), 0);
    for segment in segments(store) {
        let frames = segment.out_frame - segment.in_frame;
        target = (segment.segment_id, program_frame.saturating_sub(start));
        if program_frame < start + frames {
            break;
        }
        start += frames;
    }
    run(
        store,
        Operation::CreateMarker {
            part_id: target.0,
            local_frame: target.1,
        },
    )
}

/// User markers (not the locked program start and end).
fn markers(store: &mut Story) -> Vec<u64> {
    let Data::Markers(rows) = run(store, Operation::ListMarkers).unwrap() else {
        panic!()
    };
    rows.into_iter()
        .filter(|row| row.system_role.is_empty())
        .map(|row| row.program_frame)
        .collect()
}

/// Locked boundary markers as (role, frame, origin segment).
fn boundaries(path: &std::path::Path) -> Vec<(String, i64, String)> {
    let conn = Connection::open(path).unwrap();
    let mut statement = conn
        .prepare(
            "SELECT system_role, timeline_frame, origin_part_id FROM story_markers
             WHERE system_role != '' ORDER BY timeline_frame",
        )
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

/// Three ten-frame segments: program frames 0..10, 10..20, 20..30.
fn three_segments(store: &mut Story) -> Vec<String> {
    for start in [0, 100, 200] {
        create_segment(store, "tonovi", (start, start + 10), (50, 1)).unwrap();
    }
    segments(store)
        .into_iter()
        .map(|row| row.segment_id)
        .collect()
}

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
    assert!(move_to(&mut store, &id, 25)
        .unwrap_err()
        .contains("already exists"));
    assert!(move_to(&mut store, &id, 31)
        .unwrap_err()
        .contains("trajanja"));
    move_to(&mut store, &id, 21).unwrap();
    assert_eq!(markers(&mut store), vec![21, 25]);
    let Data::Created(start) = marker(&mut store, 0).unwrap() else {
        panic!()
    };
    assert!(move_to(&mut store, &start, 5)
        .unwrap_err()
        .contains("Početni"));
    let delete = |store: &mut Story, marker_id: &str| {
        run(
            store,
            Operation::DeleteMarker {
                marker_id: marker_id.into(),
            },
        )
    };
    assert!(delete(&mut store, &start).unwrap_err().contains("Početni"));
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

/// Slots as (start marker frame, end marker frame, has a cover).
fn slot_rows(path: &std::path::Path) -> Vec<(i64, i64, bool)> {
    let conn = Connection::open(path).unwrap();
    let mut statement = conn
        .prepare(
            "SELECT start_frame, end_frame, has_cover FROM public_story_marker_slots
             ORDER BY slot_index",
        )
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

/// The slot id (marker pair) covering a program frame.
fn slot_at_frame(path: &std::path::Path, frame: i64) -> String {
    Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT slot_id FROM story_marker_slots WHERE start_frame <= ?1 AND ?1 < end_frame",
            [frame],
            |row| row.get(0),
        )
        .unwrap()
}

/// Puts a cover into a slot the way the cover owner will (store closed first).
fn put_cover(path: &std::path::Path, cover_id: &str, slot_id: &str) {
    let conn = Connection::open(path).unwrap();
    let (start, end): (i64, i64) = conn
        .query_row(
            "SELECT start_frame, end_frame FROM story_marker_slots WHERE slot_id = ?1",
            [slot_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    conn.execute(
        "INSERT INTO story_covers (cover_id, slot_id, timeline_start_frame, timeline_end_frame,
            clip_id, source_in_frame, source_out_frame, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'c1', 0, 100, ?1, ?1)",
        rusqlite::params![cover_id, slot_id, start, end],
    )
    .unwrap();
}

/// (slot id, program start, program end) of a cover.
fn cover_row(path: &std::path::Path, cover_id: &str) -> (String, i64, i64) {
    Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT slot_id, timeline_start_frame, timeline_end_frame FROM story_covers
             WHERE cover_id = ?1",
            [cover_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
}

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

fn cover_count(path: &std::path::Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT count(*) FROM story_covers", [], |row| row.get(0))
        .unwrap()
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
            "UPDATE story_covers SET a2_source_channel = 1, source_fps_num = 50,
                virtual_shot_id = 'shot' WHERE cover_id = 'cover-1'",
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

fn covers(store: &mut Story) -> Vec<ProgramCover> {
    let Data::Covers(rows) = run(store, Operation::ListCovers).unwrap() else {
        panic!()
    };
    rows
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
            fps_num: 25,
            fps_den: 1,
            a2_source_channel: 0,
        },
    );
    assert!(mixed.is_err(), "mixed fps is refused");
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
                fps_num: 25,
                fps_den: 1,
            },
        )
        .is_err(),
        "mixed fps is refused"
    );
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
        },
    )
    .unwrap();
    assert_eq!(segments(&mut store)[0].a1_source_channel, 1, "channel 2 of the source on A1");
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
        },
    )
    .unwrap();
    assert_eq!(covers(&mut store)[0].a2_source_channel, 2, "channel 3 of the source on A2");
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
