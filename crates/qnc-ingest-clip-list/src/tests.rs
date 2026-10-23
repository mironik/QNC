use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    id: String,
    name: String,
    selected: bool,
    seen: bool,
    revision: u32,
    failed: bool,
}

impl ClipListItem for Row {
    fn clip_id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn selected(&self) -> bool {
        self.selected
    }

    fn metadata_revision(&self) -> u32 {
        self.revision
    }

    fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    fn set_previously_seen(&mut self, seen: bool) {
        self.seen = seen;
    }

    fn set_save_failed(&mut self, failed: bool) {
        self.failed = failed;
    }

    fn from_selected_clip(clip: selection::SelectedClip) -> Self {
        Self {
            id: clip.clip_id,
            name: clip.name,
            selected: clip.selected,
            seen: clip.previously_seen,
            revision: clip.metadata_revision,
            failed: false,
        }
    }
}

fn selected(id: &str, name: &str, revision: u32) -> selection::SelectedClip {
    selection::SelectedClip {
        clip_id: id.into(),
        name: name.into(),
        duration_seconds: 0.0,
        selected: false,
        imported: false,
        previously_seen: false,
        metadata_revision: revision,
        save_state: selection::SelectSaveState::Saved,
        thumb_uri: None,
        thumb_status: selection::SelectThumbStatus::Missing,
        thumb_image: None,
    }
}

#[test]
fn incoming_clips_are_sorted_and_keep_local_selection() {
    let mut clips = vec![Row {
        id: "b".into(),
        name: "B".into(),
        selected: true,
        seen: false,
        revision: 1,
        failed: false,
    }];
    let mut state = SelectEventState::default();

    apply_select_event(
        &mut clips,
        None,
        &mut state,
        selection::Event::Clip(selected("a", "A", 1)),
    );
    apply_select_event(
        &mut clips,
        None,
        &mut state,
        selection::Event::Clip(selected("b", "B2", 2)),
    );

    assert_eq!(clips[0].id, "a");
    assert_eq!(clips[1].name, "B2");
    assert!(clips[1].selected);
}

#[test]
fn removed_active_preview_is_reported() {
    let mut clips = vec![Row {
        id: "a".into(),
        name: "A".into(),
        selected: false,
        seen: false,
        revision: 1,
        failed: false,
    }];
    let mut state = SelectEventState::default();

    let outcome = apply_select_event(
        &mut clips,
        Some("a"),
        &mut state,
        selection::Event::Removed(vec!["a".into()]),
    );

    assert!(clips.is_empty());
    assert!(outcome.preview_removed);
    assert_eq!(outcome.removed_ids, ["a"]);
}
