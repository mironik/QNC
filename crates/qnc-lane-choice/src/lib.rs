//! A1 and A2 of the marked source (QNC public piece, split out of `qnc-program-segments`,
//! audit 2026-10-10): which source channel is heard on A1 (Talking Head, Voice over) and
//! on A2 (covers), each its own choice (user rules 2026-09-30, 2026-10-01: A1 on channel 1,
//! A2 on channel 2 of a new clip, channel 1 when the clip has one).
//!
//! By keyboard (user rule 2026-10-01, no mouse needed): Ctrl+1 or Ctrl+2 takes the lane
//! with its channel as draft; left/right move the draft, Enter keeps it, Escape drops it;
//! up shows the lane's wave over the whole video row at once and lets the lane go, down
//! hides it (display only, the sound is not changed).

use std::time::{Duration, Instant};

/// One press opens the wave over the whole video row, one press closes it (user rule
/// 2026-10-01: not step by step, at once to the maximum).
const MAX_WAVE_ZOOM: u8 = 1;

#[derive(Debug, Clone, Default)]
pub struct LaneChoice {
    a1: u16,
    a2: u16,
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
        (self.a1, self.a2, self.taken) = (0, 1, None);
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

    /// The channel picker chose the channel heard on A1 (lane 0) or A2; the lane is let go.
    pub fn choose(&mut self, lane: u8, channel: u16) -> bool {
        if !self.channels.is_some_and(|count| channel < count) {
            return false;
        }
        if lane == 0 {
            self.a1 = channel;
        } else {
            self.a2 = channel;
        }
        self.taken = None;
        true
    }

    /// The source channels heard on A1 and A2 now (zero based).
    pub fn heard(&self) -> (u16, u16) {
        (self.a1, self.a2)
    }

    /// The channel of A1 (lane 0) or A2 and the clip's channel count, when known.
    pub fn choice(&self, lane: u8) -> Option<(u16, u16)> {
        let channel = if lane == 0 { self.a1 } else { self.a2 };
        self.channels.filter(|count| *count > 0).map(|count| (channel, count))
    }

    /// Ctrl+1 (lane 0) or Ctrl+2: the lane is taken with its channel as draft. False
    /// when the clip has no known audio channels.
    pub fn take(&mut self, lane: u8) -> bool {
        let Some(count) = self.channels.filter(|count| *count > 0) else {
            return false;
        };
        let current = if lane == 0 { self.a1 } else { self.a2 };
        self.taken = Some((lane, current.min(count - 1)));
        true
    }

    /// The taken lane and its draft channel.
    pub fn taken(&self) -> Option<(u8, u16)> {
        self.taken
    }

    /// Left/right: the draft channel of the taken lane, within the clip's channels.
    pub fn draft(&mut self, delta: i64) -> bool {
        let (Some((lane, draft)), Some(count)) = (self.taken, self.channels) else {
            return false;
        };
        let next = (i64::from(draft) + delta).clamp(0, i64::from(count) - 1) as u16;
        self.taken = Some((lane, next));
        true
    }

    /// Up opens the wave of the taken lane over the video row, down closes it; the lane
    /// is let go with its channel unchanged, so the arrows move the playhead again.
    pub fn zoom(&mut self, delta: i8) -> bool {
        let Some((lane, _)) = self.taken.take() else {
            return false;
        };
        let level = &mut self.wave_zoom[usize::from(lane)];
        *level = level.saturating_add_signed(delta).min(MAX_WAVE_ZOOM);
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
        assert_eq!(lanes.taken(), Some((1, 3)), "never past the last channel");
        assert!(lanes.commit());
        assert_eq!(lanes.heard(), (0, 3));
        lanes.set_channels(Some(1));
        assert_eq!(lanes.heard(), (0, 0), "a one channel clip hears channel 1 on A2");
        lanes.set_channels(None);
        assert!(!lanes.take(0), "no channels known: nothing to take");
    }
}
