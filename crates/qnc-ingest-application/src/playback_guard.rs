//! Ingest only lists which of its actions are heavy; the rule itself is the public
//! `qnc-playback-priority`.

use super::{action_ids, IngestApplication, IngestDispatchResult, IngestViewModel};
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
    action_ids::INGEST_APPROVE_PROXY_POSTERS,
]);

pub(super) struct PlaybackGuard;

impl PlaybackGuard {
    pub(super) fn active(play_when_ready: bool, view: &IngestViewModel) -> bool {
        Self::state(play_when_ready, view).holds_priority()
    }

    pub(super) fn blocks_action(action_id: &str) -> bool {
        PRIORITY.is_heavy(action_id)
    }

    pub(super) fn message() -> &'static str {
        qnc_playback_priority::MESSAGE
    }

    fn state(play_when_ready: bool, view: &IngestViewModel) -> PlaybackState {
        PlaybackState {
            play_when_ready,
            preparing: view.playback.preparing,
            playing: view.playback.playing(),
        }
    }
}

impl IngestApplication {
    pub(crate) fn playback_guard_active(&self) -> bool {
        PlaybackGuard::active(self.preview.play_when_ready(), &self.view)
    }

    pub(crate) fn apply_playback_guard(&mut self) {
        let active = self.playback_guard_active();
        self.set_timeline_artifact_playback_priority(active);
        if active {
            self.cancel_thumbnail_load();
            self.cancel_source_work_for_playback();
        }
    }

    pub(crate) fn cancel_source_work_for_playback(&mut self) {
        if self.selection_session.has_pending_work() {
            self.selection_session.cancel();
            self.view.command_busy = false;
            self.view.message = playback_guard_message().to_string();
        }
        if self.browse.cancel() {
            self.view.browser_busy = false;
            self.view.browser_error = Some(playback_guard_message().to_string());
        }
        self.pending_source = None;
    }

    pub(crate) fn playback_guard_rejected(&mut self) -> IngestDispatchResult {
        let message = playback_guard_message();
        self.view.message = message.to_string();
        let mut result = IngestDispatchResult::rejected(message);
        result.request_repaint = true;
        result
    }

    pub(crate) fn playback_guard_blocks_action(action_id: &str) -> bool {
        PlaybackGuard::blocks_action(action_id)
    }
}

pub(crate) fn playback_guard_message() -> &'static str {
    PlaybackGuard::message()
}
