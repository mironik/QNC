//! The J and L of the story (user 2026-10-21, news practice), on the segment panel:
//! Ctrl+J takes the J of a cover (its A2 starts before its picture), Ctrl+L its L (its A2 goes
//! on after it): the cover under the Wrap playhead, else the chosen cover or the chosen slot's
//! cover. Ctrl+Shift+J takes the picture J of the Ton segment under the Wrap playhead (else the
//! chosen one): its picture starts before its sound, over the end of the segment before. The
//! length is free (user: "not a few frames, as long as wanted"): only the source and the room
//! next to it bound it, in the playlist. The arrows (and the wheel) make it a frame longer or
//! shorter (the left arrow moves a start earlier, the right arrow an end later), heard and
//! seen at once; Enter writes it, Escape drops it. A trim playhead shows where it starts or ends.

use qnc_program_db::{Operation, ProgramCover};

use crate::ProgramSegments;

/// Longest J or L, story frames (two hours at 50p: as long as wanted; the owner of the story
/// holds the same bound).
const MAX_FRAMES: i64 = 360_000;

/// What is taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    CoverLead,
    CoverTail,
    PictureLead,
}

/// The taken J or L: its object, the stored values and the draft of the taken one.
#[derive(Debug, Clone)]
pub(crate) struct OverlapEdit {
    pub(crate) id: String,
    pub(crate) kind: Kind,
    /// The stored (J, L) of a cover; (picture J, 0) of a segment.
    stored: (u32, u32),
    frames: i64,
    /// The cover's or segment's place on the program, `[start, end)`.
    span: (u64, u64),
}

impl OverlapEdit {
    /// Where the sound (or the picture) starts or ends on the program, for the trim playhead.
    pub(crate) fn frame(&self) -> u64 {
        match self.kind {
            Kind::CoverTail => self.span.1.saturating_add(self.frames as u64),
            Kind::CoverLead | Kind::PictureLead => self.span.0.saturating_sub(self.frames as u64),
        }
    }

    fn drafted(&self) -> (u32, u32) {
        let frames = self.frames as u32;
        match self.kind {
            Kind::CoverTail => (self.stored.0, frames),
            Kind::CoverLead | Kind::PictureLead => (frames, self.stored.1),
        }
    }
}

/// The drafted J/L as the program plays it before Enter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlapDraft {
    /// A cover, its (J, L) and its stored A2 sound (channel, level, off).
    Cover { id: String, overlap: (u32, u32), sound: (u16, i8, bool) },
    /// A Ton segment, its picture J and its stored A1 sound (channel, level, off).
    Picture { id: String, frames: u32, sound: (u16, i8, bool) },
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
        let kind = if lead { Kind::CoverLead } else { Kind::CoverTail };
        self.take(Self::overlap_of(&cover, kind))
    }

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
        let edit = OverlapEdit {
            id: row.segment_id.clone(),
            kind: Kind::PictureLead,
            stored: (stored, 0),
            frames: i64::from(stored),
            span: (row.start_frame, row.end_frame),
        };
        self.take(edit)
    }

    fn take(&mut self, edit: OverlapEdit) -> bool {
        if self.refused(crate::confirm::Take::Overlap(edit.id.clone(), edit.kind)) {
            return false;
        }
        self.let_go_edits(false);
        self.overlap = Some(edit);
        self.say_overlap();
        true
    }

    fn overlap_of(cover: &ProgramCover, kind: Kind) -> OverlapEdit {
        let stored = (cover.a2_lead_frames, cover.a2_tail_frames);
        OverlapEdit {
            id: cover.cover_id.clone(),
            kind,
            stored,
            frames: i64::from(if kind == Kind::CoverTail { stored.1 } else { stored.0 }),
            span: (cover.program_start_frame, cover.program_end_frame),
        }
    }

    /// An arrow, Enter or Escape while a J or L is taken; `None` for any other key.
    pub(crate) fn overlap_key(&mut self, action_id: &str) -> Option<bool> {
        let edit = self.overlap.as_mut()?;
        let tail = edit.kind == Kind::CoverTail;
        let longer = match action_id {
            "step_back_frame" => !tail,
            "step_forward_frame" => tail,
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
        let drafted = edit.drafted();
        if drafted != edit.stored {
            self.write(match edit.kind {
                Kind::PictureLead => Operation::SetSegmentPictureLead { segment_id: edit.id, frames: drafted.0 },
                Kind::CoverLead | Kind::CoverTail => {
                    Operation::SetCoverOverlap { cover_id: edit.id, lead_frames: drafted.0, tail_frames: drafted.1 }
                }
            });
        }
        self.refresh_view(String::new());
        true
    }

    /// The taken J or L as drafted, with the stored sound of its part; `None` while none is
    /// taken or it is as stored.
    pub fn overlap_draft(&self) -> Option<OverlapDraft> {
        let edit = self.overlap.as_ref()?;
        let drafted = edit.drafted();
        if drafted == edit.stored {
            return None;
        }
        match edit.kind {
            Kind::PictureLead => {
                let segment = self.stored.iter().find(|segment| segment.segment_id == edit.id)?;
                let sound = (segment.a1_source_channel, segment.a1_level_db, segment.a1_off);
                Some(OverlapDraft::Picture { id: edit.id.clone(), frames: drafted.0, sound })
            }
            Kind::CoverLead | Kind::CoverTail => {
                let cover = self.stored_covers.iter().find(|cover| cover.cover_id == edit.id)?;
                let sound = (cover.a2_source_channel, cover.a2_level_db, cover.a2_off);
                Some(OverlapDraft::Cover { id: edit.id.clone(), overlap: drafted, sound })
            }
        }
    }

    pub fn overlap_taken(&self) -> bool {
        self.overlap.is_some()
    }

    fn say_overlap(&mut self) {
        let Some(edit) = &self.overlap else {
            return;
        };
        let key = match edit.kind {
            Kind::CoverLead => "story.msg.cover_j_taken",
            Kind::CoverTail => "story.msg.cover_l_taken",
            Kind::PictureLead => "story.msg.picture_j_taken",
        };
        let text = qnc_ui_text::fill(key, &[("frames", &edit.frames.to_string())]);
        self.refresh_view(text);
    }
}
