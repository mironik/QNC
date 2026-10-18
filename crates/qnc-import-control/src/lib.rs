//! Import control: the order of the background work after Select and Import. The records
//! of the clips come first (they make a clip playable), then the filmstrips (user
//! 2026-10-18: a short clip is read through, a longer one gives the intra picture nearest
//! each of 13 places: a few reads a clip), then the waves (a wave reads the sound of every
//! frame, the longest work on the medium). The wave of the clip open in a preview never
//! waits.
//!
//! It only decides; it reads nothing, writes nothing and runs no worker.

pub const MODULE_ID: &str = "qnc.module.import-control";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What the workers are told, each only when it changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// The filmstrip worker takes clips of its own (`true`) or waits (records are still
    /// being completed).
    FilmstripRuns(bool),
    /// The wave worker takes clips of its own (`true`) or makes only the open clip's wave.
    WavesRun(bool),
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
    pub fn orders(&mut self, records_completing: bool, filmstrip_busy: bool) -> Vec<Order> {
        let filmstrip = !records_completing;
        let waves = !records_completing && !filmstrip_busy;
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

    #[test]
    fn records_then_filmstrips_then_waves() {
        let mut control = ImportControl::new();
        assert_eq!(control.orders(true, false), [Order::FilmstripRuns(false), Order::WavesRun(false)]);
        assert_eq!(control.orders(true, false), [], "no change, no order");
        assert_eq!(control.orders(false, true), [Order::FilmstripRuns(true)], "records done: filmstrips");
        assert_eq!(control.orders(false, false), [Order::WavesRun(true)], "filmstrips done: waves");
        assert_eq!(control.orders(false, true), [Order::WavesRun(false)], "new filmstrips first");
    }
}
