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
        self.commit_part_sound()
            || self.commit_marker_edit()
            || self.commit_slot_move()
            || self.commit_slot_trim()
            || self.commit_overlap()
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
    /// What waits for Enter now, as the take it is (`Take::key`): a Sync, a segment edit, a
    /// marker, a slot, a slot edge, a sound, a cover.
    pub(crate) fn held(&self) -> Option<Take> {
        if self.sync.is_active() || self.sync.holds_enter() {
            return Some(Take::Sync);
        }
        if let Some(id) = &self.editing {
            return Some(Take::Segment(id.clone()));
        }
        if let Some((id, _)) = &self.marker_edit {
            return Some(Take::Marker(id.clone()));
        }
        if let Some(slot) = &self.slot_move {
            return Some(Take::Slot(slot.slot_id.clone()));
        }
        if let Some(trim) = &self.slot_trim {
            return Some(Take::SlotEdge(trim.slot_id.clone(), trim.start));
        }
        if let Some(edit) = &self.overlap {
            return Some(Take::Overlap(edit.cover_id.clone(), edit.lead));
        }
        if let Some((part, _)) = self.part_sound.taken() {
            return Some(Take::Sound(part.lane, part.id.clone()));
        }
        self.cover_taken.clone().map(Take::Cover)
    }

    /// Something new is not taken while something else waits for Enter (user 2026-10-21:
    /// a marker taken while a Sync waited for Enter left the Sync hanging for minutes): the
    /// take is refused with a warning the user cannot miss. Taking again what is taken goes.
    pub(crate) fn refused(&mut self, wanted: Take) -> bool {
        if self.held().is_none_or(|held| held == wanted) {
            return false;
        }
        self.refuse()
    }

    /// The take a command makes, asked before it changes any selection (a selection lets a
    /// taken slot or marker go); `None` for a command that takes nothing.
    pub(crate) fn take_of(&self, command: &crate::SegmentCommand) -> Option<Take> {
        use crate::SegmentCommand as Command;
        let playhead = self.playhead.unwrap_or(0);
        match command {
            Command::TakeMarker(id) => Some(Take::Marker(id.clone())),
            Command::TakeSlot(id) => Some(Take::Slot(id.clone())),
            Command::TakeCover { cover_id, .. } => Some(Take::Cover(cover_id.clone())),
            Command::SelectSlotAtPlayhead => {
                let count = self.view.slots.len();
                self.view
                    .slots
                    .iter()
                    .enumerate()
                    .find(|(index, slot)| {
                        playhead >= slot.start_frame && (playhead < slot.end_frame || (index + 1 == count && playhead == slot.end_frame))
                    })
                    .map(|(_, slot)| Take::Slot(slot.slot_id.clone()))
            }
            _ => None,
        }
    }

    /// The refusal: a warning the user cannot miss, and the status line says the same.
    fn refuse(&mut self) -> bool {
        let Some(held) = self.held() else {
            return false;
        };
        let text = qnc_ui_text::fill("story.msg.take_refused", &[("held", qnc_ui_text::text(held.name_key()))]);
        self.alert = Some(qnc_alert::Alert::new(text.clone()));
        self.refresh_view(text);
        true
    }

    /// One edit is taken at a time (audit 2026-10-17: a right click on a marker while a slot
    /// was taken left both taken, and the arrows went on moving the slot); since 2026-10-21 a
    /// new take is refused while another waits (`refused`), so this lets go only of what a
    /// take of the same thing replaces.
    pub(crate) fn let_go_edits(&mut self, keep_sound: bool) {
        self.slot_trim = None;
        self.overlap = None;
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
        let taken = self.slot_trim.take().is_some() | self.slot_move.take().is_some() | self.overlap.take().is_some();
        let taken = taken | self.marker_edit.take().is_some() | self.cover_taken.take().is_some();
        if taken {
            self.refresh_view(qnc_ui_text::text("story.msg.panel_left_edit_dropped").into());
        }
        taken
    }
}

/// A thing the segment panel takes until Enter or Escape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Take {
    Sync,
    Segment(String),
    Marker(String),
    Slot(String),
    /// A slot and whether its start (else its end).
    SlotEdge(String, bool),
    /// A lane (0 = A1 of a segment, 1 = A2 of a cover) and its part.
    Sound(u8, String),
    Cover(String),
    /// A cover and whether its J (else its L).
    Overlap(String, bool),
}

impl Take {
    /// The text code of its name in a warning.
    fn name_key(&self) -> &'static str {
        match self {
            Self::Sync => "story.take.sync",
            Self::Segment(_) => "story.take.segment",
            Self::Marker(_) => "story.take.marker",
            Self::Slot(_) => "story.take.slot",
            Self::SlotEdge(..) => "story.take.slot_edge",
            Self::Sound(..) => "story.take.sound",
            Self::Cover(_) => "story.take.cover",
            Self::Overlap(..) => "story.take.overlap",
        }
    }
}
