//! A part of the program enlarged in the place of the source timeline (user 2026-10-21: editing
//! a cover or a slot is the same as on the Wrap row, "a version of a zoom of part of the Wrap
//! row"): the same Wrap row of the same program, a narrower window. What is enlarged follows
//! what was double clicked (user 2026-10-24): a slot with a cover is 70 % of the window, the
//! slots before and after 15 % each; elsewhere the segment there (with its glued lead-in or
//! lead-out); the program row gives the whole program. Every edit there is the Wrap row's own
//! action; this only says which window is shown.

use qnc_program_view::SegmentsView;

use crate::ProgramSegments;

/// The share of the window each neighbour of the slot takes.
const NEIGHBOUR_SHARE: f64 = 0.15;

/// What is enlarged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ZoomOn {
    Slot(String),
    Segment(String),
    Program,
}

impl ProgramSegments {
    /// What is at `frame` (`None`: the Wrap playhead) enlarged: the slot `slot` (or the one
    /// there) when it has a cover, else the segment there; the same again closes it.
    pub(crate) fn zoom(&mut self, slot: Option<String>, frame: Option<u64>) {
        let frame = frame.or(self.playhead).unwrap_or(0);
        let slot = slot.or_else(|| {
            let count = self.view.slots.len();
            self.view.slots.iter().enumerate().find_map(|(index, slot)| {
                (frame >= slot.start_frame && (frame < slot.end_frame || (index + 1 == count && frame == slot.end_frame)))
                    .then(|| slot.slot_id.clone())
            })
        });
        let covered = slot.as_ref().filter(|id| self.view.slots.iter().any(|slot| &slot.slot_id == *id && slot.has_cover));
        let wanted = match (covered, self.view.user_segment_at(frame)) {
            (Some(id), _) => Some(ZoomOn::Slot(id.clone())),
            (None, Some(row)) => Some(ZoomOn::Segment(row.segment_id.clone())),
            (None, None) => slot.map(ZoomOn::Slot),
        };
        self.toggle_zoom(wanted);
    }

    /// The whole program enlarged; again closes it.
    pub(crate) fn zoom_program(&mut self) {
        self.toggle_zoom(Some(ZoomOn::Program));
    }

    fn toggle_zoom(&mut self, wanted: Option<ZoomOn>) {
        self.zoom_slot = if self.zoom_slot.is_some() && self.zoom_slot == wanted { None } else { wanted };
        self.refresh_view(String::new());
    }

    /// Whether nothing waits for Enter (Escape then closes the enlarged slot).
    pub(crate) fn held_nothing(&self) -> bool {
        self.held().is_none()
    }

    /// The program window of what is enlarged in `view`; `None` when nothing is (or it is gone
    /// after an edit).
    pub(crate) fn zoom_window(&self, view: &SegmentsView) -> Option<(u64, u64)> {
        match self.zoom_slot.as_ref()? {
            ZoomOn::Slot(id) => {
                let slot = view.slots.iter().find(|slot| &slot.slot_id == id)?;
                Some(window(slot.start_frame, slot.end_frame, view.total_frames))
            }
            ZoomOn::Segment(id) => view.wrap_rows().into_iter().find(|(_, row)| &row.segment_id == id).map(|(window, _)| window),
            ZoomOn::Program => (view.total_frames > 0).then_some((0, view.total_frames)),
        }
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
