//! The A1 of the story under a Sync/B-roll (user 2026-10-20: "the story's A1 must be heard,
//! so the user knows how long to hold the clip"; "IN is only a number, no player is opened
//! while one plays"). The A1 under the Sync window does not depend on IN: the bed is
//! prepared on a Broadcast Player of its own as soon as the window is known, and IN only
//! plays it. The source player plays picture and A2 beside it.
//!
//! It owns its player and nothing else: what the bed plays (a sound-only program window)
//! and how it is launched come from the caller. It reads no database and knows no form.

use qnc_player_client::{Action, Launch, Player};

pub const MODULE_ID: &str = "qnc.module.sound-bed";

#[derive(Debug, Default)]
pub struct SoundBed {
    player: Option<Player>,
    window: Option<(u64, u64)>,
    /// Play asked from this frame of the window; sent once the player is there.
    wants: Option<u64>,
    /// Play was sent (the bed plays or is about to).
    playing: bool,
    /// The cue to the asked frame was sent (once).
    cued: bool,
}

impl SoundBed {
    /// The window the bed is prepared for.
    pub fn window(&self) -> Option<(u64, u64)> {
        self.window
    }

    /// Prepares the bed of `window` (its frames start at 0), at frame 0, paused.
    pub fn prepare(&mut self, window: (u64, u64), launch: impl FnOnce() -> Result<Launch, String> + Send + 'static) {
        if self.player.is_none() {
            match Player::new() {
                Ok(player) => self.player = Some(player),
                Err(_) => return,
            }
        }
        let Some(player) = &self.player else {
            return;
        };
        self.window = Some(window);
        (self.wants, self.playing) = (None, false);
        player.prepare_at(0, launch);
    }

    /// Plays from `offset` frames after the window start: at once when it stands there
    /// (prepared at 0, IN on the fly), else after a cue.
    pub fn play_from(&mut self, offset: u64) {
        if self.window.is_none() {
            return;
        }
        (self.wants, self.cued) = (Some(offset), false);
        self.poll();
    }

    /// Once a frame: a play asked waits until the player is prepared and on its frame.
    pub fn poll(&mut self) {
        let (Some(player), Some(offset)) = (&self.player, self.wants) else {
            return;
        };
        let view = player.view();
        if !view.has_confirmed_position() || view.error.is_some() {
            return;
        }
        if view.confirmed_source_frame() != Some(offset) {
            if !self.cued {
                self.cued = player.send(Action::Cue(offset)).is_ok();
            }
            return;
        }
        if !view.playing() && view.can_start_playback() && player.send(Action::TogglePlayPause).is_ok() {
            self.wants = None;
            self.playing = true;
        }
    }

    /// Stops a playing bed (it stays prepared where it stopped); whether it played or was
    /// asked to.
    pub fn stop(&mut self) -> bool {
        let was = self.playing || self.wants.is_some();
        self.wants = None;
        if self.playing {
            self.playing = false;
            if let Some(player) = &self.player {
                if player.view().playing() {
                    let _ = player.send(Action::TogglePlayPause);
                }
            }
        }
        was
    }

    /// The bed is not needed any more: its session ends (the player process stays for the
    /// next Sync).
    pub fn close(&mut self) {
        if let Some(player) = &self.player {
            player.close();
        }
        (self.window, self.wants, self.playing) = (None, None, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bed_without_a_window_neither_plays_nor_stops() {
        let mut bed = SoundBed::default();
        bed.play_from(0);
        assert!(!bed.stop());
        assert_eq!(bed.window(), None);
    }
}
