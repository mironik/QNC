use super::*;

#[test]
fn marking_an_uncommitted_preview_is_local_and_stale_ack_does_not_mark_it_saved() {
    let mut component = IngestApplication::default();
    let (send, receive) = mpsc::sync_channel(8);
    component.select = qnc_ingest_select_run::SelectRun::from_session_for_test(selection::SelectSession::from_receiver_for_test(receive));
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
    component.poll(None);
    assert_eq!(component.view.clips[0].save_state, SaveState::Pending);
    assert!(
        component
            .dispatch(IngestIntent::new(action_ids::INGEST_CLIP_TOGGLE, IngestPayload::ClipId("clip-pending".into())))
            .accepted
    );
    assert_eq!(component.view.clips[0].save_state, SaveState::Pending);
    send.send(selection::Event::Saved {
        revisions: vec![("clip-pending".into(), 2)],
        error: Some("write failed".into()),
    })
    .unwrap();
    component.poll(None);
    assert_eq!(component.view.clips[0].save_state, SaveState::Failed);
}

#[test]
fn project_poster_thumbnails_load_after_active_project_plan_is_set() {
    use qnc_ingest_store::content::{Access, ContentClient, ContentTarget};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8,
        0xCF, 0xC0, 0xF0, 0x1F, 0x00, 0x05, 0x00, 0x01, 0xFF, 0x89, 0x99, 0x3D, 0x1D, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60,
    ];

    let root = fixture();
    let (source_fixture, config) = selection::test_support::fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    selection::test_support::execute(&config, &calls, false, false, ".");
    assert_eq!(calls.load(Ordering::SeqCst), 4);

    let reader = SettingsReader::local(root.path().join("data/qnc-projects.db"));
    let target = ContentTarget::for_project(&reader, &reader.read().unwrap()).unwrap();
    let mut source = ContentClient::from_owner_binding(
        &source_fixture.path().join("content.db"),
        "qnc://local/db/ingest_content/p1",
        Access::ReadOnly,
    )
    .unwrap();
    let mut clip = source.list(None).unwrap().remove(0).clip;
    let poster_uri = format!(
        "qnc://local/project/p1/products/thumbnails/{}/poster.png",
        clip.id()
    );
    let poster_path = root
        .path()
        .join("p1")
        .join("products")
        .join("thumbnails")
        .join(clip.id())
        .join("poster.png");
    fs::create_dir_all(poster_path.parent().unwrap()).unwrap();
    fs::write(&poster_path, PNG_1X1).unwrap();
    clip.thumbnail_uri = Some(poster_uri);
    target
        .open(Access::ReadWrite)
        .unwrap()
        .publish(clip)
        .unwrap();

    let mut component = IngestApplication::with_store_root(root.path()).unwrap();
    wait(&mut component);

    assert_eq!(component.view.clips.len(), 1);
    assert_eq!(component.view.clips[0].thumb_status, ThumbStatus::Ready);
    assert!(
        component.view.clips[0].thumb_image.is_some(),
        "project poster URI must be decoded through the active project's folder"
    );
}

#[test]
fn reload_changes_plan_from_db_and_discards_unpersisted_ui_state() {
    let root = fixture();
    let mut component = IngestApplication::with_store_root(root.path()).unwrap();
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
    Connection::open(root.path().join("data/qnc-projects.db"))
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

    let reader = SettingsReader::local(root.path().join("data/qnc-projects.db"));
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

    let mut component = IngestApplication::with_store_root(root.path()).unwrap();
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
    let reader = SettingsReader::local(root.path().join("data/qnc-projects.db"));
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
    let project = Connection::open(root.path().join("p1/project.db")).unwrap();
    let before: String = project
        .query_row("SELECT settings_json FROM project_settings", [], |r| {
            r.get(0)
        })
        .unwrap();
    let mut component = IngestApplication::with_store_root(root.path()).unwrap();
    wait(&mut component);
    assert_eq!(component.view.clips.len(), 2);
    assert_eq!(component.view.browser.chosen.serial_number, "card-serial");
    assert_eq!(component.view.browser.chosen.volume_name, "camera-volume");
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
        1,
        "the checkbox is only a mark in the cache: instant, no database"
    );
    assert!(
        !component.has_pending_work(),
        "marking starts no thread and no database write"
    );
    assert_eq!(
        component.view.preview_clip_id.as_deref(),
        Some(preview_id.as_str()),
        "checkbox does not change preview focus"
    );
    drop(component);
    let mut restarted = IngestApplication::with_store_root(root.path()).unwrap();
    wait(&mut restarted);
    assert_eq!(restarted.view.clips.len(), 2);
    assert_eq!(
        restarted.view.selected_count(),
        0,
        "a mark that was never imported is not in the database"
    );
    assert!(
        restarted.view.clips.iter().any(|c| c.thumb_uri.is_some()),
        "poster URI survives offline reload"
    );
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
    assert_eq!(
        restarted.view.selected_count(),
        1,
        "only the visible clip is marked"
    );
    assert!(
        restarted
            .dispatch(IngestIntent::empty(action_ids::INGEST_CLEAR_SELECTION))
            .accepted
    );
    assert_eq!(restarted.view.selected_count(), 0);
    let selected_in_db = target.open(Access::ReadOnly).unwrap().list(None).unwrap();
    assert_eq!(
        selected_in_db.iter().filter(|c| c.selected).count(),
        0,
        "marking never writes the database"
    );
    // Uvezi is where the marks become active code: the whole selection is written once.
    let launched = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = launched.clone();
    restarted.import = qnc_ingest_import_run::ImportRun::default().with_wake(move |_root, _target| {
        counter.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    restarted.dispatch(IngestIntent::new(
        action_ids::INGEST_CLIP_TOGGLE,
        IngestPayload::ClipId(id.clone()),
    ));
    assert_eq!(restarted.view.selected_count(), 1);
    assert!(
        restarted
            .dispatch(IngestIntent::empty(action_ids::INGEST_IMPORT_SELECTED))
            .accepted
    );
    wait(&mut restarted);
    assert!(
        restarted.take_navigation_request(),
        "the background application is started: the shell may open the next application; message: {}",
        restarted.view.message
    );
    assert!(!restarted.take_navigation_request(), "handed over once");
    assert_eq!(
        launched.load(Ordering::SeqCst),
        1,
        "one background application per Uvezi"
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
    let registry = Connection::open(root.path().join("data/qnc-projects.db")).unwrap();
    registry
        .execute("UPDATE app_settings SET value='p2'", [])
        .unwrap();
    restarted.dispatch(IngestIntent::empty(action_ids::INGEST_RELOAD));
    wait(&mut restarted);
    assert!(restarted.view.clips.is_empty());
    assert_eq!(restarted.view.clip_filter, ClipFilter::All);
    assert!(restarted.view.browser.chosen.serial_number.is_empty());
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

