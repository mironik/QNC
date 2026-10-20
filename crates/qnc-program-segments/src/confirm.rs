//! Enter by panel (user rule 2026-10-01: the keys act only on the panel in focus;
//! Enter is the confirmation of that panel). On the source timeline it confirms a taken
//! A1/A2 lane, the edit of a segment or Sync/B-roll; on the segment panel the marker
//! taken with Ctrl+M. One Enter does one thing: the other catalog actions of the same
//! Enter (a virtual shot) are not done after a confirmation.

use crate::ProgramSegments;


impl ProgramSegments {
    /// Enter on the source timeline: the segment edit or Sync (a taken lane is the source
    /// timeline's own, piece `qnc-source-timeline`).
    pub fn confirm_source(&mut self) -> bool {
        // A Sync is what was just done: its Enter comes first (live 2026-10-20: an edit left
        // open took the Enter and the stopped Sync could never be saved).
        if self.sync.is_active() || self.sync.holds_enter() {
            if !self.finish_sync_with_cover() {
                self.commit_sync(true);
            }
            true
        } else if self.editing.is_some() {
            self.commit_edit()
        } else {
            false
        }
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
        self.source.holds_enter
            || self.editing.is_some()
            || self.sync.is_active()
            || self.sync.holds_enter()
    }
}

impl ProgramSegments {
    /// The keyboard left the segment panel (live 2026-10-16: a slot edge left taken while a
    /// clip was opened in the pool; Escape on the source timeline could no longer drop it and
    /// the story seemed blocked). What the segment panel took (a slot, a slot edge, a marker,
    /// a cover) belongs to it and is dropped as by Escape; nothing is written. The edit of a
    /// segment (Ctrl+Shift+I/O, Edit) lives on the source timeline and stays.
    /// One edit is taken at a time (audit 2026-10-17: a right click on a marker while a slot
    /// was taken left both taken, and the arrows went on moving the slot). Taking a slot, a
    /// slot edge, a marker or a sound lets go of what was taken before, unsaved, as Escape.
    pub(crate) fn let_go_edits(&mut self, keep_sound: bool) {
        self.slot_trim = None;
        self.slot_move = None;
        self.marker_edit = None;
        self.cover_taken = None;
        if !keep_sound && self.part_sound.release() {
            self.sound_dropped = true;
        }
    }

    /// The sound draft of a segment or cover was let go by another take: the program hears its
    /// stored sound again (the caller tells the preview). Asked once.
    pub fn take_dropped_sound(&mut self) -> bool {
        std::mem::take(&mut self.sound_dropped)
    }

    pub fn leave_panel(&mut self) -> bool {
        let taken = self.slot_trim.take().is_some() | self.slot_move.take().is_some();
        let taken = taken | self.marker_edit.take().is_some() | self.cover_taken.take().is_some();
        if taken {
            self.refresh_view(qnc_ui_text::text("story.msg.panel_left_edit_dropped").into());
        }
        taken
    }
}
