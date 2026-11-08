use super::*;
use rusqlite::Connection;

const PROJECT_DB: &str = "qnc://local/db/project_db/p1";

fn project() -> (tempfile::TempDir, ProjectDbTarget) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.db");
    Connection::open(&file)
        .unwrap()
        .execute_batch(
            "CREATE TABLE project_settings (project_id TEXT);
             INSERT INTO project_settings (project_id) VALUES ('p1');
             CREATE VIEW public_project_settings AS SELECT project_id FROM project_settings;",
        )
        .unwrap();
    (dir, ProjectDbTarget::from_owner_binding(&file, PROJECT_DB).unwrap())
}

#[test]
fn every_send_is_a_new_version_numbered_one_two_and_listed_in_order() {
    let (_dir, target) = project();
    assert!(versions(&target).unwrap().is_empty(), "no table yet: no versions");
    assert_eq!(new_version(&target).unwrap(), 1);
    // From a version's own target too: the list is in the project database.
    assert_eq!(new_version(&target.draft_story(1)).unwrap(), 2);
    let listed = versions(&target).unwrap();
    assert_eq!(listed.iter().map(|version| version.number).collect::<Vec<_>>(), [1, 2]);
}

/// A version's story file beside the project database (its list made again) is never given to
/// a new version (audit 2026-11-08 #18): the new one would add to that old story.
#[test]
fn a_new_version_is_numbered_past_every_story_file_there() {
    let (dir, target) = project();
    std::fs::write(dir.path().join(qnc_db_broker::draft_file(3)), b"").unwrap();
    assert_eq!(new_version(&target).unwrap(), 4);
}
