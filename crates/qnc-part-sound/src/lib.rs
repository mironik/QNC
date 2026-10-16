//! The sound of a stored segment or cover, changed on the Wrap row (user 2026-10-16):
//! Ctrl+1 takes A1 of the segment, Ctrl+2 A2 of the cover; up/down pick the channel,
//! left turns it off, right on, Ctrl+left/right set its level; Enter gives the change to
//! write, Escape lets it go. The lane rules are the piece `qnc-lane-choice`, the same as
//! on the source timeline; this piece only knows which stored part the lane belongs to.
//! No database, no player, no form.

pub use qnc_lane_choice::Lane;
use qnc_lane_choice::LaneChoice;

pub const MODULE_ID: &str = "qnc.module.part-sound";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A1 of a segment (lane 0) or A2 of a cover (lane 1), by its stored id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    pub lane: u8,
    pub id: String,
}

/// What Enter keeps: the part and its new sound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartSoundChange {
    pub part: Part,
    pub sound: Lane,
}

#[derive(Debug, Clone, Default)]
pub struct PartSound {
    lanes: LaneChoice,
    taken: Option<Part>,
}

impl PartSound {
    /// The part is taken with its kept sound as draft; `channels` is how many its clip
    /// has. False without channels (its sound is not known).
    pub fn take(&mut self, part: Part, kept: Lane, channels: Option<u16>) -> bool {
        self.lanes.set_channels(channels);
        self.lanes.keep(part.lane, kept);
        if !self.lanes.take(part.lane) {
            self.taken = None;
            return false;
        }
        self.taken = Some(part);
        true
    }

    /// The taken part and its draft sound.
    pub fn taken(&self) -> Option<(&Part, Lane)> {
        self.taken.as_ref().map(|part| (part, self.lanes.lane(part.lane)))
    }

    /// The channel count of the taken part's clip.
    pub fn channels(&self) -> Option<u16> {
        let lane = self.taken.as_ref()?.lane;
        self.lanes.choice(lane).map(|(_, count)| count)
    }

    /// Up/down: the draft channel, within the clip's channels.
    pub fn draft(&mut self, delta: i64) -> bool {
        self.taken.is_some() && self.lanes.draft(delta)
    }

    /// Left (`off`) or right.
    pub fn set_off(&mut self, off: bool) -> bool {
        self.taken.is_some() && self.lanes.set_off(off)
    }

    /// Ctrl+left/right: the level by `delta_db`.
    pub fn nudge_level(&mut self, delta_db: i8) -> bool {
        self.taken.is_some() && self.lanes.nudge_level(delta_db)
    }

    /// Enter: the change to write; the part is let go.
    pub fn commit(&mut self) -> Option<PartSoundChange> {
        let part = self.taken.take()?;
        self.lanes.commit();
        Some(PartSoundChange { sound: self.lanes.kept(part.lane), part })
    }

    /// Escape: let go, nothing written.
    pub fn release(&mut self) -> bool {
        self.lanes.release();
        self.taken.take().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kept() -> Lane {
        Lane { channel: 1, off: false, level_db: -3 }
    }

    #[test]
    fn a_taken_cover_changes_its_draft_and_enter_gives_the_change() {
        let mut sound = PartSound::default();
        assert!(sound.take(Part { lane: 1, id: "cover_1".into() }, kept(), Some(4)));
        assert_eq!(sound.taken().unwrap().1, kept(), "the stored sound is the draft");
        assert!(sound.draft(1));
        assert!(sound.nudge_level(2));
        let change = sound.commit().unwrap();
        assert_eq!(change.part, Part { lane: 1, id: "cover_1".into() });
        assert_eq!(change.sound, Lane { channel: 2, off: false, level_db: -1 });
        assert!(sound.taken().is_none(), "let go");
    }

    #[test]
    fn escape_writes_nothing_and_a_part_without_channels_is_not_taken() {
        let mut sound = PartSound::default();
        sound.take(Part { lane: 0, id: "part_1".into() }, kept(), Some(2));
        assert!(sound.set_off(true));
        assert!(sound.release());
        assert!(sound.commit().is_none());
        assert!(!sound.take(Part { lane: 0, id: "part_1".into() }, kept(), None));
        assert!(!sound.draft(1), "nothing taken");
    }
}
