use super::*;

fn clip(id: &str, name: &str) -> EditorialClip {
    EditorialClip {
        clip_id: id.into(),
        name: name.into(),
        duration_seconds: 10.0,
        duration_frames: 250,
        imported: true,
        thumb_uri: None,
        thumb_image: None,
        import_status: String::new(),
        imported_media_uri: String::new(),
        ..Default::default()
    }
}

#[test]
fn save_without_a_confirmed_range_does_not_write() {
    let mut app = EditorialApplication::default();
    app.view.clips = vec![clip("clip-a", "Mironik")];
    app.view.preview.clip_id = Some("clip-a".into());
    app.dispatch(EditorialIntent::action(action_ids::SAVE_VIRTUAL_SHOT));
    assert!(app.view().message == qnc_ui_text::text("story.msg.marks_not_confirmed"));
}

#[test]
fn virtual_tab_is_a_separate_list_and_a_short_duration_is_seconds_and_frames() {
    let mut app = EditorialApplication::default();
    app.view.clips = vec![clip("clip-a", "Mironik")];
    app.dispatch(EditorialIntent::action("editorial_tab_virtual"));
    assert_eq!(app.view().library_tab, LibraryTab::Virtual);
    assert!(app.view().shorts.is_empty());
    let label = qnc_virtual_short_cards::cards(
        vec![qnc_virtual_shots::ShortClip {
            shot_id: "shot-a".into(),
            clip_id: "clip-a".into(),
            in_frame: 0,
            out_frame: 57,
            name: "Mironik 001".into(),
            in_still_uri: None,
            out_still_uri: None,
            still_status: "pending".into(),
            b_roll: false,
        }],
        &[clip("clip-a", "Mironik")],
        &[],
    )[0]
    .duration_label
    .clone();
    assert_eq!(label, "2:07");
}

#[test]
fn starts_passive_and_empty() {
    let app = EditorialApplication::default();
    assert!(app.view().clips.is_empty());
    assert!(app.view().chosen_clip_id().is_none());
    assert!(!app.has_player());
    assert!(app.next_repaint_delay().is_none());
}

#[test]
fn missing_settings_end_in_a_controlled_error_not_a_default_project() {
    let root =
        std::env::temp_dir().join(format!("qnc_editorial_no_project_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&root);
    let app = EditorialApplication::new(&root, FocusLayout::default());
    assert!(app.view().clips.is_empty());
    assert!(app.shown_project.is_none());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn unknown_clip_is_rejected_without_creating_a_player() {
    let mut app = EditorialApplication::default();
    app.view.clips = vec![clip("a", "Prvi")];
    assert!(app.dispatch(EditorialIntent::PreviewClip("missing".into())));
    assert_eq!(app.view().message, qnc_ui_text::text("ingest.msg.clip_not_found"));
    assert!(!app.has_player());
    assert!(app.view().chosen_clip_id().is_none());
}

#[test]
fn choosing_a_clip_without_a_project_reports_it_in_the_footer() {
    let mut app = EditorialApplication::default();
    app.view.clips = vec![clip("a", "Prvi")];
    assert!(app.dispatch(EditorialIntent::PreviewClip("a".into())));
    assert_eq!(app.view().chosen_clip_id(), Some("a"));
    assert_eq!(app.footer_status().text, qnc_ui_text::text("player.work_settings_project_loaded"));
    assert!(!app.has_player());
}

#[test]
fn transport_without_a_player_says_so() {
    let mut app = EditorialApplication::default();
    for action in [
        action_ids::PLAY_PAUSE,
        action_ids::STEP_BACK_FRAME,
        action_ids::STEP_FORWARD_FRAME,
    ] {
        app.dispatch(EditorialIntent::action(action));
        assert_eq!(app.footer_status().text, qnc_ui_text::text("player.broadcast_player_connected"));
    }
    app.dispatch(EditorialIntent::action(qnc_action_event::ActionEvent::key("timeline_cue_frame").with_value(10u64).at("source_timeline")));
    assert_eq!(app.footer_status().text, qnc_ui_text::text("player.broadcast_player_connected"));
}

#[test]
fn current_clip_label_follows_the_chosen_clip() {
    let mut app = EditorialApplication::default();
    app.view.clips = vec![clip("a", "Prvi"), clip("b", "Drugi")];
    assert_eq!(app.view().current_clip_label(), None);
    app.view.preview.clip_id = Some("b".into());
    assert_eq!(app.view().current_clip_label(), Some("Drugi"));
}
