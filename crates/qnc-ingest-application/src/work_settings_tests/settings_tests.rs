use super::*;

#[test]
fn reads_existing_active_settings_and_v4_directory_roles_without_ui_payload() {
    let root = fixture();
    let mut component = IngestApplication::with_store_root(root.path()).unwrap();
    assert!(component.view.work_settings_loading);
    wait(&mut component);
    let plan = component.work_plan().unwrap();
    assert_eq!(plan.media, IngestMedia::Link);
    assert_eq!(plan.playback_input, PlaybackInput::ProxyIfAvailable);
    assert_eq!(plan.proxy_uri, "qnc://local/project/p1/proxy");
    assert_eq!(plan.original_uri, "qnc://local/project/p1/original");
    assert_eq!(
        plan.thumbnails_uri,
        "qnc://local/project/p1/products/thumbnails"
    );
    assert_eq!(
        plan.filmstrip_uri,
        "qnc://local/project/p1/products/filmstrip"
    );
    assert_eq!(plan.settings.video["fps"], 50);
    assert!(!component.view.ai_mining);
}

#[test]
fn missing_project_db_is_not_recreated_and_import_is_rejected() {
    use qnc_ingest_store::content::{Access, ContentTarget};
    let root = fixture();
    let reader = SettingsReader::local(root.path().join("data/qnc-projects.db"));
    let target = ContentTarget::for_project(&reader, &reader.read().unwrap()).unwrap();
    let file = root.path().join("p1/project.db");
    std::fs::remove_file(&file).unwrap();
    assert!(target.open(Access::ReadWrite).is_err());
    assert!(!file.exists());
    let mut component = IngestApplication::with_store_root(root.path()).unwrap();
    wait(&mut component);
    assert!(!component.view.work_settings_ready);
    assert!(
        !component
            .dispatch(IngestIntent::empty(action_ids::INGEST_IMPORT_SELECTED))
            .accepted
    );
    assert!(component.view.clips.is_empty());
}

#[test]
fn missing_settings_gate_selection_and_import_without_default_project() {
    let root = fixture();
    Connection::open(root.path().join("p1/project.db"))
        .unwrap()
        .execute("UPDATE project_settings SET settings_json='{}'", [])
        .unwrap();
    let mut component = IngestApplication::with_store_root(root.path()).unwrap();
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
    let mut settings = SettingsReader::local(root.path().join("data/qnc-projects.db"))
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
    let db = Connection::open(root.path().join("data/qnc-projects.db")).unwrap();
    db.execute(
        "UPDATE projects SET name='Loaded project' WHERE project_id='p1'",
        [],
    )
    .unwrap();
    let mut component = IngestApplication::with_store_root(root.path()).unwrap();
    assert_eq!(component.footer_status(), qnc_ui_text::text("ingest.msg.project_loading"));
    wait(&mut component);
    assert_eq!(component.footer_status(), "Loaded project");

    db.execute("UPDATE app_settings SET value='p2'", [])
        .unwrap();
    assert_eq!(component.footer_status(), "Loaded project", "not read yet");
    component.dispatch(IngestIntent::empty(action_ids::INGEST_RELOAD));
    assert_eq!(component.footer_status(), qnc_ui_text::text("ingest.msg.project_loading"));
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
        IngestApplication::default().footer_status(),
        qnc_ui_text::text("ingest.msg.project_not_loaded")
    );
}
