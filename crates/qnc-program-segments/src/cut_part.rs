//! The part of a taken cut that moves (user 2026-10-21): "the M marker is the cut of the whole
//! clip, it moves whole; when only its sound or only its picture moves, I/O and 1 or 2".
//! Ctrl+M (or a right click) takes the nearest cut whole: the arrows and the wheel move picture
//! and sound together. With it taken, Ctrl+I or Ctrl+O moves only its picture (the sound stays:
//! a J or L of the picture), Ctrl+2 only its A2 sound, Ctrl+1 only its A1 sound; Ctrl+M again
//! the whole cut. A right click is the whole cut; a second one on it over the picture its
//! picture, over A2 its sound. Parts only in the enlarged slot (user 2026-10-22: "Ctrl+M on the
//! Wrap row or in the zoom, the picture and the sound alone only in the zoom"). Enter writes
//! it, Escape drops it.
//!
//! The sound of a cut has two sides (user 2026-10-29: "J and L together"): where the sound
//! before ends (its L, `tail`) and where the sound after starts (its J, `lead`); between them
//! the two sounds cross. With the sound taken, Ctrl+1/Ctrl+2 (or a click on A1/A2 in the zoom)
//! choose the lane, the cut's sound moves on both sides, Ctrl+O only the L, Ctrl+I only the J
//! (O ends the sound before, I starts the sound after); a click on a side's red line takes
//! that side, a drag moves it, the wheel moves it as the arrows do. On A2 the sides are the
//! covers on both sides of an M marker; on A1 the two clips of the story on both sides of the
//! cut between them ("every cut is a marker": the join of two clips is one too).

use qnc_program_db::Operation;

use crate::ProgramSegments;

/// Which side of the cut's sound moves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SoundEdge {
    /// Both: the cut's sound as one (Ctrl+1, Ctrl+2).
    #[default]
    Both,
    /// The J of what comes after: where its sound starts (Ctrl+I).
    Lead,
    /// The L of what goes before: where its sound ends (Ctrl+O).
    Tail,
}

/// The two sides of a cut's sound, story frames: the L of what is before and the J of what
/// is after.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CutSound {
    pub tail: u32,
    pub lead: u32,
}

/// What of the taken cut moves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CutPart {
    #[default]
    Whole,
    Picture,
    /// Its sound on `lane` (0 A1, 1 A2): as stored, as drafted, and the side that moves.
    Sound { lane: u8, stored: CutSound, draft: CutSound, edge: SoundEdge },
}

/// The most frames a side takes (the store's bound).
const MOST: i64 = 360_000;

impl ProgramSegments {
    /// Ctrl+I/O (`picture`) or Ctrl+1/2 (`lane`) while a cut is taken, or a right click
    /// (`marker_id`) that takes that cut first. False when no cut is taken.
    pub fn take_cut_part(&mut self, marker_id: Option<String>, picture: bool, lane: u8) -> bool {
        // Only in the enlarged slot (user 2026-10-22): on a Wrap row the cut moves whole.
        // The A1/A2 buttons of the trim zoom (log 2026-10-29: refused on a Wrap row) name the cut
        // already taken: that zoom is open, so its part is taken there too.
        let from_trim_zoom = marker_id.is_some() && marker_id.as_deref() == self.marker_edit.as_ref().map(|(id, _)| id.as_str());
        if self.zoom_slot.is_none() && !from_trim_zoom && (marker_id.is_some() || self.marker_edit.is_some()) {
            self.refresh_view(qnc_ui_text::text("story.msg.cut_part_zoom_only").into());
            return true;
        }
        if let Some(marker_id) = marker_id {
            if self.marker_edit.as_ref().map(|(id, _)| id) != Some(&marker_id) {
                let Some(frame) = self.cut_frame(&marker_id) else {
                    return false;
                };
                self.start_marker_edit(marker_id, frame);
            }
        }
        let Some((marker_id, draft)) = self.marker_edit.clone() else {
            return false;
        };
        let stored = self.cut_frame(&marker_id);
        if picture {
            // A cut of the picture is only where the picture of the program changes (user
            // 2026-10-29: a cover overwrites the picture and A2, it is no layer above).
            if let Some(words) = self.no_picture_cut(&marker_id) {
                self.refresh_view(qnc_ui_text::text(words).into());
                return true;
            }
            // The picture of a cut between two clips starts where it is stored, which may be
            // apart from its sound (a J or L of the picture).
            if let Some(frame) = self.join_picture_frame(&marker_id) {
                self.marker_edit = Some((marker_id, frame));
            }
            self.cut_part = CutPart::Picture;
            return self.say_cut();
        }
        // The same lane taken again (a second Ctrl+1/2 or right click) keeps its draft (the log
        // 2026-10-22) and moves both sides again.
        if let CutPart::Sound { lane: taken, stored, draft, .. } = self.cut_part {
            if taken == lane {
                self.cut_part = CutPart::Sound { lane, stored, draft, edge: SoundEdge::Both };
                return self.say_cut();
            }
        }
        // Only the sound: the picture goes back to where it is stored.
        let cut = stored.unwrap_or(draft);
        if !self.cut_has_sound(lane, &marker_id, cut) {
            let words = if lane == 0 { "story.msg.a1_no_cut" } else { "story.cut_sound_no_cover" };
            self.refresh_view(qnc_ui_text::text(words).into());
            return true;
        }
        self.marker_edit = Some((marker_id.clone(), cut));
        let sound = self.stored_cut_sound(lane, &marker_id, cut);
        self.cut_part = CutPart::Sound { lane, stored: sound, draft: sound, edge: SoundEdge::Both };
        self.say_cut()
    }

    /// Where a cut is stored: an M marker's frame or the start of the clip after a join.
    pub(crate) fn cut_frame(&self, marker_id: &str) -> Option<u64> {
        match qnc_program_view::join_segment(marker_id) {
            Some(segment_id) => self.view.rows.iter().find(|row| row.segment_id == segment_id).map(|row| row.start_frame),
            None => self.stored_markers.iter().find(|marker| marker.marker_id == marker_id).map(|marker| marker.program_frame),
        }
    }

    /// Ctrl+I (`lead`, the J of what is after) or Ctrl+O (the L of what is before) while the
    /// sound of the cut is taken: only that side moves. False when the sound is not taken.
    pub fn take_sound_edge(&mut self, lead: bool) -> bool {
        let CutPart::Sound { lane, stored, draft, .. } = self.cut_part else {
            return false;
        };
        let Some((marker_id, cut)) = self.marker_edit.clone() else {
            return false;
        };
        let (before, after) = self.cut_sides(lane, &marker_id, cut);
        if if lead { !after } else { !before } {
            self.refresh_view(qnc_ui_text::text("story.cut_sound_no_cover").into());
            return true;
        }
        let edge = if lead { SoundEdge::Lead } else { SoundEdge::Tail };
        self.cut_part = CutPart::Sound { lane, stored, draft, edge };
        self.say_cut()
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
        let (CutPart::Sound { lane, stored, mut draft, edge }, Some((_, cut))) = (self.cut_part, self.marker_edit.clone()) else {
            return false;
        };
        let cut = cut as i64;
        match edge {
            SoundEdge::Lead => draft.lead = (cut - frame as i64).clamp(0, MOST) as u32,
            SoundEdge::Tail => draft.tail = (frame as i64 - cut).clamp(0, MOST) as u32,
            SoundEdge::Both => return false,
        }
        self.cut_part = CutPart::Sound { lane, stored, draft, edge };
        self.say_cut()
    }

    /// Whether `frame` is nearer the J (`true`) or the L of the taken cut's sound; only a side
    /// that exists counts.
    fn nearer_edge(&self, frame: u64) -> Option<bool> {
        let CutPart::Sound { lane, draft, .. } = self.cut_part else {
            return None;
        };
        let (marker_id, cut) = self.marker_edit.as_ref()?;
        let (before, after) = self.cut_sides(lane, marker_id, *cut);
        let lead = after.then(|| cut.saturating_sub(u64::from(draft.lead)).abs_diff(frame));
        let tail = before.then(|| cut.saturating_add(u64::from(draft.tail)).abs_diff(frame));
        match (lead, tail) {
            (Some(lead), Some(tail)) => Some(lead <= tail),
            (Some(_), None) => Some(true),
            (None, Some(_)) => Some(false),
            (None, None) => None,
        }
    }

    /// Ctrl+M on the taken cut: the whole cut moves again, from where it is stored.
    pub(crate) fn whole_cut(&mut self) {
        self.cut_part = CutPart::Whole;
        if let Some((marker_id, _)) = self.marker_edit.clone() {
            if let Some(frame) = self.cut_frame(&marker_id).filter(|_| qnc_program_view::join_segment(&marker_id).is_some()) {
                self.marker_edit = Some((marker_id, frame));
            }
        }
    }

    /// Why the cut has no picture cut (a text code), `None` when it has one. A cover overwrites
    /// the picture of its slot (user 2026-10-29): the join of two clips under a cover is a cut
    /// of A1 only; an M marker between two empty slots changes no picture and no A2, it only
    /// bounds the slots.
    pub(crate) fn no_picture_cut(&self, marker_id: &str) -> Option<&'static str> {
        let frame = self.cut_frame(marker_id)?;
        if qnc_program_view::join_segment(marker_id).is_some() {
            let covered = self.view.covers.iter().any(|cover| cover.start_frame < frame && frame < cover.end_frame);
            return covered.then_some("story.msg.join_under_cover");
        }
        let (before, after) = self.cut_covers(marker_id);
        (before.is_none() && after.is_none()).then_some("story.msg.marker_no_picture_cut")
    }

    /// The two clips of a cut between them, as stored: the one before and `segment_id` after.
    fn join_pair(&self, marker_id: &str) -> Option<(&qnc_program_db::ProgramSegment, &qnc_program_db::ProgramSegment)> {
        let segment_id = qnc_program_view::join_segment(marker_id)?;
        let cut = self.view.rows.iter().find(|row| row.segment_id == segment_id)?.start_frame;
        self.join_clips(cut)
    }

    /// Where the picture of a cut between two clips is stored: its sound cut plus the picture
    /// L of the clip before, less the picture J of the clip after. `None` for an M marker.
    pub(crate) fn join_picture_frame(&self, marker_id: &str) -> Option<u64> {
        let cut = self.cut_frame(marker_id)?;
        let (before, after) = self.join_pair(marker_id)?;
        Some((cut + u64::from(before.v_tail_frames)).saturating_sub(u64::from(after.v_lead_frames)))
    }

    /// The picture of every cut between two clips apart from its sound, as (picture frame,
    /// sound frame), the taken one as drafted (user 2026-10-29: the picture and the sound of a
    /// cut are each their own timecode).
    pub(crate) fn picture_splits(&self) -> Vec<(u64, u64)> {
        self.view
            .rows
            .windows(2)
            .filter(|pair| !pair[0].lead && !pair[1].lead)
            .filter_map(|pair| {
                let marker_id = format!("{}{}", qnc_program_view::JOIN_PREFIX, pair[1].segment_id);
                let cut = pair[1].start_frame;
                // Under a cover the picture is the cover's: no picture cut to show.
                if self.no_picture_cut(&marker_id).is_some() {
                    return None;
                }
                let picture = match (self.cut_part, &self.marker_edit) {
                    (CutPart::Picture, Some((id, draft))) if *id == marker_id => Some(*draft),
                    _ => self.join_picture_frame(&marker_id),
                }?;
                (picture != cut).then_some((picture, cut))
            })
            .collect()
    }

    /// The picture of the taken cut between two clips as drafted, for the program to show before
    /// Enter: the two clips with their picture (J, L); `None` while it is not taken or as stored.
    pub fn join_picture_draft(&self) -> Option<Vec<(String, (u32, u32))>> {
        let (CutPart::Picture, Some((marker_id, draft))) = (self.cut_part, &self.marker_edit) else {
            return None;
        };
        let (before, after) = self.join_pair(marker_id)?;
        let (tail, lead) = picture_sides(*draft, self.cut_frame(marker_id)?);
        if (tail, lead) == (before.v_tail_frames, after.v_lead_frames) {
            return None;
        }
        Some(vec![
            (before.segment_id.clone(), (before.v_lead_frames, tail)),
            (after.segment_id.clone(), (lead, after.v_tail_frames)),
        ])
    }

    pub fn cut_picture_taken(&self) -> bool {
        self.marker_edit.is_some() && self.cut_part == CutPart::Picture
    }

    /// Whether the cut has sound on `lane` to move: covers on A2, two clips on A1.
    fn cut_has_sound(&self, lane: u8, marker_id: &str, cut: u64) -> bool {
        let (before, after) = self.cut_sides(lane, marker_id, cut);
        before || after
    }

    /// Whether the cut has something with sound before it and after it on `lane`.
    pub(crate) fn cut_sides(&self, lane: u8, marker_id: &str, cut: u64) -> (bool, bool) {
        if lane == 0 {
            let both = self.join_clips(cut).is_some();
            return (both, both);
        }
        let (before, after) = self.cut_covers(marker_id);
        (before.is_some(), after.is_some())
    }

    /// The two clips of the story at the cut `frame` on A1, as stored.
    fn join_clips(&self, frame: u64) -> Option<(&qnc_program_db::ProgramSegment, &qnc_program_db::ProgramSegment)> {
        let (before, after) = qnc_program_view::join_at(&self.view, frame)?;
        let stored = |id: &str| self.stored.iter().find(|segment| segment.segment_id == id);
        Some((stored(&before.segment_id)?, stored(&after.segment_id)?))
    }

    /// The two sides of a cut's sound on `lane` as stored.
    pub(crate) fn stored_cut_sound(&self, lane: u8, marker_id: &str, cut: u64) -> CutSound {
        if lane == 0 {
            return self.join_clips(cut).map_or_else(CutSound::default, |(before, after)| CutSound { tail: before.a1_tail_frames, lead: after.a1_lead_frames });
        }
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
        let CutPart::Sound { lane, stored, draft, edge } = self.cut_part else {
            return false;
        };
        let Some((marker_id, cut)) = self.marker_edit.clone() else {
            return false;
        };
        let (before, after) = self.cut_sides(lane, &marker_id, cut);
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
        self.cut_part = CutPart::Sound { lane, stored, draft: moved, edge };
        self.say_cut()
    }

    /// Enter on a taken cut: the whole cut, its picture or its sound is written.
    pub(crate) fn commit_cut(&mut self, marker_id: String, frame: u64) {
        let part = std::mem::take(&mut self.cut_part);
        let join = qnc_program_view::join_segment(&marker_id).map(str::to_string);
        if let CutPart::Sound { lane: 0, stored, draft, .. } = part {
            if stored == draft {
                return self.refresh_view(String::new());
            }
            let Some((_, after)) = self.join_clips(frame) else {
                return self.refresh_view(qnc_ui_text::text("story.msg.a1_no_cut").into());
            };
            let segment_id = after.segment_id.clone();
            self.marker_committing = true;
            return self.write(Operation::SetJoinSound { segment_id, tail_frames: draft.tail, lead_frames: draft.lead });
        }
        // A cut between two clips (user 2026-10-29): whole, the OUT before and the IN after move
        // together (a roll); its picture alone, a picture L before or J after.
        if let Some(segment_id) = join {
            let (Some(cut), Some((before, after))) = (self.cut_frame(&marker_id), self.join_pair(&marker_id)) else {
                return self.refresh_view(String::new());
            };
            let operation = match part {
                CutPart::Whole if frame != cut => Operation::RollJoin { segment_id, frames: frame as i64 - cut as i64 },
                CutPart::Picture => {
                    let (tail, lead) = picture_sides(frame, cut);
                    if (tail, lead) == (before.v_tail_frames, after.v_lead_frames) {
                        return self.refresh_view(String::new());
                    }
                    Operation::SetJoinPicture { segment_id, tail_frames: tail, lead_frames: lead }
                }
                _ => return self.refresh_view(String::new()),
            };
            self.marker_committing = true;
            return self.write(operation);
        }
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
    /// before Enter; `None` while the A2 sound of the cut is not taken or is as stored.
    pub fn cut_sound_draft(&self) -> Option<Vec<(String, (u32, u32))>> {
        let CutPart::Sound { lane: 1, stored, draft, .. } = self.cut_part else {
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

    /// The two clips at the taken cut with their A1 J and L as drafted (segment, (J, L)), for
    /// the program to play before Enter; `None` while the A1 sound is not taken or as stored.
    pub fn join_sound_draft(&self) -> Option<Vec<(String, (u32, u32))>> {
        let CutPart::Sound { lane: 0, stored, draft, .. } = self.cut_part else {
            return None;
        };
        let (_, cut) = self.marker_edit.as_ref()?;
        if stored == draft {
            return None;
        }
        let (before, after) = self.join_clips(*cut)?;
        Some(vec![
            (before.segment_id.clone(), (before.a1_lead_frames, draft.tail)),
            (after.segment_id.clone(), (draft.lead, after.a1_tail_frames)),
        ])
    }

    pub fn cut_sound_taken(&self) -> bool {
        self.marker_edit.is_some() && matches!(self.cut_part, CutPart::Sound { .. })
    }

    /// The sides of a cut's sound on `lane` in the story: the end of the sound before and the
    /// start of the sound after (`None` for a side with nothing); the taken cut with its draft.
    pub(crate) fn cut_sound_frames(&self, lane: u8, marker_id: &str, cut: u64) -> (Option<u64>, Option<u64>) {
        let (before, after) = self.cut_sides(lane, marker_id, cut);
        let sound = match self.cut_part {
            CutPart::Sound { lane: taken, draft, .. } if taken == lane && self.marker_edit.as_ref().map(|(id, _)| id.as_str()) == Some(marker_id) => draft,
            _ => self.stored_cut_sound(lane, marker_id, cut),
        };
        (before.then(|| cut.saturating_add(u64::from(sound.tail))), after.then(|| cut.saturating_sub(u64::from(sound.lead))))
    }

    /// Where the trim playheads of the cut's sound stand: the moving side or both.
    pub(crate) fn cut_sound_trim_frames(&self) -> Vec<u64> {
        let CutPart::Sound { lane, edge, .. } = self.cut_part else {
            return Vec::new();
        };
        let Some((marker_id, cut)) = self.marker_edit.clone() else {
            return Vec::new();
        };
        let (tail, lead) = self.cut_sound_frames(lane, &marker_id, cut);
        match edge {
            SoundEdge::Tail => tail.into_iter().collect(),
            SoundEdge::Lead => lead.into_iter().collect(),
            SoundEdge::Both => lead.into_iter().chain(tail).collect(),
        }
    }

    /// The lane of the taken cut's sound and its side.
    pub(crate) fn taken_sound(&self) -> Option<(u8, SoundEdge)> {
        match self.cut_part {
            CutPart::Sound { lane, edge, .. } => Some((lane, edge)),
            _ => None,
        }
    }

    /// The taken cut's sound for the trim zoom: both sides and the moving ones.
    pub(crate) fn zoom_cut_sound(&self) -> Option<qnc_program_view::ZoomCutSound> {
        let (lane, _) = self.taken_sound()?;
        let (marker_id, cut) = self.marker_edit.clone()?;
        let (tail, lead) = self.cut_sound_frames(lane, &marker_id, cut);
        Some(qnc_program_view::ZoomCutSound { marker_id, lane, lead, tail, moving: self.cut_sound_trim_frames() })
    }

    fn say_cut(&mut self) -> bool {
        let message = match self.cut_part {
            CutPart::Sound { lane, draft, edge, .. } => {
                let side = match edge {
                    SoundEdge::Both => "story.msg.cut_sound_both",
                    SoundEdge::Lead => "story.msg.cut_sound_lead",
                    SoundEdge::Tail => "story.msg.cut_sound_tail",
                };
                let lane = if lane == 0 { "A1" } else { "A2" };
                qnc_ui_text::fill(side, &[("lane", lane), ("j", &draft.lead.to_string()), ("l", &draft.tail.to_string())])
            }
            _ => qnc_ui_text::text("story.msg.cut_picture_taken").into(),
        };
        self.refresh_view(message);
        true
    }
}

/// The picture of a join at `picture` with its sound cut at `cut`, as the picture L of the clip
/// before (later) and the picture J of the clip after (earlier).
fn picture_sides(picture: u64, cut: u64) -> (u32, u32) {
    let offset = picture as i64 - cut as i64;
    (offset.clamp(0, MOST) as u32, (-offset).clamp(0, MOST) as u32)
}
