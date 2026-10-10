//! Moving a slot (user 2026-10-10, not in v5): Ctrl+S takes the slot under the Wrap
//! playhead; the arrows move it a frame, with both its M markers, so it keeps its
//! length and its cover and a neighbouring slot grows or shrinks; it stops before a
//! neighbouring marker and never moves the locked start or end. The Wrap playhead
//! shows the slot's first frame. Enter writes it (`MoveSlot`), Escape drops it.

use qnc_program_db::Operation;

use crate::ProgramSegments;

/// The taken slot: its id, its stored marker frames and the draft shift.
#[derive(Debug, Clone)]
pub(crate) struct SlotMove {
    pub(crate) slot_id: String,
    pub(crate) start: u64,
    pub(crate) end: u64,
    pub(crate) shift: i64,
}

impl SlotMove {
    /// Where a marker frame of the slot is drawn while it moves.
    pub(crate) fn moved(&self, frame: u64) -> u64 {
        if frame == self.start || frame == self.end {
            frame.saturating_add_signed(self.shift)
        } else {
            frame
        }
    }
}

impl ProgramSegments {
    /// The markers a draft moves, (stored frame, draft frame): both of a taken slot, or
    /// the marker taken with Ctrl+M. The edit pictures are shown beside them.
    pub(crate) fn edit_moves(&self) -> Vec<(u64, u64)> {
        if let Some(slot) = &self.slot_move {
            return [slot.start, slot.end].iter().map(|frame| (*frame, slot.moved(*frame))).collect();
        }
        let marker = self.marker_edit.as_ref().and_then(|(id, draft)| {
            let stored = self.stored_markers.iter().find(|marker| &marker.marker_id == id)?;
            Some((stored.program_frame, *draft))
        });
        marker.into_iter().collect()
    }

    /// Ctrl+S took this slot: the arrows move it from now on.
    pub(crate) fn take_slot_move(&mut self, slot_id: &str) {
        self.slot_move = self
            .view
            .slots
            .iter()
            .find(|slot| slot.slot_id == slot_id)
            .map(|slot| SlotMove { slot_id: slot_id.to_string(), start: slot.start_frame, end: slot.end_frame, shift: 0 });
        self.refresh_view("Slot: strelice lijevo/desno pomak, Enter potvrda, Esc odustani".into());
    }

    /// An arrow, Enter or Escape while a slot is taken; `None` for any other key.
    pub(crate) fn slot_key(&mut self, action_id: &str) -> Option<bool> {
        self.slot_move.as_ref()?;
        match action_id {
            "step_back_frame" => Some(self.nudge_slot(-1)),
            "step_forward_frame" => Some(self.nudge_slot(1)),
            "clear_focus" | "close_player" => {
                self.slot_move = None;
                self.refresh_view("Pomak slota odustan.".into());
                Some(true)
            }
            _ => None,
        }
    }

    /// The slot one frame back or forward, up to its neighbouring markers.
    fn nudge_slot(&mut self, frames: i64) -> bool {
        let Some(slot) = self.slot_move.clone() else {
            return false;
        };
        let shift = slot.shift + frames;
        let pins = crate::resolve(&self.view, &self.stored_markers, None);
        let index = |frame| pins.iter().position(|pin| pin.frame == frame);
        let (Some(first), Some(last)) = (index(slot.start), index(slot.end)) else {
            return false;
        };
        if pins[first].locked || pins[last].locked {
            self.refresh_view("Slot uz pocetni ili zavrsni M marker ne pomice se.".into());
            return true;
        }
        let low = first.checked_sub(1).map_or(0, |before| pins[before].frame);
        let high = pins.get(last + 1).map_or(u64::MAX, |after| after.frame);
        let (start, end) = (slot.start.checked_add_signed(shift), slot.end.checked_add_signed(shift));
        let (Some(start), Some(end)) = (start, end) else {
            return true;
        };
        if start <= low || end >= high {
            self.refresh_view("Slot se pomice samo do susjednog M markera.".into());
            return true;
        }
        self.slot_move = Some(SlotMove { shift, ..slot });
        self.refresh_view("Slot: strelice lijevo/desno pomak, Enter potvrda, Esc odustani".into());
        self.cue_program(start);
        true
    }

    /// Enter: the moved slot is written; false when no slot is taken.
    pub(crate) fn commit_slot_move(&mut self) -> bool {
        let Some(slot) = self.slot_move.take() else {
            return false;
        };
        if slot.shift != 0 {
            self.marker_committing = true;
            self.write(Operation::MoveSlot {
                slot_id: slot.slot_id,
                start_frame: slot.start.saturating_add_signed(slot.shift),
            });
        } else {
            self.refresh_view(String::new());
        }
        true
    }
}
