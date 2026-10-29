//! Undo selected (user 2026-10-29: "two kinds of undo: undo in time, back along the edits, and
//! undo selected: what is selected is undone or redone, whenever it was"). The selected object
//! is the taken cut (an M marker, or the clip after a join), the taken or selected cover, the
//! selected marker, the cover of the selected slot, else the selected segment; its owner
//! (`qnc-program-db` UndoSelected / RedoSelected) undoes its newest change alone.

use qnc_program_db::Operation;

use crate::ProgramSegments;

impl ProgramSegments {
    /// The selected object as (type, id) for undo selected; `None` when nothing is selected.
    pub(crate) fn selected_object(&self) -> Option<(&'static str, String)> {
        if let Some((id, _)) = &self.marker_edit {
            return Some(match qnc_program_view::join_segment(id) {
                Some(segment_id) => ("segment", segment_id.to_string()),
                None => ("marker", id.clone()),
            });
        }
        if let Some(cover) = self.cover_taken.clone().or_else(|| self.selected_cover.clone()) {
            return Some(("cover", cover));
        }
        if let Some(marker) = self.selected_marker.clone().filter(|id| qnc_program_view::join_segment(id).is_none()) {
            return Some(("marker", marker));
        }
        if let Some(slot) = &self.selected_slot {
            if let Some(cover) = self.stored_covers.iter().find(|cover| &cover.slot_id == slot) {
                return Some(("cover", cover.cover_id.clone()));
            }
        }
        self.selected.clone().map(|segment| ("segment", segment))
    }

    /// Ctrl+Backspace (`redo` with Shift): the selected object's newest change undone, or what
    /// that took brought back. Nothing selected says so.
    pub(crate) fn undo_selected(&mut self, redo: bool) {
        let Some((object_type, object_id)) = self.selected_object() else {
            return self.refresh_view(qnc_ui_text::text("story.msg.nothing_selected_to_undo").into());
        };
        // What is taken goes: its frames belong to the story before the undo.
        self.let_go_edits(false);
        let object_type = object_type.to_string();
        self.write(if redo { Operation::RedoSelected { object_type, object_id } } else { Operation::UndoSelected { object_type, object_id } });
    }
}
