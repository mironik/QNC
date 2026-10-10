//! The IN/OUT a clip opens with (user rule 2026-10-01: one block per responsibility;
//! moved out of the editorial application, where a virtual short used it, so an
//! edited segment uses it too). The marks wait until the player has confirmed a
//! picture of that clip on a timeline long enough for them; then they go on the
//! source timeline once. A segment edge taken from the segment panel (user 2026-10-10)
//! opens with that mark taken, so the arrows move it at once. Local UI state only: the
//! database stays the truth.

use qnc_timeline::{TimelineProjection, TimelineSourceMarkFocus};

/// IN/OUT waiting for their clip, and the mark to take (`Some(true)` IN, `Some(false)` OUT).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PendingMarks {
    pending: Option<(String, u64, u64, Option<bool>)>,
}

impl PendingMarks {
    pub fn new() -> Self {
        Self::default()
    }

    /// `clip_id` opens with `marks` (IN, OUT) and, when `take` is given, that mark taken.
    /// Returns the source frame the clip opens at: IN, or the last frame before OUT
    /// when OUT is taken.
    pub fn set(&mut self, clip_id: &str, (in_frame, out_frame): (u64, u64), take: Option<bool>) -> u64 {
        self.pending = Some((clip_id.to_string(), in_frame, out_frame, take));
        match take {
            Some(false) => out_frame.saturating_sub(1).max(in_frame),
            _ => in_frame,
        }
    }

    /// Another choice drops the waiting marks.
    pub fn clear(&mut self) {
        self.pending = None;
    }

    /// Puts the waiting marks on `timeline` when it shows `clip_id`, the player
    /// confirmed a picture and the clip is long enough; then they are done.
    pub fn apply(&mut self, clip_id: Option<&str>, timeline: &mut TimelineProjection) {
        let Some((pending_clip, in_frame, out_frame, take)) = &self.pending else {
            return;
        };
        if clip_id != Some(pending_clip.as_str())
            || timeline.duration_frames < *out_frame
            || timeline.playhead_frame.is_none()
        {
            return;
        }
        // Both lie inside the clip: the timeline is at least as long as the range.
        timeline.source_in_frame = Some(*in_frame);
        timeline.source_out_frame = Some((*out_frame).max(in_frame.saturating_add(1)));
        timeline.source_mark_focus = match take {
            Some(true) => TimelineSourceMarkFocus::In,
            Some(false) => TimelineSourceMarkFocus::Out,
            None => timeline.source_mark_focus,
        };
        self.pending = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_wait_for_their_clip_and_a_confirmed_long_enough_timeline() {
        let mut marks = PendingMarks::new();
        assert_eq!(marks.set("a", (10, 40), None), 10);
        let mut timeline = TimelineProjection { duration_frames: 30, playhead_frame: Some(0), ..Default::default() };
        marks.apply(Some("a"), &mut timeline);
        assert_eq!(timeline.source_in_frame, None, "the clip is not known long enough yet");
        timeline.duration_frames = 100;
        marks.apply(Some("b"), &mut timeline);
        assert_eq!(timeline.source_in_frame, None, "another clip");
        marks.apply(Some("a"), &mut timeline);
        assert_eq!((timeline.source_in_frame, timeline.source_out_frame), (Some(10), Some(40)));
        assert_eq!(timeline.source_mark_focus, TimelineSourceMarkFocus::Playhead, "nothing taken");
        timeline.source_in_frame = Some(5);
        marks.apply(Some("a"), &mut timeline);
        assert_eq!(timeline.source_in_frame, Some(5), "applied once");
    }

    #[test]
    fn a_taken_edge_opens_on_its_frame_with_its_mark_taken() {
        let mut marks = PendingMarks::new();
        assert_eq!(marks.set("a", (10, 40), Some(false)), 39, "the last frame before OUT");
        let mut timeline = TimelineProjection { duration_frames: 100, playhead_frame: Some(39), ..Default::default() };
        marks.apply(Some("a"), &mut timeline);
        assert_eq!(timeline.source_mark_focus, TimelineSourceMarkFocus::Out);
        assert_eq!(marks.set("a", (10, 40), Some(true)), 10);
        marks.apply(Some("a"), &mut timeline);
        assert_eq!(timeline.source_mark_focus, TimelineSourceMarkFocus::In);
    }
}
