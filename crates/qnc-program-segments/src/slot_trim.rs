//! A slot longer or shorter at one edge (user 2026-10-13, not in v5): on the segment
//! panel Ctrl+I takes the start and Ctrl+O the end of the clicked slot, else of the slot
//! under the Wrap playhead; the Wrap playhead goes to the slot's first or last picture,
//! so the monitor shows the cut at once, and to the new edge after Enter (the player
//! plays the stored story: while the arrows move the edge, the zoom shows the draft).
//! The other slots keep their lengths: the markers after the slot move with its end and
//! the last slot of the story takes the difference. The arrows move the picture at the
//! taken edge a frame (at the start the slot's cover starts earlier or later in its
//! source, the slot keeps its place), Up/Down to a key picture; Enter writes it
//! (`TrimSlot`), Escape drops it.

use qnc_program_db::Operation;

use crate::ProgramSegments;

/// The slot whose edge is taken: its stored frames and the draft change of length.
#[derive(Debug, Clone)]
pub(crate) struct SlotTrim {
    pub(crate) slot_id: String,
    pub(crate) start: bool,
    pub(crate) slot_start: u64,
    pub(crate) slot_end: u64,
    /// Story frames the slot is longer (negative: shorter).
    pub(crate) frames: i64,
    /// The first slot after it without a cover, (start, end): it takes the difference and
    /// the slots after it stay (user 2026-10-13); `None`: the last slot takes it.
    pub(crate) empty: Option<(u64, u64)>,
}

impl SlotTrim {
    /// Where a marker frame of the program is drawn: from the slot's end on (the locked
    /// end of the story excepted) it moves with the trim.
    pub(crate) fn moved(&self, frame: u64, total: u64) -> u64 {
        let reach = self.empty.map_or(total.saturating_sub(1), |(start, _)| start);
        if frame >= self.slot_end && frame <= reach && frame < total {
            frame.saturating_add_signed(self.frames)
        } else {
            frame
        }
    }
}

impl ProgramSegments {
    /// The picture the stored story shows at a program frame (a cover, else the
    /// segment; `None` on Off): what the player shows there, while the zoom draws drafts.
    pub fn stored_picture_at(&self, frame: u64) -> Option<qnc_program_view::StillPicture> {
        let stored = crate::SegmentsView {
            rows: self.view.rows.clone(),
            covers: self.view.covers.clone(),
            slots: crate::slots(&self.stored_slots, None),
            timebase: self.view.timebase,
            total_frames: self.view.total_frames,
            ..crate::SegmentsView::default()
        };
        qnc_program_view::program_picture(&stored, frame)
    }

    /// Ctrl+I (`start`) or Ctrl+O on the segment panel: that edge of the clicked slot,
    /// else of the slot under the Wrap playhead (the last slot also takes its end).
    /// False when there is none.
    pub(crate) fn take_slot_edge(&mut self, start: bool) -> bool {
        let frame = self.playhead.unwrap_or(0);
        let count = self.view.slots.len();
        let clicked = self.view.slots.iter().find(|slot| self.selected_slot.as_ref() == Some(&slot.slot_id));
        let found = clicked.or_else(|| {
            self.view.slots.iter().enumerate().find_map(|(index, slot)| {
                (frame >= slot.start_frame && (frame < slot.end_frame || (index + 1 == count && frame == slot.end_frame)))
                    .then_some(slot)
            })
        });
        let Some(slot) = found.cloned() else {
            self.refresh_view(qnc_ui_text::text("story.msg.no_slot_under_playhead").into());
            return false;
        };
        if slot.end_frame >= self.view.total_frames {
            self.refresh_view(qnc_ui_text::text("story.msg.last_slot_ends_story").into());
            return false;
        }
        self.marker_edit = None;
        self.slot_move = None;
        self.selected_slot = Some(slot.slot_id.clone());
        self.selected_marker = None;
        self.write(Operation::SelectSlot { slot_id: slot.slot_id.clone() });
        let empty = self
            .view
            .slots
            .iter()
            .filter(|later| later.start_frame >= slot.end_frame && !later.has_cover)
            .min_by_key(|later| later.start_frame)
            .map(|later| (later.start_frame, later.end_frame));
        self.slot_trim = Some(SlotTrim {
            slot_id: slot.slot_id,
            start,
            slot_start: slot.start_frame,
            slot_end: slot.end_frame,
            frames: 0,
            empty,
        });
        // The monitor shows the cut at once: the slot's first or last picture.
        self.cue_program(if start { slot.start_frame } else { slot.end_frame.saturating_sub(1) });
        self.refresh_view(Self::trim_message(start));
        true
    }

    fn trim_message(start: bool) -> String {
        qnc_ui_text::text(if start { "story.msg.slot_start_taken" } else { "story.msg.slot_end_taken" }).to_string()
    }

    /// An arrow, Enter or Escape while a slot edge is taken; `None` for any other key.
    pub(crate) fn trim_key(&mut self, action_id: &str) -> Option<bool> {
        self.slot_trim.as_ref()?;
        match action_id {
            "step_back_frame" => Some(self.nudge_trim(-1)),
            "step_forward_frame" => Some(self.nudge_trim(1)),
            "activate_focused_item" => Some(self.commit_slot_trim()),
            "clear_focus" | "close_player" => {
                self.slot_trim = None;
                self.refresh_view(qnc_ui_text::text("story.msg.slot_trim_cancelled").into());
                Some(true)
            }
            _ => None,
        }
    }

    /// The picture at the taken edge `picture` frames later (negative: earlier): at the
    /// start the slot gets shorter by as much, at the end longer.
    pub(crate) fn nudge_trim(&mut self, picture: i64) -> bool {
        let Some(trim) = self.slot_trim.clone() else {
            return false;
        };
        if self.at_clip_end(picture) {
            return true;
        }
        let frames = if trim.start { trim.frames - picture } else { trim.frames + picture };
        let total = self.view.total_frames;
        let length = trim.slot_end as i64 - trim.slot_start as i64 + frames;
        let last = self
            .stored_markers
            .iter()
            .filter(|marker| marker.system_role.is_empty() && marker.program_frame >= trim.slot_end)
            .map(|marker| marker.program_frame)
            .max()
            .unwrap_or(trim.slot_end);
        if length < 1 {
            self.refresh_view(qnc_ui_text::text("story.msg.slot_one_picture").into());
            return true;
        }
        match trim.empty {
            Some((empty_start, empty_end)) if empty_end as i64 - (empty_start as i64 + frames) < 1 => {
                self.refresh_view(qnc_ui_text::text("story.msg.empty_slot_one_picture").into());
                return true;
            }
            None if last as i64 + frames >= total as i64 => {
                self.refresh_view(qnc_ui_text::text("story.msg.last_slot_one_picture").into());
                return true;
            }
            _ => {}
        }
        self.slot_trim = Some(SlotTrim { frames, ..trim.clone() });
        self.refresh_view(Self::trim_message(trim.start));
        if self.past_clip_end() {
            self.slot_trim = Some(trim);
            self.refresh_view(qnc_ui_text::text("story.msg.clip_at_end").into());
        }
        true
    }

    /// Enter: the trimmed slot is written; false when no slot edge is taken.
    pub(crate) fn commit_slot_trim(&mut self) -> bool {
        let Some(trim) = self.slot_trim.take() else {
            return false;
        };
        if trim.frames != 0 {
            self.marker_committing = true;
            self.write(Operation::TrimSlot { slot_id: trim.slot_id, start: trim.start, frames: trim.frames });
            // The Wrap playhead on the new edge: the monitor shows the written cut.
            let edge = if trim.start { trim.slot_start } else { trim.slot_end.saturating_add_signed(trim.frames - 1) };
            self.cue_program(edge);
        } else {
            self.refresh_view(String::new());
        }
        true
    }
}
