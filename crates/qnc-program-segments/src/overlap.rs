//! The picture J of a Ton segment (user 2026-10-21, news practice: the speaker appears before
//! his sound): on the segment panel Ctrl+Shift+J takes it for the Ton segment under the Wrap
//! playhead (else the chosen one); its picture starts before its sound, over the end of the
//! segment before. The length is free (user: "not a few frames, as long as wanted"): only the
//! source and the segment before bound it, in the playlist. The arrows (and the wheel) make it
//! a frame longer (left) or shorter (right), seen and heard at once; Enter writes it, Escape
//! drops it. A trim playhead shows where the picture starts. (The J and L of a cover's sound
//! are the sound of its M marker's cut, `cut_part.rs`.)

use qnc_program_db::Operation;

use crate::ProgramSegments;

/// Longest picture J, story frames (two hours at 50p: as long as wanted; the owner of the
/// story holds the same bound).
const MAX_FRAMES: i64 = 360_000;

/// What is taken (one kind so far).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    PictureLead,
}

/// The taken picture J: its segment, the stored value and the draft.
#[derive(Debug, Clone)]
pub(crate) struct OverlapEdit {
    pub(crate) id: String,
    pub(crate) kind: Kind,
    stored: u32,
    frames: i64,
    /// The segment's place on the program, `[start, end)`.
    start: u64,
}

impl OverlapEdit {
    /// Where the picture starts on the program, for the trim playhead.
    pub(crate) fn frame(&self) -> u64 {
        self.start.saturating_sub(self.frames as u64)
    }
}

/// The drafted picture J as the program plays it before Enter: the segment, its picture J and
/// its stored A1 sound (channel, level, off).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlapDraft {
    pub id: String,
    pub frames: u32,
    pub sound: (u16, i8, bool),
}

impl ProgramSegments {
    /// Ctrl+Shift+J on the segment panel: the picture J of the Ton segment under the Wrap
    /// playhead, else of the chosen one. False when there is none or it is the first.
    pub fn take_picture_lead(&mut self, frame: u64) -> bool {
        let found = self
            .view
            .rows
            .iter()
            .position(|row| row.start_frame <= frame && frame < row.end_frame)
            .or_else(|| self.view.rows.iter().position(|row| row.selected));
        let Some(index) = found else {
            self.refresh_view(qnc_ui_text::text("story.msg.no_segment_at_playhead").into());
            return false;
        };
        let row = &self.view.rows[index];
        if row.kind != crate::SegmentKind::Ton || index == 0 {
            self.refresh_view(qnc_ui_text::text("story.picture_lead_ton_only").into());
            return false;
        }
        let stored = self.stored.iter().find(|segment| segment.segment_id == row.segment_id).map_or(0, |segment| segment.v_lead_frames);
        let edit = OverlapEdit { id: row.segment_id.clone(), kind: Kind::PictureLead, stored, frames: i64::from(stored), start: row.start_frame };
        if self.refused(crate::confirm::Take::Overlap(edit.id.clone(), edit.kind)) {
            return false;
        }
        self.let_go_edits(false);
        self.overlap = Some(edit);
        self.say_overlap();
        true
    }

    /// An arrow, Enter or Escape while a picture J is taken; `None` for any other key.
    pub(crate) fn overlap_key(&mut self, action_id: &str) -> Option<bool> {
        let edit = self.overlap.as_mut()?;
        let longer = match action_id {
            "step_back_frame" => true,
            "step_forward_frame" => false,
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

    /// Enter: the picture J is written; false when none is taken.
    pub(crate) fn commit_overlap(&mut self) -> bool {
        let Some(edit) = self.overlap.take() else {
            return false;
        };
        let frames = edit.frames as u32;
        if frames != edit.stored {
            self.write(Operation::SetSegmentPictureLead { segment_id: edit.id, frames });
        }
        self.refresh_view(String::new());
        true
    }

    /// The taken picture J as drafted, with the stored A1 sound of its segment; `None` while
    /// none is taken or it is as stored.
    pub fn overlap_draft(&self) -> Option<OverlapDraft> {
        let edit = self.overlap.as_ref()?;
        let frames = edit.frames as u32;
        if frames == edit.stored {
            return None;
        }
        let segment = self.stored.iter().find(|segment| segment.segment_id == edit.id)?;
        Some(OverlapDraft { id: edit.id.clone(), frames, sound: (segment.a1_source_channel, segment.a1_level_db, segment.a1_off) })
    }

    pub fn overlap_taken(&self) -> bool {
        self.overlap.is_some()
    }

    fn say_overlap(&mut self) {
        let Some(edit) = &self.overlap else {
            return;
        };
        let text = qnc_ui_text::fill("story.msg.picture_j_taken", &[("frames", &edit.frames.to_string())]);
        self.refresh_view(text);
    }
}
