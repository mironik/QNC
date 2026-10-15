//! A1 and A2 of the marked source (QNC public piece, split out of `qnc-program-segments`,
//! audit 2026-10-10): which source channel is heard on A1 (Talking Head, Voice over) and
//! on A2 (covers), each its own choice (user rules 2026-09-30, 2026-10-01: A1 on channel 1,
//! A2 on channel 2 of a new clip, channel 1 when the clip has one).
//!
//! By keyboard (user rule 2026-10-01, no mouse needed): Ctrl+1 or Ctrl+2 takes the lane
//! with its channel as draft. While it is taken the lane's height is expanded over the
//! video row and the channels stand in a column (user 2026-10-14): up/down or a click on
//! a channel move the draft, Enter keeps it and closes, Escape drops it and closes.
//!
//! Level 0 (user 2026-10-14): under the channels the column has a "0": the lane keeps its
//! channel and is heard at level 0, until a channel is chosen again. In a choice, draft
//! or column the place after the last channel (`count`) is that level 0.

use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
pub struct LaneChoice {
    a1: u16,
    a2: u16,
    /// A1 and A2 at level 0: the channel is kept, nothing of it is heard.
    silent: [bool; 2],
    /// How many audio channels the marked clip has (its stored record).
    channels: Option<u16>,
    taken: Option<(u8, u16)>,
    wave_zoom: [u8; 2],
    /// When Enter last kept a lane channel.
    committed: Option<Instant>,
}

impl LaneChoice {
    /// Another clip is marked: A1 on channel 1, A2 on channel 2; nothing taken.
    pub fn new_clip(&mut self) {
        (self.a1, self.a2, self.taken, self.silent) = (0, 1, None, [false, false]);
    }

    /// The channel count of the marked clip; a chosen channel it does not have goes back
    /// to channel 1 (A2: to channel 2 when there is one). False when nothing changed.
    pub fn set_channels(&mut self, count: Option<u16>) -> bool {
        if self.channels == count {
            return false;
        }
        self.channels = count;
        if count.is_none_or(|count| self.a1 >= count) {
            self.a1 = 0;
        }
        if let Some(count) = count.filter(|count| self.a2 >= *count) {
            self.a2 = u16::from(count >= 2);
        }
        true
    }

    /// The channel picker chose the channel heard on A1 (lane 0) or A2, or level 0 (the
    /// place after the last channel: the channel is kept); the lane is let go.
    pub fn choose(&mut self, lane: u8, channel: u16) -> bool {
        let Some(count) = self.channels.filter(|count| channel <= *count) else {
            return false;
        };
        let slot = usize::from(lane.min(1));
        if channel == count {
            self.silent[slot] = true;
        } else {
            if lane == 0 {
                self.a1 = channel;
            } else {
                self.a2 = channel;
            }
            self.silent[slot] = false;
        }
        self.taken = None;
        true
    }

    /// The source channels kept on A1 and A2 (zero based), whatever their level.
    pub fn heard(&self) -> (u16, u16) {
        (self.a1, self.a2)
    }

    /// The source channels the preview plays on A1 and A2 now and their levels (1 or 0):
    /// the draft of a taken lane is heard at once, before Enter keeps it (user 2026-10-14).
    pub fn audible(&self) -> ((u16, u16), (f32, f32)) {
        let level = |silent: bool| if silent { 0.0 } else { 1.0 };
        let mut channels = [self.a1, self.a2];
        let mut levels = [level(self.silent[0]), level(self.silent[1])];
        if let (Some((lane, draft)), Some(count)) = (self.taken, self.channels) {
            let slot = usize::from(lane.min(1));
            if draft >= count {
                levels[slot] = 0.0;
            } else {
                (channels[slot], levels[slot]) = (draft, 1.0);
            }
        }
        ((channels[0], channels[1]), (levels[0], levels[1]))
    }

    /// The place chosen on A1 (lane 0) or A2 (the channel, or `count` at level 0) and the
    /// clip's channel count, when known.
    pub fn choice(&self, lane: u8) -> Option<(u16, u16)> {
        let channel = if lane == 0 { self.a1 } else { self.a2 };
        let silent = self.silent[usize::from(lane.min(1))];
        self.channels.filter(|count| *count > 0).map(|count| (if silent { count } else { channel }, count))
    }

    /// Ctrl+1 (lane 0) or Ctrl+2: the lane is taken with its channel as draft. False
    /// when the clip has no known audio channels.
    pub fn take(&mut self, lane: u8) -> bool {
        let Some(count) = self.channels.filter(|count| *count > 0) else {
            return false;
        };
        let current = if self.silent[usize::from(lane.min(1))] {
            count
        } else {
            (if lane == 0 { self.a1 } else { self.a2 }).min(count - 1)
        };
        self.taken = Some((lane, current));
        true
    }

    /// The taken lane and its draft channel.
    pub fn taken(&self) -> Option<(u8, u16)> {
        self.taken
    }

    /// Up/down: the draft of the taken lane, within the clip's channels and level 0.
    pub fn draft(&mut self, delta: i64) -> bool {
        let (Some((lane, draft)), Some(count)) = (self.taken, self.channels) else {
            return false;
        };
        let next = (i64::from(draft) + delta).clamp(0, i64::from(count)) as u16;
        self.taken = Some((lane, next));
        true
    }

    /// A click on a channel of the taken lane's column: it becomes the draft, the lane
    /// stays taken and expanded until Enter or Escape (user 2026-10-14). False when that
    /// lane is not taken or the clip has no such channel.
    pub fn set_draft(&mut self, lane: u8, channel: u16) -> bool {
        match (self.taken, self.channels) {
            (Some((taken, _)), Some(count)) if taken == lane && channel <= count => {
                self.taken = Some((lane, channel));
                true
            }
            _ => false,
        }
    }

    /// Down with no lane taken: the open large wave closes.
    pub fn close_wave(&mut self) -> bool {
        if self.wave_zoom == [0, 0] {
            return false;
        }
        self.wave_zoom = [0, 0];
        true
    }

    /// The display level of the A1 and A2 wave over the video row (0: not shown).
    pub fn wave_zoom(&self) -> [u8; 2] {
        self.wave_zoom
    }

    /// Enter: the draft becomes the lane's channel; the lane is let go.
    pub fn commit(&mut self) -> bool {
        let Some((lane, draft)) = self.taken.take() else {
            return false;
        };
        self.mark_committed();
        self.choose(lane, draft);
        true
    }

    /// Escape: the lane is let go, its channel unchanged.
    pub fn release(&mut self) -> bool {
        self.taken.take().is_some()
    }

    /// An Enter confirmed something; its other catalog actions are its.
    pub fn mark_committed(&mut self) {
        self.committed = Some(Instant::now());
    }

    /// Whether an Enter confirmed something within `within`.
    pub fn committed_within(&self, within: Duration) -> bool {
        self.committed.is_some_and(|at| at.elapsed() < within)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_clip_hears_channel_1_on_a1_and_2_on_a2_and_a_lane_drafts_within_its_channels() {
        let mut lanes = LaneChoice::default();
        lanes.new_clip();
        lanes.set_channels(Some(4));
        assert_eq!((lanes.choice(0), lanes.choice(1)), (Some((0, 4)), Some((1, 4))));
        assert!(lanes.take(1));
        lanes.draft(5);
        assert_eq!(lanes.taken(), Some((1, 4)), "never past level 0, after the last channel");
        lanes.draft(-1);
        assert!(lanes.set_draft(1, 2), "a click drafts, the lane stays taken");
        assert!(!lanes.set_draft(0, 1), "not on the lane that is not taken");
        assert!(!lanes.set_draft(1, 5), "not on a channel the clip does not have");
        assert_eq!(lanes.taken(), Some((1, 2)));
        assert_eq!((lanes.audible(), lanes.heard()), (((0, 2), (1.0, 1.0)), (0, 1)), "the draft is heard, not kept");
        lanes.draft(1);
        assert!(lanes.commit());
        assert_eq!(lanes.heard(), (0, 3));
        lanes.set_channels(Some(1));
        assert_eq!(lanes.heard(), (0, 0), "a one channel clip hears channel 1 on A2");
        lanes.set_channels(None);
        assert!(!lanes.take(0), "no channels known: nothing to take");
    }

    #[test]
    fn level_0_keeps_the_channel_and_is_heard_as_nothing_until_a_channel_is_chosen() {
        let mut lanes = LaneChoice::default();
        lanes.new_clip();
        lanes.set_channels(Some(4));
        lanes.take(0);
        lanes.draft(9);
        assert_eq!(lanes.taken(), Some((0, 4)), "after the last channel is level 0");
        assert_eq!(lanes.audible(), ((0, 1), (0.0, 1.0)), "heard at level 0 before Enter");
        assert!(lanes.commit());
        assert_eq!((lanes.choice(0), lanes.heard()), (Some((4, 4)), (0, 1)), "the channel is kept");
        assert_eq!(lanes.audible(), ((0, 1), (0.0, 1.0)));
        lanes.take(0);
        assert_eq!(lanes.taken(), Some((0, 4)), "taken again at level 0");
        lanes.draft(-2);
        lanes.commit();
        assert_eq!((lanes.choice(0), lanes.audible()), (Some((2, 4)), ((2, 1), (1.0, 1.0))));
        assert!(lanes.choose(1, 4), "a click on the 0");
        assert_eq!(lanes.audible(), ((2, 1), (1.0, 0.0)));
        lanes.new_clip();
        assert_eq!(lanes.audible().1, (1.0, 1.0), "a new clip is heard");
    }
}
