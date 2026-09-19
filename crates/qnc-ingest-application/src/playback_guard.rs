//! Ingest only lists which of its actions are heavy; the rule itself is the public
//! `qnc-playback-priority`.

use super::{action_ids, IngestViewModel};
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
