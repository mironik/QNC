use super::*;

/// The lead-in and lead-out seconds of the project.
fn set_leads(path: &Path, lead_in: f64, lead_out: f64) {
    let conn = Connection::open(path).unwrap();
    conn.pragma_update(None, "journal_mode", "PERSIST").unwrap();
    let settings = serde_json::json!({ "export": { "lead_in_seconds": lead_in, "tail_seconds": lead_out } });
    conn.execute("UPDATE project_settings SET settings_json = ?1", [settings.to_string()]).unwrap();
}

/// (lead, kind, in, out, length, A1 off) of the program in order.
fn program(store: &mut Story) -> Vec<(String, String, u64, u64, u64, bool)> {
    segments(store)
        .into_iter()
        .map(|row| (row.lead, row.kind, row.in_frame, row.out_frame, row.duration_frames, row.a1_off))
        .collect()
}

fn row(lead: &str, kind: &str, range: (u64, u64), frames: u64, off: bool) -> (String, String, u64, u64, u64, bool) {
    (lead.into(), kind.into(), range.0, range.1, frames, off)
}

#[test]
fn a_ton_gets_its_picture_before_and_after_and_an_off_black_and_silence() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    set_leads(&path, 2.0, 2.0);
    create_segment(&mut store, "tonovi", (300, 400), (50, 1)).unwrap();
    // A Ton alone: its picture runs 2 s before and after it, without its sound.
    assert_eq!(
        program(&mut store),
        vec![
            row("in", "tonovi", (200, 300), 100, true),
            row("", "tonovi", (300, 400), 100, false),
            row("out", "tonovi", (400, 500), 100, true),
        ]
    );
    create_segment(&mut store, "offovi", (500, 600), (50, 1)).unwrap();
    let rows = program(&mut store);
    assert_eq!(rows.len(), 4);
    // The last segment is an Off: black and silence after it.
    assert_eq!((rows[3].0.as_str(), rows[3].1.as_str(), rows[3].4, rows[3].5), ("out", "offovi", 100, true));
    let Data::Markers(all) = run(&mut store, Operation::ListMarkers).unwrap() else { panic!() };
    let end = all.iter().find(|marker| marker.system_role == "program_end").unwrap().program_frame;
    assert_eq!(end, 400, "the program is the leads and the segments");
    // A lead is the owner's.
    let lead = segments(&mut store)[0].segment_id.clone();
    let refused = run(&mut store, Operation::DeleteSegment { segment_id: lead }).unwrap_err();
    assert!(refused.contains("lead-in and lead-out"), "{refused}");
}

#[test]
fn a_clip_without_enough_frames_before_its_in_is_black_for_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    set_leads(&path, 2.0, 0.0);
    create_segment(&mut store, "tonovi", (30, 130), (50, 1)).unwrap();
    let rows = program(&mut store);
    assert_eq!(rows.len(), 3, "{rows:?}");
    assert_eq!((rows[0].0.as_str(), rows[0].1.as_str(), rows[0].4), ("in", "offovi", 70));
    assert_eq!(rows[1], row("in", "tonovi", (0, 30), 30, true));
    assert_eq!(rows[2].0, "");
}

#[test]
fn a_new_first_segment_keeps_the_lead_in_and_a_longer_lead_in_moves_the_markers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    set_leads(&path, 2.0, 2.0);
    create_segment(&mut store, "offovi", (300, 400), (50, 1)).unwrap();
    create_segment(&mut store, "tonovi", (500, 600), (50, 1)).unwrap();
    marker(&mut store, 150).unwrap();
    assert_eq!(markers(&mut store), vec![150]);
    // The second segment moves first: the lead-in follows it, as long as before.
    let second = segments(&mut store)[2].segment_id.clone();
    run(&mut store, Operation::MoveSegment { segment_id: second, up: true }).unwrap();
    let rows = program(&mut store);
    assert_eq!(rows[0], row("in", "tonovi", (400, 500), 100, true));
    assert_eq!(rows[1].2, 500);
    assert_eq!(markers(&mut store), vec![150]);
    // A lead-in of 3 s moves everything after it by 1 s on the next edit.
    set_leads(&path, 3.0, 2.0);
    create_segment(&mut store, "offovi", (700, 710), (50, 1)).unwrap();
    assert_eq!(program(&mut store)[0].4, 150);
    assert_eq!(markers(&mut store), vec![200]);
}

#[test]
fn a_story_without_lead_settings_is_not_edited() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    let conn = Connection::open(&path).unwrap();
    conn.pragma_update(None, "journal_mode", "PERSIST").unwrap();
    conn.execute("UPDATE project_settings SET settings_json = '{}'", []).unwrap();
    drop(conn);
    let refused = create_segment(&mut store, "tonovi", (300, 400), (50, 1)).unwrap_err();
    assert!(refused.contains("lead_in_seconds"), "{refused}");
    assert!(segments(&mut store).is_empty());
}

/// Audit 2026-10-25: a shorter lead-out left a user's marker past the end of the story,
/// and its slot past the program.
#[test]
fn a_shorter_lead_out_takes_the_markers_past_the_new_end_with_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = imported_store(&path);
    set_leads(&path, 0.0, 2.0);
    create_segment(&mut store, "offovi", (300, 400), (50, 1)).unwrap();
    // In the lead-out: the segment is 0..100, the lead-out 100..200.
    marker(&mut store, 150).unwrap();
    assert_eq!(markers(&mut store), vec![150]);
    // No lead-out any more: the next edit makes the story 0..100, the marker at 150 goes.
    set_leads(&path, 0.0, 0.0);
    marker(&mut store, 50).unwrap();
    assert_eq!(markers(&mut store), vec![50]);
}
