//! The JKL shuttle (docs/98 1.1, user 2026-11-04; every editor has it): L plays forward, L
//! again 2x, 4x, 8x; J plays backward 1x, 2x, 4x, 8x; K stops. 1x forward is the player's own
//! play (picture and sound); every other speed moves the playhead from where the shuttle
//! started at that speed, one cue after another (picture only), so the player needs nothing of
//! its own for it. Pure: the owner (the preview) plays, pauses and cues.

use std::time::{Duration, Instant};

pub const MODULE_ID: &str = "qnc.module.shuttle";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The catalog actions (contracts/qnc-keyboard-shortcuts.json).
pub const BACK: &str = "shuttle_back";
pub const STOP: &str = "shuttle_stop";
pub const FORWARD: &str = "shuttle_forward";

const FASTEST: i32 = 8;

/// What the player is to do when a key changed the speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Do {
    /// Play at 1x forward (the player's own play).
    Play,
    /// Pause: stopped, or the shuttle moves the playhead itself from now on.
    Pause,
}

#[derive(Debug, Clone, Default)]
pub struct Shuttle {
    /// Frames per frame of play: 0 stopped, 1 the player's play, else the shuttle's.
    rate: i32,
    /// Where and when a speed of the shuttle's own started.
    from: Option<(Instant, u64)>,
}

impl Shuttle {
    pub fn new() -> Self {
        Self::default()
    }

    /// The speed now: 0 stopped, negative backward.
    pub fn rate(&self) -> i32 {
        self.rate
    }

    /// Whether the shuttle moves the playhead itself (any speed but stop and 1x forward).
    pub fn running(&self) -> bool {
        self.rate != 0 && self.rate != 1
    }

    /// J, K or L at `now` with the playhead on `frame`; `None` for another action.
    pub fn press(&mut self, action_id: &str, now: Instant, frame: u64) -> Option<Do> {
        self.rate = match action_id {
            FORWARD if self.rate <= 0 => 1,
            FORWARD => (self.rate * 2).min(FASTEST),
            BACK if self.rate >= 0 => -1,
            BACK => (self.rate * 2).max(-FASTEST),
            STOP => 0,
            _ => return None,
        };
        // Each speed counts from where the playhead is when it changes.
        self.from = self.running().then_some((now, frame));
        Some(if self.rate == 1 { Do::Play } else { Do::Pause })
    }

    /// Any other transport (Space, a click, a cue) stops the shuttle's own movement.
    pub fn stop(&mut self) {
        (self.rate, self.from) = (0, None);
    }

    /// The frame the playhead is to show at `now` (`frame_interval`: one frame of the source),
    /// never before the first frame; `None` when the shuttle does not move it.
    pub fn frame_at(&self, now: Instant, frame_interval: Duration) -> Option<u64> {
        let (started, from) = self.from.filter(|_| self.running())?;
        let interval = frame_interval.as_secs_f64();
        if interval <= 0.0 {
            return None;
        }
        let frames = (now.duration_since(started).as_secs_f64() / interval * f64::from(self.rate)).round() as i64;
        Some((from as i64 + frames).max(0) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_and_j_speed_up_k_stops_and_the_playhead_runs_at_the_speed() {
        let (mut shuttle, start) = (Shuttle::new(), Instant::now());
        let frame = Duration::from_millis(40);
        assert_eq!(shuttle.press(FORWARD, start, 100), Some(Do::Play), "L: the player's own play");
        assert!(!shuttle.running());
        assert_eq!(shuttle.press(FORWARD, start, 100), Some(Do::Pause), "L again: 2x, the shuttle's");
        assert_eq!(shuttle.rate(), 2);
        assert_eq!(shuttle.frame_at(start + Duration::from_secs(1), frame), Some(150), "25 frames a second at 2x");
        shuttle.press(FORWARD, start, 100);
        shuttle.press(FORWARD, start, 100);
        shuttle.press(FORWARD, start, 100);
        assert_eq!(shuttle.rate(), 8, "never past 8x");
        assert_eq!(shuttle.press(BACK, start, 300), Some(Do::Pause), "J: backward at 1x");
        assert_eq!(shuttle.rate(), -1);
        assert_eq!(shuttle.frame_at(start + Duration::from_secs(2), frame), Some(250));
        assert_eq!(shuttle.frame_at(start + Duration::from_secs(60), frame), Some(0), "never before the first frame");
        assert_eq!(shuttle.press(STOP, start, 0), Some(Do::Pause));
        assert_eq!(shuttle.frame_at(start + Duration::from_secs(1), frame), None);
        assert_eq!(shuttle.press("play_pause", start, 0), None);
    }
}
