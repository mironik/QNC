//! The level curve of a stored segment (A1) or cover (A2) taken on the Wrap row (docs/98 2.1,
//! user 2026-11-06: a right click on the level line takes it, as Ctrl+; a click places the
//! playhead, and the line of the playhead is the point that is controlled). The points are
//! frames of the item's clip, so the curve stays on its sound when the item is trimmed or
//! moved; here they are placed on the program and drafted until Enter.

use qnc_program_db::{LevelKey, LEVEL_KEY_RANGE};

pub const MODULE_ID: &str = "qnc.module.level-curve";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// One step of the keys and of Ctrl+wheel: a dB.
pub const STEP_DB10: i16 = 10;

/// Where a segment or cover lies on the program, `[start, end)`, and the frames of its clip
/// it plays there, `[source_in, source_out)` (twice or half as many at another rate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u64,
    pub end: u64,
    pub source_in: u64,
    pub source_out: u64,
}

impl Span {
    /// The clip frame that sounds at program frame `program`.
    pub fn source_at(&self, program: u64) -> u64 {
        let length = self.end.saturating_sub(self.start).max(1);
        let played = self.source_out.saturating_sub(self.source_in);
        let offset = program.clamp(self.start, self.end).saturating_sub(self.start);
        let moved = u128::from(offset) * u128::from(played) / u128::from(length);
        self.source_in + u64::try_from(moved).unwrap_or(played)
    }

    /// The program position (fractional) where clip frame `source` sounds.
    pub fn program_at(&self, source: u64) -> f64 {
        let length = self.end.saturating_sub(self.start).max(1) as f64;
        let played = self.source_out.saturating_sub(self.source_in).max(1) as f64;
        self.start as f64 + (source as f64 - self.source_in as f64) * length / played
    }
}

/// The curve of an item on the program: its level at both ends (tenths of a dB) and its
/// points inside `[start, end]`, for drawing.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub start_db10: i16,
    pub end_db10: i16,
    pub points: Vec<(f64, i16)>,
}

impl Placed {
    /// Its level at program position `at` inside `span`, tenths of a dB: a line between its
    /// corners (its start, its points, its end).
    pub fn level_at(&self, span: Span, at: f64) -> f64 {
        let mut corners = vec![(span.start as f64, f64::from(self.start_db10))];
        corners.extend(self.points.iter().map(|(point, db10)| (*point, f64::from(*db10))));
        corners.push((span.end as f64, f64::from(self.end_db10)));
        for pair in corners.windows(2) {
            let ((a, da), (b, db)) = (pair[0], pair[1]);
            if at <= b {
                let t = if b > a { ((at - a) / (b - a)).clamp(0.0, 1.0) } else { 1.0 };
                return da + (db - da) * t;
            }
        }
        f64::from(self.end_db10)
    }

    /// The part of the curve inside program frames `[from, to)` (a Wrap row's window): its
    /// level at both ends of that part and its points there.
    pub fn within(&self, span: Span, from: u64, to: u64) -> Option<(u64, u64, Placed)> {
        let (a, b) = (span.start.max(from), span.end.min(to));
        if a >= b {
            return None;
        }
        let part = Placed {
            start_db10: self.level_at(span, a as f64).round() as i16,
            end_db10: self.level_at(span, b as f64).round() as i16,
            points: self.points.iter().copied().filter(|(at, _)| *at >= a as f64 && *at <= b as f64).collect(),
        };
        Some((a, b, part))
    }
}

/// `keys` of an item placed over its span.
pub fn place(span: Span, keys: &[LevelKey]) -> Placed {
    let level = |source: u64| LevelKey::level_at(keys, source as f64).round() as i16;
    Placed {
        start_db10: level(span.source_in),
        end_db10: level(span.source_out),
        points: keys
            .iter()
            .map(|key| (span.program_at(key.frame), key.db10))
            .filter(|(at, _)| *at >= span.start as f64 && *at <= span.end as f64)
            .collect(),
    }
}

/// The taken curve of one item and its draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelLine {
    /// 0: a segment's A1, 1: a cover's A2.
    pub lane: u8,
    pub id: String,
    span: Span,
    stored: Vec<LevelKey>,
    draft: Vec<LevelKey>,
}

impl LevelLine {
    pub fn new(lane: u8, id: impl Into<String>, span: Span, keys: Vec<LevelKey>) -> Self {
        Self { lane, id: id.into(), span, draft: keys.clone(), stored: keys }
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn draft(&self) -> &[LevelKey] {
        &self.draft
    }

    pub fn changed(&self) -> bool {
        self.draft != self.stored
    }

    /// The level of the draft at program frame `program`, tenths of a dB.
    pub fn level_at(&self, program: u64) -> i16 {
        LevelKey::level_at(&self.draft, self.span.source_at(program) as f64).round() as i16
    }

    /// The point on the playhead line (`program`), if there is one.
    pub fn point_at(&self, program: u64) -> Option<LevelKey> {
        let frame = self.span.source_at(program);
        self.draft.iter().copied().find(|key| key.frame == frame)
    }

    /// Up or down by `steps` dB at the playhead line: its point moves, or a point is made
    /// there at the curve's level and moved. False when the level stays (at its bound).
    pub fn nudge(&mut self, program: u64, steps: i16) -> bool {
        let frame = self.span.source_at(program);
        let now = self.level_at(program);
        let level = now.saturating_add(steps.saturating_mul(STEP_DB10)).clamp(LEVEL_KEY_RANGE.0, LEVEL_KEY_RANGE.1);
        match self.draft.iter_mut().find(|key| key.frame == frame) {
            Some(key) if key.db10 == level => false,
            Some(key) => {
                key.db10 = level;
                true
            }
            None if level == now => false,
            None => {
                self.draft.push(LevelKey { frame, db10: level });
                self.draft.sort_by_key(|key| key.frame);
                true
            }
        }
    }

    /// Delete: the point on the playhead line goes. False when none is there.
    pub fn remove_at(&mut self, program: u64) -> bool {
        let frame = self.span.source_at(program);
        let before = self.draft.len();
        self.draft.retain(|key| key.frame != frame);
        self.draft.len() != before
    }

    /// Enter: the draft becomes the kept curve; the curve to write when it changed.
    pub fn keep(&mut self) -> Option<Vec<LevelKey>> {
        if !self.changed() {
            return None;
        }
        self.stored = self.draft.clone();
        Some(self.draft.clone())
    }

    /// The draft placed on the program, for drawing.
    pub fn placed(&self) -> Placed {
        place(self.span, &self.draft)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_point_on_the_playhead_line_is_made_moved_and_removed_on_the_clips_frames() {
        // A source of half the story rate: two program frames for every clip frame.
        let span = Span { start: 100, end: 200, source_in: 1000, source_out: 1050 };
        assert_eq!((span.source_at(100), span.source_at(150), span.source_at(199)), (1000, 1025, 1049));
        assert_eq!(span.program_at(1025), 150.0);
        let mut line = LevelLine::new(0, "s1", span, Vec::new());
        assert!(!line.changed());
        assert!(line.nudge(150, -3), "a point made at the playhead line, 3 dB down");
        assert_eq!(line.draft(), &[LevelKey { frame: 1025, db10: -30 }]);
        assert_eq!(line.level_at(100), -30, "before the only point its level");
        assert!(line.nudge(120, 1), "a second point at the curve's level, one up");
        assert_eq!(line.point_at(120), Some(LevelKey { frame: 1010, db10: -20 }));
        assert!(line.remove_at(150));
        assert!(!line.remove_at(150), "nothing there any more");
        assert!(line.nudge(130, 200), "bounded at +12 dB");
        assert_eq!(line.point_at(130).map(|key| key.db10), Some(LEVEL_KEY_RANGE.1));
        assert!(!line.nudge(130, 1), "the bound stays");
        let kept = line.keep().unwrap();
        assert_eq!(kept.len(), 2);
        assert!(line.keep().is_none(), "nothing more to write");
        let placed = line.placed();
        assert_eq!(placed.points.len(), 2);
        assert_eq!(placed.start_db10, -20);
    }
}
