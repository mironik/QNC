//! The part of a taken M marker's cut that moves (user 2026-10-21): "the M marker is the cut of
//! the whole clip, it moves whole; when only its sound or only its picture moves, I/O and 1 or
//! 2". Ctrl+M (or a right click) takes the nearest marker whole: the arrows and the wheel move
//! picture and sound together. With the marker taken, Ctrl+I or Ctrl+O moves only its picture
//! (the sound stays: a J or L of the picture), Ctrl+2 only its A2 sound; Ctrl+M again the whole
//! cut. A right click is the whole marker; a second one on the taken marker over the picture
//! its picture, over A2 its sound. Parts only in the enlarged slot (user 2026-10-22: "Ctrl+M on
//! the Wrap row or in the zoom, the picture and the sound alone only in the zoom"). Enter
//! writes it, Escape drops it. A1 is the story's segment: an M marker is no cut of it.
//!
//! The A2 sound of a cut has two sides (user 2026-10-29: "J and L together"): where the sound
//! of the cover before ends (its L, `tail`) and where the sound of the cover after starts (its
//! J, `lead`); between them the two sounds overlap. With the sound taken, Ctrl+2 moves both,
//! Ctrl+O only the L, Ctrl+I only the J (O ends the sound before, I starts the sound after); a
//! click on a side's red line takes that side, a drag moves it, the wheel moves it as the
//! arrows do.

use qnc_program_db::Operation;

use crate::ProgramSegments;

/// Which side of the cut's A2 sound moves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SoundEdge {
    /// Both: the cut's sound as one (Ctrl+2).
    #[default]
    Both,
    /// The J of the cover after: where its sound starts (Ctrl+I).
    Lead,
    /// The L of the cover before: where its sound ends (Ctrl+O).
    Tail,
}

/// The two sides of a cut's A2 sound, story frames: the L of the cover before and the J of the
/// cover after.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CutSound {
    pub tail: u32,
    pub lead: u32,
}

/// What of the taken marker's cut moves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CutPart {
    #[default]
    Whole,
    Picture,
    /// Its A2 sound: as stored, as drafted, and the side that moves.
    Sound { stored: CutSound, draft: CutSound, edge: SoundEdge },
}

/// The most frames a side takes (the store's bound).
const MOST: i64 = 360_000;

impl ProgramSegments {
    /// Ctrl+I/O (`picture`) or Ctrl+1/2 (`lane`) while a marker is taken, or a right click
    /// (`marker_id`) that takes that marker first. False when no marker is taken.
    pub fn take_cut_part(&mut self, marker_id: Option<String>, picture: bool, lane: u8) -> bool {
        // Only in the enlarged slot (user 2026-10-22): on a Wrap row the marker moves whole.
        if self.zoom_slot.is_none() && (marker_id.is_some() || self.marker_edit.is_some()) {
            self.refresh_view(qnc_ui_text::text("story.msg.cut_part_zoom_only").into());
            return true;
        }
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
        // The sound taken again (Ctrl+2, a second right click on it) keeps its draft (the log
        // 2026-10-22) and moves both sides again.
        if let CutPart::Sound { stored, draft, .. } = self.cut_part {
            self.cut_part = CutPart::Sound { stored, draft, edge: SoundEdge::Both };
            return self.say_cut("story.msg.cut_sound_taken");
        }
        // Only the sound: the picture goes back to where it is stored.
        if stored.is_some_and(|stored| stored != draft) {
            self.marker_edit = Some((marker_id.clone(), stored.unwrap_or(draft)));
        }
        let sound = self.stored_cut_sound(&marker_id);
        self.cut_part = CutPart::Sound { stored: sound, draft: sound, edge: SoundEdge::Both };
        self.say_cut("story.msg.cut_sound_taken")
    }

    /// Ctrl+I (`lead`, the J of the cover after) or Ctrl+O (the L of the cover before) while
    /// the sound of the cut is taken: only that side moves. False when the sound is not taken.
    pub fn take_sound_edge(&mut self, lead: bool) -> bool {
        let CutPart::Sound { stored, draft, .. } = self.cut_part else {
            return false;
        };
        let Some((marker_id, _)) = self.marker_edit.clone() else {
            return false;
        };
        let (before, after) = self.cut_covers(&marker_id);
        if if lead { after.is_none() } else { before.is_none() } {
            self.refresh_view(qnc_ui_text::text("story.cut_sound_no_cover").into());
            return true;
        }
        let edge = if lead { SoundEdge::Lead } else { SoundEdge::Tail };
        self.cut_part = CutPart::Sound { stored, draft, edge };
        self.say_cut("story.msg.cut_sound_taken")
    }

    /// A click on a side of the taken cut's sound (`frame`, story): the nearer side is taken.
    pub(crate) fn take_sound_edge_at(&mut self, marker_id: &str, frame: u64) -> bool {
        if self.marker_edit.as_ref().map(|(id, _)| id.as_str()) != Some(marker_id) || !self.cut_sound_taken() {
            return false;
        }
        match self.nearer_edge(frame) {
            Some(lead) => self.take_sound_edge(lead),
            None => false,
        }
    }

    /// A drag of a side of the taken cut's sound to `frame` (story): the side the drag started
    /// on (the nearer one when both are taken) goes there.
    pub(crate) fn drag_sound_edge(&mut self, marker_id: &str, frame: u64) -> bool {
        if self.marker_edit.as_ref().map(|(id, _)| id.as_str()) != Some(marker_id) {
            return false;
        }
        let CutPart::Sound { edge, .. } = self.cut_part else {
            return false;
        };
        if edge == SoundEdge::Both {
            let Some(lead) = self.nearer_edge(frame) else {
                return false;
            };
            self.take_sound_edge(lead);
        }
        let (CutPart::Sound { stored, mut draft, edge }, Some((_, cut))) = (self.cut_part, self.marker_edit.clone()) else {
            return false;
        };
        let cut = cut as i64;
        match edge {
            SoundEdge::Lead => draft.lead = (cut - frame as i64).clamp(0, MOST) as u32,
            SoundEdge::Tail => draft.tail = (frame as i64 - cut).clamp(0, MOST) as u32,
            SoundEdge::Both => return false,
        }
        self.cut_part = CutPart::Sound { stored, draft, edge };
        self.say_cut("story.msg.cut_sound_taken")
    }

    /// Whether `frame` is nearer the J (`true`) or the L of the taken cut's sound; only a side
    /// with a cover counts.
    fn nearer_edge(&self, frame: u64) -> Option<bool> {
        let CutPart::Sound { draft, .. } = self.cut_part else {
            return None;
        };
        let (marker_id, cut) = self.marker_edit.as_ref()?;
        let (before, after) = self.cut_covers(marker_id);
        let lead = after.map(|_| cut.saturating_sub(u64::from(draft.lead)).abs_diff(frame));
        let tail = before.map(|_| cut.saturating_add(u64::from(draft.tail)).abs_diff(frame));
        match (lead, tail) {
            (Some(lead), Some(tail)) => Some(lead <= tail),
            (Some(_), None) => Some(true),
            (None, Some(_)) => Some(false),
            (None, None) => None,
        }
    }

    /// Ctrl+M on the taken marker: the whole cut moves again.
    pub(crate) fn whole_cut(&mut self) {
        self.cut_part = CutPart::Whole;
    }

    /// The two sides of a marker's cut as stored.
    pub(crate) fn stored_cut_sound(&self, marker_id: &str) -> CutSound {
        let (before, after) = self.cut_covers(marker_id);
        CutSound { tail: before.map_or(0, |cover| cover.a2_tail_frames), lead: after.map_or(0, |cover| cover.a2_lead_frames) }
    }

    pub(crate) fn cut_covers(&self, marker_id: &str) -> (Option<&qnc_program_db::ProgramCover>, Option<&qnc_program_db::ProgramCover>) {
        let before = self.stored_covers.iter().find(|cover| cover.slot_id.ends_with(&format!("|{marker_id}")));
        let after = self.stored_covers.iter().find(|cover| cover.slot_id.starts_with(&format!("{marker_id}|")));
        (before, after)
    }

    /// The arrows on the cut's sound: `frames` later (positive) or earlier. One side moves
    /// alone; both move as one sound cut (a J or an L as the cut goes), or together keeping the
    /// overlap between them. False when the sound of the cut is not taken.
    pub(crate) fn nudge_cut_sound(&mut self, frames: i64) -> bool {
        let CutPart::Sound { stored, draft, edge } = self.cut_part else {
            return false;
        };
        let Some((marker_id, _)) = self.marker_edit.clone() else {
            return false;
        };
        let (before, after) = self.cut_covers(&marker_id);
        let (before, after) = (before.is_some(), after.is_some());
        let (tail, lead) = (i64::from(draft.tail), i64::from(draft.lead));
        let (tail, lead) = match edge {
            SoundEdge::Tail => (tail + frames, lead),
            SoundEdge::Lead => (tail, lead - frames),
            // Both sides apart: the overlap moves whole.
            SoundEdge::Both if tail > 0 && lead > 0 => (tail + frames, lead - frames),
            // One sound cut: an L when it is after the picture, a J before it.
            SoundEdge::Both => {
                let offset = tail - lead + frames;
                (offset.max(0), (-offset).max(0))
            }
        };
        let moved = CutSound { tail: tail.clamp(0, MOST) as u32, lead: lead.clamp(0, MOST) as u32 };
        if moved == draft || moved.tail > 0 && !before || moved.lead > 0 && !after {
            self.refresh_view(qnc_ui_text::text("story.cut_sound_no_cover").into());
            return true;
        }
        self.cut_part = CutPart::Sound { stored, draft: moved, edge };
        self.say_cut("story.msg.cut_sound_taken")
    }

    /// Enter on a taken marker: the whole cut, its picture or its sound is written.
    pub(crate) fn commit_cut(&mut self, marker_id: String, frame: u64) {
        let part = std::mem::take(&mut self.cut_part);
        self.marker_committing = true;
        self.write(match part {
            CutPart::Whole => Operation::MoveMarker { marker_id, program_frame: frame },
            CutPart::Picture => Operation::MoveMarkerPicture { marker_id, program_frame: frame },
            CutPart::Sound { stored, draft, .. } if stored != draft => {
                Operation::SetCutSoundEdges { marker_id, tail_frames: draft.tail, lead_frames: draft.lead }
            }
            CutPart::Sound { .. } => return self.refresh_view(String::new()),
        });
    }

    /// The covers of the taken cut with their J and L as drafted, for the program to play
    /// before Enter; `None` while the sound of the cut is not taken or is as stored.
    pub fn cut_sound_draft(&self) -> Option<Vec<(String, (u32, u32))>> {
        let CutPart::Sound { stored, draft, .. } = self.cut_part else {
            return None;
        };
        let (marker_id, _) = self.marker_edit.as_ref()?;
        if stored == draft {
            return None;
        }
        let (before, after) = self.cut_covers(marker_id);
        let mut covers = Vec::new();
        if let Some(cover) = before {
            covers.push((cover.cover_id.clone(), (cover.a2_lead_frames, draft.tail)));
        }
        if let Some(cover) = after {
            covers.push((cover.cover_id.clone(), (draft.lead, cover.a2_tail_frames)));
        }
        Some(covers)
    }

    pub fn cut_sound_taken(&self) -> bool {
        self.marker_edit.is_some() && matches!(self.cut_part, CutPart::Sound { .. })
    }

    /// Where the sides of a marker's cut sound are in the story: the end of the sound before
    /// and the start of the sound after (`None` for a side with no cover); the taken marker
    /// with its draft.
    pub(crate) fn cut_sound_frames(&self, marker_id: &str, cut: u64) -> (Option<u64>, Option<u64>) {
        let (before, after) = self.cut_covers(marker_id);
        let sound = match self.cut_part {
            CutPart::Sound { draft, .. } if self.marker_edit.as_ref().map(|(id, _)| id.as_str()) == Some(marker_id) => draft,
            _ => self.stored_cut_sound(marker_id),
        };
        (
            before.map(|_| cut.saturating_add(u64::from(sound.tail))),
            after.map(|_| cut.saturating_sub(u64::from(sound.lead))),
        )
    }

    /// Where the trim playheads of the cut's sound stand: the moving side or both.
    pub(crate) fn cut_sound_trim_frames(&self) -> Vec<u64> {
        let CutPart::Sound { edge, .. } = self.cut_part else {
            return Vec::new();
        };
        let Some((marker_id, cut)) = self.marker_edit.clone() else {
            return Vec::new();
        };
        let (tail, lead) = self.cut_sound_frames(&marker_id, cut);
        match edge {
            SoundEdge::Tail => tail.into_iter().collect(),
            SoundEdge::Lead => lead.into_iter().collect(),
            SoundEdge::Both => lead.into_iter().chain(tail).collect(),
        }
    }

    fn say_cut(&mut self, key: &'static str) -> bool {
        let (tail, lead, edge) = match self.cut_part {
            CutPart::Sound { draft, edge, .. } => (draft.tail, draft.lead, edge),
            _ => (0, 0, SoundEdge::Both),
        };
        let side = match edge {
            SoundEdge::Both => "story.msg.cut_sound_both",
            SoundEdge::Lead => "story.msg.cut_sound_lead",
            SoundEdge::Tail => "story.msg.cut_sound_tail",
        };
        let mut message = qnc_ui_text::fill(key, &[("frames", &(i64::from(tail) - i64::from(lead)).to_string())]);
        if matches!(self.cut_part, CutPart::Sound { .. }) {
            message = qnc_ui_text::fill(side, &[("j", &lead.to_string()), ("l", &tail.to_string())]);
        }
        self.refresh_view(message);
        true
    }
}
