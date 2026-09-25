//! The program model against a real project content database in a temp dir.

use super::*;
use qnc_content_store::{ContentStore, Data, Request};

const URI: &str = "qnc://local/db/ingest_content/p1";

fn store_with_story(file: &std::path::Path) -> Vec<String> {
    {
        let conn = rusqlite::Connection::open(file).unwrap();
        conn.execute_batch(
            "CREATE TABLE project_settings (project_id TEXT);
             INSERT INTO project_settings (project_id) VALUES ('p1');
             CREATE VIEW public_project_settings AS SELECT project_id FROM project_settings;",
        )
        .unwrap();
    }
    drop(ContentStore::open_owner_binding(file, URI, Access::ReadWrite).unwrap());
    rusqlite::Connection::open(file)
        .unwrap()
        .execute(
            "INSERT INTO clips (clip_id, source_uri, original_uri, name, catalog_json,
                revision, final, selected, import_status)
             VALUES ('c1', 'qnc://local/source/card-a', 'qnc://local/source/card-a/c1.mxf',
                'C1', '{}', 1, 1, 0, 'imported')",
            [],
        )
        .unwrap();
    let mut store = ContentStore::open_owner_binding(file, URI, Access::ReadWrite).unwrap();
    let mut ids = Vec::new();
    for (kind, start) in [("offovi", 0), ("tonovi", 100), ("tonovi", 200)] {
        let Data::Created(id) = store
            .execute(&Request {
                version: qnc_content_store::VERSION.into(),
                db_uri: URI.into(),
                operation: Operation::CreateSegment {
                    project_id: "p1".into(),
                    kind: kind.into(),
                    clip_id: "c1".into(),
                    in_frame: start,
                    out_frame: start + 50,
                    fps_num: 50,
                    fps_den: 1,
                },
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
        if !segments.has_pending_work() {
            segments.poll();
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
    segments.configure(ContentTarget::from_owner_binding(&file, URI).unwrap(), "p1");
    assert_eq!(segments.view().rows.len(), 3);
    segments.apply(SegmentCommand::Select(ids[1].clone()));
    settle(&mut segments);
    // The keyboard catalog sends both delete_part and delete_marker for Delete.
    segments.apply_action("delete_part");
    segments.apply_action("delete_marker");
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
