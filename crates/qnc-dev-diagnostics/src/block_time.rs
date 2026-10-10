//! What each named block of a board costs to draw, summed and written once a second as
//! `ui-block-ms name=ms ...` (only with player diagnostics on). A board names its blocks;
//! this piece measures and writes, so no form keeps timing state of its own.

use std::{cell::RefCell, collections::BTreeMap, time::Instant};

thread_local! {
    static TIMES: RefCell<(Option<Instant>, BTreeMap<String, f64>)> = RefCell::new((None, BTreeMap::new()));
}

/// Draws one block and adds the time it took to its sum.
pub fn timed_block<T>(block: &str, draw: impl FnOnce() -> T) -> T {
    if !crate::player_diagnostics_enabled() {
        return draw();
    }
    let started = Instant::now();
    let result = draw();
    let spent = started.elapsed().as_secs_f64() * 1000.0;
    TIMES.with(|times| {
        let (since, sums) = &mut *times.borrow_mut();
        *sums.entry(block.to_string()).or_default() += spent;
        let since = since.get_or_insert_with(Instant::now);
        if since.elapsed().as_secs_f32() >= 1.0 {
            let parts: Vec<String> = sums.iter().map(|(name, ms)| format!("{name}={ms:.0}")).collect();
            crate::log_line(crate::DiagnosticsStream::Player, format!("ui-block-ms {}", parts.join(" ")));
            sums.clear();
            *since = Instant::now();
        }
    });
    result
}
