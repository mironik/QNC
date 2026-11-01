//! The program model against a real project database in a temp dir (its content
//! and its story, each through its own module).

use super::*;
use qnc_content_store::{Access, ContentStore};

const PROJECT_DB: &str = "qnc://local/db/project_db/p1";

fn story_target(file: &std::path::Path) -> ProjectDbTarget {
    ProjectDbTarget::from_owner_binding(file, PROJECT_DB).unwrap()
}

fn configure(segments: &mut ProgramSegments, file: &std::path::Path) {
    segments.configure(story_target(file), "p1", None, None);
    settle(segments);
}

const URI: &str = "qnc://local/db/ingest_content/p1";

fn store_with_story(file: &std::path::Path) -> Vec<String> {
    store_in(file, story_target(file))
}

/// The same three segments in a story of `target` (the Story or an S-Story version).
fn store_in(file: &std::path::Path, target: ProjectDbTarget) -> Vec<String> {
    {
        let conn = rusqlite::Connection::open(file).unwrap();
        conn.execute_batch(
            "CREATE TABLE project_settings (project_id TEXT, settings_json TEXT);
             INSERT INTO project_settings VALUES ('p1', '{\"export\":{\"lead_in_seconds\":0,\"tail_seconds\":0}}');
             CREATE VIEW public_project_settings AS SELECT project_id, settings_json FROM project_settings;",
        )
        .unwrap();
    }
    drop(ContentStore::open_owner_binding(file, URI, Access::ReadWrite).unwrap());
    rusqlite::Connection::open(file)
        .unwrap()
        .execute(
            "INSERT INTO clips (clip_id, source_uri, original_uri, name, catalog_json,
                revision, final, selected, import_status, duration_frames, fps_num, fps_den)
             VALUES ('c1', 'qnc://local/source/card-a', 'qnc://local/source/card-a/c1.mxf',
                'C1', '{}', 1, 1, 0, 'imported', 10000, 50, 1)",
            [],
        )
        .unwrap();
    let story = StoryWriter::start(target).unwrap();
    let mut ids = Vec::new();
    for (kind, start) in [("offovi", 0), ("tonovi", 100), ("tonovi", 200)] {
        let StoryData::Created(id) = story
            .call(&Operation::CreateSegment {
                project_id: "p1".into(),
                kind: kind.into(),
                clip_id: "c1".into(),
                in_frame: start,
                out_frame: start + 50,
                fps_num: 50,
                fps_den: 1,
                a1_source_channel: 0,
                a1_level_db: 0,
                a1_off: false,
                a2_source_channel: 1,
                a2_level_db: 0,
                a2_off: false,
            })
            .unwrap()
        else {
            panic!()
        };
        ids.push(id);
    }
    ids
}

fn settle(segments: &mut ProgramSegments) {
    for _ in 0..400 {
        segments.poll();
        if !segments.has_pending_work() && !segments.reading() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("writes did not land");
}

#[test]
fn delete_removes_the_selected_segment_from_the_program() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.db");
    let ids = store_with_story(&file);
    let mut segments = ProgramSegments::new();
    configure(&mut segments, &file);
    assert_eq!(segments.view().rows.len(), 3);
    segments.apply(SegmentCommand::Select(ids[1].clone()));
    settle(&mut segments);
    // The Delete key never deletes a segment chosen by a click; the Del button does.
    for action in ["delete_part", "delete_marker", "delete_segment"] {
        segments.apply_action(action);
    }
    settle(&mut segments);
    assert_eq!(segments.view().rows.len(), 3, "a click does not arm Delete");
    segments.apply(SegmentCommand::DeleteSelected);
    settle(&mut segments);
    let left: Vec<&str> = segments
        .view()
        .rows
        .iter()
        .map(|row| row.segment_id.as_str())
        .collect();
    assert_eq!(
        left,
        vec![ids[0].as_str(), ids[2].as_str()],
        "{}",
        segments.view().message
    );
}

fn marker_frames(segments: &ProgramSegments) -> Vec<u64> {
    segments
        .view()
        .markers
        .iter()
        .map(|pin| pin.frame)
        .collect()
}

#[test]
fn ctrl_m_moves_a_marker_by_arrows_or_to_the_playhead_and_enter_confirms_it() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.db");
    store_with_story(&file);
    let mut segments = ProgramSegments::new();
    configure(&mut segments, &file);
    segments.set_playhead(Some(60));
    segments.apply_action("add_marker");
    settle(&mut segments);
    assert_eq!(marker_frames(&segments), vec![0, 60, 150]);

    // Ctrl+M takes the marker nearest to the playhead; arrows move its draft.
    segments.set_playhead(Some(70));
    assert!(segments.handles("select_marker"));
    segments.apply_action("select_marker");
    assert!(segments.editing_marker());
    for _ in 0..3 {
        segments.apply_action("step_forward_frame");
    }
    segments.apply_action("step_back_frame");
    assert_eq!(
        marker_frames(&segments),
        vec![0, 62, 150],
        "the draft is drawn"
    );
    assert_eq!(
        segments.view().playhead,
        Some(62),
        "the playhead shows the draft"
    );
    assert!(segments.sync_holds_enter(), "Enter belongs to the marker");
    segments.apply_action("activate_focused_item");
    settle(&mut segments);
    assert_eq!(
        marker_frames(&segments),
        vec![0, 62, 150],
        "stored after Enter"
    );

    // Escape drops a draft.
    segments.apply_action("select_marker");
    segments.apply_action("step_forward_frame");
    segments.apply_action("clear_focus");
    assert!(!segments.editing_marker());
    assert_eq!(marker_frames(&segments), vec![0, 62, 150]);

    // A marker moves only after Ctrl+M: a drag of a marker not taken does nothing.
    let id = segments.view().markers[1].marker_id.clone();
    segments.apply(SegmentCommand::DragMarker {
        marker_id: id.clone(),
        frame: 90,
    });
    assert!(!segments.editing_marker());
    assert_eq!(marker_frames(&segments), vec![0, 62, 150]);
    // Ctrl+M, then M puts the draft on the playhead; Enter confirms.
    segments.set_playhead(Some(62));
    segments.apply_action("select_marker");
    segments.set_playhead(Some(100));
    segments.apply_action("add_marker");
    assert_eq!(marker_frames(&segments), vec![0, 100, 150], "only a draft");
    segments.apply_action("activate_focused_item");
    settle(&mut segments);
    assert_eq!(marker_frames(&segments), vec![0, 100, 150]);

    // A draft never passes a neighbour; the locked end stays.
    segments.apply_action("select_marker");
    segments.apply(SegmentCommand::DragMarker {
        marker_id: segments.view().markers[1].marker_id.clone(),
        frame: 150,
    });
    assert_eq!(marker_frames(&segments), vec![0, 100, 150]);

    // Shift+M adds a new marker even while one is selected.
    segments.set_playhead(Some(120));
    segments.apply_action("add_marker_continue");
    settle(&mut segments);
    assert_eq!(marker_frames(&segments), vec![0, 100, 120, 150]);
    segments.set_playhead(Some(100));
    segments.apply_action("select_marker");

    // Ctrl+M then Delete removes the marker only, not the segment as well.
    for action in ["delete_part", "delete_marker", "delete_segment"] {
        segments.apply_action(action);
    }
    settle(&mut segments);
    assert_eq!(marker_frames(&segments), vec![0, 120, 150]);
    assert!(!segments.editing_marker());
    assert_eq!(segments.view().rows.len(), 3, "no segment deleted with it");
}

#[test]
fn a_cover_is_deleted_only_after_ctrl_click_took_it() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.db");
    store_with_story(&file);
    let mut segments = ProgramSegments::new();
    configure(&mut segments, &file);
    let slot_id = segments.view().slots[0].slot_id.clone();
    // v5 create_cover: the B-roll virtual shot first, then the cover of the slot.
    segments.cover(
        NewCover {
            slot_id,
            clip_id: "c1".into(),
            in_frame: 10,
            out_frame: 30,
            fps_num: 50,
            fps_den: 1,
            a2_source_channel: 0,
            a2_level_db: 0,
            a2_off: false,
        },
        "C1".into(),
    );
    settle(&mut segments);
    assert_eq!(segments.view().covers.len(), 1, "{}", segments.view().message);
    let cover_id = segments.view().covers[0].cover_id.clone();
    assert!(
        segments.stored_covers[0].virtual_shot_id.starts_with("c1_broll_"),
        "the cover plays its B-roll virtual shot"
    );
    let made = segments.take_created_cover_shots();
    assert_eq!(made.len(), 1, "the caller gets the B-roll shot for its poster");
    assert_eq!((made[0].1.as_str(), made[0].2), ("c1", 10));
    assert!(segments.take_created_cover_shots().is_empty(), "once");
    assert!(segments.marks_used_by_cover(Some("c1"), Some(10)), "its IN makes no short");
    assert!(!segments.marks_used_by_cover(Some("c1"), Some(11)), "a new IN does");

    segments.apply(SegmentCommand::SelectCover { cover_id: cover_id.clone(), frame: 5 });
    segments.apply_action("delete_marker");
    settle(&mut segments);
    assert_eq!(segments.view().covers.len(), 1, "a click does not arm Delete");

    segments.apply(SegmentCommand::TakeCover { cover_id: cover_id.clone(), frame: 5 });
    segments.apply_action("clear_focus");
    segments.apply_action("delete_marker");
    settle(&mut segments);
    assert_eq!(segments.view().covers.len(), 1, "Escape lets it go");

    segments.apply(SegmentCommand::TakeCover { cover_id, frame: 5 });
    segments.apply_action("delete_marker");
    settle(&mut segments);
    assert!(segments.view().covers.is_empty(), "{}", segments.view().message);
}

#[test]
fn ctrl_o_on_the_segment_panel_lengthens_the_slot_under_the_playhead_and_moves_the_ones_after() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.db");
    store_with_story(&file);
    let mut segments = ProgramSegments::new();
    configure(&mut segments, &file);
    for frame in [40, 80] {
        segments.set_playhead(Some(frame));
        segments.apply_action("add_marker");
        settle(&mut segments);
    }
    assert_eq!(marker_frames(&segments), vec![0, 40, 80, 150]);
    // Ctrl+O on the slot 40..80: its end, the slot after moves with it.
    segments.set_playhead(Some(50));
    assert!(segments.take_slot_edge_at_playhead(false));
    assert_eq!(segments.view().playhead, Some(79), "the playhead on the slot's last picture");
    for _ in 0..3 {
        segments.apply_action("step_forward_frame");
    }
    assert_eq!(marker_frames(&segments), vec![0, 40, 83, 150], "the draft is drawn");
    assert_eq!(segments.view().edit_stills.len(), 2, "its new last picture and the next slot's first");
    let cut: Vec<&str> = segments.view().edit_stills.iter().filter(|still| still.cut).map(|still| still.label.as_str()).collect();
    assert_eq!(cut, vec![qnc_ui_text::text("story.still.slot_new_last")], "the picture at the cut is marked");
    segments.apply_action("activate_focused_item");
    settle(&mut segments);
    assert_eq!(marker_frames(&segments), vec![0, 40, 83, 150], "stored after Enter");
    assert_eq!(segments.view().playhead, Some(82), "the playhead on the new last picture");
    // Ctrl+I on the clicked slot 0..40: a later first picture makes it shorter; Escape drops it.
    let first = segments.view().slots[0].slot_id.clone();
    segments.apply(SegmentCommand::SelectSlot { slot_id: first, frame: 20 });
    settle(&mut segments);
    assert!(segments.take_slot_edge_at_playhead(true));
    assert_eq!(segments.view().playhead, Some(0), "the playhead on the slot's first picture");
    segments.apply_action("step_forward_frame");
    assert_eq!(marker_frames(&segments), vec![0, 39, 83, 150], "the empty slot after it takes the difference");
    segments.apply_action("clear_focus");
    assert_eq!(marker_frames(&segments), vec![0, 40, 83, 150]);
    // The last slot ends with the story.
    let last = segments.view().slots.last().unwrap().slot_id.clone();
    segments.apply(SegmentCommand::SelectSlot { slot_id: last, frame: 100 });
    settle(&mut segments);
    assert!(!segments.take_slot_edge_at_playhead(false));
}

#[test]
fn at_the_end_of_its_clip_a_slot_edge_does_not_go_further_that_way() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.db");
    store_with_story(&file);
    let mut segments = ProgramSegments::new();
    configure(&mut segments, &file);
    for frame in [40, 80] {
        segments.set_playhead(Some(frame));
        segments.apply_action("add_marker");
        settle(&mut segments);
    }
    // The slot 40..80 ends in the Ton segment 50..100 (clip c1 from 100): its last
    // picture is c1:129; the clip has 131 frames, so it may end one picture later.
    segments.view_with_waves([("c1", 131u64, None)].into_iter());
    segments.set_playhead(Some(60));
    assert!(segments.take_slot_edge_at_playhead(false));
    segments.apply_action("step_forward_frame");
    assert_eq!(marker_frames(&segments), vec![0, 40, 81, 150], "to the last picture of the clip");
    segments.apply_action("step_forward_frame");
    assert_eq!(marker_frames(&segments), vec![0, 40, 81, 150], "not past it");
    assert!(segments.view().message == qnc_ui_text::text("story.msg.clip_at_end"));
    segments.apply_action("step_back_frame");
    assert_eq!(marker_frames(&segments), vec![0, 40, 80, 150], "back is allowed");
    // A jump to a key picture past the end of the clip goes to its end.
    segments.set_playhead(Some(60));
    assert!(segments.jump_edit(12));
    assert_eq!(marker_frames(&segments), vec![0, 40, 81, 150], "the jump stops at the clip's last picture");
}

#[test]
fn leaving_the_segment_panel_drops_a_taken_slot_edge_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.db");
    store_with_story(&file);
    let mut segments = ProgramSegments::new();
    configure(&mut segments, &file);
    for frame in [40, 80] {
        segments.set_playhead(Some(frame));
        segments.apply_action("add_marker");
        settle(&mut segments);
    }
    segments.set_playhead(Some(50));
    assert!(segments.take_slot_edge_at_playhead(false));
    segments.apply_action("step_forward_frame");
    assert_eq!(marker_frames(&segments), vec![0, 40, 81, 150], "the draft");
    assert!(segments.leave_panel(), "a clip chosen in the pool: the edge is let go");
    assert_eq!(marker_frames(&segments), vec![0, 40, 80, 150], "nothing written");
    assert!(!segments.handles("step_forward_frame"), "the arrows are free again");
    assert!(!segments.leave_panel(), "nothing more to drop");
}

#[test]
fn ctrl_m_takes_a_marker_after_a_segment_edge_was_dropped_with_escape() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.db");
    store_with_story(&file);
    let mut segments = ProgramSegments::new();
    configure(&mut segments, &file);
    segments.set_playhead(Some(60));
    segments.apply_action("add_marker");
    settle(&mut segments);
    segments.set_playhead(Some(70));
    assert!(segments.take_edge(true), "{}", segments.view().message);
    assert!(segments.apply_action("clear_focus"));
    assert!(!segments.trimming_edge());
    settle(&mut segments);
    segments.set_playhead(Some(60));
    segments.apply_action("select_marker");
    assert!(segments.editing_marker(), "{}", segments.view().message);
    segments.apply_action("step_forward_frame");
    assert_eq!(marker_frames(&segments), vec![0, 61, 150]);
}

#[test]
fn a_refused_cover_leaves_no_b_roll_shot_behind() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.db");
    store_with_story(&file);
    // A clip of 100 pictures: an OUT past its end is refused by the story's writer (a short
    // B-roll is no longer refused: it splits its slot, user 2026-10-18).
    rusqlite::Connection::open(&file).unwrap().execute("UPDATE clips SET duration_frames = 100 WHERE clip_id = 'c1'", []).unwrap();
    let mut segments = ProgramSegments::new();
    configure(&mut segments, &file);
    let slot_id = segments.view().slots[0].slot_id.clone();
    segments.cover(
        NewCover { slot_id, clip_id: "c1".into(), in_frame: 10, out_frame: 120, fps_num: 50, fps_den: 1, a2_source_channel: 0, a2_level_db: 0, a2_off: false },
        "C1".into(),
    );
    settle(&mut segments);
    assert!(segments.view().covers.is_empty(), "the cover is refused");
    assert!(segments.take_created_cover_shots().is_empty(), "no poster for it");
    let shots = || -> i64 {
        rusqlite::Connection::open(&file)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM virtual_shots WHERE class = 'b_roll'", [], |row| row.get(0))
            .unwrap()
    };
    for _ in 0..400 {
        if shots() == 0 {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the B-roll shot of the refused cover stayed");
}

/// B laid over the offs one after another (user 2026-10-18): once a B-roll is in, the Wrap
/// playhead is at its end, so the next B fills the next empty part of the slot.
#[test]
fn after_a_b_roll_the_wrap_playhead_is_at_its_end() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.db");
    store_with_story(&file);
    let mut segments = ProgramSegments::new();
    configure(&mut segments, &file);
    segments.set_playhead(Some(0));
    segments.set_source(SourcePick::new(Some("c1"), Some("C1"), Some((10, 30)), (Some(10), 1000), Some((50, 1))));
    segments.cover_from_source(false);
    settle(&mut segments);
    assert_eq!(segments.view().covers.len(), 1, "the cover is written");
    assert_eq!(segments.view().playhead, Some(20), "at the end of the 20 frames of B-roll");
    segments.set_source(SourcePick::new(Some("c1"), Some("C1"), Some((40, 55)), (Some(40), 1000), Some((50, 1))));
    segments.cover_from_source(false);
    settle(&mut segments);
    assert_eq!(segments.view().covers.len(), 2, "the next B fills the next part");
    assert_eq!(segments.view().playhead, Some(35));
}

/// A cover written through the story's writer confirms its B-roll shot (third audit
/// 2026-10-17): it is kept and the B-roll tab lists it.
#[test]
fn a_written_cover_confirms_its_b_roll_shot() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.db");
    store_with_story(&file);
    let mut segments = ProgramSegments::new();
    configure(&mut segments, &file);
    let slot_id = segments.view().slots[0].slot_id.clone();
    segments.cover(
        NewCover { slot_id, clip_id: "c1".into(), in_frame: 10, out_frame: 30, fps_num: 50, fps_den: 1, a2_source_channel: 0, a2_level_db: 0, a2_off: false },
        "C1".into(),
    );
    settle(&mut segments);
    assert_eq!(segments.view().covers.len(), 1, "the cover is written");
    let confirmed = || -> i64 {
        rusqlite::Connection::open(&file)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM virtual_shots WHERE class = 'b_roll' AND cover_pending = 0", [], |row| row.get(0))
            .unwrap()
    };
    for _ in 0..400 {
        if confirmed() == 1 {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the B-roll shot of the written cover was not confirmed");
}

#[test]
fn normalising_is_on_at_start_and_opening_a_project_keeps_what_the_user_chose() {
    // User 2026-10-25: the preview normalises as the export does, on when the application
    // starts; opening a project must not turn it off, nor on again once the user turned it off.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.db");
    store_with_story(&file);
    let mut segments = ProgramSegments::new();
    configure(&mut segments, &file);
    assert!(segments.normalization().is_some(), "on after a project opened");
    segments.apply_action("toggle_loudness_normalize");
    let other = dir.path().join("other").join("project.db");
    std::fs::create_dir_all(other.parent().unwrap()).unwrap();
    store_with_story(&other);
    segments.configure(ProjectDbTarget::from_owner_binding(&other, "qnc://local/db/project_db/p2").unwrap(), "p2", None, None);
    assert_eq!(segments.normalization(), None, "off as the user turned it");
}

/// User 2026-10-31: in an S-Story every clip is a slot and the slot's tools work on it: Ctrl+O
/// makes the clip longer and the story with it, also on the last clip; Ctrl+I at a clip's start
/// shortens it and the story; the trim pictures follow the clip as they follow a cover; a double
/// click enlarges the clip's slot with 15 % of its neighbours.
#[test]
fn in_an_s_story_the_slot_tools_make_the_clip_and_the_story_longer_or_shorter() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.db");
    let draft = story_target(&file).draft_story(1);
    store_in(&file, draft.clone());
    let mut segments = ProgramSegments::new();
    segments.configure(draft, "p1", None, None);
    settle(&mut segments);
    let spans = |segments: &ProgramSegments| segments.view().slots.iter().map(|slot| (slot.start_frame, slot.end_frame)).collect::<Vec<_>>();
    assert_eq!(spans(&segments), vec![(0, 50), (50, 100), (100, 150)], "every clip is a slot");
    // Ctrl+O on the middle clip (a Ton 100..150 of the clip): 3 pictures longer.
    segments.set_playhead(Some(60));
    assert!(segments.take_slot_edge_at_playhead(false));
    for _ in 0..3 {
        segments.apply_action("step_forward_frame");
    }
    assert_eq!(marker_frames(&segments), vec![0, 50, 103, 150], "the draft: only the taken OUT moves, the other cuts stay (user 2026-11-01)");
    let last: Vec<_> = segments.view().edit_stills.iter().filter(|still| still.cut).filter_map(|still| still.picture.as_ref().map(|p| p.source_frame)).collect();
    assert_eq!(last, vec![152], "the clip plays on to its new last picture");
    segments.apply_action("activate_focused_item");
    settle(&mut segments);
    assert_eq!(spans(&segments), vec![(0, 50), (50, 103), (103, 153)], "the clip and the story are longer");
    // Ctrl+O on the last clip also works: the story ends with it.
    let last = segments.view().slots[2].slot_id.clone();
    segments.apply(SegmentCommand::SelectSlot { slot_id: last, frame: 120 });
    settle(&mut segments);
    assert!(segments.take_slot_edge_at_playhead(false));
    segments.apply_action("step_back_frame");
    segments.apply_action("activate_focused_item");
    settle(&mut segments);
    assert_eq!(spans(&segments), vec![(0, 50), (50, 103), (103, 152)], "shorter: the story is shorter");
    // The taken IN moves with the arrows: its cut and its trim line go where it goes, the
    // other cuts stay (user 2026-11-01: "the IN is stuck").
    let middle = segments.view().slots[1].slot_id.clone();
    segments.apply(SegmentCommand::SelectSlot { slot_id: middle, frame: 60 });
    settle(&mut segments);
    assert!(segments.take_slot_edge_at_playhead(true));
    segments.apply_action("step_forward_frame");
    segments.apply_action("step_forward_frame");
    assert_eq!(segments.view().trim_frames, vec![52]);
    assert_eq!(marker_frames(&segments), vec![0, 52, 103, 152]);
    segments.apply_action("clear_focus");
    // A double click enlarges the clip's slot, as a cover's.
    segments.apply(SegmentCommand::Zoom { slot: None, frame: Some(70) });
    assert_eq!(segments.view().zoom, Some((39, 114)), "53 frames: 11 on each side");
    segments.apply(SegmentCommand::Zoom { slot: None, frame: Some(70) });
    // A right click on a cut takes the edge of the clip on the clicked side (user 2026-11-01):
    // the IN of the first clip at the story's start, the OUT of the last at its end.
    let marker_at = |segments: &ProgramSegments, frame: u64| segments.view().markers.iter().find(|pin| pin.frame == frame).unwrap().marker_id.clone();
    for (cut, clicked, slot, start) in [(0, 3, 0, true), (152, 150, 2, false), (50, 48, 0, false), (50, 52, 1, true)] {
        let marker_id = marker_at(&segments, cut);
        segments.apply(SegmentCommand::TakeMarkerAt { marker_id, frame: clicked });
        let slot_id = segments.view().slots[slot].slot_id.clone();
        assert_eq!(segments.slot_trim.as_ref().map(|trim| (trim.slot_id.clone(), trim.start)), Some((slot_id, start)), "the cut at {cut} clicked at {clicked}");
        if start {
            // The IN taken: one trim line, on the IN, none on the OUT (user 2026-11-01).
            let slot_start = segments.view().slots[slot].start_frame;
            assert_eq!(segments.view().trim_frames, vec![slot_start], "the cut at {cut} clicked at {clicked}");
        }
        segments.apply_action("clear_focus");
    }
    // A left click on the left half of a clip puts the playhead on its IN, on the right half
    // on its OUT (its last picture) (user 2026-11-01).
    for (clicked, playhead) in [(60, 50), (90, 102), (3, 0), (140, 151)] {
        segments.apply(SegmentCommand::Cue(clicked));
        assert_eq!(segments.playhead, Some(playhead), "clicked at {clicked}");
    }
}
