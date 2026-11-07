//! The level curve of A1 or A2 on the source timeline (user 2026-11-07: "the level line must be
//! universal, whatever the form"): the same tool as on a Wrap row (`qnc-level-curve`). Ctrl+Shift+1
//! / Ctrl+Shift+2 or a right click on the line takes it, the line of the playhead is the point
//! controlled (up/down, Ctrl+arrows and Ctrl+wheel a dB, Delete removes it), every step is heard
//! at once in the clip's player, Enter keeps it for the clip in the project database (with its
//! A1/A2 sound, `qnc-clip-sound`), Esc gives it up. A new segment or cover of the clip takes it.

use qnc_level_curve::{LevelLine, Span};
use qnc_program_db::LevelKey;
use qnc_source_preview::SourcePreview;
use qnc_timeline_model::TimelineProjection;

use crate::SourceTimeline;

fn keys_of(pairs: &[(u64, i16)]) -> Vec<LevelKey> {
    pairs.iter().map(|(frame, db10)| LevelKey { frame: *frame, db10: *db10 }).collect()
}

impl SourceTimeline {
    /// The clip's whole length as the span of its curve: the source timeline is the clip.
    fn clip_span(timeline: &TimelineProjection) -> Span {
        let (start, duration) = (timeline.range_start(), timeline.duration_frames().max(1));
        Span { start, end: start + duration, source_in: start, source_out: start + duration }
    }

    /// The level curve of A1 or A2 is taken: its keys are its.
    pub fn level_taken(&self) -> bool {
        self.level.is_some()
    }

    /// The level keys; `None` for a key that is not the curve's.
    pub(crate) fn level_key(&mut self, action_id: &str, preview: &mut SourcePreview, timeline: &TimelineProjection) -> Option<bool> {
        let at = timeline.playhead_frame.unwrap_or(0);
        // A click on the A1/A2 button of the taken curve's lane keeps it and closes the zoom, as
        // Enter (user 2026-11-07); the other lane taken lets the curve go (one thing at a time).
        let same = |line: &LevelLine| (action_id == "select_audio_a1" && line.lane == 0) || (action_id == "select_audio_a2" && line.lane == 1);
        if self.level.as_ref().is_some_and(same) {
            return self.level_key("activate_focused_item", preview, timeline);
        }
        if matches!(action_id, "select_audio_a1" | "select_audio_a2") && self.level.take().is_some() {
            self.message.clear();
            self.hear_curves(preview);
            return None;
        }
        if matches!(action_id, "select_level_a1" | "select_level_a2") {
            let lane = u8::from(action_id == "select_level_a2");
            self.clip.as_ref()?;
            self.lanes.release();
            self.level = Some(LevelLine::new(lane, lane.to_string(), Self::clip_span(timeline), keys_of(&self.curves[usize::from(lane)])));
            self.say_level(at);
            return Some(true);
        }
        let line = self.level.as_mut()?;
        let changed = match action_id {
            "step_prev_part" => line.nudge(at, 1),
            "step_next_part" => line.nudge(at, -1),
            "delete_marker" => line.remove_at(at),
            "activate_focused_item" => {
                let lane = usize::from(line.lane);
                if let Some(keys) = line.keep() {
                    self.curves[lane] = LevelKey::pairs(&keys);
                    self.keep_sound(preview);
                }
                self.level = None;
                self.message.clear();
                self.hear_curves(preview);
                return Some(true);
            }
            "clear_focus" => {
                self.level = None;
                self.message.clear();
                self.hear_curves(preview);
                return Some(true);
            }
            key => match qnc_lane_choice::LaneKey::of(key) {
                Some(qnc_lane_choice::LaneKey::Level(steps)) => line.nudge(at, i16::from(steps)),
                _ => return None,
            },
        };
        if changed {
            self.hear_curves(preview);
        }
        self.say_level(at);
        Some(true)
    }

    /// The curves the clip is heard with now: the kept ones, the taken one as drafted.
    pub(crate) fn heard_curves(&self) -> [Vec<(u64, i16)>; 2] {
        let mut curves = self.curves.clone();
        if let Some(line) = &self.level {
            curves[usize::from(line.lane)] = LevelKey::pairs(line.draft());
        }
        curves
    }

    pub(crate) fn hear_curves(&self, preview: &mut SourcePreview) -> bool {
        preview.hear_curves(self.heard_curves())
    }

    /// The curves drawn in the lanes (the taken one as drafted).
    pub(crate) fn drawn_curves(&self, timeline: &TimelineProjection) -> Vec<qnc_program_view::LevelCurve> {
        let span = Self::clip_span(timeline);
        let taken = self.level.as_ref().map(|line| line.lane);
        // Without a clip there is no line; with one both lanes have theirs (flat 0 dB without
        // points), always drawn so a right click can take it.
        if self.clip.is_none() {
            return Vec::new();
        }
        self.heard_curves()
            .iter()
            .enumerate()
            .map(|(lane, pairs)| {
                let lane = lane as u8;
                qnc_program_view::LevelCurve { lane, id: lane.to_string(), span, placed: qnc_level_curve::place(span, &keys_of(pairs)), taken: taken == Some(lane) }
            })
            .collect()
    }

    fn say_level(&mut self, frame: u64) {
        let Some(line) = &self.level else { return };
        let level = format!("{:+.1}", f64::from(line.level_at(frame)) / 10.0);
        let key = if line.point_at(frame).is_some() { "story.msg.level_point" } else { "story.msg.level_line" };
        let lane = if line.lane == 0 { "A1" } else { "A2" };
        self.message = qnc_ui_text::fill(key, &[("lane", lane), ("level", &level)]).into();
    }
}
