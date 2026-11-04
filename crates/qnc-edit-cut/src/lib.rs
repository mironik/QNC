//! The one rule of a cut's sound (AGENTS 0.12, docs/97 step 4, user 2026-11-04): a cut where
//! two pieces meet has its picture and its sound; the sound has two sides, the J of the piece
//! after it (where its sound starts, `lead` frames before the cut) and the L of the piece before
//! it (where its sound ends, `tail` frames after the cut). A story's M marker and cut between two
//! clips, and a SubSegment's join, are all such cuts: their owners keep where the cut is and what
//! is written, this block says how the sides move. Pure: no state, no database, no drawing.

pub const MODULE_ID: &str = "qnc.module.edit-cut";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Which side of the cut's sound moves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum SoundSide {
    /// The cut's sound as one (Ctrl+1/Ctrl+2 on a taken cut).
    #[default]
    Both,
    /// The J of the piece after the cut: where its sound starts (Ctrl+I).
    Lead,
    /// The L of the piece before the cut: where its sound ends (Ctrl+O).
    Tail,
}

impl SoundSide {
    /// The side of a chosen piece at a cut: the piece before it moves its L, the one after it
    /// its J (user 2026-11-04: "only the chosen side moves, never the neighbour's").
    pub fn of_piece(before_the_cut: bool) -> Self {
        if before_the_cut {
            SoundSide::Tail
        } else {
            SoundSide::Lead
        }
    }

    /// The side the zoom frames red: `Some(true)` after the cut, `Some(false)` before it.
    pub fn focus(self) -> Option<bool> {
        match self {
            SoundSide::Lead => Some(true),
            SoundSide::Tail => Some(false),
            SoundSide::Both => None,
        }
    }
}

/// The two sides of a cut's sound in frames of the owner's axis. Positive: the sounds cross
/// (the J starts before the cut, the L ends after it). Negative (an owner that allows it): a
/// hole, the sound after starts late or the sound before ends early.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CutSound {
    pub lead: i64,
    pub tail: i64,
}

impl CutSound {
    /// The sides moved `step` frames (an arrow, the wheel). `holes`: the owner allows negative
    /// sides (a SubSegment's join, user 2026-11-04). Both sides as one move the sound's cut: the
    /// sound before ends `step` later and the sound after starts `step` later; without holes the
    /// cut of a sound that does not cross is kept one cut (the L after the picture, the J before).
    pub fn moved(self, side: SoundSide, step: i64, holes: bool) -> Self {
        match side {
            SoundSide::Lead => CutSound { lead: self.lead - step, ..self },
            SoundSide::Tail => CutSound { tail: self.tail + step, ..self },
            SoundSide::Both if holes || (self.lead > 0 && self.tail > 0) => CutSound { lead: self.lead - step, tail: self.tail + step },
            SoundSide::Both => {
                let offset = self.tail - self.lead + step;
                CutSound { tail: offset.max(0), lead: (-offset).max(0) }
            }
        }
    }

    /// Within the owner's bounds: each side between its lowest and `most`.
    pub fn bounded(self, lowest: (i64, i64), most: i64) -> Self {
        CutSound { lead: self.lead.clamp(lowest.0, most), tail: self.tail.clamp(lowest.1, most) }
    }

    /// Where the sides stand on the axis: (where the J starts, where the L ends).
    pub fn sides_at(self, cut: u64) -> (u64, u64) {
        let at = |shift: i64| (cut as i64 + shift).max(0) as u64;
        (at(-self.lead), at(self.tail))
    }

    /// The side dragged to `frame` (a drag of its red line): that side ends up there.
    pub fn dragged(self, side: SoundSide, cut: u64, frame: u64) -> Self {
        let (cut, frame) = (cut as i64, frame as i64);
        match side {
            SoundSide::Lead => CutSound { lead: cut - frame, ..self },
            SoundSide::Tail => CutSound { tail: frame - cut, ..self },
            SoundSide::Both => self,
        }
    }

    /// The side nearer `frame` (a click near a red line); only a side the cut has counts
    /// (`has`: the J side, the L side).
    pub fn nearer(self, cut: u64, frame: u64, has: (bool, bool)) -> Option<SoundSide> {
        let (lead_at, tail_at) = self.sides_at(cut);
        let lead = has.0.then(|| lead_at.abs_diff(frame));
        let tail = has.1.then(|| tail_at.abs_diff(frame));
        match (lead, tail) {
            (Some(lead), Some(tail)) => Some(if lead <= tail { SoundSide::Lead } else { SoundSide::Tail }),
            (Some(_), None) => Some(SoundSide::Lead),
            (None, Some(_)) => Some(SoundSide::Tail),
            (None, None) => None,
        }
    }

    /// The frames that move (the trim playheads): both sides, or the one that moves alone.
    pub fn moving(self, side: SoundSide, cut: u64) -> Vec<u64> {
        let (lead, tail) = self.sides_at(cut);
        match side {
            SoundSide::Both => vec![lead, tail],
            SoundSide::Lead => vec![lead],
            SoundSide::Tail => vec![tail],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sound(lead: i64, tail: i64) -> CutSound {
        CutSound { lead, tail }
    }

    /// A SubSegment's join (holes allowed): each side moves alone, both move the sound's cut.
    #[test]
    fn with_holes_the_sides_move_alone_and_together_move_the_sound_cut() {
        assert_eq!(sound(0, 0).moved(SoundSide::Both, 3, true), sound(-3, 3), "the sound cut 3 frames after the picture cut");
        assert_eq!(sound(4, 6).moved(SoundSide::Both, 2, true), sound(2, 8));
        assert_eq!(sound(0, 0).moved(SoundSide::Tail, -5, true), sound(0, -5), "the sound before ends 5 early");
        assert_eq!(sound(0, -5).moved(SoundSide::Lead, 4, true), sound(-4, -5), "the sound after starts 4 late");
        assert_eq!(sound(0, 0).moved(SoundSide::Tail, -100, true).bounded((-40, -40), 10_000), sound(0, -40));
    }

    /// A story's cut (no holes): a sound that does not cross stays one cut, an overlap moves whole.
    #[test]
    fn without_holes_one_sound_cut_stays_one_and_an_overlap_moves_whole() {
        assert_eq!(sound(0, 0).moved(SoundSide::Both, 3, false), sound(0, 3), "an L after the picture");
        assert_eq!(sound(0, 0).moved(SoundSide::Both, -3, false), sound(3, 0), "a J before the picture");
        assert_eq!(sound(2, 0).moved(SoundSide::Both, 3, false), sound(0, 1), "the J becomes an L across the cut");
        assert_eq!(sound(4, 6).moved(SoundSide::Both, 2, false), sound(2, 8), "an overlap moves whole");
        assert_eq!(sound(4, 0).moved(SoundSide::Lead, 9, false).bounded((0, 0), 360_000), sound(0, 0));
    }

    #[test]
    fn a_click_takes_the_nearer_side_a_drag_puts_it_there() {
        let cut = 100;
        let shape = sound(10, 20);
        assert_eq!(shape.sides_at(cut), (90, 120));
        assert_eq!(shape.nearer(cut, 95, (true, true)), Some(SoundSide::Lead));
        assert_eq!(shape.nearer(cut, 112, (true, true)), Some(SoundSide::Tail));
        assert_eq!(shape.nearer(cut, 95, (false, true)), Some(SoundSide::Tail), "only a side the cut has");
        assert_eq!(shape.dragged(SoundSide::Lead, cut, 80), sound(20, 20));
        assert_eq!(shape.dragged(SoundSide::Tail, cut, 104), sound(10, 4));
        assert_eq!(shape.moving(SoundSide::Tail, cut), vec![120]);
        assert_eq!(SoundSide::of_piece(true), SoundSide::Tail, "the piece before: its L");
        assert_eq!(SoundSide::Lead.focus(), Some(true));
    }
}
