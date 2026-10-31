//! The IN or OUT of an S-Story element (user 2026-10-31: "this is not the Story"; every element
//! is a SubClip with its own IN and OUT, the edge of one element is not a cut shared with its
//! neighbour). Each Wrap row shows its element's IN and OUT; a right click takes the nearer
//! one, the arrows and the wheel move it a picture of its clip and a drag moves it with the
//! mouse; the element gets shorter or longer and the elements after it move with it. Enter
//! writes it (`TrimSegment`), Escape drops it. There are no locked edges: the first IN and the
//! last OUT are taken as any other. The player plays the stored story; the trim zoom shows the
//! draft's pictures, as at a slot edge.

use qnc_program_db::Operation;
use qnc_program_view::{element_edge, element_pins, program_picture, EditStill, SegmentsView, StillPicture};

use crate::ProgramSegments;

/// The taken edge: its element as stored and how many pictures of its clip it moved.
#[derive(Debug, Clone)]
pub(crate) struct ElementTrim {
    pub(crate) segment_id: String,
    pub(crate) start: bool,
    clip_id: String,
    has_picture: bool,
    row_start: u64,
    row_end: u64,
    source_in: u64,
    source_out: u64,
    /// Pictures of the clip the edge moved (negative: earlier).
    pub(crate) frames: i64,
}

impl ElementTrim {
    /// Program frames a picture of the clip takes (the story may play it at 2:1 or 1:2).
    fn ratio(&self) -> f64 {
        (self.row_end - self.row_start) as f64 / self.source_out.saturating_sub(self.source_in).max(1) as f64
    }

    /// The stored program frame of the taken edge.
    fn edge(&self) -> u64 {
        if self.start { self.row_start } else { self.row_end }
    }

    /// Where the moved edge is drawn on the program.
    pub(crate) fn drawn(&self) -> u64 {
        self.edge().saturating_add_signed((self.frames as f64 * self.ratio()).round() as i64)
    }

    /// The element's IN and OUT in its clip with the draft.
    fn range(&self) -> (u64, u64) {
        self.range_with(self.frames)
    }

    fn range_with(&self, frames: i64) -> (u64, u64) {
        if self.start {
            (self.source_in.saturating_add_signed(frames), self.source_out)
        } else {
            (self.source_in, self.source_out.saturating_add_signed(frames))
        }
    }

    /// Whether `frames` keeps at least one picture and stays within the clip (`length`: its
    /// pictures when known).
    fn fits(&self, frames: i64, length: Option<u64>) -> bool {
        let (in_frame, out_frame) = (self.source_in as i64, self.source_out as i64);
        if self.start {
            in_frame + frames >= 0 && in_frame + frames < out_frame
        } else {
            out_frame + frames > in_frame && length.is_none_or(|length| out_frame + frames <= length as i64)
        }
    }
}

impl ProgramSegments {
    /// The story is an S-Story (its own draft story, no lead-in or lead-out).
    pub(crate) fn is_s_story(&self) -> bool {
        self.target.as_ref().is_some_and(qnc_db_broker::ProjectDbTarget::is_draft)
    }

    /// A right click on an element's IN or OUT; false for any other pin.
    pub(crate) fn take_element_edge(&mut self, marker_id: &str) -> bool {
        let Some((segment_id, start)) = element_edge(marker_id) else {
            return false;
        };
        let Some(row) = self.view.rows.iter().find(|row| row.segment_id == segment_id && !row.lead).cloned() else {
            return false;
        };
        self.let_go_edits(false);
        self.selected = Some(row.segment_id.clone());
        self.element_trim = Some(ElementTrim {
            segment_id: row.segment_id,
            start,
            clip_id: row.clip_id,
            has_picture: row.kind.has_base_video(),
            row_start: row.start_frame,
            row_end: row.end_frame,
            source_in: row.source_in_frame,
            source_out: row.source_out_frame,
            frames: 0,
        });
        // The monitor shows the cut at once: the element's first or last picture.
        self.cue_program(if start { row.start_frame } else { row.end_frame.saturating_sub(1) });
        self.refresh_view(Self::element_message(start));
        true
    }

    fn element_message(start: bool) -> String {
        qnc_ui_text::text(if start { "story.msg.element_in_taken" } else { "story.msg.element_out_taken" }).to_string()
    }

    /// An arrow, Enter or Escape while an element edge is taken; `None` for any other key.
    pub(crate) fn element_key(&mut self, action_id: &str) -> Option<bool> {
        self.element_trim.as_ref()?;
        match action_id {
            "step_back_frame" => Some(self.nudge_element(-1)),
            "step_forward_frame" => Some(self.nudge_element(1)),
            "activate_focused_item" => Some(self.commit_element_trim()),
            "clear_focus" => {
                self.element_trim = None;
                self.refresh_view(qnc_ui_text::text("story.msg.element_trim_cancelled").into());
                Some(true)
            }
            _ => None,
        }
    }

    /// The taken edge `pictures` of its clip later (negative: earlier).
    pub(crate) fn nudge_element(&mut self, pictures: i64) -> bool {
        let Some(trim) = self.element_trim.as_ref() else {
            return false;
        };
        let frames = trim.frames + pictures;
        self.set_element_frames(frames)
    }

    /// The taken edge dragged by the mouse to a program frame.
    pub(crate) fn drag_element_edge(&mut self, marker_id: &str, frame: u64) -> bool {
        let Some(trim) = self.element_trim.as_ref().filter(|trim| element_edge(marker_id) == Some((trim.segment_id.as_str(), trim.start))) else {
            return false;
        };
        let frames = ((frame as f64 - trim.edge() as f64) / trim.ratio()).round() as i64;
        self.set_element_frames(frames)
    }

    fn set_element_frames(&mut self, frames: i64) -> bool {
        let Some(trim) = self.element_trim.clone() else {
            return false;
        };
        if !trim.fits(frames, self.clip_frames.get(&trim.clip_id).copied()) {
            let (in_frame, out_frame) = trim.range_with(frames);
            let text = if in_frame >= out_frame { "story.msg.element_one_picture" } else { "story.msg.clip_at_end" };
            self.refresh_view(qnc_ui_text::text(text).into());
            return true;
        }
        self.element_trim = Some(ElementTrim { frames, ..trim.clone() });
        self.refresh_view(Self::element_message(trim.start));
        true
    }

    /// Enter: the element's new IN or OUT is written; false when no element edge is taken.
    pub(crate) fn commit_element_trim(&mut self) -> bool {
        let Some(trim) = self.element_trim.take() else {
            return false;
        };
        if trim.frames == 0 {
            self.refresh_view(String::new());
            return true;
        }
        let (in_frame, out_frame) = trim.range();
        self.marker_committing = true;
        self.write(Operation::TrimSegment { segment_id: trim.segment_id.clone(), in_frame, out_frame });
        // The Wrap playhead on the new edge: the monitor shows the written cut.
        let length = ((out_frame - in_frame) as f64 * trim.ratio()).round() as u64;
        self.cue_program(if trim.start { trim.row_start } else { trim.row_start + length.saturating_sub(1) });
        true
    }

    /// What the view shows of the S-Story: every element's IN and OUT in the place of the
    /// markers and cuts, the taken one where it is going, and its pictures in the trim zoom.
    pub(crate) fn element_view(&self, view: &mut SegmentsView) {
        if !self.is_s_story() {
            return;
        }
        view.markers.clear();
        view.joins.clear();
        view.element_edges = element_pins(view);
        view.zoom_element = match &self.zoom_slot {
            Some(crate::zoom::ZoomOn::Segment(id)) => Some(id.clone()),
            _ => None,
        };
        let Some(trim) = &self.element_trim else {
            return;
        };
        let taken = format!("{}{}", if trim.start { qnc_program_view::ELEMENT_IN } else { qnc_program_view::ELEMENT_OUT }, trim.segment_id);
        for pin in view.element_edges.iter_mut().filter(|pin| pin.marker_id == taken) {
            pin.taken = true;
            pin.frame = trim.drawn();
        }
        view.trim_frames = vec![trim.drawn()];
        let (in_frame, out_frame) = trim.range();
        let own = |frame: u64| trim.has_picture.then(|| StillPicture { clip_id: trim.clip_id.clone(), source_frame: frame });
        let drawn = trim.drawn();
        view.edit_stills = if trim.start {
            let mut stills = Vec::new();
            if trim.row_start > 0 {
                stills.push(EditStill::new(drawn, false, program_picture(view, trim.row_start - 1)).labelled(qnc_ui_text::text("story.still.before_segment_last")));
            }
            stills.push(EditStill::new(drawn, true, own(in_frame)).labelled(qnc_ui_text::text("story.still.segment_new_first")).at_cut());
            stills
        } else {
            let mut stills = vec![EditStill::new(drawn, false, own(out_frame.saturating_sub(1))).labelled(qnc_ui_text::text("story.still.segment_new_last")).at_cut()];
            if trim.row_end < view.total_frames {
                stills.push(EditStill::new(drawn, true, program_picture(view, trim.row_end)).labelled(qnc_ui_text::text("story.still.after_segment_first")));
            }
            stills
        };
    }
}
