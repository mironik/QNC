//! A sound level in dB as a linear gain (audit 2026-10-25: the same arithmetic was written
//! in 13 places). Pure arithmetic, no state, nothing read or written.

pub const MODULE_ID: &str = "qnc.module.gain";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The gain of `db` decibels (0 dB is 1, +6 dB about 2).
pub fn db_to_gain(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

/// The gain of a lane or part level: its dB, or silence when it is off.
pub fn level_gain(level_db: i8, off: bool) -> f32 {
    if off {
        0.0
    } else {
        db_to_gain(f64::from(level_db)) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_db_is_unity_and_off_is_silence() {
        assert_eq!(db_to_gain(0.0), 1.0);
        assert!((db_to_gain(-20.0) - 0.1).abs() < 1e-12);
        assert!((level_gain(6, false) - 1.995_262_3).abs() < 1e-6);
        assert_eq!(level_gain(12, true), 0.0);
    }
}
