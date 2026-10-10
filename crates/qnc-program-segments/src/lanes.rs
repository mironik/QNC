//! A1 and A2 by keyboard (user rule 2026-10-01, no mouse needed): the lane state and its
//! rules are the piece `qnc-lane-choice`; here the program model tells the user what a
//! taken lane does and refreshes its view.

use crate::ProgramSegments;

impl ProgramSegments {
    /// Ctrl+1 (lane 0) or Ctrl+2 (lane 1): the lane is taken with its channel as draft.
    pub fn take_lane(&mut self, lane: u8) -> bool {
        if !self.lanes.take(lane) {
            self.refresh_view("Klip nema poznate audio kanale.".into());
            return false;
        }
        let name = if lane == 0 { "A1" } else { "A2" };
        self.refresh_view(format!("{name}: lijevo/desno kanal, gore/dolje prikaz vala, Enter sprema, Esc odustaje"));
        true
    }

    pub fn lane_is_taken(&self) -> bool {
        self.lanes.taken().is_some()
    }

    /// Left/right: the draft channel of the taken lane, within the clip's channels.
    pub fn lane_draft(&mut self, delta: i64) -> bool {
        let moved = self.lanes.draft(delta);
        self.view.lane_taken = self.lanes.taken();
        moved
    }

    /// Up opens the wave of the taken lane over the video row, down closes it; the lane
    /// is let go with its channel unchanged (user rule 2026-10-01).
    pub fn lane_zoom(&mut self, delta: i8) -> bool {
        let zoomed = self.lanes.zoom(delta);
        if zoomed {
            self.refresh_view(String::new());
        }
        zoomed
    }

    /// Down with no lane taken: the open large wave closes.
    pub fn close_wave(&mut self) -> bool {
        let closed = self.lanes.close_wave();
        if closed {
            self.refresh_view(String::new());
        }
        closed
    }

    /// Enter: the draft becomes the lane's channel; the lane is let go.
    pub fn commit_lane(&mut self) -> bool {
        let committed = self.lanes.commit();
        if committed {
            self.refresh_view(String::new());
        }
        committed
    }

    /// Escape: the lane is let go, its channel unchanged.
    pub fn release_lane(&mut self) -> bool {
        let released = self.lanes.release();
        if released {
            self.refresh_view(String::new());
        }
        released
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
        assert_eq!(segments.view().wave_zoom, [0, 1], "one press opens it fully");
        assert_eq!(segments.view().lane_taken, None, "and lets the lane go: the arrows move the playhead");
        assert!(!segments.lane_zoom(1), "nothing taken any more");
        assert!(segments.close_wave(), "down closes it without taking the lane");
        assert_eq!(segments.view().wave_zoom, [0, 0]);
        segments.take_lane(1);
        segments.lane_draft(5);
        assert!(segments.sync_holds_enter(), "Enter belongs to the taken lane");
        segments.commit_lane();
        assert!(segments.sync_holds_enter(), "the same Enter does not also save a short");
        assert_eq!((segments.view().a2_choice, segments.view().lane_taken), (Some((3, 4)), None));
        segments.take_lane(1);
        segments.lane_draft(-3);
        segments.release_lane();
        assert_eq!(segments.view().a2_choice, Some((3, 4)), "Escape keeps the channel");
        assert_eq!(segments.view().wave_zoom, [0, 0], "Escape does not touch the wave view");
    }
}
