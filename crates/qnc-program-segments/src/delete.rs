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
            return self.refresh_view(qnc_ui_text::text("story.msg.join_not_deleted").into());
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
        let taken = self.slot_move.take().map(|taken| taken.slot_id).or_else(|| self.slot_trim.as_ref().map(|trim| trim.slot_id.clone()));
        if let Some(slot_id) = taken.or_else(|| self.selected_slot.clone()) {
            let cover = self.stored_covers.iter().find(|cover| cover.slot_id == slot_id).map(|cover| cover.cover_id.clone());
            // In an S-Story every clip is a slot (user 2026-11-07: "a chosen slot must be
            // deleted and brought back with Undo or Backspace"): its clip goes from the story,
            // one undo step.
            let clip = self.is_s_story().then(|| self.clip_of_slot(&slot_id)).flatten();
            return match (cover, clip) {
                (Some(cover_id), _) => {
                    self.selected_cover = None;
                    self.slot_trim = None;
                    self.write(Operation::DeleteCover { cover_id })
                }
                (None, Some(segment_id)) => {
                    self.slot_trim = None;
                    self.selected_slot = None;
                    self.end_edit();
                    self.write(Operation::DeleteSegment { segment_id })
                }
                (None, None) => self.refresh_view(qnc_ui_text::text("story.msg.slot_has_no_broll").into()),
            };
        }
        // Delete takes only what is taken (rule 2026-09-25): a click alone does not arm it.
        self.refresh_view(qnc_ui_text::text("story.msg.take_before_delete").into());
    }

    /// The clip (segment) that is the slot `slot_id` of an S-Story: the one inside its span.
    fn clip_of_slot(&self, slot_id: &str) -> Option<String> {
        let slot = self.view.slots.iter().find(|slot| slot.slot_id == slot_id)?;
        self.view
            .rows
            .iter()
            .filter(|row| !row.lead)
            .find(|row| row.start_frame < slot.end_frame && slot.start_frame < row.end_frame)
            .map(|row| row.segment_id.clone())
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
