//! How many readers of a medium run at once, found by the reading itself (user 2026-10-18:
//! "a short test, so it adapts to the speed of the reader"). The first passes are the test:
//! no extra reading. A level is measured by the work its passes finish per second of wall
//! time; while one more reader brings at least a tenth more, one more is tried; when it
//! does not, the best level found stays for the rest of the run. A USB 2.0 card settles
//! low, a USB 3 stick or an SSD high. It reads nothing, writes nothing and runs nothing.

use std::time::{Duration, Instant};

/// A level is judged once it has finished this many passes per reader...
const PASSES_PER_READER: usize = 2;
/// ...and run at least this long.
const LEAST_MEASURE: Duration = Duration::from_secs(3);
/// One more reader stays only when it brings at least this much more.
const GAIN: f64 = 1.10;

#[derive(Debug, Clone)]
pub struct ReadPace {
    readers: usize,
    most: usize,
    best: Option<(usize, f64)>,
    settled: bool,
    since: Instant,
    work: f64,
    passes: usize,
}

/// What a judged level decided, for the log.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Step {
    /// One more reader is tried.
    More { readers: usize, rate: f64 },
    /// The run stays at this many readers.
    Settled { readers: usize, rate: f64 },
}

impl ReadPace {
    /// Starts with `start` readers and never goes above `most`.
    pub fn new(start: usize, most: usize) -> Self {
        let most = most.max(1);
        Self {
            readers: start.clamp(1, most),
            most,
            best: None,
            settled: false,
            since: Instant::now(),
            work: 0.0,
            passes: 0,
        }
    }

    /// How many readers may run now.
    pub fn readers(&self) -> usize {
        self.readers
    }

    /// The level is not measured meanwhile: the readers gave way (a player, a clip that
    /// reads alone) or had nothing to read. Measuring starts again from now.
    pub fn interrupted(&mut self) {
        self.restart(Instant::now());
    }

    /// A pass finished `work` (e.g. seconds of media read) with every reader busy.
    pub fn finished(&mut self, work: f64) -> Option<Step> {
        self.finished_at(work, Instant::now())
    }

    fn finished_at(&mut self, work: f64, now: Instant) -> Option<Step> {
        if self.settled || !(work > 0.0) {
            return None;
        }
        self.work += work;
        self.passes += 1;
        let elapsed = now.duration_since(self.since);
        if self.passes < self.readers * PASSES_PER_READER || elapsed < LEAST_MEASURE {
            return None;
        }
        let rate = self.work / elapsed.as_secs_f64();
        let step = match self.best {
            Some((_, best)) if rate < best * GAIN => {
                let (readers, best) = self.best.expect("checked");
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
        self.restart(now);
        Some(step)
    }

    fn restart(&mut self, now: Instant) {
        self.since = now;
        self.work = 0.0;
        self.passes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds a level that reads `rate` units per second until it is judged.
    fn run(pace: &mut ReadPace, start: Instant, rate: f64) -> (Instant, Step) {
        let mut now = start;
        loop {
            now += Duration::from_secs(1);
            if let Some(step) = pace.finished_at(rate, now) {
                return (now, step);
            }
        }
    }

    #[test]
    fn readers_grow_while_they_bring_more_and_settle_on_the_best() {
        let mut pace = ReadPace::new(2, 8);
        let start = pace.since;
        let (now, step) = run(&mut pace, start, 94.0);
        assert!(matches!(step, Step::More { readers: 3, .. }));
        let (now, step) = run(&mut pace, now, 130.0);
        assert!(matches!(step, Step::More { readers: 4, .. }));
        let (now, step) = run(&mut pace, now, 158.0);
        assert!(matches!(step, Step::More { readers: 5, .. }));
        // A fifth reader brings almost nothing: the run stays at four.
        let (_, step) = run(&mut pace, now, 160.0);
        assert!(matches!(step, Step::Settled { readers: 4, .. }));
        assert_eq!(pace.readers(), 4);
        assert_eq!(pace.finished(1000.0), None, "settled for the rest of the run");
    }

    #[test]
    fn a_slow_reader_goes_back_to_where_it_was_faster() {
        let mut pace = ReadPace::new(2, 8);
        let start = pace.since;
        let (now, _) = run(&mut pace, start, 28.0);
        let (_, step) = run(&mut pace, now, 24.0);
        assert!(matches!(step, Step::Settled { readers: 2, .. }));
    }

    #[test]
    fn readers_never_go_above_the_most() {
        let mut pace = ReadPace::new(2, 3);
        let start = pace.since;
        let (now, _) = run(&mut pace, start, 50.0);
        let (_, step) = run(&mut pace, now, 100.0);
        assert!(matches!(step, Step::Settled { readers: 3, .. }));
    }

    #[test]
    fn an_interruption_starts_the_measure_again() {
        let mut pace = ReadPace::new(2, 8);
        let start = pace.since;
        assert_eq!(pace.finished_at(10.0, start + Duration::from_secs(4)), None);
        pace.interrupted();
        assert_eq!(pace.passes, 0);
        assert_eq!(pace.readers(), 2);
    }
}
