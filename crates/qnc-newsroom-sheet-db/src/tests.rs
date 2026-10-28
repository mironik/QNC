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

fn sheet(slug: &str) -> StorySheet {
    StorySheet {
        sheet: serde_json::json!({ "sheet_id": "news", "fields": [{ "id": "slug", "label": "Slug" }] }),
        values: [("slug".to_string(), Value::String(slug.into())), ("target_seconds".to_string(), Value::from(90))].into(),
    }
}

#[test]
fn a_project_without_a_sheet_reads_an_empty_one_and_a_saved_sheet_replaces_the_last() {
    let (_dir, target) = project();
    assert_eq!(read(&target).unwrap(), StorySheet::default());
    save(&target, sheet("PROTEST")).unwrap();
    let mut again = sheet("PROTEST_SPLIT");
    again.values.remove("target_seconds");
    save(&target, again.clone()).unwrap();
    assert_eq!(read(&target).unwrap(), again);
}

#[test]
fn a_bad_code_or_a_sheet_that_is_no_object_keeps_nothing() {
    let (_dir, target) = project();
    let mut bad = sheet("X");
    bad.values.insert("Slug Name".into(), Value::Null);
    assert!(save(&target, bad).is_err());
    assert!(save(&target, StorySheet { sheet: Value::from(3), values: BTreeMap::new() }).is_err());
    assert_eq!(read(&target).unwrap(), StorySheet::default());
}
