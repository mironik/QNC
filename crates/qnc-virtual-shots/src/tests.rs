use super::*;
use qnc_content_store::{Access as ContentAccess, ContentStore};

const CONTENT: &str = "qnc://local/db/ingest_content/p1";
const PROJECT_DB: &str = "qnc://local/db/project_db/p1";

/// A project database with clip `clip-a` imported and `clip-b` only detected.
fn project() -> (tempfile::TempDir, ProjectDbTarget) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.db");
    {
        let conn = rusqlite::Connection::open(&file).unwrap();
        conn.execute_batch(
            "CREATE TABLE project_settings (project_id TEXT);
             INSERT INTO project_settings (project_id) VALUES ('p1');
             CREATE VIEW public_project_settings AS SELECT project_id FROM project_settings;",
        )
        .unwrap();
    }
    drop(ContentStore::open_owner_binding(&file, CONTENT, ContentAccess::ReadWrite).unwrap());
    let conn = rusqlite::Connection::open(&file).unwrap();
    for (clip, status) in [("clip-a", "imported"), ("clip-b", "detected")] {
        conn.execute(
            "INSERT INTO clips (
                clip_id, source_uri, original_uri, name, catalog_json, revision, final,
                selected, import_status
             ) VALUES (?1, 'qnc://local/source/card-a', 'qnc://local/source/card-a/' || ?1,
                'Mironik', '{}', 1, 1, 0, ?2)",
            [clip, status],
        )
        .unwrap();
    }
    drop(conn);
    let target = ProjectDbTarget::from_owner_binding(&file, PROJECT_DB).unwrap();
    (dir, target)
}

fn save(target: &ProjectDbTarget, clip: &str, range: (u64, u64)) -> Result<SavedShort> {
    save_short_now(target, "p1", clip, "Mironik", range.0, range.1)
}

#[test]
fn a_project_without_virtual_shots_lists_none() {
    let (_dir, target) = project();
    assert!(list_pool_shots(&target).unwrap().is_empty());
}

#[test]
fn a_short_is_saved_through_the_intermediary_and_listed_oldest_first() {
    let (_dir, target) = project();
    let first = save(&target, "clip-a", (10, 40)).unwrap();
    let second = save(&target, "clip-a", (50, 60)).unwrap();
    assert_eq!(first.shot_id, "clip-a_shot_001");
    assert_eq!(second.shot_id, "clip-a_shot_002");
    let shorts = list_shorts(&target).unwrap();
    assert_eq!(shorts.len(), 2);
    assert_eq!((shorts[0].in_frame, shorts[0].out_frame), (10, 40));
    assert_eq!(shorts[0].name, "Mironik 001");
    assert_eq!(shorts[0].still_status, "pending");
    assert!(!shorts[0].b_roll);
}

#[test]
fn only_an_imported_clip_and_a_real_range_make_a_short() {
    let (_dir, target) = project();
    assert!(save(&target, "clip-b", (0, 10)).unwrap_err().contains("nije uvezen"));
    assert!(save(&target, "clip-z", (0, 10)).is_err());
    assert!(save(&target, "clip-a", (10, 10)).is_err());
    assert!(list_shorts(&target).unwrap().is_empty());
}

#[test]
fn stills_of_a_short_are_published_ready_or_failed() {
    let (_dir, target) = project();
    let shot = save(&target, "clip-a", (10, 40)).unwrap();
    let stills = Ok((
        "qnc://local/artifact/in.jpg".to_string(),
        "qnc://local/artifact/out.jpg".to_string(),
    ));
    publish_stills_now(&target, &shot.shot_id, stills).unwrap();
    let short = &list_shorts(&target).unwrap()[0];
    assert_eq!(short.still_status, "ready");
    assert_eq!(short.in_still_uri.as_deref(), Some("qnc://local/artifact/in.jpg"));
    publish_stills_now(&target, &shot.shot_id, Err("disk".into())).unwrap();
    assert_eq!(list_shorts(&target).unwrap()[0].still_status, "failed");
    assert!(publish_stills_now(&target, "missing", Err("x".into())).is_err());
}

#[test]
fn a_cover_shot_is_a_b_roll_shot_listed_after_the_shorts() {
    let (_dir, target) = project();
    save(&target, "clip-a", (10, 40)).unwrap();
    let writer = VirtualShotsWriter::start(target.clone()).unwrap();
    let pending = writer
        .submit(&Operation::CreateCoverShot {
            project_id: "p1".into(),
            clip_id: "clip-a".into(),
            clip_name: "Mironik".into(),
            in_frame: 5,
            out_frame: 25,
        })
        .unwrap();
    let reply = loop {
        if let Some(reply) = pending.try_take() {
            break reply;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    assert_eq!(reply.unwrap(), Data::Created("clip-a_broll_001".into()));
    let pool = list_pool_shots(&target).unwrap();
    assert_eq!(pool.len(), 2);
    assert!(!pool[0].b_roll && pool[1].b_roll);
    assert_eq!(pool[1].name, "Mironik B001");
    assert!(list_shorts(&target).unwrap().iter().all(|shot| !shot.b_roll));
}

#[test]
fn attaching_again_to_a_project_keeps_its_shots() {
    let (dir, target) = project();
    save(&target, "clip-a", (10, 40)).unwrap();
    let conn = rusqlite::Connection::open(dir.path().join("project.db")).unwrap();
    let mut store = store::Store::attach(conn, Access::ReadWrite).unwrap();
    let Data::ShortClips(shorts) = store.execute(&Operation::ListShorts).unwrap() else {
        panic!()
    };
    assert_eq!(shorts.len(), 1);
}
