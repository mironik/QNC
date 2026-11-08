//! Edit of a segment (user rule 2026-10-01): the Edit button of the Segment tab opens
//! the segment's clip in the source timeline with its IN/OUT; I/O move them, Enter
//! writes the new range (`TrimSegment`, the markers follow the picture), Escape drops
//! the edit. Nothing here plays or opens media: the source view is asked to.
//!
//! A segment edge (user 2026-10-10): on the segment panel Ctrl+I takes the start and
//! Ctrl+O the end of the segment under the Wrap playhead (else the selected one). Its
//! clip opens on that edge with the mark taken, so the arrows trim or extend it frame
//! by frame and the monitor shows the new first or last picture, also beyond the
//! segment. Enter writes it the same way, Escape drops it; both go back to Wrap.

use qnc_program_db::Operation;

use crate::ProgramSegments;

/// A segment edge taken from the segment panel.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EdgeTrim {
    start: bool,
    /// The Wrap playhead when the edge was taken (Escape goes back there).
    wrap_from: u64,
    /// The segment's first program frame and length, its source length.
    program_start: u64,
    program_frames: u64,
    source_frames: u64,
}

impl ProgramSegments {
    /// Edit: the source view opens the segment's clip with its IN/OUT.
    pub(crate) fn edit(&mut self, segment_id: String) {
        self.edge = None;
        self.open_for_edit(segment_id, None);
        if self.editing.is_some() {
            self.refresh_view(qnc_ui_text::text("story.msg.segment_edit").into());
        }
    }

    fn open_for_edit(&mut self, segment_id: String, take: Option<bool>) {
        if self.refused(crate::confirm::Take::Segment(segment_id.clone())) {
            return;
        }
        let Some(segment) = self.stored.iter().find(|row| row.segment_id == segment_id && row.active) else {
            return self.refresh_view(qnc_ui_text::text("story.msg.segment_edit_only_active").into());
        };
        self.source_request = Some((segment.clip_id.clone(), segment.in_frame, segment.out_frame, take));
        self.selected = Some(segment_id.clone());
        self.editing = Some(segment_id);
    }

    /// Ctrl+I (`start`) or Ctrl+O on the segment panel: that edge of the segment under
    /// the Wrap playhead, else of the selected one, is taken on the source timeline.
    pub fn take_edge(&mut self, start: bool) -> bool {
        let playhead = self.playhead.unwrap_or(0);
        let row = self
            .view
            .user_segment_at(playhead)
            .or_else(|| self.view.rows.iter().find(|row| !row.lead && self.selected.as_ref() == Some(&row.segment_id)))
            .cloned();
        let Some(row) = row else {
            self.refresh_view(qnc_ui_text::text("story.msg.no_segment_to_trim").into());
            return false;
        };
        self.open_for_edit(row.segment_id.clone(), Some(start));
        if self.editing.is_none() {
            return false;
        }
        self.edge = Some(EdgeTrim {
            start,
            wrap_from: playhead,
            program_start: row.start_frame,
            program_frames: row.duration_frames(),
            source_frames: row.source_out_frame.saturating_sub(row.source_in_frame).max(1),
        });
        self.refresh_view(qnc_ui_text::text(if start { "story.msg.segment_start_taken" } else { "story.msg.segment_end_taken" }).to_string());
        true
    }

    /// The edit pictures of a taken edge on the source timeline (user 2026-10-10): the
    /// start shows the last picture of the program before the segment and the segment's
    /// new first one, the end its new last one and the first picture after the segment.
    /// They follow the IN/OUT on the source timeline of the segment's clip.
    pub(crate) fn edge_stills(&self) -> Vec<qnc_program_view::EditStill> {
        use qnc_program_view::{program_picture, CutPictures, StillPicture};
        let (Some(edge), Some(segment_id)) = (self.edge.as_ref(), self.editing.as_ref()) else {
            return Vec::new();
        };
        let row = self.view.rows.iter().find(|row| &row.segment_id == segment_id);
        let Some(row) = row.filter(|row| self.source.clip_id.as_deref() == Some(row.clip_id.as_str())) else {
            return Vec::new();
        };
        let Some((in_frame, out_frame)) = self.source.marks else {
            return Vec::new();
        };
        let own = |frame: u64| row.kind.has_base_video().then(|| StillPicture { clip_id: row.clip_id.clone(), source_frame: frame });
        let pictures: CutPictures<'_> = if edge.start {
            CutPictures {
                cut: in_frame,
                before: (row.start_frame > 0).then(|| (program_picture(&self.view, row.start_frame - 1), qnc_ui_text::text("story.still.before_segment_last"))),
                after: Some((own(in_frame), qnc_ui_text::text("story.still.segment_new_first"))),
                moving_after: true,
            }
        } else {
            CutPictures {
                cut: out_frame,
                before: Some((own(out_frame.saturating_sub(1)), qnc_ui_text::text("story.still.segment_new_last"))),
                after: (row.end_frame < self.view.total_frames).then(|| (program_picture(&self.view, row.end_frame), qnc_ui_text::text("story.still.after_segment_first"))),
                moving_after: false,
            }
        };
        pictures.stills()
    }

    /// The pictures of the source timeline's trim zoom: a segment edge's, else those of the
    /// taken IN or OUT (user 2026-10-25: as an M marker's, the picture before the cut and the
    /// one after it, the cut between them).
    pub(crate) fn source_stills(&self) -> Vec<qnc_program_view::EditStill> {
        use qnc_program_view::{CutPictures, StillPicture};
        let edge = self.edge_stills();
        if !edge.is_empty() {
            return edge;
        }
        let (Some(clip_id), Some(taken), Some((in_frame, out_frame))) = (self.source.clip_id.as_ref(), self.source.taken, self.source.marks) else {
            return Vec::new();
        };
        let picture = |frame: u64| Some(StillPicture { clip_id: clip_id.clone(), source_frame: frame });
        if taken {
            CutPictures { cut: in_frame, before: (in_frame > 0).then(|| (picture(in_frame - 1), qnc_ui_text::text("story.still.before_in"))), after: Some((picture(in_frame), qnc_ui_text::text("story.still.in_first"))), moving_after: true }.stills()
        } else {
            CutPictures { cut: out_frame, before: Some((picture(out_frame.saturating_sub(1)), qnc_ui_text::text("story.still.out_last"))), after: (out_frame < self.source.duration_frames).then(|| (picture(out_frame), qnc_ui_text::text("story.still.after_out"))), moving_after: false }.stills()
        }
    }

    /// The sound around the cut of the trim zoom (user 2026-10-25: "zoom the audio so the
    /// boundary is seen", a thin line through the cut): a second on each side of it, on the
    /// clip's own frames for a source cut (A1 and A2 as the source timeline hears them), on
    /// the program's for a cut of the story (its segments on A1, its covers on A2).
    pub(crate) fn zoom_sound(&self, view: &qnc_program_view::SegmentsView) -> Option<qnc_program_view::ZoomSound> {
        let cut_of = |stills: &[qnc_program_view::EditStill]| stills.iter().find(|still| still.cut).or(stills.first()).map(|still| still.marker_frame);
        let second = |(num, den): (u64, u64)| (num as f64 / den.max(1) as f64).round().max(1.0) as u64;
        if let Some(cut) = cut_of(&view.source_edit_stills) {
            let clip_id = self.source.clip_id.clone()?;
            let (num, den) = self.source.timebase?;
            let half = second((num.max(0) as u64, den.max(1) as u64));
            let position = self.source.playing.filter(|(program, _, _)| !program).map(|(_, frame, playing)| (frame, playing));
            // The whole clip, so a played window can read its sound ahead.
            let length = self.source.duration_frames.max(cut + 1);
            let lane = |lane: &qnc_lane_choice::Lane| qnc_program_waveform::Placement {
                clip_id: clip_id.clone(),
                channel: lane.channel,
                program_start: 0,
                program_end: length,
                source_in: 0,
                source_out: length,
                level_db: lane.level_db,
                off: lane.off,
                curve: Vec::new(),
            };
            // The clip's kept level curves, on its own axis as the source timeline has them.
            let span = qnc_level_curve::Span { start: 0, end: length, source_in: 0, source_out: length };
            let level_curves = (0..2u8)
                .map(|index| {
                    let keys = self.source.curve(usize::from(index));
                    qnc_program_view::LevelCurve { lane: index, id: index.to_string(), span, placed: qnc_level_curve::place(span, &keys), taken: false }
                })
                .collect();
            let mut lanes = [vec![lane(&self.source.lanes[0])], vec![lane(&self.source.lanes[1])]];
            for (index, placements) in lanes.iter_mut().enumerate() {
                placements[0].curve = self.source.curves[index].clone();
            }
            let axis = qnc_program_view::ZoomAxis { cut, holds: (cut, cut), half, total: self.source.duration_frames, position, follow: &self.zoom_follow, lanes };
            return Some(qnc_program_view::ZoomSound { lane_choices: self.source.lane_choices, level_curves, ..axis.sound() });
        }
        if let Some(zoom) = self.clip_edge_zoom(view) {
            return Some(zoom);
        }
        let cut = cut_of(&view.edit_stills)?;
        let (num, den) = view.timebase?;
        let half = second((u64::from(num), u64::from(den)));
        // The taken cut's sound (user 2026-10-29: its J and L in the zoom): the window holds both
        // sides with a second around them.
        let cut_sound = self.zoom_cut_sound();
        let (earliest, latest) = cut_sound.as_ref().map_or((cut, cut), |sound| {
            (sound.lead.unwrap_or(cut).min(cut), sound.tail.unwrap_or(cut).max(cut))
        });
        let position = self.source.playing.filter(|(program, _, _)| *program).map(|(_, frame, playing)| (frame, playing));
        let (mut segments, mut covers) = view.wave_placements();
        // A taken level curve (docs/98 2.1): its wave is its draft and the curve is drawn over it.
        let level_curve = view.level_curves().into_iter().find(|curve| curve.taken);
        if let Some(curve) = &level_curve {
            let (placements, index) = if curve.lane == 0 {
                (&mut segments, view.rows.iter().position(|row| row.segment_id == curve.id))
            } else {
                (&mut covers, view.covers.iter().position(|cover| cover.cover_id == curve.id))
            };
            if let (Some(placement), Some((_, _, keys))) = (index.and_then(|index| placements.get_mut(index)), &view.level_line) {
                placement.curve = qnc_program_db::LevelKey::pairs(keys);
            }
        }
        let taken_cut = self.marker_edit.as_ref().map(|(id, _)| id.clone());
        let focus_side = self.zoom_focus();
        let axis = qnc_program_view::ZoomAxis { cut, holds: (earliest, latest), half, total: view.total_frames, position, follow: &self.zoom_follow, lanes: [segments, covers] };
        Some(qnc_program_view::ZoomSound { cut_sound, picture_taken: taken_cut.is_some() && matches!(self.cut_part, crate::CutPart::Picture), taken_cut, focus_side, level_curves: view.level_curves(), ..axis.sound() })
    }

    /// The zoom of a taken S-Story clip edge is the zoom of an IN or OUT on the source timeline
    /// (user 2026-11-01: "use the original pieces, the sound does not move"): the clip's own
    /// sound around the drafted edge, on the clip's axis, so the sound runs under the cut as
    /// the arrows and the wheel move it. `None` outside the S-Story.
    fn clip_edge_zoom(&self, view: &qnc_program_view::SegmentsView) -> Option<qnc_program_view::ZoomSound> {
        let trim = self.slot_trim.as_ref().filter(|trim| trim.ripple)?;
        let (segments, _) = view.wave_placements();
        let at = segments.into_iter().find(|at| at.program_start == trim.slot_start && at.program_end == trim.slot_end)?;
        let program = (at.program_end - at.program_start).max(1) as i64;
        let source = at.source_out.saturating_sub(at.source_in) as i64;
        // A later IN is a shorter clip (`frames` below 0); a later OUT a longer one.
        let shift = if trim.start { -trim.frames } else { trim.frames } * source / program;
        let cut = if trim.start { at.source_in } else { at.source_out }.saturating_add_signed(shift);
        let duration = self.clip_frames.get(&at.clip_id).copied().unwrap_or(at.source_out).max(cut);
        let (num, den) = view.timebase?;
        let half = ((f64::from(num) / f64::from(den.max(1))) * source as f64 / program as f64).round().max(1.0) as u64;
        // The player plays the story: its frame inside this clip, on the clip's axis.
        let (slot_start, slot_end, source_in) = (at.program_start, at.program_end, at.source_in);
        let position = self.source.playing.filter(|(program, frame, _)| *program && (slot_start..slot_end).contains(frame)).map(|(_, frame, playing)| (source_in + (frame - slot_start) * source as u64 / program as u64, playing));
        let length = duration.max(cut + 1);
        let whole = qnc_program_waveform::Placement { program_start: 0, program_end: length, source_in: 0, source_out: length, ..at };
        let axis = qnc_program_view::ZoomAxis { cut, holds: (cut, cut), half, total: duration, position, follow: &self.zoom_follow, lanes: [vec![whole], Vec::new()] };
        Some(axis.sound())
    }

    /// Whether a segment edge taken from the segment panel waits for Enter or Escape.
    pub fn trimming_edge(&self) -> bool {
        self.edge.is_some() && self.editing.is_some()
    }

    /// The Wrap frame to go back to after an edge was written or dropped, once.
    pub fn take_wrap_return(&mut self) -> Option<u64> {
        self.wrap_return.take()
    }

    /// The clip, its IN/OUT and the mark to take that the source view should open, once.
    pub fn take_source_request(&mut self) -> Option<(String, u64, u64, Option<bool>)> {
        self.source_request.take()
    }

    /// The story changed under an open edit (Replace, Exclude, Purge, a new segment, Undo,
    /// Redo): the edit ends without a write (live 2026-10-20: Edit then Replace left it
    /// open, and every later Enter went to it).
    pub(crate) fn end_edit(&mut self) {
        self.editing = None;
        self.edge = None;
    }

    /// Escape: the edit is dropped; an edge goes back to where Wrap was.
    pub(crate) fn cancel_edit(&mut self) {
        self.editing = None;
        if let Some(edge) = self.edge.take() {
            self.wrap_return = Some(edge.wrap_from);
        }
        self.refresh_view(qnc_ui_text::text("story.msg.segment_edit_cancelled").into());
    }

    /// Enter while a segment is edited: its new IN/OUT from the source marks of the
    /// same clip. False when no segment is edited.
    pub(crate) fn commit_edit(&mut self) -> bool {
        let Some(segment_id) = self.editing.clone() else {
            return false;
        };
        let clip = self.stored.iter().find(|row| row.segment_id == segment_id).map(|row| row.clip_id.clone());
        let Some((clip_id, (in_frame, out_frame), _)) = self.marked_source() else {
            return true;
        };
        if clip.as_deref() != Some(clip_id.as_str()) {
            self.refresh_view(qnc_ui_text::text("story.msg.segment_edit_own_clip").into());
            return true;
        }
        self.editing = None;
        self.adopt_selection = true;
        if let Some(edge) = self.edge.take() {
            self.wrap_return = Some(edge.wrap_frame(out_frame.saturating_sub(in_frame)));
        }
        self.write(Operation::TrimSegment { segment_id, in_frame, out_frame });
        // The edit is over: say so (live 2026-10-25: the edit hint stayed and it looked open).
        self.refresh_view(qnc_ui_text::text("story.msg.segment_edit_saved").into());
        true
    }
}

impl EdgeTrim {
    /// The Wrap frame on the written edge: the segment's first frame, or its last one
    /// with the new length in story frames (the story may play its source at 2:1 or 1:2).
    fn wrap_frame(&self, source_frames: u64) -> u64 {
        if self.start {
            return self.program_start;
        }
        let frames = (source_frames * self.program_frames + self.source_frames / 2) / self.source_frames;
        self.program_start + frames.max(1) - 1
    }
}

#[cfg(test)]
mod tests {
    use super::EdgeTrim;

    #[test]
    fn the_end_edge_goes_back_to_the_new_last_frame_of_the_segment() {
        let edge = EdgeTrim { start: false, wrap_from: 7, program_start: 100, program_frames: 50, source_frames: 50 };
        assert_eq!(edge.wrap_frame(40), 139, "shortened by ten");
        assert_eq!(edge.wrap_frame(60), 159, "extended by ten");
        let half = EdgeTrim { program_frames: 100, ..edge };
        assert_eq!(half.wrap_frame(60), 219, "a 25 source in a 50 story: two story frames each");
        assert_eq!(EdgeTrim { start: true, ..edge }.wrap_frame(10), 100, "the start stays on the segment's first frame");
    }
}
