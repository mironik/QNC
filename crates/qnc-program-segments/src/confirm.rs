//! Enter by panel (user rule 2026-10-01: the keys act only on the panel in focus;
//! Enter is the confirmation of that panel). On the source timeline it confirms a taken
//! A1/A2 lane, the edit of a segment or Sync/B-roll; on the segment panel the marker
//! taken with Ctrl+M. One Enter does one thing: the other catalog actions of the same
//! Enter (a virtual shot) are not done after a confirmation.

use crate::ProgramSegments;

/// The other actions of one Enter arrive at once; within this time they are its.
const SAME_ENTER: std::time::Duration = std::time::Duration::from_millis(150);

impl ProgramSegments {
    /// Enter on the source timeline: the lane, the segment edit or Sync, in that order.
    pub fn confirm_source(&mut self) -> bool {
        let confirmed = if self.lanes.taken().is_some() {
            self.commit_lane()
        } else if self.editing.is_some() {
            self.commit_edit()
        } else if self.sync.is_active() || self.sync.holds_enter() {
            if !self.finish_sync_with_cover() {
                self.commit_sync(true);
            }
            true
        } else {
            false
        };
        if confirmed {
            self.lanes.mark_committed();
        }
        confirmed
    }

    /// Enter on the segment panel: the marker taken with Ctrl+M, the slot taken with
    /// Ctrl+S or the slot edge taken with Ctrl+I/O.
    pub fn confirm_program(&mut self) -> bool {
        self.commit_part_sound() || self.commit_marker_edit() || self.commit_slot_move() || self.commit_slot_trim()
    }

    /// Ctrl+I (`start`) or Ctrl+O on the segment panel: that edge of the slot under the
    /// Wrap playhead (user 2026-10-13).
    pub fn take_slot_edge_at_playhead(&mut self, start: bool) -> bool {
        self.take_slot_edge(start)
    }

    /// Something on the source timeline waits for Enter, or this Enter confirmed it:
    /// the same Enter saves no virtual shot.
    pub fn source_waits_enter(&self) -> bool {
        self.lanes.taken().is_some()
            || self.editing.is_some()
            || self.sync.is_active()
            || self.sync.holds_enter()
            || self.lanes.committed_within(SAME_ENTER)
    }
}
