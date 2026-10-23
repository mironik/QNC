use super::*;
use qnc_work_settings::SettingsReader;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::{
    fs,
    time::{Duration, Instant},
};

/// The camera profiles and the catalog of record formats shipped with QNC, into a test root.
fn shipped_catalogs(root: &std::path::Path) {
    let shipped = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for dir in ["catalogs/cameras/sony", "catalogs/record-formats"] {
        fs::create_dir_all(root.join(dir)).unwrap();
        for entry in fs::read_dir(shipped.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            fs::copy(&path, root.join(dir).join(path.file_name().unwrap())).unwrap();
        }
    }
}

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("data")).unwrap();
    shipped_catalogs(root.path());
    let db = Connection::open(root.path().join("data/qnc-projects.db")).unwrap();
    db.execute_batch(
        "CREATE TABLE app_settings(key TEXT PRIMARY KEY, value TEXT);
        CREATE VIEW public_app_settings AS SELECT * FROM app_settings;
        CREATE TABLE projects(project_id TEXT PRIMARY KEY, name TEXT, project_uri TEXT);
        CREATE VIEW public_projects AS SELECT * FROM projects;
        CREATE TABLE project_storage_locations(project_id TEXT PRIMARY KEY, local_path TEXT);
        INSERT INTO app_settings VALUES ('active_project_id','p1');",
    )
    .unwrap();
    for id in ["p1", "p2"] {
        let dir = root.path().join(id);
        fs::create_dir(&dir).unwrap();
        db.execute(
            "INSERT INTO projects VALUES (?1,?1,?2)",
            [id, &format!("qnc://local/project/{id}")],
        )
        .unwrap();
        db.execute(
            "INSERT INTO project_storage_locations VALUES (?1,?2)",
            [id, dir.to_str().unwrap()],
        )
        .unwrap();
        let workspace = Connection::open(dir.join("project.db")).unwrap();
        workspace
            .execute_batch(
                "CREATE TABLE project_settings(project_id TEXT, settings_json TEXT);
            CREATE VIEW public_project_settings AS SELECT * FROM project_settings;",
            )
            .unwrap();
        let value = json!({
            "storage": {"ingest_profile":"field", "ingest_media": if id=="p1" {"link"} else {"original"},
                "proxy_policy":"link_when_available", "original_policy":"link_when_available"},
            "products": {
                "root": "products",
                "thumbnails": "products/thumbnails",
                "filmstrip": "products/filmstrip",
                "virtual_shorts": "products/virtual_shorts",
                "virtual_segments": "products/virtual_segments",
                "b_roll_virtual_clips": "products/b_roll_virtual_clips"
            },
            "input":{"mode":"auto"}, "playback":{"input":"proxy_if_available"},
            "video":{"fps":50}, "audio":{"sample_rate":48000, "channels":2},
            "ai":{"enabled":id=="p2"}, "keyboard_shortcuts":{"active_preset":"default"}
        });
        workspace
            .execute(
                "INSERT INTO project_settings VALUES (?1,?2)",
                [id, &value.to_string()],
            )
            .unwrap();
    }
    root
}

fn wait(component: &mut IngestApplication) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while component.has_pending_work() {
        component.poll(None);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
}

mod catalog_tests;
mod settings_tests;
mod source_tests;
