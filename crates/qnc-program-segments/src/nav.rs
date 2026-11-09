//! Keyboard navigation of the program (v5 `story.rs` `select_current_marker_slot`,
//! `focus_empty_marker_slot`, `navigate_adjacent_program_object`,
//! `mark_in_fit_duration` and `segment_program.rs`). It selects; nothing plays.

use qnc_program_db::Operation;

use crate::{ProgramSegments, SegmentCommand};

impl ProgramSegments {
    /// Ctrl+S: the M-M slot under the Wrap playhead (the last slot also takes its end).
    pub(crate) fn select_slot_at_playhead(&mut self) {
        // The one rule of the slot at a frame (audit 2026-10-25: three copies disagreed).
        let found = qnc_program_view::slot_at(&self.view.slots, self.playhead.unwrap_or(0));
        match found.map(|slot| slot.slot_id.clone()) {
            Some(slot_id) => {
                self.take_slot(&slot_id);
                self.take_slot_move(&slot_id);
            }
            None => self.refresh_view(qnc_ui_text::text("story.msg.no_slot_under_playhead").into()),
        }
    }

    /// Shift+S: the first slot without a cover; the Wrap playhead goes to its start.
    pub(crate) fn focus_empty_slot(&mut self) {
        match self.first_empty_slot() {
            Some(slot_id) => {
                self.apply(SegmentCommand::SelectSlot { frame: self.slot_start(&slot_id), slot_id });
            }
            None => self.refresh_view(qnc_ui_text::text("story.msg.no_empty_slot").into()),
        }
    }

    /// The playhead placed at `frame`: a slot it leaves is let go, so Ctrl+I/O take the slot
    /// under the playhead, and nothing picked stays picked, so Backspace is the undo in time.
    pub(crate) fn place_playhead(&mut self, frame: u64) {
        let inside = self.view.slots.iter().any(|slot| self.selected_slot.as_deref() == Some(slot.slot_id.as_str()) && (slot.start_frame..slot.end_frame).contains(&frame));
        // The stored selection goes too, or a later reread brings it back (user 2026-10-23:
        // after Sync the B-roll button did nothing).
        if !inside && self.selected_slot.take().is_some() {
            self.write(Operation::SelectSlot { slot_id: String::new() });
        }
        self.drop_selected_cover();
        self.selected_marker = None;
        self.cue_at(frame);
    }

    /// Alt+arrows (user 2026-11-09: everywhere they only move the playhead): the next M marker
    /// or start of a segment from the playhead. Nothing is taken; with something taken they do
    /// nothing.
    pub(crate) fn navigate_object(&mut self, up: bool) {
        if self.held().is_some() {
            return;
        }
        let last = self.view.rows.iter().map(|row| row.end_frame).max().unwrap_or(0).saturating_sub(1);
        let stops: Vec<u64> = self.view.rows.iter().map(|row| row.start_frame).chain(self.view.markers.iter().map(|marker| marker.frame.min(last))).collect();
        if let Some(frame) = qnc_edit_step::next_stop(&stops, self.playhead.unwrap_or(0), up) {
            self.place_playhead(frame);
        }
    }

    /// Shift+I on the source (v5 `mark_in_fit_duration`): the selected slot, else the
    /// first empty one, and its length in frames; the slot becomes the selected one.
    pub fn fit_slot(&mut self) -> Option<u64> {
        let slot_id = self.selected_slot.clone().or_else(|| self.first_empty_slot());
        let Some(slot) = slot_id.and_then(|id| self.view.slots.iter().find(|slot| slot.slot_id == id).cloned()) else {
            self.refresh_view(qnc_ui_text::text("story.msg.no_slot_for_length").into());
            return None;
        };
        self.take_slot(&slot.slot_id);
        Some(slot.end_frame.saturating_sub(slot.start_frame).max(1))
    }

    /// Selects a slot without moving the Wrap playhead (v5 `select_marker_slot`).
    pub(crate) fn take_slot(&mut self, slot_id: &str) {
        self.selected_slot = Some(slot_id.to_string());
        self.selected_marker = None;
        self.write(Operation::SelectSlot { slot_id: slot_id.to_string() });
        self.refresh_view(String::new());
    }

    fn first_empty_slot(&self) -> Option<String> {
        self.view.slots.iter().find(|slot| !slot.has_cover).map(|slot| slot.slot_id.clone())
    }

    fn slot_start(&self, slot_id: &str) -> u64 {
        self.view.slots.iter().find(|slot| slot.slot_id == slot_id).map_or(0, |slot| slot.start_frame)
    }
}

