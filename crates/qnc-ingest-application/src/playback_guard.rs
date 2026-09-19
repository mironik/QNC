use super::{action_ids, IngestViewModel};

pub(super) struct PlaybackGuard;

impl PlaybackGuard {
    pub(super) fn active(play_when_ready: bool, view: &IngestViewModel) -> bool {
        play_when_ready || view.playback.preparing || view.playback.playing()
    }

    /// Heavy source work only. Clip selection (`ingest_clip_toggle`,
    /// `ingest_select_all`, `ingest_clear_selection`) is deliberately not
    /// listed: it writes just the `selected` flag through the serialized write
    /// transport and must stay possible while a preview prepares or plays.
    pub(super) fn blocks_action(action_id: &str) -> bool {
        matches!(
            action_id,
            action_ids::INGEST_RELOAD
                | action_ids::INGEST_SOURCE_KIND_LOCAL
                | action_ids::INGEST_SOURCE_KIND_LAN
                | action_ids::INGEST_SOURCE_KIND_INTERNET
                | action_ids::INGEST_DIR_ROOTS
                | action_ids::INGEST_DIR_UP
                | action_ids::INGEST_DIR_OPEN
                | action_ids::INGEST_DIR_CONFIRM
                | action_ids::INGEST_APPROVE_PROXY_POSTERS
        )
    }

    pub(super) fn message() -> &'static str {
        "Zaustavi Broadcast Player prije ove radnje."
    }
}
