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
        INSERT INTO project_settings VALUES('p1','{\"storage\":{\"ingest_media\":\"link\"},\"export\":{\"lead_in_seconds\":0,\"tail_seconds\":0}}');
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
        lease: None,
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
    content(path, ContentOperation::QueueSelected { needs: qnc_import_fit::Needs::original_only() }).unwrap();
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
    // The clip's stored record: a long 50p clip (the writer checks the rate and the range
    // a client sends against it).
    set_clip_timing(path, 100_000, (50, 1));
    story
}

/// What the catalog stores for clip c1: its length in frames and its rate.
fn set_clip_timing(path: &Path, frames: u64, (num, den): (u32, u32)) {
    let conn = Connection::open(path).unwrap();
    // The same journal as the story's open connection.
    conn.pragma_update(None, "journal_mode", "PERSIST").unwrap();
    conn
        .execute(
            "UPDATE clips SET duration_frames = ?1, fps_num = ?2, fps_den = ?3 WHERE clip_id = 'c1'",
            rusqlite::params![frames as i64, num, den],
        )
        .unwrap();
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
    let written = run(
        store,
        Operation::CreateCover {
            project_id: "p1".into(),
            slot_id: slot_id.into(),
            clip_id: "c1".into(),
            virtual_shot_id: shot_id.clone(),
            in_frame: range.0,
            out_frame: range.1,
            fps_num: 50,
            fps_den: 1,
            a2_source_channel: 0,
            a2_level_db: 0,
            a2_off: false,
        },
    )?;
    // As the story's writer does: the cover is in, its B-roll shot is kept.
    shots(&store.path.clone(), qnc_virtual_shots::Operation::ConfirmCoverShot { shot_id })?;
    Ok(written)
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
            a1_level_db: 0,
            a1_off: false,
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

fn cover_count(path: &std::path::Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT count(*) FROM story_covers", [], |row| row.get(0))
        .unwrap()
}

fn covers(store: &mut Story) -> Vec<ProgramCover> {
    let Data::Covers(rows) = run(store, Operation::ListCovers).unwrap() else {
        panic!()
    };
    rows
}

/// The end marker of the program: its length in story frames.
fn program_length(store: &mut Story) -> u64 {
    let Data::Markers(rows) = run(store, Operation::ListMarkers).unwrap() else {
        panic!()
    };
    rows.iter().find(|m| m.system_role == "program_end").unwrap().program_frame
}

fn markers_id(path: &std::path::Path, frame: i64) -> String {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT marker_id FROM story_markers WHERE timeline_frame = ?1", [frame], |row| row.get(0))
        .unwrap()
}

/// Locked (user 2026-10-21): a taken M marker moves whole, only its picture or only its A2.
/// Only the sound: an L of the cover before (later) or a J of the cover after (earlier). Only
/// the picture: the marker moves and the sound stays where it was (the J/L take the difference).
#[test]
fn the_picture_or_the_sound_of_a_marker_cut_moves_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    let Data::Created(m14) = marker(&mut store, 14).unwrap() else { panic!() };
    create_cover(&mut store, &slot_at_frame(&path, 5), (1000, 1100)).unwrap();
    create_cover(&mut store, &slot_at_frame(&path, 20), (2000, 2100)).unwrap();
    let overlaps = |store: &mut Story| {
        let mut rows: Vec<_> = covers(store).into_iter().map(|c| (c.program_start_frame, c.a2_lead_frames, c.a2_tail_frames)).collect();
        rows.sort();
        rows
    };
    run(&mut store, Operation::SetCutSound { marker_id: m14.clone(), offset: -5 }).unwrap();
    assert_eq!(overlaps(&mut store), vec![(0, 0, 0), (14, 5, 0)], "a J of the cover after");
    run(&mut store, Operation::SetCutSound { marker_id: m14.clone(), offset: 3 }).unwrap();
    assert_eq!(overlaps(&mut store), vec![(0, 0, 3), (14, 0, 0)], "an L of the cover before");
    run(&mut store, Operation::MoveMarkerPicture { marker_id: m14.clone(), program_frame: 18 }).unwrap();
    // The picture 4 later, the sound where it was (user 2026-10-29): the cover after still
    // starts at 14, a J of 4; the cover before ends at the new cut, it cannot end before it.
    assert_eq!(overlaps(&mut store), vec![(0, 0, 0), (18, 4, 0)]);
    assert_eq!(markers(&mut store), vec![18]);
    run(&mut store, Operation::UndoStory).unwrap();
    assert_eq!(markers(&mut store), vec![14], "one Undo step");
    assert_eq!(overlaps(&mut store), vec![(0, 0, 3), (14, 0, 0)]);
}

/// User 2026-10-29 ("every cut is a marker"): the cut between two clips on A1 has the L of the
/// clip before and the J of the clip after, written together in one Undo step; the first clip
/// has no cut on A1 before it.
#[test]
fn the_a1_cut_between_two_clips_has_the_l_before_and_the_j_after_together() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    let users: Vec<String> = segments(&mut store).into_iter().filter(|row| row.lead.is_empty()).map(|row| row.segment_id).collect();
    let sound = |store: &mut Story| {
        segments(store).into_iter().filter(|row| row.lead.is_empty()).map(|row| (row.a1_lead_frames, row.a1_tail_frames)).collect::<Vec<_>>()
    };
    run(&mut store, Operation::SetJoinSound { segment_id: users[1].clone(), tail_frames: 3, lead_frames: 5 }).unwrap();
    assert_eq!(sound(&mut store), vec![(0, 3), (5, 0), (0, 0)], "the L of the first, the J of the second");
    run(&mut store, Operation::UndoStory).unwrap();
    assert_eq!(sound(&mut store), vec![(0, 0), (0, 0), (0, 0)], "one Undo step");
    // The first clip's cut on A1 is with nothing or with the lead-in, which is not edited.
    assert!(run(&mut store, Operation::SetJoinSound { segment_id: users[0].clone(), tail_frames: 2, lead_frames: 2 }).is_err());
}

/// User 2026-10-29: the J of the cover after and the L of the cover before at one cut, both at
/// once (an overlap), one Undo step; a picture moved keeps both where they are in the story.
#[test]
fn a_cut_has_the_j_of_the_cover_after_and_the_l_of_the_cover_before_together() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    three_segments(&mut store);
    let Data::Created(m14) = marker(&mut store, 14).unwrap() else { panic!() };
    create_cover(&mut store, &slot_at_frame(&path, 5), (1000, 1100)).unwrap();
    create_cover(&mut store, &slot_at_frame(&path, 20), (2000, 2100)).unwrap();
    let overlaps = |store: &mut Story| {
        let mut rows: Vec<_> = covers(store).into_iter().map(|c| (c.program_start_frame, c.a2_lead_frames, c.a2_tail_frames)).collect();
        rows.sort();
        rows
    };
    run(&mut store, Operation::SetCutSoundEdges { marker_id: m14.clone(), tail_frames: 3, lead_frames: 5 }).unwrap();
    assert_eq!(overlaps(&mut store), vec![(0, 0, 3), (14, 5, 0)], "the L before and the J after together");
    // Sound from 9 (J) to 17 (L); the picture to 12: J 3, L 5.
    run(&mut store, Operation::MoveMarkerPicture { marker_id: m14.clone(), program_frame: 12 }).unwrap();
    assert_eq!(overlaps(&mut store), vec![(0, 0, 5), (12, 3, 0)]);
    run(&mut store, Operation::UndoStory).unwrap();
    run(&mut store, Operation::UndoStory).unwrap();
    assert_eq!(overlaps(&mut store), vec![(0, 0, 0), (14, 0, 0)], "each an Undo step");
}

mod segment_tests;
mod marker_tests;
mod slot_tests;
mod cover_tests;
mod sound_tests;
mod store_tests;
mod lead_tests;
