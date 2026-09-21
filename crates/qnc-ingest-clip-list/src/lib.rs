//! The clip list reducer for Ingest Select events.
//!
//! This component owns the list mutation rules: incoming clips, existing marks,
//! save status, removed clips and Select completion text. It has no form, player,
//! database or source browser dependency.

use qnc_ingest_select as selection;

pub const MODULE_ID: &str = "qnc.module.ingest-clip-list";
pub const VERSION: &str = "0.1.0";

pub trait ClipListItem: Sized {
    fn clip_id(&self) -> &str;
    fn name(&self) -> &str;
    fn selected(&self) -> bool;
    fn metadata_revision(&self) -> u32;
    fn set_selected(&mut self, selected: bool);
    fn set_previously_seen(&mut self, seen: bool);
    fn set_save_failed(&mut self, failed: bool);
    fn from_selected_clip(clip: selection::SelectedClip) -> Self;
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectEventState {
    warning_count: usize,
    last_warning: Option<String>,
}

impl SelectEventState {
    pub fn reset(&mut self) {
        self.warning_count = 0;
        self.last_warning = None;
    }

    pub fn warning_count(&self) -> usize {
        self.warning_count
    }

    fn add_warning(&mut self, message: String) {
        self.warning_count += 1;
        self.last_warning = Some(message);
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectEventOutcome {
    pub message: Option<String>,
    pub warning_count: Option<usize>,
    pub removed_ids: Vec<String>,
    pub preview_removed: bool,
    pub finished_ok: bool,
    pub finished: bool,
}

pub fn apply_select_event<T: ClipListItem>(
    clips: &mut Vec<T>,
    preview_clip_id: Option<&str>,
    state: &mut SelectEventState,
    event: selection::Event,
) -> SelectEventOutcome {
    match event {
        selection::Event::Status(message) => SelectEventOutcome {
            message: Some(message),
            ..Default::default()
        },
        selection::Event::Warning(message) => {
            state.add_warning(message.clone());
            SelectEventOutcome {
                message: Some(message),
                warning_count: Some(state.warning_count()),
                ..Default::default()
            }
        }
        selection::Event::Clip(clip) => {
            apply_clip(clips, clip);
            SelectEventOutcome::default()
        }
        selection::Event::Existing(ids) => {
            for clip in clips {
                if ids.contains(clip.clip_id()) {
                    clip.set_previously_seen(true);
                }
            }
            SelectEventOutcome::default()
        }
        selection::Event::Saved { revisions, error } => {
            let failed = error.is_some();
            for clip in clips {
                if revisions.iter().any(|(id, revision)| {
                    id == clip.clip_id() && *revision == clip.metadata_revision()
                }) {
                    clip.set_save_failed(failed);
                }
            }
            if let Some(error) = error {
                state.add_warning(error.clone());
                SelectEventOutcome {
                    message: Some(error),
                    warning_count: Some(state.warning_count()),
                    ..Default::default()
                }
            } else {
                SelectEventOutcome::default()
            }
        }
        selection::Event::Removed(ids) => {
            clips.retain(|clip| !ids.iter().any(|id| id == clip.clip_id()));
            let preview_removed =
                preview_clip_id.is_some_and(|id| ids.iter().any(|removed| removed == id));
            SelectEventOutcome {
                removed_ids: ids,
                preview_removed,
                ..Default::default()
            }
        }
        selection::Event::Finished(result) => {
            let finished_ok = result.is_ok();
            let message = match result {
                Ok(summary) if state.warning_count() == 0 => format!(
                    "Select: {} postojećih; {} obrađenih; {} uklonjenih ({:.1} s).",
                    summary.unchanged,
                    summary.processed,
                    summary.removed,
                    summary.elapsed_ms as f64 / 1000.0
                ),
                Ok(_) => format!(
                    "Select: {} klipova; {} upozorenja. {}",
                    clips.len(),
                    state.warning_count(),
                    state.last_warning.as_deref().unwrap_or_default()
                ),
                Err(error) => {
                    state.add_warning(error.clone());
                    error
                }
            };
            SelectEventOutcome {
                message: Some(message),
                warning_count: Some(state.warning_count()),
                finished_ok,
                finished: true,
                ..Default::default()
            }
        }
    }
}

fn apply_clip<T: ClipListItem>(clips: &mut Vec<T>, clip: selection::SelectedClip) {
    let mut clip = T::from_selected_clip(clip);
    if let Some(existing) = clips
        .iter_mut()
        .find(|existing| existing.clip_id() == clip.clip_id())
    {
        clip.set_selected(existing.selected());
        *existing = clip;
    } else {
        let index = clips.partition_point(|existing| existing.name() < clip.name());
        clips.insert(index, clip);
    }
}

#[cfg(test)]
mod tests {
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
}
