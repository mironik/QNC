//! The J and L of a cover's A2 (user 2026-10-21, news practice): on the segment panel Ctrl+J
//! takes the J (its sound starts before its picture) and Ctrl+L the L (its sound goes on
//! after it) of the cover under the Wrap playhead, else of the chosen cover or of the chosen
//! slot's cover. The arrows make it a frame longer or shorter (the left arrow moves the start
//! of the sound earlier for a J, the right arrow its end later for an L), Enter writes it
//! (`SetCoverOverlap`), Escape drops it. A trim playhead shows where the sound starts or ends.
//! The playlist crosses the sounds over it (`qnc-program-playlist`).

use qnc_program_db::{Operation, ProgramCover};

use crate::ProgramSegments;

/// Longest J or L, story frames (the owner of the story holds the same bound).
const MAX_FRAMES: i64 = 500;

/// The taken J (`lead`) or L of a cover: its stored J and L and the draft of the taken one.
#[derive(Debug, Clone)]
pub(crate) struct OverlapEdit {
    pub(crate) cover_id: String,
    pub(crate) lead: bool,
    stored: (u32, u32),
    frames: i64,
    /// The cover's picture on the program, `[start, end)`.
    span: (u64, u64),
}

impl OverlapEdit {
    /// Where the sound starts (J) or ends (L) on the program, for the trim playhead.
    pub(crate) fn frame(&self) -> u64 {
        if self.lead {
            self.span.0.saturating_sub(self.frames as u64)
        } else {
            self.span.1.saturating_add(self.frames as u64)
        }
    }
}

impl ProgramSegments {
    /// Ctrl+J (`lead`) or Ctrl+L on the segment panel. False when there is no cover.
    pub fn take_overlap(&mut self, lead: bool) -> bool {
        let frame = self.playhead.unwrap_or(0);
        let slot = self.selected_slot.clone();
        let found = self
            .stored_covers
            .iter()
            .find(|cover| cover.program_start_frame <= frame && frame < cover.program_end_frame)
            .or_else(|| self.stored_covers.iter().find(|cover| self.selected_cover.as_deref() == Some(cover.cover_id.as_str())))
            .or_else(|| self.stored_covers.iter().find(|cover| slot.as_deref() == Some(cover.slot_id.as_str())))
            .cloned();
        let Some(cover) = found else {
            self.refresh_view(qnc_ui_text::text("story.msg.no_cover_at_playhead").into());
            return false;
        };
        if self.refused(crate::confirm::Take::Overlap(cover.cover_id.clone(), lead)) {
            return false;
        }
        self.let_go_edits(false);
        self.overlap = Some(Self::overlap_of(&cover, lead));
        self.say_overlap();
        true
    }

    fn overlap_of(cover: &ProgramCover, lead: bool) -> OverlapEdit {
        let stored = (cover.a2_lead_frames, cover.a2_tail_frames);
        OverlapEdit {
            cover_id: cover.cover_id.clone(),
            lead,
            stored,
            frames: i64::from(if lead { stored.0 } else { stored.1 }),
            span: (cover.program_start_frame, cover.program_end_frame),
        }
    }

    /// An arrow, Enter or Escape while a J or L is taken; `None` for any other key.
    pub(crate) fn overlap_key(&mut self, action_id: &str) -> Option<bool> {
        let edit = self.overlap.as_mut()?;
        let longer = match action_id {
            "step_back_frame" => edit.lead,
            "step_forward_frame" => !edit.lead,
            "activate_focused_item" => return Some(self.commit_overlap()),
            "clear_focus" => {
                self.overlap = None;
                self.refresh_view(qnc_ui_text::text("story.msg.overlap_cancelled").into());
                return Some(true);
            }
            _ => return None,
        };
        edit.frames = (edit.frames + if longer { 1 } else { -1 }).clamp(0, MAX_FRAMES);
        self.say_overlap();
        Some(true)
    }

    /// Enter: the J or L is written; false when none is taken.
    pub(crate) fn commit_overlap(&mut self) -> bool {
        let Some(edit) = self.overlap.take() else {
            return false;
        };
        let frames = edit.frames as u32;
        let (lead_frames, tail_frames) = if edit.lead { (frames, edit.stored.1) } else { (edit.stored.0, frames) };
        if (lead_frames, tail_frames) != edit.stored {
            self.write(Operation::SetCoverOverlap { cover_id: edit.cover_id, lead_frames, tail_frames });
        }
        self.refresh_view(String::new());
        true
    }

    pub fn overlap_taken(&self) -> bool {
        self.overlap.is_some()
    }

    fn say_overlap(&mut self) {
        let Some(edit) = &self.overlap else {
            return;
        };
        let key = if edit.lead { "story.msg.cover_j_taken" } else { "story.msg.cover_l_taken" };
        let text = qnc_ui_text::fill(key, &[("frames", &edit.frames.to_string())]);
        self.refresh_view(text);
    }
}
