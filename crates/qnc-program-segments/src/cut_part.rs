//! The part of a taken M marker's cut that moves (user 2026-10-21): "the M marker is the cut of
//! the whole clip, it moves whole; when only its sound or only its picture moves, I/O and 1 or
//! 2". Ctrl+M (or a right click) takes the nearest marker whole: the arrows and the wheel move
//! picture and sound together. With the marker taken, Ctrl+I or Ctrl+O moves only its picture
//! (the sound stays: a J or L of the picture), Ctrl+2 only its A2 sound (an L of the cover
//! before, a J of the cover after); Ctrl+M again the whole cut. A right click over the picture
//! takes the nearest marker's picture, over A2 its sound. Enter writes it, Escape drops it.
//! A1 is the story's segment: an M marker is no cut of it.

use qnc_program_db::Operation;

use crate::ProgramSegments;

/// What of the taken marker's cut moves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CutPart {
    #[default]
    Whole,
    Picture,
    /// Its A2 sound: the stored offset and the draft, story frames after the picture cut.
    Sound { stored: i64, draft: i64 },
}

impl ProgramSegments {
    /// Ctrl+I/O (`picture`) or Ctrl+1/2 (`lane`) while a marker is taken, or a right click
    /// (`marker_id`) that takes that marker first. False when no marker is taken.
    pub fn take_cut_part(&mut self, marker_id: Option<String>, picture: bool, lane: u8) -> bool {
        if let Some(marker_id) = marker_id {
            if self.marker_edit.as_ref().map(|(id, _)| id) != Some(&marker_id) {
                let Some(frame) = self.stored_markers.iter().find(|marker| marker.marker_id == marker_id).map(|marker| marker.program_frame) else {
                    return false;
                };
                self.start_marker_edit(marker_id, frame);
            }
        }
        let Some((marker_id, draft)) = self.marker_edit.clone() else {
            return false;
        };
        let stored = self.stored_markers.iter().find(|marker| marker.marker_id == marker_id).map(|marker| marker.program_frame);
        if picture {
            self.cut_part = CutPart::Picture;
            return self.say_cut("story.msg.cut_picture_taken");
        }
        if lane == 0 {
            self.refresh_view(qnc_ui_text::text("story.msg.a1_no_cut").into());
            return true;
        }
        // Only the sound: the picture goes back to where it is stored.
        if stored.is_some_and(|stored| stored != draft) {
            self.marker_edit = Some((marker_id.clone(), stored.unwrap_or(draft)));
        }
        let offset = self.cut_offset(&marker_id);
        self.cut_part = CutPart::Sound { stored: offset, draft: offset };
        self.say_cut("story.msg.cut_sound_taken")
    }

    /// Ctrl+M on the taken marker: the whole cut moves again.
    pub(crate) fn whole_cut(&mut self) {
        self.cut_part = CutPart::Whole;
    }

    /// The A2 offset of a marker's cut: the L of the cover before minus the J of the cover after.
    fn cut_offset(&self, marker_id: &str) -> i64 {
        let (before, after) = self.cut_covers(marker_id);
        before.map_or(0, |cover| i64::from(cover.a2_tail_frames)) - after.map_or(0, |cover| i64::from(cover.a2_lead_frames))
    }

    fn cut_covers(&self, marker_id: &str) -> (Option<&qnc_program_db::ProgramCover>, Option<&qnc_program_db::ProgramCover>) {
        let before = self.stored_covers.iter().find(|cover| cover.slot_id.ends_with(&format!("|{marker_id}")));
        let after = self.stored_covers.iter().find(|cover| cover.slot_id.starts_with(&format!("{marker_id}|")));
        (before, after)
    }

    /// The arrows on the cut's sound: `frames` later (positive) or earlier. False when the
    /// sound of the cut is not taken.
    pub(crate) fn nudge_cut_sound(&mut self, frames: i64) -> bool {
        let CutPart::Sound { stored, draft } = self.cut_part else {
            return false;
        };
        let Some((marker_id, _)) = self.marker_edit.clone() else {
            return false;
        };
        let (before, after) = self.cut_covers(&marker_id);
        let (lowest, highest) = (if after.is_some() { -i64::MAX } else { 0 }, if before.is_some() { i64::MAX } else { 0 });
        let moved = (draft + frames).clamp(lowest, highest);
        if moved == draft {
            self.refresh_view(qnc_ui_text::text("story.cut_sound_no_cover").into());
            return true;
        }
        self.cut_part = CutPart::Sound { stored, draft: moved };
        self.say_cut("story.msg.cut_sound_taken")
    }

    /// Enter on a taken marker: the whole cut, its picture or its sound is written.
    pub(crate) fn commit_cut(&mut self, marker_id: String, frame: u64) {
        let part = std::mem::take(&mut self.cut_part);
        self.marker_committing = true;
        self.write(match part {
            CutPart::Whole => Operation::MoveMarker { marker_id, program_frame: frame },
            CutPart::Picture => Operation::MoveMarkerPicture { marker_id, program_frame: frame },
            CutPart::Sound { stored, draft } if stored != draft => Operation::SetCutSound { marker_id, offset: draft },
            CutPart::Sound { .. } => return self.refresh_view(String::new()),
        });
    }

    /// The covers of the taken cut with their J and L as drafted, for the program to play
    /// before Enter; `None` while the sound of the cut is not taken or is as stored.
    pub fn cut_sound_draft(&self) -> Option<Vec<(String, (u32, u32))>> {
        let CutPart::Sound { stored, draft } = self.cut_part else {
            return None;
        };
        let (marker_id, _) = self.marker_edit.as_ref()?;
        if stored == draft {
            return None;
        }
        let (before, after) = self.cut_covers(marker_id);
        let mut covers = Vec::new();
        if let Some(cover) = before {
            covers.push((cover.cover_id.clone(), (cover.a2_lead_frames, draft.max(0) as u32)));
        }
        if let Some(cover) = after {
            covers.push((cover.cover_id.clone(), ((-draft).max(0) as u32, cover.a2_tail_frames)));
        }
        Some(covers)
    }

    pub fn cut_sound_taken(&self) -> bool {
        self.marker_edit.is_some() && matches!(self.cut_part, CutPart::Sound { .. })
    }

    /// Where the trim playhead of the cut's sound stands: the picture cut and the offset.
    pub(crate) fn cut_sound_frame(&self) -> Option<u64> {
        let CutPart::Sound { draft, .. } = self.cut_part else {
            return None;
        };
        let (_, frame) = self.marker_edit.as_ref()?;
        Some(frame.saturating_add_signed(draft))
    }

    fn say_cut(&mut self, key: &'static str) -> bool {
        let frames = match self.cut_part {
            CutPart::Sound { draft, .. } => draft,
            _ => 0,
        };
        self.refresh_view(qnc_ui_text::fill(key, &[("frames", &frames.to_string())]));
        true
    }
}
