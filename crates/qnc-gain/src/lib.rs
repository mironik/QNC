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

/// A level curve (docs/98 2.1) at position `at`: `points` are (position, level) sorted by
/// position; between two the level goes in a line, before the first and after the last it stays
/// theirs; 0 without points. The one rule of every curve (the story, the playlist, the wave,
/// the timeline).
pub fn curve_at(points: &[(f64, f64)], at: f64) -> f64 {
    let (Some(first), Some(last)) = (points.first(), points.last()) else { return 0.0 };
    if at <= first.0 {
        return first.1;
    }
    if at >= last.0 {
        return last.1;
    }
    let after = points.iter().position(|point| point.0 > at).unwrap_or(points.len() - 1);
    let ((a, da), (b, db)) = (points[after - 1], points[after]);
    let t = if b > a { (at - a) / (b - a) } else { 1.0 };
    da + (db - da) * t
}

/// The gain of a level in tenths of a dB (a point of a level curve).
pub fn db10_to_gain(db10: f64) -> f64 {
    db_to_gain(db10 / 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_curve_is_a_line_between_its_points_and_theirs_outside() {
        let points = [(10.0, 0.0), (20.0, -100.0)];
        assert_eq!(curve_at(&points, 0.0), 0.0);
        assert_eq!(curve_at(&points, 15.0), -50.0);
        assert_eq!(curve_at(&points, 30.0), -100.0);
        assert_eq!(curve_at(&[], 5.0), 0.0);
        assert!((db10_to_gain(-200.0) - 0.1).abs() < 1e-12);
    }

    #[test]
    fn zero_db_is_unity_and_off_is_silence() {
        assert_eq!(db_to_gain(0.0), 1.0);
        assert!((db_to_gain(-20.0) - 0.1).abs() < 1e-12);
        assert!((level_gain(6, false) - 1.995_262_3).abs() < 1e-6);
        assert_eq!(level_gain(12, true), 0.0);
    }
}
