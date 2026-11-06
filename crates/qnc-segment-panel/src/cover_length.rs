//! The colour of a saved cover by its length (user 2026-10-12): the montage sees at a
//! glance how long every cover is. The bands come from the layout contract of the form.

use eframe::egui::Color32;
use serde::Deserialize;

/// One band of cover lengths: up to (not including) `below_seconds`, or up to and
/// including `at_most_seconds`; a band with neither holds every longer cover.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverLengthBand {
    #[serde(default)]
    pub below_seconds: Option<f64>,
    #[serde(default)]
    pub at_most_seconds: Option<f64>,
    pub rgb: [u8; 3],
}

impl CoverLengthBand {
    fn holds(&self, seconds: f64) -> bool {
        match (self.below_seconds, self.at_most_seconds) {
            (Some(below), _) => seconds < below,
            (None, Some(at_most)) => seconds <= at_most,
            (None, None) => true,
        }
    }
}

/// The colour of the first band holding a cover of `frames` at the story `timebase`;
/// none without a timebase or bands (the timeline then uses its theme).
pub fn cover_fill(frames: u64, timebase: Option<(u32, u32)>, bands: &[CoverLengthBand]) -> Option<Color32> {
    let (num, den) = timebase.filter(|(num, den)| *num > 0 && *den > 0)?;
    let seconds = frames as f64 * f64::from(den) / f64::from(num);
    bands
        .iter()
        .find(|band| band.holds(seconds))
        .map(|band| qnc_desktop_theme::rgb(band.rgb))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bands() -> Vec<CoverLengthBand> {
        let band = |below: Option<f64>, at_most: Option<f64>, rgb: [u8; 3]| CoverLengthBand {
            below_seconds: below,
            at_most_seconds: at_most,
            rgb,
        };
        vec![
            band(Some(2.0), None, [1, 0, 0]),
            band(Some(3.0), None, [2, 0, 0]),
            band(Some(4.0), None, [3, 0, 0]),
            band(None, Some(5.0), [4, 0, 0]),
            band(None, None, [5, 0, 0]),
        ]
    }

    #[test]
    fn a_cover_takes_the_colour_of_its_length_band() {
        let at = |frames| cover_fill(frames, Some((50, 1)), &bands()).map(|c| c.r());
        assert_eq!(at(99), Some(1), "under 2 s");
        assert_eq!(at(100), Some(2), "2 s");
        assert_eq!(at(149), Some(2));
        assert_eq!(at(150), Some(3), "3 s");
        assert_eq!(at(200), Some(4), "4 s");
        assert_eq!(at(250), Some(4), "5 s is still the 5 s band");
        assert_eq!(at(251), Some(5), "over 5 s");
        assert_eq!(cover_fill(100, None, &bands()), None, "no story rate yet");
        assert_eq!(cover_fill(100, Some((50, 1)), &[]), None, "no bands");
    }
}

/// The trim zoom with every picture's slot strip in the colour of its length, as the strips
/// on the timeline (user 2026-11-05: the zoom is the timeline enlarged); one place for the Story,
/// the S-Story and a composite statement.
pub fn trim_zoom(ctx: &eframe::egui::Context, zoom: &qnc_program_view::SegmentsView, bands: &[CoverLengthBand], theme: &qnc_timeline::TimelineTheme) -> Option<qnc_action_event::ActionEvent> {
    let has_strips = zoom.edit_stills.iter().chain(&zoom.source_edit_stills).any(|still| still.strip.is_some());
    if !has_strips {
        return qnc_trim_overlay::show(ctx, zoom, theme);
    }
    let mut zoom = zoom.clone();
    let timebase = zoom.timebase;
    for strip in zoom.edit_stills.iter_mut().chain(zoom.source_edit_stills.iter_mut()).filter_map(|still| still.strip.as_mut()) {
        strip.fill = cover_fill(strip.frames, timebase, bands).map(|fill| [fill.r(), fill.g(), fill.b()]);
    }
    qnc_trim_overlay::show(ctx, &zoom, theme)
}
