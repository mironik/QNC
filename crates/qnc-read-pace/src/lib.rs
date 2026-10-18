//! How many readers of a medium run at once, found by the reading itself (user 2026-10-18:
//! "a short test, so it adapts to the speed of the reader"). The first passes are the test:
//! no extra reading. Each pass started at a level tells its own speed (work it did per second
//! it took); the level's speed is the median pass speed times its readers, so a long clip
//! or a short one does not decide by finishing inside or outside a window (live 2026-10-18:
//! a window of 3 s and 4-6 passes settled once at 3 readers, once at 2). While one more
//! reader brings at least a tenth more, one more is tried; when it does not, the best level
//! found stays for the rest of the run. A USB 2.0 card settles low, a USB 3 stick or an SSD
//! high. It reads nothing, writes nothing and runs nothing.

/// A level is judged once this many passes started at it have finished, per reader...
const PASSES_PER_READER: usize = 2;
/// ...and at least this many.
const LEAST_PASSES: usize = 6;
/// One more reader stays only when it brings at least this much more.
const GAIN: f64 = 1.10;

#[derive(Debug, Clone)]
pub struct ReadPace {
    readers: usize,
    most: usize,
    best: Option<(usize, f64)>,
    settled: bool,
    /// Grows each time the measure starts again: a pass counts only for the round it
    /// started in.
    round: u64,
    speeds: Vec<f64>,
}

/// What a judged level decided, for the log.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Step {
    /// One more reader is tried.
    More { readers: usize, rate: f64 },
    /// The run stays at this many readers.
    Settled { readers: usize, rate: f64 },
}

/// The round a pass started in, kept by its caller until the pass ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Round(u64);

impl ReadPace {
    /// Starts with `start` readers and never goes above `most`.
    pub fn new(start: usize, most: usize) -> Self {
        let most = most.max(1);
        Self {
            readers: start.clamp(1, most),
            most,
            best: None,
            settled: false,
            round: 0,
            speeds: Vec::new(),
        }
    }

    /// How many readers may run now.
    pub fn readers(&self) -> usize {
        self.readers
    }

    /// The round a pass starting now belongs to.
    pub fn round(&self) -> Round {
        Round(self.round)
    }

    /// The level is not measured meanwhile: the readers gave way (a player, a clip that
    /// reads alone) or had nothing to read. Passes running now no longer count.
    pub fn interrupted(&mut self) {
        self.next_round();
    }

    /// A pass of round `round` did `work` (e.g. seconds of media) in `seconds`.
    pub fn finished(&mut self, round: Round, work: f64, seconds: f64) -> Option<Step> {
        if self.settled || round.0 != self.round || !(work > 0.0) || !(seconds > 0.0) {
            return None;
        }
        self.speeds.push(work / seconds);
        if self.speeds.len() < (self.readers * PASSES_PER_READER).max(LEAST_PASSES) {
            return None;
        }
        let rate = median(&mut self.speeds) * self.readers as f64;
        let step = match self.best {
            Some((readers, best)) if rate < best * GAIN => {
                self.readers = readers;
                self.settled = true;
                Step::Settled { readers, rate: best }
            }
            _ if self.readers >= self.most => {
                self.settled = true;
                Step::Settled { readers: self.readers, rate }
            }
            _ => {
                self.best = Some((self.readers, rate));
                self.readers += 1;
                Step::More { readers: self.readers, rate }
            }
        };
        self.next_round();
        Some(step)
    }

    fn next_round(&mut self) {
        self.round += 1;
        self.speeds.clear();
    }
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds a level whose passes each read `speed` until it is judged.
    fn level(pace: &mut ReadPace, speed: f64) -> Step {
        loop {
            let round = pace.round();
            if let Some(step) = pace.finished(round, speed * 3.0, 3.0) {
                return step;
            }
        }
    }

    #[test]
    fn readers_grow_while_they_bring_more_and_settle_on_the_best() {
        let mut pace = ReadPace::new(2, 8);
        // 2 readers at 47 each, 3 at 43, 4 at 40, 5 at 32 (the medium is full).
        assert!(matches!(level(&mut pace, 47.0), Step::More { readers: 3, .. }));
        assert!(matches!(level(&mut pace, 43.0), Step::More { readers: 4, .. }));
        assert!(matches!(level(&mut pace, 40.0), Step::More { readers: 5, .. }));
        assert!(matches!(level(&mut pace, 32.0), Step::Settled { readers: 4, .. }));
        assert_eq!(pace.readers(), 4);
        assert_eq!(pace.finished(pace.round(), 1000.0, 1.0), None, "settled for the rest of the run");
    }

    #[test]
    fn a_slow_reader_goes_back_to_where_it_was_faster() {
        let mut pace = ReadPace::new(2, 8);
        level(&mut pace, 14.0);
        assert!(matches!(level(&mut pace, 8.0), Step::Settled { readers: 2, .. }));
    }

    #[test]
    fn readers_never_go_above_the_most() {
        let mut pace = ReadPace::new(2, 3);
        level(&mut pace, 25.0);
        assert!(matches!(level(&mut pace, 33.0), Step::Settled { readers: 3, .. }));
    }

    #[test]
    fn a_long_odd_pass_does_not_decide_the_level() {
        let mut pace = ReadPace::new(2, 8);
        let round = pace.round();
        // One pass that crawled (a long clip beside others) among normal ones.
        assert_eq!(pace.finished(round, 130.0, 30.0), None);
        let mut step = None;
        while step.is_none() {
            step = pace.finished(round, 30.0, 1.0);
        }
        assert!(matches!(step, Some(Step::More { rate, .. }) if rate == 60.0));
    }

    #[test]
    fn a_pass_of_an_earlier_round_does_not_count() {
        let mut pace = ReadPace::new(2, 8);
        let earlier = pace.round();
        pace.interrupted();
        assert_eq!(pace.finished(earlier, 10.0, 1.0), None);
        assert!(pace.speeds.is_empty());
    }
}
