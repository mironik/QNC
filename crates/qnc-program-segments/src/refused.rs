//! A command that could not be done is told in a warning over the screen; what was done stays
//! only in the status line (user 2026-11-09: the status line is not noticed).

use crate::ProgramSegments;

/// The story's messages that say a command could not be done.
const REFUSALS: &[&str] = &[
    "story.msg.draft_empty",
    "story.msg.pick_clip",
    "story.msg.marks_not_confirmed",
    "story.msg.db_not_writable",
    "story.msg.no_active_project",
    "story.msg.segment_edit_only_active",
    "story.msg.no_segment_to_trim",
    "story.msg.segment_edit_own_clip",
    "story.msg.no_audio_channels",
    "story.msg.no_adjacent_marker",
    "story.msg.no_adjacent_slot",
    "story.msg.take_before_delete",
    "story.msg.slot_has_no_broll",
    "story.msg.start_marker_locked",
    "story.msg.marker_on_wrap",
    "story.msg.no_wrap_segment",
    "story.msg.pick_clip_and_marks",
    "story.msg.pick_segment",
    "story.msg.db_unavailable",
    "story.msg.bad_broll_reply",
    "story.msg.no_marker_to_move",
    "story.msg.edge_markers_locked",
    "story.msg.clip_at_end",
    "story.msg.no_segment_under_playhead",
    "story.msg.no_slot_under_playhead",
    "story.msg.no_empty_slot",
    "story.msg.no_slot_for_length",
    "story.msg.sync_draft_is_escape",
    "story.msg.purge_needs_excluded",
    "story.msg.edge_slot_fixed",
    "story.msg.slot_to_neighbour",
    "story.msg.last_slot_ends_story",
    "story.msg.cut_part_zoom_only",
    "story.msg.a1_no_cut",
    "story.msg.join_not_deleted",
    "story.msg.slot_one_picture",
    "story.msg.empty_slot_one_picture",
    "story.msg.last_slot_one_picture",
    "story.msg.program_empty",
    "story.msg.broll_needs_segments",
    "story.msg.no_wrap_for_sync_end",
    "story.msg.sync_slot_not_ready",
    "story.msg.sync_slot_has_broll",
    "story.msg.pick_slot_for_broll",
    "story.msg.slot_has_broll",
    "story.msg.marker_not_found",
    "story.msg.marker_between_neighbours",
    "story.msg.sync_no_in",
    "story.msg.sync_rate",
    "story.msg.sync_no_marker",
    "story.msg.sync_not_playing",
    "story.msg.sync_out_after_in",
    "story.msg.join_under_cover",
    "story.msg.marker_no_picture_cut",
    "story.msg.nothing_selected_to_undo",
    "story.msg.sound_assist_keep_off",
    "story.msg.sound_assist_keep_none",
    "story.msg.no_segment_at_playhead",
    "story.msg.no_cover_at_playhead",
    "story.msg.level_no_point",
];

/// Whether `message` says a command could not be done.
pub(crate) fn is_refusal(message: &str) -> bool {
    !message.is_empty() && REFUSALS.iter().any(|code| qnc_ui_text::text(*code) == message)
}

impl ProgramSegments {
    /// The warning over the screen.
    pub(crate) fn warn(&mut self, message: &str) {
        self.alert = Some(qnc_notice::Alert::new(message));
    }

    /// A refusal that is not one of the story's own messages (the database's answer).
    pub(crate) fn refuse_with(&mut self, message: String) {
        self.warn(&message);
        self.refresh_view(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refusal_is_in_the_dictionary_and_what_was_done_is_not_one() {
        for code in REFUSALS.iter().copied() {
            assert_ne!(qnc_ui_text::text(code), code, "{code}");
        }
        assert!(is_refusal(qnc_ui_text::text("story.msg.slot_has_broll")));
        assert!(!is_refusal(qnc_ui_text::text("story.msg.cover_slot_done")));
        assert!(!is_refusal(""));
    }
}
