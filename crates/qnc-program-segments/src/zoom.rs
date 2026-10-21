//! A slot enlarged in the place of the source timeline (user 2026-10-21: editing a cover or a
//! slot is the same as on the Wrap row, "a version of a zoom of part of the Wrap row"): the
//! same Wrap row of the same program, a narrower window. The slot is 70 % of the window, the
//! slots before and after 15 % each. Every edit there is the Wrap row's own action; this only
//! says which window is shown.

use qnc_program_view::SegmentsView;

use crate::ProgramSegments;

/// The share of the window each neighbour of the slot takes.
const NEIGHBOUR_SHARE: f64 = 0.15;

impl ProgramSegments {
    /// The slot (`None`: under the Wrap playhead) enlarged; the same slot again closes it.
    pub(crate) fn zoom(&mut self, slot_id: Option<String>) {
        let playhead = self.playhead.unwrap_or(0);
        let wanted = slot_id.or_else(|| {
            let count = self.view.slots.len();
            self.view.slots.iter().enumerate().find_map(|(index, slot)| {
                (playhead >= slot.start_frame && (playhead < slot.end_frame || (index + 1 == count && playhead == slot.end_frame)))
                    .then(|| slot.slot_id.clone())
            })
        });
        self.zoom_slot = if self.zoom_slot.is_some() && self.zoom_slot == wanted { None } else { wanted };
        self.refresh_view(String::new());
    }

    /// Whether nothing waits for Enter (Escape then closes the enlarged slot).
    pub(crate) fn held_nothing(&self) -> bool {
        self.held().is_none()
    }

    /// The program window of the enlarged slot in `view`; `None` when none is (or the slot is
    /// gone after an edit).
    pub(crate) fn zoom_window(&self, view: &SegmentsView) -> Option<(u64, u64)> {
        let id = self.zoom_slot.as_ref()?;
        let slot = view.slots.iter().find(|slot| &slot.slot_id == id)?;
        Some(window(slot.start_frame, slot.end_frame, view.total_frames))
    }
}

/// The window of a slot `[start, end)`: it takes 70 % of the width, 15 % on each side.
pub(crate) fn window(start: u64, end: u64, total: u64) -> (u64, u64) {
    let length = end.saturating_sub(start).max(1) as f64;
    let side = (length * NEIGHBOUR_SHARE / (1.0 - 2.0 * NEIGHBOUR_SHARE)).round() as u64;
    (start.saturating_sub(side), end.saturating_add(side).min(total.max(end)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Locked (user 2026-10-21): the slot is 70 % of the enlarged window, each neighbour 15 %.
    #[test]
    fn the_slot_takes_seventy_percent_and_each_neighbour_fifteen() {
        assert_eq!(window(700, 1400, 5000), (550, 1550), "700 frames: 150 on each side");
        assert_eq!(window(0, 700, 5000), (0, 850), "the program starts at the slot");
        assert_eq!(window(4300, 5000, 5000), (4150, 5000), "the program ends at the slot");
    }
}
