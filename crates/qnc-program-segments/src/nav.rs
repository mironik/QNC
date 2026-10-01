//! Keyboard navigation of the program (v5 `story.rs` `select_current_marker_slot`,
//! `focus_empty_marker_slot`, `navigate_adjacent_program_object`,
//! `mark_in_fit_duration` and `segment_program.rs`). It selects; nothing plays.

use qnc_program_db::Operation;

use crate::{ProgramSegments, SegmentCommand};

impl ProgramSegments {
    /// Ctrl+S: the M-M slot under the Wrap playhead (the last slot also takes its end).
    pub(crate) fn select_slot_at_playhead(&mut self) {
        let frame = self.playhead.unwrap_or(0);
        let count = self.view.slots.len();
        let found = self.view.slots.iter().enumerate().find(|(index, slot)| {
            frame >= slot.start_frame && (frame < slot.end_frame || (index + 1 == count && frame == slot.end_frame))
        });
        match found.map(|(_, slot)| slot.slot_id.clone()) {
            Some(slot_id) => self.take_slot(&slot_id),
            None => self.refresh_view("Nema M-M slota pod playheadom".into()),
        }
    }

    /// Shift+S: the first slot without a cover; the Wrap playhead goes to its start.
    pub(crate) fn focus_empty_slot(&mut self) {
        match self.first_empty_slot() {
            Some(slot_id) => {
                self.apply(SegmentCommand::SelectSlot { frame: self.slot_start(&slot_id), slot_id });
            }
            None => self.refresh_view("Nema praznog M-M slota".into()),
        }
    }

    /// Alt+arrows: marker after marker, slot after slot, else segment after segment.
    pub(crate) fn navigate_object(&mut self, up: bool) {
        let command = if self.selected_marker.is_some() {
            SegmentCommand::StepMarker { up }
        } else if self.selected_slot.is_some() {
            SegmentCommand::StepSlot { up }
        } else {
            SegmentCommand::Step { up }
        };
        self.apply(command);
    }

    /// Shift+I on the source (v5 `mark_in_fit_duration`): the selected slot, else the
    /// first empty one, and its length in frames; the slot becomes the selected one.
    pub fn fit_slot(&mut self) -> Option<u64> {
        let slot_id = self.selected_slot.clone().or_else(|| self.first_empty_slot());
        let Some(slot) = slot_id.and_then(|id| self.view.slots.iter().find(|slot| slot.slot_id == id).cloned()) else {
            self.refresh_view("Nema M-M slota za trajanje".into());
            return None;
        };
        self.take_slot(&slot.slot_id);
        Some(slot.end_frame.saturating_sub(slot.start_frame).max(1))
    }

    /// Selects a slot without moving the Wrap playhead (v5 `select_marker_slot`).
    fn take_slot(&mut self, slot_id: &str) {
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

