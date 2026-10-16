//! A1 and A2 of the marked source (QNC public piece, split out of `qnc-program-segments`,
//! audit 2026-10-10): which source channel is heard on A1 (Talking Head, Voice over) and
//! on A2 (covers), each its own choice (user rules 2026-09-30, 2026-10-01: A1 on channel 1,
//! A2 on channel 2 of a new clip, channel 1 when the clip has one).
//!
//! By keyboard (user rule 2026-10-01, no mouse needed): Ctrl+1 or Ctrl+2 takes the lane
//! with its channel as draft. While it is taken the lane's height is expanded over the
//! video row and the channels stand in a column (user 2026-10-14): up/down or a click on
//! a channel move the draft channel, left turns the lane off (level 0: the channel is kept,
//! nothing of it is heard), right turns it on, Ctrl+left/right lower or raise its level by
//! 1 dB (user 2026-10-15). The draft is heard at once; Enter keeps it and closes, Escape
//! drops it and closes.

use std::time::{Duration, Instant};

/// The lowest and highest level of a lane, in dB (the player takes up to four times).
pub const MIN_LEVEL_DB: i8 = -40;
pub const MAX_LEVEL_DB: i8 = 12;

/// What one lane plays: its channel, whether it is off (level 0) and its level in dB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lane {
    pub channel: u16,
    pub off: bool,
    pub level_db: i8,
}

impl Lane {
    const fn on(channel: u16) -> Self {
        Self { channel, off: false, level_db: 0 }
    }

    /// The level as a gain: 0 when off, else 10^(dB/20).
    pub fn gain(&self) -> f32 {
        if self.off { 0.0 } else { 10f32.powf(f32::from(self.level_db) / 20.0) }
    }
}

impl Default for Lane {
    fn default() -> Self {
        Self::on(0)
    }
}

#[derive(Debug, Clone, Default)]
pub struct LaneChoice {
    lanes: [Lane; 2],
    /// How many audio channels the marked clip has (its stored record).
    channels: Option<u16>,
    /// The taken lane (0: A1) and its draft.
    taken: Option<(u8, Lane)>,
    wave_zoom: [u8; 2],
    /// When Enter last kept a lane channel.
    committed: Option<Instant>,
}

fn slot(lane: u8) -> usize {
    usize::from(lane.min(1))
}

impl LaneChoice {
    /// Another clip is marked: A1 on channel 1, A2 on channel 2, both on at their recorded
    /// level; nothing taken.
    pub fn new_clip(&mut self) {
        (self.lanes, self.taken) = ([Lane::on(0), Lane::on(1)], None);
    }

    /// The channel count of the marked clip; a chosen channel it does not have goes back
    /// to channel 1 (A2: to channel 2 when there is one). False when nothing changed.
    pub fn set_channels(&mut self, count: Option<u16>) -> bool {
        if self.channels == count {
            return false;
        }
        self.channels = count;
        if count.is_none_or(|count| self.lanes[0].channel >= count) {
            self.lanes[0].channel = 0;
        }
        if let Some(count) = count.filter(|count| self.lanes[1].channel >= *count) {
            self.lanes[1].channel = u16::from(count >= 2);
        }
        true
    }

    /// The channel picker chose the channel heard on A1 (lane 0) or A2; the lane is on and
    /// let go.
    pub fn choose(&mut self, lane: u8, channel: u16) -> bool {
        if !self.channels.is_some_and(|count| channel < count) {
            return false;
        }
        let kept = &mut self.lanes[slot(lane)];
        (kept.channel, kept.off) = (channel, false);
        self.taken = None;
        true
    }

    /// The source channels kept on A1 and A2 (zero based), whatever their level.
    pub fn heard(&self) -> (u16, u16) {
        (self.lanes[0].channel, self.lanes[1].channel)
    }

    /// A1 (lane 0) or A2 is kept as `kept` (a stored segment or cover taken again,
    /// user 2026-10-16); a taken lane is let go.
    pub fn keep(&mut self, lane: u8, kept: Lane) {
        self.lanes[slot(lane)] = kept;
        self.taken = None;
    }

    /// A1 (lane 0) or A2 as kept (Enter), without a draft: what a new segment or cover
    /// takes with it (user 2026-10-16).
    pub fn kept(&self, lane: u8) -> Lane {
        self.lanes[slot(lane)]
    }

    /// A1 (lane 0) or A2 as it plays now: the taken lane's draft, else what is kept.
    pub fn lane(&self, lane: u8) -> Lane {
        match self.taken {
            Some((taken, draft)) if taken == lane => draft,
            _ => self.lanes[slot(lane)],
        }
    }

    /// The source channels the preview plays on A1 and A2 now and their gains (0 when
    /// off): the draft of a taken lane is heard at once, before Enter keeps it.
    pub fn audible(&self) -> ((u16, u16), (f32, f32)) {
        let (a1, a2) = (self.lane(0), self.lane(1));
        ((a1.channel, a2.channel), (a1.gain(), a2.gain()))
    }

    /// The channel of A1 (lane 0) or A2 and the clip's channel count, when known.
    pub fn choice(&self, lane: u8) -> Option<(u16, u16)> {
        let channel = self.lanes[slot(lane)].channel;
        self.channels.filter(|count| *count > 0).map(|count| (channel, count))
    }

    /// Ctrl+1 (lane 0) or Ctrl+2: the lane is taken with what it plays as draft. False
    /// when the clip has no known audio channels.
    pub fn take(&mut self, lane: u8) -> bool {
        let Some(count) = self.channels.filter(|count| *count > 0) else {
            return false;
        };
        let mut draft = self.lanes[slot(lane)];
        draft.channel = draft.channel.min(count - 1);
        self.taken = Some((lane, draft));
        true
    }

    /// The taken lane and its draft channel.
    pub fn taken(&self) -> Option<(u8, u16)> {
        self.taken.map(|(lane, draft)| (lane, draft.channel))
    }

    /// Up/down: the draft channel of the taken lane, within the clip's channels.
    pub fn draft(&mut self, delta: i64) -> bool {
        let (Some((lane, mut draft)), Some(count)) = (self.taken, self.channels) else {
            return false;
        };
        draft.channel = (i64::from(draft.channel) + delta).clamp(0, i64::from(count) - 1) as u16;
        self.taken = Some((lane, draft));
        true
    }

    /// A click on a channel of the taken lane's column: it becomes the draft and the lane
    /// is on; the lane stays taken and expanded until Enter or Escape (user 2026-10-14).
    /// False when that lane is not taken or the clip has no such channel.
    pub fn set_draft(&mut self, lane: u8, channel: u16) -> bool {
        match (self.taken, self.channels) {
            (Some((taken, mut draft)), Some(count)) if taken == lane && channel < count => {
                (draft.channel, draft.off) = (channel, false);
                self.taken = Some((lane, draft));
                true
            }
            _ => false,
        }
    }

    /// Left (`off`) or right: the taken lane is off (level 0, its channel kept) or on.
    pub fn set_off(&mut self, off: bool) -> bool {
        let Some((lane, mut draft)) = self.taken else {
            return false;
        };
        draft.off = off;
        self.taken = Some((lane, draft));
        true
    }

    /// Ctrl+left/right: the level of the taken lane by `delta_db`, within its limits.
    pub fn nudge_level(&mut self, delta_db: i8) -> bool {
        let Some((lane, mut draft)) = self.taken else {
            return false;
        };
        draft.level_db = draft.level_db.saturating_add(delta_db).clamp(MIN_LEVEL_DB, MAX_LEVEL_DB);
        self.taken = Some((lane, draft));
        true
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

    /// Enter: the draft becomes the lane's channel, on/off and level; the lane is let go.
    pub fn commit(&mut self) -> bool {
        let Some((lane, draft)) = self.taken.take() else {
            return false;
        };
        self.mark_committed();
        self.lanes[slot(lane)] = draft;
        true
    }

    /// Escape: the lane is let go, as it was before it was taken.
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
        assert_eq!(lanes.taken(), Some((1, 3)), "never past the last channel");
        assert!(lanes.set_draft(1, 2), "a click drafts, the lane stays taken");
        assert!(!lanes.set_draft(0, 1), "not on the lane that is not taken");
        assert!(!lanes.set_draft(1, 4), "not on a channel the clip does not have");
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
    fn left_turns_a_lane_off_right_on_and_ctrl_arrows_set_its_level_all_drafts_until_enter() {
        let mut lanes = LaneChoice::default();
        lanes.new_clip();
        lanes.set_channels(Some(4));
        lanes.take(0);
        assert!(lanes.set_off(true));
        assert_eq!(lanes.audible(), ((0, 1), (0.0, 1.0)), "off is heard at once");
        assert!(lanes.release());
        assert_eq!(lanes.audible(), ((0, 1), (1.0, 1.0)), "Escape: on again");
        lanes.take(0);
        lanes.set_off(true);
        lanes.commit();
        assert!(lanes.lane(0).off && lanes.heard() == (0, 1), "the channel is kept");
        lanes.take(0);
        assert!(lanes.lane(0).off, "taken again off");
        lanes.set_off(false);
        for _ in 0..60 {
            lanes.nudge_level(-1);
        }
        assert_eq!(lanes.lane(0).level_db, MIN_LEVEL_DB);
        for _ in 0..46 {
            lanes.nudge_level(1);
        }
        lanes.commit();
        assert_eq!(lanes.lane(0), Lane { channel: 0, off: false, level_db: 6 });
        assert!((lanes.audible().1 .0 - 1.995).abs() < 0.01, "+6 dB is about twice");
        assert!(lanes.choose(0, 2) && !lanes.lane(0).off, "a click on a channel is on");
        lanes.new_clip();
        assert_eq!(lanes.lane(0), Lane::on(0), "a new clip is heard as recorded");
    }
}
