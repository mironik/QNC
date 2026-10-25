//! Select events applied to a clip list: clips found, saved, removed, warnings, the end.

use crate::{selection, ClipListItem};

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
                Ok(summary) if state.warning_count() == 0 => qnc_ui_text::fill(
                    "ingest.msg.select_done",
                    &[
                        ("unchanged", &summary.unchanged.to_string()),
                        ("processed", &summary.processed.to_string()),
                        ("removed", &summary.removed.to_string()),
                        ("seconds", &format!("{:.1}", summary.elapsed_ms as f64 / 1000.0)),
                    ],
                ),
                Ok(_) => qnc_ui_text::fill(
                    "ingest.msg.select_done_warnings",
                    &[
                        ("clips", &clips.len().to_string()),
                        ("warnings", &state.warning_count().to_string()),
                        ("last", state.last_warning.as_deref().unwrap_or_default()),
                    ],
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

