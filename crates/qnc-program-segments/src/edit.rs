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
        use qnc_program_view::{program_picture, EditStill, StillPicture};
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
        let mut stills = Vec::new();
        if edge.start {
            if row.start_frame > 0 {
                stills.push(EditStill::new(in_frame, false, program_picture(&self.view, row.start_frame - 1)).labelled(qnc_ui_text::text("story.still.before_segment_last")));
            }
            stills.push(EditStill::new(in_frame, true, own(in_frame)).labelled(qnc_ui_text::text("story.still.segment_new_first")).at_cut());
        } else {
            stills.push(EditStill::new(out_frame, false, own(out_frame.saturating_sub(1))).labelled(qnc_ui_text::text("story.still.segment_new_last")).at_cut());
            if row.end_frame < self.view.total_frames {
                stills.push(EditStill::new(out_frame, true, program_picture(&self.view, row.end_frame)).labelled(qnc_ui_text::text("story.still.after_segment_first")));
            }
        }
        stills
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
