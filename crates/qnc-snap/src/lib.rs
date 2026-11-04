//! Snapping (docs/98 1.2, user 2026-11-04): what the mouse drags on a timeline (an IN or OUT, an
//! M marker, a J/L side, a part's edge) sticks to the nearest cut, marker, playhead or mark when
//! it comes within `REACH_PIXELS` of it, as in every editor. S turns it off and on for this
//! process (a choice of the view, nothing is written). Pure apart from that switch.

use std::sync::atomic::{AtomicBool, Ordering};

pub const MODULE_ID: &str = "qnc.module.snap";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The catalog action that turns snapping off and on (contracts/qnc-keyboard-shortcuts.json).
pub const TOGGLE: &str = "toggle_snapping";

/// How near on the screen a drag sticks.
pub const REACH_PIXELS: f32 = 8.0;

static ON: AtomicBool = AtomicBool::new(true);

/// Whether snapping is on (on when the program starts).
pub fn enabled() -> bool {
    ON.load(Ordering::Relaxed)
}

/// S: snapping off when it was on, on when it was off; returns whether it is on now.
pub fn toggle() -> bool {
    !ON.fetch_xor(true, Ordering::Relaxed)
}

/// `frame` stuck to the nearest of `targets` within `reach_frames` (the reach in frames at the
/// timeline's zoom), when snapping is on; else `frame` as it is.
pub fn snapped(frame: u64, targets: impl IntoIterator<Item = u64>, reach_frames: f64) -> u64 {
    if !enabled() {
        return frame;
    }
    nearest(frame, targets, reach_frames).unwrap_or(frame)
}

/// The target nearest `frame` within `reach_frames`, whatever the switch.
pub fn nearest(frame: u64, targets: impl IntoIterator<Item = u64>, reach_frames: f64) -> Option<u64> {
    targets
        .into_iter()
        .map(|target| (target.abs_diff(frame), target))
        .filter(|(distance, _)| (*distance as f64) <= reach_frames)
        .min()
        .map(|(_, target)| target)
}

/// The reach in frames of `REACH_PIXELS` on a track `width` pixels wide showing `duration` frames.
pub fn reach_frames(width: f32, duration: u64) -> f64 {
    if width <= 0.0 {
        return 0.0;
    }
    f64::from(REACH_PIXELS) * duration as f64 / f64::from(width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drag_sticks_to_the_nearest_target_within_reach() {
        assert_eq!(nearest(102, [100, 110, 50], 3.0), Some(100));
        assert_eq!(nearest(106, [100, 110], 5.0), Some(110), "the nearer one");
        assert_eq!(nearest(105, [100, 112], 3.0), None, "nothing within reach");
        assert_eq!(reach_frames(800.0, 1000), 10.0);
        assert!(enabled());
        assert_eq!(snapped(102, [100], 3.0), 100);
        assert!(!toggle(), "S turns it off");
        assert_eq!(snapped(102, [100], 3.0), 102);
        assert!(toggle(), "and on again");
    }
}
