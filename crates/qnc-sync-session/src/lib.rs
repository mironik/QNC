//! Every key of Sync/B-roll in one place (user 2026-10-18: the Sync keys were spread over
//! the panel router, the source timeline and the preview, and each fix broke another).
//! Space starts a Sync play from IN (or plays and pauses when no Sync is armed); a stopped
//! Sync takes the left/right arrows (its end a frame), Enter writes its cover, Delete drops
//! the draft, wherever the keyboard is; O on the source closes the slot; I arms it.
//!
//! The Sync state and its story writes stay in the program model (`qnc-program-segments`,
//! session math in `qnc-sync-cover`); the Sync plays on the source player of the one preview. This block only
//! decides what a key does while Sync is on. It owns no player, database or form.

use std::ops::ControlFlow;

use qnc_program_segments::{ProgramSegments, SyncSpace};
use qnc_source_preview::SourcePreview;
use qnc_wrap_session::WrapSession;

pub const MODULE_ID: &str = "qnc.module.sync-session";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// One catalog key: `Break(answer)` when it is Sync's (`answer` as the panel router gives
/// it: `None` for the application), `Continue` when it is not. `on_source`: a panel
/// of the source has the keyboard (the source timeline, its buttons or the media pool; not
/// the segments, where O is a slot edge).
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
            moved && show_draft_end(preview, segments)
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
    // The draft reached the next M marker or the source OUT: the source player stops on its
    // last picture (the program window used to end there by itself).
    if segments.sync_stopped() && preview.player_view().playing() && !preview.in_program() {
        preview.pause();
        show_draft_end(preview, segments);
    }
    // The story's A1 plays only under a running Sync; the next Sync's is prepared ahead.
    if !segments.sync_drafting() || segments.sync_stopped() {
        preview.stop_sync_bed();
    }
    // The clip session carries the A1 of the next Sync's window (not while one runs); kept
    // while the program plays too, so the next clip opened carries the right one (live
    // 2026-10-20: after Enter the next clip opened with the window of the slot before).
    if !segments.sync_drafting() {
        preview.want_sync_bed(segments.sync_window_ahead());
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
    show_draft_end(preview, segments)
}

/// The monitor shows the cover's last picture at the draft's new end (user 2026-10-16): the
/// source frame before its OUT, on the source player.
fn show_draft_end(preview: &mut SourcePreview, segments: &ProgramSegments) -> bool {
    if let Some((out, _)) = segments.sync_source() {
        preview.cue(out.saturating_sub(1));
    }
    true
}

/// Space (v5 `playback_transport_toggle_intent`): a Sync play starts in the Source view once
/// IN armed it, a stopped one plays on from its end, else plain play or pause.
fn space(preview: &mut SourcePreview, wrap: &mut WrapSession, segments: &mut ProgramSegments) -> bool {
    match segments.sync_space(wrap.is_active()) {
        // The Sync plays on the source player (user 2026-10-20: "the player plays, IN is only
        // the number the cover starts at; no player is opened while one plays"): an IN on the
        // fly lets the source play on; else the source plays from IN (or the draft's end).
        SyncSpace::Start(sync) => {
            wrap.hold(sync.window.0);
            // The story's A1 under the slot, prepared before IN (user 2026-10-20: "the
            // story's A1 must be heard, so the user knows how long to hold the clip").
            preview.play_sync_bed(sync.window, sync.source_in);
            if preview.player_view().playing() && !preview.in_program() {
                return true;
            }
            if preview.in_program() {
                preview.open_at(&sync.clip_id, sync.source_in);
                return preview.toggle_play();
            }
            preview.play_from(sync.source_in)
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
