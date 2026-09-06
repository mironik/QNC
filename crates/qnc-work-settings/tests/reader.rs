use qnc_work_settings::{ReaderConfig, SettingsReader};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::{fs, time::Duration};

fn settings() -> Value {
    let seed: Value = serde_json::from_str(include_str!("../../../seed/system_seed.json")).unwrap();
    seed["project_templates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["template_id"] == "tpl_breaking_news")
        .unwrap()["settings"]
        .clone()
}

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("data")).unwrap();
    let db = Connection::open(root.path().join("data/project_store.db")).unwrap();
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
            "INSERT INTO projects VALUES (?1, ?1, ?2)",
            [id, &format!("qnc://local/project/{id}")],
        )
        .unwrap();
        db.execute(
            "INSERT INTO project_storage_locations VALUES (?1, ?2)",
            [id, dir.to_str().unwrap()],
        )
        .unwrap();
        let workspace = Connection::open(dir.join("qnc_project.db")).unwrap();
        workspace
            .execute_batch(
                "CREATE TABLE project_settings(project_id TEXT PRIMARY KEY, settings_json TEXT);
            CREATE VIEW public_project_settings AS SELECT * FROM project_settings;",
            )
            .unwrap();
        let mut value = settings();
        if id == "p2" {
            value["storage"]["ingest_media"] = json!("original");
        }
        workspace
            .execute(
                "INSERT INTO project_settings VALUES (?1,?2)",
                [id, &value.to_string()],
            )
            .unwrap();
    }
    root
}

#[test]
fn active_selection_is_read_from_database_not_cached_or_supplied_by_ui() {
    let root = fixture();
    let path = root.path().join("data/project_store.db");
    let reader = SettingsReader::local(&path);
    let first = reader.read().unwrap();
    assert_eq!(first.project_id, "p1");
    assert_eq!(first.storage.ingest_media, "link");
    assert_eq!(first.output_root_uri, "qnc://local/project/p1");
    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE app_settings SET value='p2' WHERE key='active_project_id'",
        [],
    )
    .unwrap();
    let second = reader.read().unwrap();
    assert_eq!(second.project_id, "p2");
    assert_eq!(second.storage.ingest_media, "original");
    assert_eq!(first.project_id, "p1", "an existing snapshot is immutable");
}

#[test]
fn standalone_read_has_no_project_app_and_changes_no_database_bytes() {
    let root = fixture();
    let paths = [
        root.path().join("data/project_store.db"),
        root.path().join("p1/qnc_project.db"),
    ];
    let before = paths.each_ref().map(|p| fs::read(p).unwrap());
    let result = SettingsReader::local(&paths[0]).read().unwrap();
    assert_eq!(before, paths.each_ref().map(|p| fs::read(p).unwrap()));
    let result = serde_json::to_string(&result).unwrap();
    assert!(!result.contains("projects_root"));
    assert!(!result.contains(root.path().to_str().unwrap()));
    assert!(!root.path().join("apps").exists());
}

#[test]
fn missing_database_is_not_created() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("absent.db");
    assert_eq!(
        SettingsReader::local(&file).read().unwrap_err().code,
        "database_unavailable"
    );
    assert!(!file.exists());
}

#[test]
fn no_active_project_missing_record_and_missing_binding_fail_closed() {
    for sql in [
        "DELETE FROM app_settings",
        "UPDATE app_settings SET value='absent'",
        "DELETE FROM project_storage_locations",
    ] {
        let root = fixture();
        let file = root.path().join("data/project_store.db");
        Connection::open(&file).unwrap().execute_batch(sql).unwrap();
        assert!(SettingsReader::local(&file).read().is_err());
    }
}

#[test]
fn old_public_view_is_not_migrated_or_bypassed() {
    let root = fixture();
    let file = root.path().join("p1/qnc_project.db");
    let conn = Connection::open(&file).unwrap();
    conn.execute_batch(
        "DROP VIEW public_project_settings;
        CREATE VIEW public_project_settings AS SELECT project_id FROM project_settings;",
    )
    .unwrap();
    drop(conn);
    let before = fs::read(&file).unwrap();
    assert_eq!(
        SettingsReader::local(root.path().join("data/project_store.db"))
            .read()
            .unwrap_err()
            .code,
        "settings_not_public"
    );
    assert_eq!(before, fs::read(file).unwrap());
}

#[test]
fn incomplete_or_invalid_saved_settings_are_not_replaced_with_defaults() {
    for payload in ["{}", "not json"] {
        let root = fixture();
        Connection::open(root.path().join("p1/qnc_project.db"))
            .unwrap()
            .execute("UPDATE project_settings SET settings_json=?1", [payload])
            .unwrap();
        assert!(
            SettingsReader::local(root.path().join("data/project_store.db"))
                .read()
                .is_err()
        );
    }
}

#[test]
fn real_http_transport_reads_same_db_for_lan_and_intranet_and_rejects_unauthorized() {
    let root = fixture();
    let token_var = format!("QNC_TEST_READ_TOKEN_{}", std::process::id());
    std::env::set_var(&token_var, "test-only-transport-token");
    for environment in ["lan", "intranet"] {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", server.server_addr());
        let uri = format!("qnc://{environment}/storage/db/project_registry");
        let file = root.path().join("data/project_store.db");
        let expected_uri = uri.clone();
        let handler = std::thread::spawn(move || {
            for _ in 0..3 {
                let request = server
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap()
                    .expect("request");
                qnc_work_settings::server::respond(
                    request,
                    &file,
                    &expected_uri,
                    "test-only-transport-token",
                );
            }
        });
        let mut config = ReaderConfig {
            registry_uri: uri,
            registry_file: None,
            endpoint: Some(endpoint),
            token_env: Some(token_var.clone()),
        };
        let result = SettingsReader::from_config(config.clone())
            .unwrap()
            .read()
            .unwrap();
        assert_eq!(result.storage.ingest_media, "link");
        assert_eq!(
            result.output_root_uri,
            format!("qnc://{environment}/storage/project/p1")
        );
        config.registry_uri = format!("qnc://{environment}/other/db/project_registry");
        assert!(SettingsReader::from_config(config.clone())
            .unwrap()
            .read()
            .is_err());
        config.token_env = None;
        assert!(SettingsReader::from_config(config).unwrap().read().is_err());
        handler.join().unwrap();
    }
    std::env::remove_var(token_var);
}

#[test]
fn failed_network_transport_never_falls_back_to_local_registry() {
    let root = fixture();
    assert!(
        SettingsReader::local(root.path().join("data/project_store.db"))
            .read()
            .is_ok()
    );
    let config = ReaderConfig {
        registry_uri: "qnc://lan/absent/db/project_registry".into(),
        registry_file: None,
        endpoint: Some("http://192.0.2.1".into()),
        token_env: None,
    };
    assert_eq!(
        SettingsReader::from_config(config)
            .unwrap()
            .read()
            .unwrap_err()
            .code,
        "transport_config"
    );
}

#[test]
fn module_does_not_depend_on_application_crates_or_run_media_tools() {
    let cargo = include_str!("../Cargo.toml");
    for name in ["qnc-project", "qnc-ingest", "qnc-app", "qnc-shell"] {
        assert!(!cargo.contains(name));
    }
    let local = include_str!("../src/local.rs");
    assert!(!local.contains("SQLITE_OPEN_CREATE"));
    assert!(!local.contains("SELECT settings_json FROM project_settings"));
    assert!(!local.contains("Command::new"));
}
