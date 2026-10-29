//! Delete removes an object, one per press, the nearest first (user 2026-10-21: "Backspace
//! deletes the last action, Delete deletes an object"): the taken M marker, the taken B-roll,
//! the B-roll of a taken or chosen slot. Only what is taken (rule 2026-09-25); a click alone
//! does not arm it. Something not saved (a Sync draft) is not an object: Escape
//! drops it. Ctrl+Delete removes an excluded segment for good.

use qnc_program_db::Operation;

use crate::ProgramSegments;

impl ProgramSegments {
    pub(crate) fn delete_focused(&mut self) {
        if self.sync_drafting() {
            return self.refresh_view(qnc_ui_text::text("story.msg.sync_draft_is_escape").into());
        }
        // A cut between two clips is not deleted (it goes with its clips).
        if self.marker_edit.as_ref().is_some_and(|(id, _)| qnc_program_view::join_segment(id).is_some()) {
            return self.refresh_view(qnc_ui_text::text("story.msg.join_moves_with_edit").into());
        }
        if let Some((marker_id, _)) = self.marker_edit.take() {
            self.selected_marker = None;
            return self.write(Operation::DeleteMarker { marker_id });
        }
        if let Some(cover_id) = self.cover_taken.take() {
            self.selected_cover = None;
            return self.write(Operation::DeleteCover { cover_id });
        }
        // A taken or chosen slot (Ctrl+S, a click on its colour strip): its B-roll; the slot and
        // its M markers stay (user 2026-10-17).
        if let Some(slot_id) = self.slot_move.take().map(|taken| taken.slot_id).or_else(|| self.selected_slot.clone()) {
            let cover = self.stored_covers.iter().find(|cover| cover.slot_id == slot_id).map(|cover| cover.cover_id.clone());
            return match cover {
                Some(cover_id) => {
                    self.selected_cover = None;
                    self.write(Operation::DeleteCover { cover_id })
                }
                None => self.refresh_view(qnc_ui_text::text("story.msg.slot_has_no_broll").into()),
            };
        }
        // Delete takes only what is taken (rule 2026-09-25): a click alone does not arm it.
        self.refresh_view(qnc_ui_text::text("story.msg.take_before_delete").into());
    }

    /// Ctrl+Delete: the chosen segment, when excluded, removed for good.
    pub(crate) fn purge_selected(&mut self) {
        match self.selected.clone() {
            Some(segment_id) if self.stored.iter().any(|row| row.segment_id == segment_id && !row.active) => {
                self.write(Operation::PurgeSegment { segment_id })
            }
            _ => self.refresh_view(qnc_ui_text::text("story.msg.purge_needs_excluded").into()),
        }
    }
}
