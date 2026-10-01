//! The IN/OUT a clip opens with (user rule 2026-10-01: one block per responsibility;
//! moved out of the editorial application, where a virtual short used it, so an
//! edited segment uses it too). The marks wait until the player has confirmed a
//! picture of that clip on a timeline long enough for them; then they go on the
//! source timeline once. Local UI state only: the database stays the truth.

use qnc_timeline::TimelineProjection;

/// IN/OUT waiting for their clip.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PendingMarks {
    pending: Option<(String, u64, u64)>,
}

impl PendingMarks {
    pub fn new() -> Self {
        Self::default()
    }

    /// `clip_id` opens with `in_frame`..`out_frame` marked.
    pub fn set(&mut self, clip_id: &str, in_frame: u64, out_frame: u64) {
        self.pending = Some((clip_id.to_string(), in_frame, out_frame));
    }

    /// Another choice drops the waiting marks.
    pub fn clear(&mut self) {
        self.pending = None;
    }

    /// Puts the waiting marks on `timeline` when it shows `clip_id`, the player
    /// confirmed a picture and the clip is long enough; then they are done.
    pub fn apply(&mut self, clip_id: Option<&str>, timeline: &mut TimelineProjection) {
        let Some((pending_clip, in_frame, out_frame)) = &self.pending else {
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
        self.pending = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_wait_for_their_clip_and_a_confirmed_long_enough_timeline() {
        let mut marks = PendingMarks::new();
        marks.set("a", 10, 40);
        let mut timeline = TimelineProjection { duration_frames: 30, playhead_frame: Some(0), ..Default::default() };
        marks.apply(Some("a"), &mut timeline);
        assert_eq!(timeline.source_in_frame, None, "the clip is not known long enough yet");
        timeline.duration_frames = 100;
        marks.apply(Some("b"), &mut timeline);
        assert_eq!(timeline.source_in_frame, None, "another clip");
        marks.apply(Some("a"), &mut timeline);
        assert_eq!((timeline.source_in_frame, timeline.source_out_frame), (Some(10), Some(40)));
        timeline.source_in_frame = Some(5);
        marks.apply(Some("a"), &mut timeline);
        assert_eq!(timeline.source_in_frame, Some(5), "applied once");
    }
}
