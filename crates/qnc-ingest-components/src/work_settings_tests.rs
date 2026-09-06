use super::*;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::{
    fs,
    time::{Duration, Instant},
};

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
            "INSERT INTO projects VALUES (?1,?1,?2)",
            [id, &format!("qnc://local/project/{id}")],
        )
        .unwrap();
        db.execute(
            "INSERT INTO project_storage_locations VALUES (?1,?2)",
            [id, dir.to_str().unwrap()],
        )
        .unwrap();
        let workspace = Connection::open(dir.join("qnc_project.db")).unwrap();
        workspace
            .execute_batch(
                "CREATE TABLE project_settings(project_id TEXT, settings_json TEXT);
            CREATE VIEW public_project_settings AS SELECT * FROM project_settings;",
            )
            .unwrap();
        let value = json!({
            "storage": {"ingest_profile":"field", "ingest_media": if id=="p1" {"link"} else {"original"},
                "proxy_policy":"link_when_available", "original_policy":"link_when_available"},
            "input":{"mode":"auto"}, "playback":{"input":"proxy_if_available"},
            "video":{"fps":50}, "audio":{"sample_rate":48000},
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

fn wait(component: &mut IngestComponent) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while component.view.work_settings_loading {
        component.poll();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn reads_existing_active_settings_and_v4_directory_roles_without_ui_payload() {
    let root = fixture();
    let mut component = IngestComponent::with_store_root(root.path()).unwrap();
    assert!(component.view.work_settings_loading);
    wait(&mut component);
    let plan = component.work_plan().unwrap();
    assert_eq!(plan.media, IngestMedia::Link);
    assert_eq!(plan.playback_input, PlaybackInput::ProxyIfAvailable);
    assert_eq!(plan.proxy_uri, "qnc://local/project/p1/proxy");
    assert_eq!(plan.original_uri, "qnc://local/project/p1/original");
    assert_eq!(
        plan.thumbnails_uri,
        "qnc://local/project/p1/ingest/thumbnails"
    );
    assert_eq!(plan.settings.video["fps"], 50);
    assert!(!component.view.ai_mining);
}

#[test]
fn reload_changes_plan_from_db_and_clears_only_other_projects_selection() {
    let root = fixture();
    let mut component = IngestComponent::with_store_root(root.path()).unwrap();
    wait(&mut component);
    component.view.clips.push(ClipView {
        clip_id: "clip".into(),
        selected: true,
        ..Default::default()
    });
    component.dispatch(IngestIntent::empty(action_ids::INGEST_RELOAD));
    wait(&mut component);
    assert_eq!(component.view.selected_count(), 1);
    Connection::open(root.path().join("data/project_store.db"))
        .unwrap()
        .execute("UPDATE app_settings SET value='p2'", [])
        .unwrap();
    component.dispatch(IngestIntent::empty(action_ids::INGEST_RELOAD));
    assert!(
        component.work_plan().is_none(),
        "do not expose stale settings while reading"
    );
    wait(&mut component);
    assert_eq!(component.work_plan().unwrap().settings.project_id, "p2");
    assert_eq!(component.work_plan().unwrap().media, IngestMedia::Original);
    assert!(component.view.clips.is_empty());
    assert!(component.view.ai_mining);
    assert!(
        !component
            .dispatch(IngestIntent::new(
                action_ids::INGEST_SET_AI_MINING,
                IngestPayload::Bool(false)
            ))
            .accepted
    );
    assert!(component.view.ai_mining);
}

#[test]
fn select_rereads_current_active_project_and_cancel_prevents_source_write() {
    let root = fixture();
    let mut component = IngestComponent::with_store_root(root.path()).unwrap();
    wait(&mut component);
    Connection::open(root.path().join("data/project_store.db"))
        .unwrap()
        .execute("UPDATE app_settings SET value='p2'", [])
        .unwrap();
    assert!(
        component
            .dispatch(IngestIntent::new(
                action_ids::INGEST_DIR_CONFIRM,
                IngestPayload::LocationUri("qnc://local/source/test".into())
            ))
            .accepted
    );
    assert!(
        !component
            .dispatch(IngestIntent::empty(action_ids::INGEST_RELOAD))
            .accepted,
        "one worker only"
    );
    component.dispatch(IngestIntent::empty(action_ids::INGEST_DIR_CANCEL));
    wait(&mut component);
    assert_eq!(component.work_plan().unwrap().settings.project_id, "p2");
    let count: i64 = Connection::open(root.path().join("data/ingest_registry.db"))
        .unwrap()
        .query_row("SELECT COUNT(*) FROM source_sessions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn missing_settings_gate_selection_and_import_without_default_project() {
    let root = fixture();
    Connection::open(root.path().join("p1/qnc_project.db"))
        .unwrap()
        .execute("UPDATE project_settings SET settings_json='{}'", [])
        .unwrap();
    let mut component = IngestComponent::with_store_root(root.path()).unwrap();
    wait(&mut component);
    assert!(component.work_plan().is_none());
    assert!(component.view.work_settings_error.is_some());
    assert!(
        !component
            .dispatch(IngestIntent::empty(action_ids::INGEST_IMPORT_SELECTED))
            .accepted
    );
    component.dispatch(IngestIntent::new(
        action_ids::INGEST_DIR_CONFIRM,
        IngestPayload::LocationUri("qnc://local/source/test".into()),
    ));
    wait(&mut component);
    assert!(component.view.selected_source_uri.is_none());
}

#[test]
fn unknown_media_policy_is_not_silently_reinterpreted() {
    let root = fixture();
    let mut settings = SettingsReader::local(root.path().join("data/project_store.db"))
        .read()
        .unwrap();
    settings.storage.ingest_media = "invented-mode".into();
    assert!(IngestWorkPlan::from_settings(settings.clone()).is_err());
    for (value, expected) in [
        ("link", IngestMedia::Link),
        ("proxy", IngestMedia::Proxy),
        ("original", IngestMedia::Original),
    ] {
        settings.storage.ingest_media = value.into();
        assert_eq!(
            IngestWorkPlan::from_settings(settings.clone())
                .unwrap()
                .media,
            expected
        );
    }
    settings.playback["input"] = Value::String("unknown".into());
    assert!(IngestWorkPlan::from_settings(settings).is_err());
}

#[test]
fn footer_reports_only_the_project_loaded_by_ingest_not_stale_or_guessed_names() {
    let root = fixture();
    let db = Connection::open(root.path().join("data/project_store.db")).unwrap();
    db.execute(
        "UPDATE projects SET name='Loaded project' WHERE project_id='p1'",
        [],
    )
    .unwrap();
    let mut component = IngestComponent::with_store_root(root.path()).unwrap();
    assert_eq!(component.footer_status(), "Ucitavanje projekta...");
    wait(&mut component);
    assert_eq!(component.footer_status(), "Loaded project");

    db.execute("UPDATE app_settings SET value='p2'", [])
        .unwrap();
    assert_eq!(component.footer_status(), "Loaded project", "not read yet");
    component.dispatch(IngestIntent::empty(action_ids::INGEST_RELOAD));
    assert_eq!(component.footer_status(), "Ucitavanje projekta...");
    wait(&mut component);
    assert_eq!(component.footer_status(), "p2");

    db.execute("DELETE FROM app_settings", []).unwrap();
    component.dispatch(IngestIntent::empty(action_ids::INGEST_RELOAD));
    wait(&mut component);
    assert!(component.work_plan().is_none());
    assert_eq!(
        Some(component.footer_status()),
        component.view.work_settings_error.as_deref()
    );
    assert_ne!(component.footer_status(), "p2");
    assert_eq!(
        IngestComponent::default().footer_status(),
        "Projekt nije ucitan."
    );
}
