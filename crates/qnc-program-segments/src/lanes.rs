//! A1 and A2 by keyboard (user rule 2026-10-01, no mouse needed): Ctrl+1 or Ctrl+2
//! takes the lane and opens its channel picker; left/right choose the source channel
//! (a draft), Enter keeps it, Escape drops it; up shows the lane's wave over the whole
//! video row at once, down hides it (display only, the sound is not changed).

use crate::ProgramSegments;

/// One press opens the wave over the whole video row, one press closes it (user rule
/// 2026-10-01: not step by step, at once to the maximum).
const MAX_WAVE_ZOOM: u8 = 1;

impl ProgramSegments {
    /// Ctrl+1 (lane 0) or Ctrl+2 (lane 1): the lane is taken with its channel as draft.
    pub fn take_lane(&mut self, lane: u8) -> bool {
        let Some(count) = self.source_channels.filter(|count| *count > 0) else {
            self.refresh_view("Klip nema poznate audio kanale.".into());
            return false;
        };
        let current = if lane == 0 { self.a1_channel } else { self.a2_channel };
        self.lane_taken = Some((lane, current.min(count - 1)));
        let name = if lane == 0 { "A1" } else { "A2" };
        self.refresh_view(format!("{name}: lijevo/desno kanal, gore/dolje prikaz vala, Enter sprema, Esc odustaje"));
        true
    }

    pub fn lane_is_taken(&self) -> bool {
        self.lane_taken.is_some()
    }

    /// Left/right: the draft channel of the taken lane, within the clip's channels.
    pub fn lane_draft(&mut self, delta: i64) -> bool {
        let (Some((lane, draft)), Some(count)) = (self.lane_taken, self.source_channels) else {
            return false;
        };
        let next = (i64::from(draft) + delta).clamp(0, i64::from(count) - 1) as u16;
        self.lane_taken = Some((lane, next));
        self.view.lane_taken = self.lane_taken;
        true
    }

    /// Up opens the wave of the taken lane over the video row, down closes it.
    pub fn lane_zoom(&mut self, delta: i8) -> bool {
        let Some((lane, _)) = self.lane_taken else {
            return false;
        };
        let level = &mut self.wave_zoom[usize::from(lane)];
        *level = level.saturating_add_signed(delta).min(MAX_WAVE_ZOOM);
        self.view.wave_zoom = self.wave_zoom;
        true
    }

    /// Enter: the draft becomes the lane's channel; the lane is let go.
    pub fn commit_lane(&mut self) -> bool {
        let Some((lane, draft)) = self.lane_taken.take() else {
            return false;
        };
        if lane == 0 {
            self.choose_a1_channel(draft);
        } else {
            self.choose_a2_channel(draft);
        }
        self.refresh_view(String::new());
        true
    }

    /// Escape: the lane is let go, its channel unchanged.
    pub fn release_lane(&mut self) -> bool {
        if self.lane_taken.take().is_none() {
            return false;
        }
        self.refresh_view(String::new());
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn ctrl_2_takes_a2_arrows_pick_and_zoom_enter_keeps_escape_drops() {
        let mut segments = ProgramSegments::new();
        segments.set_source(SourcePick::new(Some("c"), Some("C"), None, (None, 100), Some((50, 1))), None);
        segments.set_source_channels(Some(4));
        assert!(segments.take_lane(1));
        assert_eq!(segments.view().lane_taken, Some((1, 1)), "A2 starts on its channel 2");
        segments.lane_draft(1);
        segments.lane_draft(5);
        assert_eq!(segments.view().lane_taken, Some((1, 3)), "never past the last channel");
        segments.lane_zoom(1);
        segments.lane_zoom(1);
        assert_eq!(segments.view().wave_zoom, [0, 1], "one press opens it fully");
        assert!(segments.sync_holds_enter(), "Enter belongs to the taken lane");
        segments.commit_lane();
        assert_eq!((segments.view().a2_choice, segments.view().lane_taken), (Some((3, 4)), None));
        segments.take_lane(1);
        segments.lane_draft(-3);
        segments.release_lane();
        assert_eq!(segments.view().a2_choice, Some((3, 4)), "Escape keeps the channel");
        assert_eq!(segments.view().wave_zoom, [0, 1], "the wave view stays");
    }
}
