//! Every key of Sync/B-roll in one place (user 2026-10-18: the Sync keys were spread over
//! the panel router, the source timeline and the preview, and each fix broke another).
//! Space starts a Sync play from IN (or plays and pauses when no Sync is armed); a stopped
//! Sync takes the left/right arrows (its end a frame), Enter writes its cover, Delete drops
//! the draft, wherever the keyboard is; O on the source closes the slot; I arms it.
//!
//! The Sync state and its story writes stay in the program model (`qnc-program-segments`,
//! session math in `qnc-sync-cover`); the program plays in the one preview. This block only
//! decides what a key does while Sync is on. It owns no player, database or form.

use std::ops::ControlFlow;

use qnc_program_segments::{ProgramSegments, SyncSpace};
use qnc_source_preview::{SourcePreview, TransientCover};
use qnc_wrap_session::WrapSession;

pub const MODULE_ID: &str = "qnc.module.sync-session";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// One catalog key: `Break(answer)` when it is Sync's (`answer` as the panel router gives
/// it: `None` for the application), `Continue` when it is not. `on_source`: the source
/// timeline (or its dock buttons) has the keyboard.
pub fn route(
    action_id: &str,
    (preview, wrap, segments): (&mut SourcePreview, &mut WrapSession, &mut ProgramSegments),
    on_source: bool,
) -> ControlFlow<Option<bool>> {
    ControlFlow::Break(Some(match action_id {
        // A Sync (user 2026-10-13): stopped, the arrows move its end; Enter writes the
        // cover, Delete drops the draft, wherever the keyboard is.
        "step_back_frame" | "step_forward_frame" if segments.sync_drafting() && !preview.player_view().playing() => {
            let moved = segments.sync_nudge(if action_id == "step_back_frame" { -1 } else { 1 });
            // The monitor shows the cover's last picture at the new end (user 2026-10-16).
            if let Some(frame) = segments.sync_window_frame().filter(|_| moved) {
                preview.show_program_frame(frame, false);
            }
            moved
        }
        "activate_focused_item" if segments.sync_drafting() => segments.confirm_source(),
        "delete_marker" if segments.sync_drafting() => segments.discard_sync(),
        "play_pause" => space(preview, wrap, segments),
        // O stops a running Sync (user 2026-10-19): the arrows or the mouse still move its
        // end, Enter writes the cover.
        "mark_out" if on_source && segments.sync_drafting() => {
            preview.pause();
            segments.sync_stop();
            true
        }
        _ => return ControlFlow::Continue(()),
    }))
}

/// I pressed on the source: every press arms Sync/B-roll (v5 `mark_in_action`); pressed
/// while the source plays, Sync starts at once (user 2026-10-19: "IN chosen on the fly
/// while it plays, and Sync/B-roll writes the cover").
pub fn armed_by_in(segments: &mut ProgramSegments, preview: &SourcePreview) {
    segments.arm_sync(preview.player_view().playing());
}

/// Once a frame: an IN armed on the fly starts its Sync.
pub fn poll(preview: &mut SourcePreview, wrap: &mut WrapSession, segments: &mut ProgramSegments) {
    if segments.take_sync_flight() {
        space(preview, wrap, segments);
    }
}

/// The OUT of a stopped Sync dragged on the source timeline to `value` (a source frame):
/// its end moves as the arrows move it; false when no stopped Sync takes it.
pub fn dragged(preview: &mut SourcePreview, segments: &mut ProgramSegments, value: Option<i64>) -> bool {
    let Some(source_out) = value.and_then(|frame| u64::try_from(frame).ok()) else {
        return false;
    };
    if !segments.sync_drafting() || preview.player_view().playing() || !segments.sync_drag_end(source_out) {
        return false;
    }
    if let Some(frame) = segments.sync_window_frame() {
        preview.show_program_frame(frame, false);
    }
    true
}

/// Space (v5 `playback_transport_toggle_intent`): a Sync play starts in the Source view once
/// IN armed it, a stopped one plays on from its end, else plain play or pause.
fn space(preview: &mut SourcePreview, wrap: &mut WrapSession, segments: &mut ProgramSegments) -> bool {
    match segments.sync_space(wrap.is_active()) {
        SyncSpace::Start(sync) => {
            wrap.hold(sync.window.0);
            let a2 = segments.kept_lane(1);
            let cover = TransientCover {
                clip_id: sync.clip_id,
                source_in: sync.source_in,
                timebase: sync.timebase,
                a2_source_channel: a2.channel,
                a2_level_db: a2.level_db,
                a2_off: a2.off,
            };
            preview.open_program_window(sync.window, cover)
        }
        SyncSpace::Blocked => true,
        SyncSpace::Play => preview.toggle_play(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_sync_only_space_is_taken_and_it_plays() {
        let (mut preview, mut wrap, mut segments) = (SourcePreview::default(), WrapSession::new(), ProgramSegments::new());
        for key in ["step_back_frame", "step_forward_frame", "activate_focused_item", "delete_marker", "mark_out", "mark_in"] {
            // Without a Sync nothing here is Sync's.
            assert_eq!(route(key, (&mut preview, &mut wrap, &mut segments), true), ControlFlow::Continue(()), "{key}");
        }
        assert!(route("play_pause", (&mut preview, &mut wrap, &mut segments), false).is_break());
    }
}
