//! Playback has priority over heavy source work.
//!
//! The rule is the same for every form: while the player prepares, plays or has a play
//! command waiting, heavy work (scans, browsing, reloads) must not run. The component
//! decides *whether* playback holds the priority and *which actions* it blocks; the form
//! supplies the action ids that are heavy for it and does the cancelling.

pub const MODULE_ID: &str = "qnc.module.playback-priority";
pub const VERSION: &str = "0.1.0";

/// The message a form shows when an action is refused for this reason.
pub const MESSAGE: &str = "Zaustavi Broadcast Player prije ove radnje.";

/// What the player is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlaybackState {
    pub play_when_ready: bool,
    pub preparing: bool,
    pub playing: bool,
}

impl PlaybackState {
    /// Playback holds the priority.
    pub fn holds_priority(&self) -> bool {
        self.play_when_ready || self.preparing || self.playing
    }
}

/// The heavy actions of one form.
#[derive(Debug, Clone, Copy)]
pub struct Priority {
    heavy_actions: &'static [&'static str],
}

impl Priority {
    pub const fn new(heavy_actions: &'static [&'static str]) -> Self {
        Self { heavy_actions }
    }

    /// Light actions (for example only changing a flag through a write transport) are
    /// never listed and stay possible during playback.
    pub fn is_heavy(&self, action_id: &str) -> bool {
        self.heavy_actions.contains(&action_id)
    }

    /// Whether `action_id` must be refused in this state.
    pub fn blocks(&self, state: PlaybackState, action_id: &str) -> bool {
        state.holds_priority() && self.is_heavy(action_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIORITY: Priority = Priority::new(&["reload", "browse"]);

    #[test]
    fn an_idle_player_holds_no_priority() {
        assert!(!PlaybackState::default().holds_priority());
        assert!(!PRIORITY.blocks(PlaybackState::default(), "reload"));
    }

    #[test]
    fn preparing_playing_or_a_waiting_play_hold_the_priority() {
        for state in [
            PlaybackState {
                preparing: true,
                ..Default::default()
            },
            PlaybackState {
                playing: true,
                ..Default::default()
            },
            PlaybackState {
                play_when_ready: true,
                ..Default::default()
            },
        ] {
            assert!(state.holds_priority());
            assert!(PRIORITY.blocks(state, "browse"));
        }
    }

    #[test]
    fn light_actions_are_never_blocked() {
        let state = PlaybackState {
            playing: true,
            ..Default::default()
        };
        assert!(!PRIORITY.blocks(state, "toggle_clip"));
    }
}
