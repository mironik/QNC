//! Import control: the order of the background work after Select and Import, fitted to
//! the medium (user 2026-10-19: "users will have all kinds of media and readers"). The
//! records of the clips come first (they make a clip playable); then every clip's coarse
//! wave (a few reads a clip, and the first of them tests the medium); then the filmstrips
//! (the processor decodes 13 pictures a clip). The full waves read every frame: on a
//! medium that serves several readers at once (a CFexpress or SSD reader) they read while
//! the filmstrips decode, on a slow one (a stick, an SD card on USB 2.0) they wait for the
//! filmstrips so the card is not choked. The wave of the clip open in a preview never
//! waits.
//!
//! It only decides; it reads nothing, writes nothing and runs no worker.

pub const MODULE_ID: &str = "qnc.module.import-control";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What the workers are told, each only when it changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// The filmstrip worker takes clips of its own (`true`) or waits.
    FilmstripRuns(bool),
    /// The wave worker takes clips of its own (`true`) or makes only the open clip's wave.
    WavesRun(bool),
}

/// The state of the work the orders follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Work {
    /// Records of clips are still being completed.
    pub records_completing: bool,
    /// Some clip has no wave at all yet (its coarse one is to come).
    pub coarse_pending: bool,
    /// The filmstrip worker still has clips to make.
    pub filmstrip_busy: bool,
    /// The medium's test found that it serves several readers at once.
    pub medium_parallel: bool,
}

#[derive(Debug, Default)]
pub struct ImportControl {
    filmstrip: Option<bool>,
    waves: Option<bool>,
}

impl ImportControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// The orders for the work as it is now.
    pub fn orders(&mut self, work: Work) -> Vec<Order> {
        let filmstrip = !work.records_completing && !work.coarse_pending;
        let waves = !work.records_completing && (work.coarse_pending || !work.filmstrip_busy || work.medium_parallel);
        let mut orders = Vec::new();
        if self.filmstrip.replace(filmstrip) != Some(filmstrip) {
            orders.push(Order::FilmstripRuns(filmstrip));
        }
        if self.waves.replace(waves) != Some(waves) {
            orders.push(Order::WavesRun(waves));
        }
        orders
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work(records_completing: bool, coarse_pending: bool, filmstrip_busy: bool, medium_parallel: bool) -> Work {
        Work { records_completing, coarse_pending, filmstrip_busy, medium_parallel }
    }

    #[test]
    fn records_then_coarse_waves_then_filmstrips_and_full_waves_after_them_on_a_slow_medium() {
        let mut control = ImportControl::new();
        assert_eq!(control.orders(work(true, true, true, false)), [Order::FilmstripRuns(false), Order::WavesRun(false)]);
        assert_eq!(control.orders(work(true, true, true, false)), [], "no change, no order");
        assert_eq!(control.orders(work(false, true, true, false)), [Order::WavesRun(true)], "records done: coarse waves");
        assert_eq!(control.orders(work(false, false, true, false)), [Order::FilmstripRuns(true), Order::WavesRun(false)], "coarse done: filmstrips");
        assert_eq!(control.orders(work(false, false, false, false)), [Order::WavesRun(true)], "filmstrips done: full waves");
    }

    #[test]
    fn on_a_fast_medium_the_full_waves_read_while_the_filmstrips_decode() {
        let mut control = ImportControl::new();
        control.orders(work(false, true, true, true));
        assert_eq!(control.orders(work(false, false, true, true)), [Order::FilmstripRuns(true)], "waves keep running");
    }
}
