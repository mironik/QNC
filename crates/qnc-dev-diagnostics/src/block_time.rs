//! What each named block of a board costs to draw, summed and written once a second as
//! `ui-block-ms name=ms ...` (only with player diagnostics on). A board names its blocks;
//! this piece measures and writes, so no form keeps timing state of its own.

use std::{cell::RefCell, collections::BTreeMap, time::Instant};

thread_local! {
    static TIMES: RefCell<(Option<Instant>, BTreeMap<String, f64>)> = RefCell::new((None, BTreeMap::new()));
    /// The blocks of the frame being drawn (docs/99 step 5: a slow frame says which block took it).
    static FRAME: RefCell<BTreeMap<String, f64>> = RefCell::new(BTreeMap::new());
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
        FRAME.with(|frame| *frame.borrow_mut().entry(block.to_string()).or_default() += spent);
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

/// The blocks of the frame drawn since the last call that took 1 ms or more, as `name=ms ...`
/// (the desktop writes them with a slow frame); the frame's sums start again.
pub fn take_frame_blocks() -> String {
    FRAME.with(|frame| {
        let blocks = std::mem::take(&mut *frame.borrow_mut());
        blocks.into_iter().filter(|(_, ms)| *ms >= 1.0).map(|(name, ms)| format!("{name}={ms:.0}")).collect::<Vec<_>>().join(" ")
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_frame_gives_its_blocks_once() {
        let _ = super::take_frame_blocks();
        super::FRAME.with(|frame| frame.borrow_mut().insert("form-poll".into(), 3.4));
        assert_eq!(super::take_frame_blocks(), "form-poll=3");
        assert_eq!(super::take_frame_blocks(), "");
    }
}
