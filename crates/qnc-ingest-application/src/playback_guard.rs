//! Ingest only lists which of its actions are heavy; the rule itself is the public
//! `qnc-playback-priority`.

use super::{action_ids, IngestApplication, IngestDispatchResult};
use qnc_playback_priority::{PlaybackState, Priority};

/// Heavy source work only. Clip selection (`ingest_clip_toggle`, `ingest_select_all`,
/// `ingest_clear_selection`) is deliberately not listed: it writes just the `selected`
/// flag through the serialized write transport and must stay possible while a preview
/// prepares or plays.
const PRIORITY: Priority = Priority::new(&[
    action_ids::INGEST_RELOAD,
    action_ids::INGEST_SOURCE_KIND_LOCAL,
    action_ids::INGEST_SOURCE_KIND_LAN,
    action_ids::INGEST_SOURCE_KIND_INTERNET,
    action_ids::INGEST_DIR_ROOTS,
    action_ids::INGEST_DIR_UP,
    action_ids::INGEST_DIR_OPEN,
    action_ids::INGEST_DIR_CONFIRM,
]);

impl IngestApplication {
    pub(crate) fn playback_guard_active(&self) -> bool {
        let playback = &self.view.playback;
        let state = PlaybackState { play_when_ready: self.preview.play_when_ready(), preparing: playback.preparing, playing: playback.playing() };
        state.holds_priority()
    }

    pub(crate) fn apply_playback_guard(&mut self) {
        if self.playback_guard_active() {
            self.cancel_thumbnail_load();
            self.cancel_source_work_for_playback();
        }
    }

    pub(crate) fn cancel_source_work_for_playback(&mut self) {
        if self.select.is_busy() {
            self.select.cancel();
            self.view.command_busy = false;
            self.view.message = playback_guard_message().to_string();
        }
        if self.browse.cancel() {
            self.view.browser.busy = false;
            self.view.browser.error = Some(playback_guard_message().to_string());
        }
        self.pending_source = None;
    }

    pub(crate) fn playback_guard_rejected(&mut self) -> IngestDispatchResult {
        self.refuse_shown(playback_guard_message().to_string())
    }

    pub(crate) fn playback_guard_blocks_action(action_id: &str) -> bool {
        PRIORITY.is_heavy(action_id)
    }
}

pub(crate) fn playback_guard_message() -> &'static str {
    qnc_playback_priority::MESSAGE
}
