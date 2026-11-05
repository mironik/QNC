//! Dynamic trim (docs/98 1.4, user 2026-11-05: "kreni s dinamickim trimom"): with a cut taken
//! (an IN or OUT, a slot edge, an M marker, a J or L side, an edge of a SubSegment part) the
//! play around it repeats two seconds before to two seconds after the cut, and wherever the
//! shuttle's K stops the playhead (JKL in trim, as in every editor) the taken edge goes there.
//! Pure: the owner (the preview) cues and plays, the owner of the cut moves it with the same
//! steps the arrows give it.

pub const MODULE_ID: &str = "qnc.module.trim-play";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The catalog action (contracts/qnc-keyboard-shortcuts.json).
pub const AROUND: &str = "trim_play_around";

/// How far the play goes on each side of the cut.
pub const SECONDS_AROUND: f64 = 2.0;

/// What the player is to do now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Nothing,
    /// Back to this frame (the start of the loop).
    Rewind(u64),
    /// Play from where the rewind landed.
    Play,
    /// The loop is over (the player was paused by someone else).
    Over,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Rewinding,
    Starting,
    Playing,
}

/// The play around a cut, on the axis the player plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrimLoop {
    from: u64,
    to: u64,
    state: State,
}

impl TrimLoop {
    /// Around `cut` at `frames_per_second`; the caller first cues `from()`.
    pub fn around(cut: u64, frames_per_second: f64) -> Self {
        let half = (SECONDS_AROUND * frames_per_second.max(1.0)).round() as u64;
        Self { from: cut.saturating_sub(half), to: cut + half, state: State::Rewinding }
    }

    pub fn from(&self) -> u64 {
        self.from
    }

    pub fn to(&self) -> u64 {
        self.to
    }

    /// One look at the player: the frame it confirmed and whether it plays.
    pub fn step(&mut self, frame: u64, playing: bool) -> Step {
        match self.state {
            State::Rewinding if !playing && frame.abs_diff(self.from) <= 1 => {
                self.state = State::Starting;
                Step::Play
            }
            State::Rewinding => Step::Nothing,
            State::Starting if playing => {
                self.state = State::Playing;
                Step::Nothing
            }
            State::Starting => Step::Nothing,
            State::Playing if !playing => Step::Over,
            State::Playing if frame >= self.to => {
                self.state = State::Rewinding;
                Step::Rewind(self.from)
            }
            State::Playing => Step::Nothing,
        }
    }
}

/// How many frames (and which way) a taken cut goes to where the playhead stopped.
pub fn moved_by(cut: u64, stopped: u64) -> i64 {
    stopped as i64 - cut as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_play_goes_round_the_cut_and_ends_when_paused() {
        let mut around = TrimLoop::around(200, 25.0);
        assert_eq!((around.from(), around.to()), (150, 250), "two seconds on each side");
        assert_eq!(around.step(100, false), Step::Nothing, "still rewinding");
        assert_eq!(around.step(150, false), Step::Play, "landed: play");
        assert_eq!(around.step(150, false), Step::Nothing, "the play starts");
        assert_eq!(around.step(160, true), Step::Nothing);
        assert_eq!(around.step(250, true), Step::Rewind(150), "past the cut by two seconds: back");
        assert_eq!(around.step(150, false), Step::Play);
        assert_eq!(around.step(151, true), Step::Nothing);
        assert_eq!(around.step(180, false), Step::Over, "paused by someone else");
        assert_eq!(TrimLoop::around(10, 25.0).from(), 0, "never before the first frame");
        assert_eq!(moved_by(200, 194), -6);
    }
}
