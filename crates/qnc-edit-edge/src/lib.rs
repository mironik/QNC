//! The one rule of an edge (AGENTS 0.12, docs/97 step 5, user 2026-11-04): a piece plays
//! `[from, to)` of its clip (`length` frames); its IN is `from`, its OUT `to` (exclusive).
//! A source timeline's IN/OUT, a SubSegment's part and the clip of a segment are such pieces:
//! their owners keep where the piece stands and what is written, this block says how its edges
//! move. A piece always keeps a frame and stays inside its clip. Pure: no state, no database.

pub const MODULE_ID: &str = "qnc.module.edit-edge";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Which edge of a piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Edge {
    In,
    Out,
}

impl Edge {
    pub fn of_out(out: bool) -> Self {
        if out {
            Edge::Out
        } else {
            Edge::In
        }
    }
}

/// What a piece plays of its clip, `[from, to)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub from: u64,
    pub to: u64,
}

impl Range {
    /// The edge moved `step` frames, refused (`None`) when it would cross the other edge or
    /// leave the clip: a key that cannot move does nothing (a source timeline's IN/OUT).
    pub fn nudged(self, edge: Edge, step: i64, length: u64) -> Option<Range> {
        let next = u64::try_from(self.at(edge) as i64 + step).ok()?;
        let moved = self.with(edge, next);
        (moved.from < moved.to && moved.to <= length).then_some(moved)
    }

    /// The edge moved `step` frames as far as it can go (a SubSegment's part: an arrow at the
    /// end of the clip leaves it there).
    pub fn moved(self, edge: Edge, step: i64, length: u64) -> Range {
        self.dragged(edge, (self.at(edge) as i64 + step).max(0) as u64, length)
    }

    /// The edge put on `frame` of the clip (a drag, a word clicked), as far as it can go.
    pub fn dragged(self, edge: Edge, frame: u64, length: u64) -> Range {
        match edge {
            Edge::In => Range { from: frame.min(self.to.saturating_sub(1)), ..self },
            Edge::Out => Range { to: frame.clamp(self.from + 1, length.max(self.from + 1)), ..self },
        }
    }

    /// The frame the edge stands on (IN, or OUT exclusive).
    pub fn at(self, edge: Edge) -> u64 {
        match edge {
            Edge::In => self.from,
            Edge::Out => self.to,
        }
    }

    /// The picture at the edge: the IN itself, the last frame before the OUT.
    pub fn picture(self, edge: Edge) -> u64 {
        match edge {
            Edge::In => self.from,
            Edge::Out => self.to.saturating_sub(1),
        }
    }

    fn with(self, edge: Edge, frame: u64) -> Range {
        match edge {
            Edge::In => Range { from: frame, ..self },
            Edge::Out => Range { to: frame, ..self },
        }
    }
}

/// A roll (the whole cut where two pieces meet): the OUT of `before` (in a clip of
/// `before_length`) and the IN of `after` move together by as much of `wanted` as both can
/// take, each keeping a frame and staying in its clip; the length of the two stays.
pub fn rolled(before: Range, before_length: u64, after: Range, wanted: i64) -> (Range, Range) {
    let earliest = (before.from as i64 + 1 - before.to as i64).max(-(after.from as i64));
    let latest = (before_length as i64 - before.to as i64).min(after.to as i64 - 1 - after.from as i64);
    let by = wanted.clamp(earliest.min(0), latest.max(0));
    (Range { to: (before.to as i64 + by) as u64, ..before }, Range { from: (after.from as i64 + by) as u64, ..after })
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Range = Range { from: 10, to: 20 };

    #[test]
    fn a_nudge_never_crosses_the_other_edge_or_leaves_the_clip() {
        assert_eq!(R.nudged(Edge::In, 3, 100), Some(Range { from: 13, to: 20 }));
        assert_eq!(R.nudged(Edge::In, 10, 100), None, "IN onto OUT");
        assert_eq!(R.nudged(Edge::In, -11, 100), None, "before the clip");
        assert_eq!(R.nudged(Edge::Out, 5, 25), Some(Range { from: 10, to: 25 }));
        assert_eq!(R.nudged(Edge::Out, 6, 25), None, "past the clip");
    }

    #[test]
    fn a_move_or_a_drag_goes_as_far_as_it_can() {
        assert_eq!(R.moved(Edge::In, 50, 100), Range { from: 19, to: 20 }, "a frame is kept");
        assert_eq!(R.moved(Edge::In, -50, 100), Range { from: 0, to: 20 });
        assert_eq!(R.moved(Edge::Out, 50, 30), Range { from: 10, to: 30 });
        assert_eq!(R.dragged(Edge::Out, 3, 30), Range { from: 10, to: 11 });
        assert_eq!(R.picture(Edge::Out), 19);
    }

    #[test]
    fn a_roll_moves_both_edges_as_far_as_both_can() {
        let after = Range { from: 40, to: 45 };
        assert_eq!(rolled(R, 100, after, 3), (Range { from: 10, to: 23 }, Range { from: 43, to: 45 }));
        assert_eq!(rolled(R, 100, after, 10), (Range { from: 10, to: 24 }, Range { from: 44, to: 45 }), "the piece after keeps a frame");
        assert_eq!(rolled(R, 21, after, 3), (Range { from: 10, to: 21 }, Range { from: 41, to: 45 }), "the piece before stays in its clip");
        assert_eq!(rolled(R, 100, after, -20), (Range { from: 10, to: 11 }, Range { from: 31, to: 45 }));
    }
}
