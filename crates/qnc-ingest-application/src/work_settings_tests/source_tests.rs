use super::*;

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
    let mut component = IngestApplication::with_store_root(root.path()).unwrap();
    wait(&mut component);
    assert!(component.selection_config_error.is_none());
    assert!(component.selection_config.is_some());
    assert!(component.view.work_settings_ready);
    // The two registered local sources are listed; the disks attached to this computer are
    // listed with them (user 2026-10-16).
    let listed = |uri: &str| component.view.browser.entries.iter().any(|entry| entry.qnc_uri == uri);
    assert!(listed("qnc://local/source/offline") && listed("qnc://local/source/online"));
    component.dispatch(IngestIntent::new(
        action_ids::INGEST_DIR_OPEN,
        IngestPayload::LocationUri("qnc://local/source/offline".into()),
    ));
    wait(&mut component);
    assert!(component
        .view
        .browser.error
        .as_ref()
        .unwrap()
        .contains("qnc://local/source/offline"));
    component.dispatch(IngestIntent::new(
        action_ids::INGEST_DIR_OPEN,
        IngestPayload::LocationUri("qnc://local/source/online".into()),
    ));
    wait(&mut component);
    assert!(component.view.browser.error.is_none());
    assert_eq!(
        component.view.browser.current_uri.as_deref(),
        Some("qnc://local/source/online")
    );
    component.dispatch(IngestIntent::empty(action_ids::INGEST_SOURCE_KIND_INTERNET));
    wait(&mut component);
    assert!(component.view.browser.error.is_none());
    assert_eq!(
        component.view.browser.entries[0].qnc_uri,
        "qnc://intranet/test/source/remote"
    );
    assert!(!component.preview.has_player());
    assert!(!component.select.is_busy());
    assert!(component.work_plan().is_some());
}

#[test]
fn select_rereads_current_active_project_and_cancel_prevents_source_write() {
    let root = fixture();
    let mut component = IngestApplication::with_store_root(root.path()).unwrap();
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
    component.apply_source_browser_result(browser.open(uri).map(Into::into));
    component.browse.connect(browser);
    Connection::open(root.path().join("data/qnc-projects.db"))
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
fn source_selection_records_private_local_path_for_local_owner_binding() {
    let root = fixture();
    let source_root = root.path().join("p1");
    let config = json!({
        "version": "0.1.0", "parallelism": 1,
        "catalog": {"uri":"qnc://local/catalog/camera-patterns", "file":"catalog.db"},
        "source_index": {"uri":"qnc://local/db/source_index", "file":"source.db"},
        "media_records": {"uri":"qnc://local/db/media_records", "file":"media.db"},
        "sources": [{
            "location": {"uri": "qnc://local/source/test-card", "file": "../p1"},
            "name": "Test Card",
            "serial_number": "",
            "volume_name": "",
            "scope": "card_relative",
            "probe": {"kind": "local", "executable": "never-executed", "probe_size_bytes": 8388608, "analyze_duration_us": 1000000}
        }]
    });
    fs::write(
        root.path().join("data/ingest-transport.json"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
    let mut component = IngestApplication::with_store_root(root.path()).unwrap();
    wait(&mut component);
    component.dispatch(IngestIntent::empty(action_ids::INGEST_DIR_ROOTS));
    wait(&mut component);
    let uri = component.view.browser.entries[0].qnc_uri.clone();

    assert!(
        component
            .dispatch(IngestIntent::new(
                action_ids::INGEST_DIR_OPEN,
                IngestPayload::LocationUri(uri.clone())
            ))
            .accepted
    );
    wait(&mut component);
    assert!(
        component
            .dispatch(IngestIntent::new(
                action_ids::INGEST_DIR_CONFIRM,
                IngestPayload::LocationUri(uri)
            ))
            .accepted
    );
    wait(&mut component);

    let stored: String = Connection::open(root.path().join("data/ingest_registry.db"))
        .unwrap()
        .query_row(
            "SELECT local_path FROM source_location_bindings WHERE source_uri='qnc://local/source/test-card'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        stored,
        source_root.canonicalize().unwrap().to_string_lossy()
    );
}

