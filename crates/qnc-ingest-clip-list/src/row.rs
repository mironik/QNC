//! The clip row every form shows as an item of the Select clip list.

use crate::{selection, ClipListItem};

/// The clip row every form shows (piece `qnc-clip-row`), kept by Select events.
impl ClipListItem for qnc_clip_row::ClipRow {
    fn clip_id(&self) -> &str {
        &self.clip_id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn selected(&self) -> bool {
        self.selected
    }

    fn metadata_revision(&self) -> u32 {
        self.metadata_revision
    }

    fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    fn set_previously_seen(&mut self, seen: bool) {
        self.previously_seen = seen;
    }

    fn set_save_failed(&mut self, failed: bool) {
        self.save_state = if failed { qnc_clip_row::SaveState::Failed } else { qnc_clip_row::SaveState::Saved };
    }

    fn from_selected_clip(clip: selection::SelectedClip) -> Self {
        use qnc_clip_row::{SaveState, ThumbStatus};
        qnc_clip_row::ClipRow {
            clip_id: clip.clip_id,
            name: clip.name,
            duration_seconds: clip.duration_seconds,
            selected: clip.selected,
            imported: clip.imported,
            previously_seen: clip.previously_seen,
            metadata_revision: clip.metadata_revision,
            save_state: match clip.save_state {
                selection::SelectSaveState::Pending => SaveState::Pending,
                selection::SelectSaveState::Failed => SaveState::Failed,
                selection::SelectSaveState::Saved => SaveState::Saved,
            },
            thumb_uri: clip.thumb_uri,
            thumb_status: match clip.thumb_status {
                selection::SelectThumbStatus::Ready => ThumbStatus::Ready,
                selection::SelectThumbStatus::Pending => ThumbStatus::Pending,
                selection::SelectThumbStatus::Missing => ThumbStatus::Missing,
            },
            thumb_image: clip.thumb_image,
            ..Default::default()
        }
    }
}
