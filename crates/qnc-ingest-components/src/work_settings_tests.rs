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
    while component.has_pending_work() {
        component.poll();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn unavailable_card_does_not_disable_registered_browser_or_start_player() {
    let root = fixture();
    let source = |id: &str, file: &str| {
        json!({
            "location": {"uri": format!("qnc://local/source/{id}"), "file": file},
            "name": id, "serial_number": "", "volume_name": "", "scope": "card_relative",
            "probe": {"kind": "local", "executable": "never-executed", "probe_size_bytes": 8388608, "analyze_duration_us": 1000000}
        })
    };
    let mut intranet = source("remote", "unused");
    intranet["location"] = json!({"uri": "qnc://intranet/test/source/remote", "endpoint": "http://127.0.0.1:1", "token_env": "QNC_TEST_UNAVAILABLE_CREDENTIAL"});
    let config = json!({
        "version": "0.1.0", "parallelism": 1,
        "catalog": {"uri":"qnc://local/catalog/camera-patterns", "file":"catalog.db"},
        "source_index": {"uri":"qnc://local/db/source_index", "file":"source.db"},
        "media_records": {"uri":"qnc://local/db/media_records", "file":"media.db"},
        "sources": [source("offline", "../not-connected"), source("online", "../p1"), intranet]
    });
    fs::write(
        root.path().join("data/ingest-transport.json"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
    let mut component = IngestComponent::with_store_root(root.path()).unwrap();
    wait(&mut component);
    assert!(component.selection_config_error.is_none());
    assert!(component.selection_config.is_some());
    assert!(component.view.work_settings_ready);
    assert_eq!(component.view.browser_entries.len(), 2);
    component.dispatch(IngestIntent::new(
        action_ids::INGEST_DIR_OPEN,
        IngestPayload::LocationUri("qnc://local/source/offline".into()),
    ));
    wait(&mut component);
    assert!(component
        .view
        .browser_error
        .as_ref()
        .unwrap()
        .contains("qnc://local/source/offline"));
    component.dispatch(IngestIntent::new(
        action_ids::INGEST_DIR_OPEN,
        IngestPayload::LocationUri("qnc://local/source/online".into()),
    ));
    wait(&mut component);
    assert!(component.view.browser_error.is_none());
    assert_eq!(
        component.view.browser_current_uri.as_deref(),
        Some("qnc://local/source/online")
    );
    component.dispatch(IngestIntent::empty(action_ids::INGEST_SOURCE_KIND_INTERNET));
    wait(&mut component);
    assert!(component.view.browser_error.is_none());
    assert_eq!(
        component.view.browser_entries[0].qnc_uri,
        "qnc://intranet/test/source/remote"
    );
    assert!(component.player.is_none());
    assert!(component.selection_thread.is_none());
    assert!(component.work_plan().is_some());
}

#[test]
fn uncommitted_preview_cannot_be_selected_and_stale_ack_does_not_mark_it_saved() {
    let mut component = IngestComponent::default();
    let (send, receive) = mpsc::sync_channel(8);
    component.selection_result = Some(receive);
    send.send(selection::Event::Clip(selection::SelectedClip {
        clip_id: "clip-pending".into(),
        metadata_revision: 2,
        save_state: selection::SelectSaveState::Pending,
        ..Default::default()
    }))
    .unwrap();
    send.send(selection::Event::Saved {
        revisions: vec![("clip-pending".into(), 1)],
        error: None,
    })
    .unwrap();
    component.poll();
    assert_eq!(component.view.clips[0].save_state, SaveState::Pending);
    assert!(
        !component
            .select_clips(vec!["clip-pending".into()], true)
            .accepted
    );
    send.send(selection::Event::Saved {
        revisions: vec![("clip-pending".into(), 2)],
        error: Some("write failed".into()),
    })
    .unwrap();
    component.poll();
    assert_eq!(component.view.clips[0].save_state, SaveState::Failed);
    assert!(!component.view.clips[0].selected);
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
fn reload_changes_plan_from_db_and_discards_unpersisted_ui_state() {
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
    assert_eq!(
        component.view.selected_count(),
        0,
        "UI is not the selection authority"
    );
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
fn activation_checks_catalog_signature_and_reloads_only_when_db_changed() {
    use qnc_ingest_store::content::{Access, ContentClient, ContentTarget};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    let root = fixture();
    let (source_fixture, config) = selection::test_support::fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    selection::test_support::execute(&config, &calls, false, false, ".");
    assert_eq!(calls.load(Ordering::SeqCst), 4);

    let reader = SettingsReader::local(root.path().join("data/project_store.db"));
    let target = ContentTarget::for_project(&reader, &reader.read().unwrap()).unwrap();
    let mut source = ContentClient::from_owner_binding(
        &source_fixture.path().join("content.db"),
        "qnc://local/db/ingest_content/p1",
        Access::ReadOnly,
    )
    .unwrap();
    let source_clips = source.list(None).unwrap();
    let mut db = target.open(Access::ReadWrite).unwrap();
    db.publish(source_clips[0].clip.clone()).unwrap();
    drop(db);

    let mut component = IngestComponent::with_store_root(root.path()).unwrap();
    wait(&mut component);
    assert_eq!(component.view.clips.len(), 1);
    let preview_id = component.view.clips[0].clip_id.clone();
    assert!(
        component
            .dispatch(IngestIntent::new(
                action_ids::INGEST_PREVIEW_FOCUS,
                IngestPayload::ClipId(preview_id.clone())
            ))
            .accepted
    );

    assert!(component.refresh_active_project().accepted);
    assert!(
        component.work_plan().is_some(),
        "tab activation must not blank the current project while checking DB"
    );
    wait(&mut component);
    assert_eq!(component.view.clips.len(), 1);
    assert_eq!(
        component.view.preview_clip_id.as_deref(),
        Some(preview_id.as_str())
    );

    let mut db = target.open(Access::ReadWrite).unwrap();
    db.publish(source_clips[1].clip.clone()).unwrap();
    drop(db);
    assert!(component.refresh_active_project().accepted);
    wait(&mut component);
    assert_eq!(component.view.clips.len(), 2);
    assert_eq!(
        component.view.preview_clip_id.as_deref(),
        Some(preview_id.as_str()),
        "same project refresh keeps focused clip if it still exists"
    );
}

#[test]
fn catalog_selection_and_source_metadata_survive_restart_and_project_switch_without_probe() {
    use qnc_ingest_store::content::{Access, ContentClient, ContentTarget};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let root = fixture();
    let (source_fixture, config) = selection::test_support::fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    selection::test_support::execute(&config, &calls, false, false, ".");
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    let reader = SettingsReader::local(root.path().join("data/project_store.db"));
    let target = ContentTarget::for_project(&reader, &reader.read().unwrap()).unwrap();
    let mut db = target.open(Access::ReadWrite).unwrap();
    let mut source = ContentClient::from_owner_binding(
        &source_fixture.path().join("content.db"),
        "qnc://local/db/ingest_content/p1",
        Access::ReadOnly,
    )
    .unwrap();
    let mut seeded = Vec::new();
    for stored in source.list(None).unwrap() {
        let mut clip = stored.clip;
        clip.serial_number = "card-serial".into();
        clip.volume_name = "camera-volume".into();
        seeded.push(db.publish(clip).unwrap());
    }
    drop(db);
    let id = seeded[0].clip.id().to_string();
    let project = Connection::open(root.path().join("p1/qnc_project.db")).unwrap();
    let before: String = project
        .query_row("SELECT settings_json FROM project_settings", [], |r| {
            r.get(0)
        })
        .unwrap();
    let mut component = IngestComponent::with_store_root(root.path()).unwrap();
    wait(&mut component);
    assert_eq!(component.view.clips.len(), 2);
    assert_eq!(component.view.selected_source_serial_number, "card-serial");
    assert_eq!(component.view.selected_source_volume_name, "camera-volume");
    let preview_id = component
        .view
        .clips
        .iter()
        .find(|c| c.clip_id != id)
        .unwrap()
        .clip_id
        .clone();
    assert!(
        component
            .dispatch(IngestIntent::new(
                action_ids::INGEST_PREVIEW_FOCUS,
                IngestPayload::ClipId(preview_id.clone()),
            ))
            .accepted
    );
    assert_eq!(
        component.view.selected_count(),
        0,
        "preview does not check the clip"
    );
    assert!(
        !component.has_pending_work(),
        "preview does not start a DB selection write"
    );
    assert!(
        component
            .dispatch(IngestIntent::new(
                action_ids::INGEST_CLIP_TOGGLE,
                IngestPayload::ClipId(id.clone())
            ))
            .accepted
    );
    assert_eq!(
        component.view.selected_count(),
        0,
        "no optimistic selection before DB acknowledgement"
    );
    wait(&mut component);
    assert_eq!(component.view.selected_count(), 1);
    assert_eq!(
        component.view.preview_clip_id.as_deref(),
        Some(preview_id.as_str()),
        "checkbox does not change preview focus"
    );
    drop(component);
    let mut restarted = IngestComponent::with_store_root(root.path()).unwrap();
    wait(&mut restarted);
    assert_eq!(restarted.view.clips.len(), 2);
    assert!(
        restarted
            .view
            .clips
            .iter()
            .find(|c| c.clip_id == id)
            .unwrap()
            .selected
    );
    assert!(
        restarted.view.clips.iter().any(|c| c.thumb_uri.is_some()),
        "poster URI survives offline reload"
    );
    // Keep the selected old clip hidden while batch selection targets only the new clip.
    restarted
        .view
        .clips
        .iter_mut()
        .find(|c| c.clip_id != id)
        .unwrap()
        .previously_seen = false;
    restarted.dispatch(IngestIntent::new(
        action_ids::INGEST_SET_CLIP_FILTER,
        IngestPayload::ClipFilter(ClipFilter::New),
    ));
    assert_eq!(restarted.view.visible_clips().count(), 1);
    assert!(
        restarted
            .dispatch(IngestIntent::empty(action_ids::INGEST_SELECT_ALL))
            .accepted
    );
    wait(&mut restarted);
    assert_eq!(restarted.view.selected_count(), 2);
    assert!(
        restarted
            .dispatch(IngestIntent::empty(action_ids::INGEST_CLEAR_SELECTION))
            .accepted
    );
    wait(&mut restarted);
    assert_eq!(
        restarted.view.selected_count(),
        1,
        "hidden old selection survives clear"
    );
    assert!(
        restarted
            .view
            .clips
            .iter()
            .find(|c| c.clip_id == id)
            .unwrap()
            .selected
    );
    let selected_in_db = target.open(Access::ReadOnly).unwrap().list(None).unwrap();
    assert_eq!(selected_in_db.iter().filter(|c| c.selected).count(), 1);
    assert!(
        selected_in_db
            .iter()
            .find(|c| c.clip.id() == id)
            .unwrap()
            .selected
    );
    let registry = Connection::open(root.path().join("data/project_store.db")).unwrap();
    registry
        .execute("UPDATE app_settings SET value='p2'", [])
        .unwrap();
    restarted.dispatch(IngestIntent::empty(action_ids::INGEST_RELOAD));
    wait(&mut restarted);
    assert!(restarted.view.clips.is_empty());
    assert_eq!(restarted.view.clip_filter, ClipFilter::All);
    assert!(restarted.view.selected_source_serial_number.is_empty());
    assert_eq!(
        calls.load(Ordering::SeqCst),
        4,
        "reload never invokes probe"
    );
    assert_eq!(
        project
            .query_row::<String, _, _>("SELECT settings_json FROM project_settings", [], |r| r
                .get(0))
            .unwrap(),
        before
    );
    assert!(!root.path().join("data/ingest_content.db").exists());
}

#[test]
fn missing_project_db_is_not_recreated_and_rejected_selection_never_changes_ui() {
    use qnc_ingest_store::content::{Access, ContentTarget};
    let root = fixture();
    let reader = SettingsReader::local(root.path().join("data/project_store.db"));
    let target = ContentTarget::for_project(&reader, &reader.read().unwrap()).unwrap();
    let file = root.path().join("p1/qnc_project.db");
    std::fs::remove_file(&file).unwrap();
    assert!(target.open(Access::ReadWrite).is_err());
    assert!(!file.exists());
    let mut component = IngestComponent::with_store_root(root.path()).unwrap();
    wait(&mut component);
    assert!(!component.view.work_settings_ready);
    assert!(
        !component
            .dispatch(IngestIntent::empty(action_ids::INGEST_SELECT_ALL))
            .accepted
    );
    assert!(component.view.clips.is_empty());
}

#[test]
fn select_rereads_current_active_project_and_cancel_prevents_source_write() {
    let root = fixture();
    let mut component = IngestComponent::with_store_root(root.path()).unwrap();
    wait(&mut component);
    let uri = "qnc://local/source/test";
    let source_path = root.path().to_path_buf();
    let mut browser =
        qnc_dir_browser::TransportBrowserSession::new(vec![qnc_dir_browser::BrowserSource::new(
            qnc_dir_browser::BrowserEntry {
                name: "Test".into(),
                qnc_uri: uri.into(),
                ..Default::default()
            },
            move || {
                qnc_source_reader::SourceReader::local(uri, &source_path).map_err(|e| e.to_string())
            },
        )])
        .unwrap();
    browser.roots("local").unwrap();
    component.apply_source_browser_result(browser.open(uri));
    component.transport_browser = Some(browser);
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
