//! The keys of a source timeline, the same in every form that shows one (user 2026-10-16:
//! the source timeline behaves the same and has the same means everywhere; a new form
//! takes this piece as it is). Play, a frame step, IN/OUT at the playhead, IN or OUT
//! taken (Ctrl+I, Ctrl+O) and moved by the arrows, start/IN/OUT in order (Alt+arrows),
//! Home. It drives the universal source preview and the shown timeline; it knows no form,
//! no story, no database. A form routes a catalog action here only while its source
//! timeline has the keyboard (the panels are the form's own).

use qnc_source_mark_focus::MarkKey;
use qnc_source_preview::SourcePreview;
use qnc_timeline::TimelineProjection;

pub const MODULE_ID: &str = "qnc.module.source-panel";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The catalog actions this piece takes.
pub const ACTIONS: &[&str] = &[
    "play_pause",
    "step_back_frame",
    "step_forward_frame",
    "mark_in",
    "mark_out",
    "select_mark_in",
    "select_mark_out",
    "navigate_prev_object",
    "navigate_next_object",
    "playlist_input_start",
    "go_to_mark_in",
    "clear_focus",
    "monitor_auto",
    "monitor_on",
];

/// What the keyboard took on the source timeline: a mark taken with Ctrl+I/O or the
/// Alt+arrows (the timeline keeps which one).
#[derive(Debug, Clone, Default)]
pub struct SourcePanel {
    mark_taken: bool,
}

impl SourcePanel {
    pub fn new() -> Self {
        Self::default()
    }

    /// IN or OUT is taken: the arrows move it, Escape gives them back to the playhead.
    pub fn mark_taken(&mut self, timeline: &TimelineProjection) -> bool {
        self.mark_taken &= qnc_source_mark_focus::is_taken(timeline);
        self.mark_taken
    }

    /// A mark was taken by someone else (a segment edge opened with its mark taken).
    pub fn set_mark_taken(&mut self, taken: bool) {
        self.mark_taken = taken;
    }

    /// One catalog action of the source timeline. `None`: not one of its actions (or
    /// Escape with nothing taken), the form handles it; `Some(changed)`: taken here.
    pub fn route(&mut self, action_id: &str, preview: &mut SourcePreview, timeline: &mut TimelineProjection) -> Option<bool> {
        Some(match action_id {
            "play_pause" => preview.toggle_play(),
            "step_back_frame" | "step_forward_frame" => {
                let frames = if action_id == "step_back_frame" { -1 } else { 1 };
                if self.mark_taken(timeline) {
                    mark_cue(preview, qnc_source_mark_focus::nudge(timeline, frames))
                } else {
                    preview.step(frames)
                }
            }
            "mark_in" => {
                self.mark_taken = false;
                *timeline = timeline.with_source_in_at_confirmed();
                true
            }
            // OUT stops the play there (user 2026-10-19).
            "mark_out" => {
                self.mark_taken = false;
                *timeline = timeline.with_source_out_at_confirmed();
                preview.pause();
                true
            }
            "select_mark_in" | "select_mark_out" => {
                let key = qnc_source_mark_focus::take(timeline, action_id == "select_mark_in");
                self.mark_taken = key != MarkKey::Refused;
                mark_cue(preview, key)
            }
            "navigate_prev_object" | "navigate_next_object" => {
                let key = qnc_source_mark_focus::adjacent(timeline, action_id == "navigate_prev_object");
                self.mark_taken = qnc_source_mark_focus::is_taken(timeline);
                mark_cue(preview, key)
            }
            "playlist_input_start" => preview.cue(timeline.range_start_frame),
            // Alt+I: the playhead back to IN, the start of the SubClip (without a mark the start
            // of the clip), so Space plays it again (user 2026-10-28).
            "go_to_mark_in" => match in_frame(timeline) {
                Some(frame) => preview.cue(frame),
                None => false,
            },
            // The monitor's Auto | On switch (`qnc-monitor`), the same in every form.
            "monitor_auto" | "monitor_on" => preview.set_monitor_mode(action_id),
            "clear_focus" if self.mark_taken(timeline) => {
                self.mark_taken = false;
                mark_cue(preview, qnc_source_mark_focus::release(timeline))
            }
            _ => return None,
        })
    }
}

/// The source frame of IN the timeline shows (the user's, else the start of the clip).
pub fn in_frame(timeline: &TimelineProjection) -> Option<u64> {
    timeline.visible_source_marks().map(|(in_frame, _)| timeline.range_start_frame + in_frame)
}

/// A taken or moved mark: the playhead goes to it.
pub fn mark_cue(preview: &mut SourcePreview, key: MarkKey) -> bool {
    match key {
        MarkKey::Cue(frame) => preview.cue(frame),
        MarkKey::Changed => true,
        MarkKey::Refused => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marked() -> TimelineProjection {
        TimelineProjection::new(0, 100).with_playhead(10).with_source_in_at_confirmed().with_playhead(40).with_source_out_at_confirmed()
    }

    #[test]
    fn a_taken_in_moves_with_the_arrows_and_escape_gives_them_back() {
        let (mut panel, mut preview, mut timeline) = (SourcePanel::new(), SourcePreview::default(), marked());
        panel.route("select_mark_in", &mut preview, &mut timeline);
        assert!(panel.mark_taken(&timeline));
        panel.route("step_forward_frame", &mut preview, &mut timeline);
        assert_eq!(timeline.source_in_frame, Some(11), "the arrow moved IN");
        assert_eq!(panel.route("clear_focus", &mut preview, &mut timeline), Some(true));
        assert!(!panel.mark_taken(&timeline));
        assert_eq!(panel.route("clear_focus", &mut preview, &mut timeline), None, "nothing taken: the form's Escape");
    }

    #[test]
    fn go_to_mark_in_is_the_users_in_or_the_start_of_the_clip() {
        assert_eq!(in_frame(&marked()), Some(10));
        assert_eq!(in_frame(&TimelineProjection::new(0, 100).with_playhead(25)), Some(0));
    }

    #[test]
    fn i_and_o_mark_at_the_playhead_and_other_actions_are_the_forms() {
        let (mut panel, mut preview) = (SourcePanel::new(), SourcePreview::default());
        let mut timeline = TimelineProjection::new(0, 100).with_playhead(25);
        assert_eq!(panel.route("mark_in", &mut preview, &mut timeline), Some(true));
        assert_eq!(timeline.source_in_frame, Some(25));
        assert_eq!(panel.route("ingest_import_selected", &mut preview, &mut timeline), None);
    }
}
