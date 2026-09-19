use super::*;

fn clip(id: &str, previously_seen: bool, selected: bool) -> ClipView {
    ClipView {
        clip_id: id.into(),
        name: id.into(),
        previously_seen,
        selected,
        ..Default::default()
    }
}

fn selected_clip(id: &str, previously_seen: bool, selected: bool) -> selection::SelectedClip {
    selection::SelectedClip {
        clip_id: id.into(),
        name: id.into(),
        previously_seen,
        selected,
        ..Default::default()
    }
}

#[test]
fn filter_projects_existing_catalog_without_changing_selection_or_preview() {
    for source_kind in [SourceKind::Local, SourceKind::Lan, SourceKind::Internet] {
        let mut component = IngestApplication::default();
        component.view.source_kind = source_kind;
        component.view.clips = vec![clip("old", true, true), clip("new", false, false)];
        component.view.preview_clip_id = Some("old".into());
        let before = component.view.clone();
        assert_eq!(component.view.clip_filter, ClipFilter::All);
        assert_eq!(component.view.visible_clips().count(), 2);
        for filter in [ClipFilter::New, ClipFilter::All, ClipFilter::New] {
            let result = component.dispatch(IngestIntent::new(
                action_ids::INGEST_SET_CLIP_FILTER,
                IngestPayload::ClipFilter(filter),
            ));
            assert!(result.accepted && result.request_repaint);
            assert_eq!(
                component.view.visible_clips().count(),
                if filter == ClipFilter::New { 1 } else { 2 }
            );
            assert_eq!(component.view.clips, before.clips);
            assert_eq!(component.view.preview_clip_id, before.preview_clip_id);
            assert_eq!(component.view.selected_count(), 1);
            assert_eq!(component.view.message, before.message);
            assert!(!component.has_pending_work());
            assert!(!component.catalog_loader.is_busy());
            assert!(!component.selection_writer.is_busy());
            assert!(!component.selection_session.has_pending_work());
            assert!(!component.browse.is_busy());
        }
    }
}

#[test]
fn new_filter_accepts_incoming_clips_during_select_without_losing_existing_ones() {
    let mut component = IngestApplication::default();
    component.view.command_busy = true;
    component.view.clips = vec![clip("old", true, true)];
    assert!(
        component
            .dispatch(IngestIntent::new(
                action_ids::INGEST_SET_CLIP_FILTER,
                IngestPayload::ClipFilter(ClipFilter::New),
            ))
            .accepted
    );
    assert_eq!(component.view.visible_clips().count(), 0);
    let (send, receive) = mpsc::channel();
    component.selection_session = selection::SelectSession::from_receiver_for_test(receive);
    send.send(selection::Event::Clip(selected_clip("new", false, false)))
        .unwrap();
    component.poll();
    assert_eq!(
        component
            .view
            .visible_clips()
            .map(|c| c.clip_id.as_str())
            .collect::<Vec<_>>(),
        ["new"]
    );
    assert_eq!(component.view.clips.len(), 2);
    send.send(selection::Event::Existing(
        ["new".into()].into_iter().collect(),
    ))
    .unwrap();
    component.poll();
    assert_eq!(component.view.visible_clips().count(), 0);
    assert_eq!(component.view.clips.len(), 2);
}

#[test]
fn malformed_filter_intent_does_not_change_view() {
    let mut component = IngestApplication::default();
    let before = component.view.clone();
    assert!(
        !component
            .dispatch(IngestIntent::empty(action_ids::INGEST_SET_CLIP_FILTER))
            .accepted
    );
    assert_eq!(component.view, before);
}

#[test]
fn filter_action_is_declared_in_external_catalog_without_hardcoded_key() {
    let catalog: serde_json::Value = serde_json::from_str(include_str!(
        "../../../contracts/qnc-keyboard-shortcuts.json"
    ))
    .unwrap();
    assert!(catalog["actions"]
        .get(action_ids::INGEST_SET_CLIP_FILTER)
        .is_some());
}
