//! The zoom is the timeline enlarged (AGENTS 0.12; user 2026-11-08: "the zoom is only an enlarged
//! part of the timeline"). One way to make it: its owner says what its axis is, and the window,
//! the playhead and the sound in reach are worked out here, the same for every owner (the source
//! timeline, a clip of an S-Story, the story, a composite statement). `position` is a field no
//! owner can leave out: a zoom that does not follow the player can no longer be written by
//! forgetting it (log 2026-11-08: the zoom of an S-Story clip edge did). The pictures either side
//! of a cut are one shape too.

use crate::{zoom_reach, zoom_window, EditStill, StillPicture, ZoomFollow, ZoomSound};

/// What the owner of a timeline says of the axis of its zoom.
pub struct ZoomAxis<'a> {
    /// The cut the zoom is around.
    pub cut: u64,
    /// The earliest and the latest frame the window must hold (the cut, or both sides of its sound).
    pub holds: (u64, u64),
    /// Frames on each side of them.
    pub half: u64,
    /// The length of the axis.
    pub total: u64,
    /// The player's confirmed frame on this axis and whether it plays; `None` while it plays
    /// something that is not on this axis.
    pub position: Option<(u64, bool)>,
    pub follow: &'a ZoomFollow,
    /// What A1 and A2 play on this axis.
    pub lanes: [Vec<qnc_program_waveform::Placement>; 2],
}

impl ZoomAxis<'_> {
    /// The zoom of this axis; the owner adds what only it knows.
    pub fn sound(self) -> ZoomSound {
        let playhead = self.follow.playhead(self.cut, self.position);
        let (from, to) = zoom_window(self.holds.0, self.holds.1, self.half, self.total, playhead);
        let reach = zoom_reach(from, to, playhead);
        let lanes = self.lanes.map(|lane| lane.into_iter().filter(|at| at.program_start < reach && from < at.program_end).collect());
        ZoomSound { from, to, cut: self.cut, lanes, playhead, ..Default::default() }
    }
}

/// The pictures either side of a cut: each with what it is called; `None` where the axis has no
/// such side (its very start or end).
pub struct CutPictures<'a> {
    pub cut: u64,
    pub before: Option<(Option<StillPicture>, &'a str)>,
    pub after: Option<(Option<StillPicture>, &'a str)>,
    /// Which of the two is the picture at the cut that moves: the one after it, or before.
    pub moving_after: bool,
}

impl CutPictures<'_> {
    pub fn stills(self) -> Vec<EditStill> {
        let side = |side: Option<(Option<StillPicture>, &str)>, after: bool| {
            side.map(|(picture, label)| {
                let still = EditStill::new(self.cut, after, picture).labelled(label);
                if after == self.moving_after {
                    still.at_cut()
                } else {
                    still
                }
            })
        };
        side(self.before, false).into_iter().chain(side(self.after, true)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(start: u64, end: u64) -> qnc_program_waveform::Placement {
        qnc_program_waveform::Placement { clip_id: "c".into(), channel: 0, program_start: start, program_end: end, source_in: 0, source_out: end - start, level_db: 0, off: false, curve: Vec::new() }
    }

    fn axis<'a>(follow: &'a ZoomFollow, position: Option<(u64, bool)>) -> ZoomAxis<'a> {
        ZoomAxis { cut: 100, holds: (100, 100), half: 50, total: 1000, position, follow, lanes: [vec![placement(0, 200), placement(900, 1000)], Vec::new()] }
    }

    #[test]
    fn every_zoom_is_a_window_of_its_axis_around_the_cut_or_the_playing_frame() {
        let follow = ZoomFollow::default();
        let still = axis(&follow, None).sound();
        assert_eq!((still.from, still.to, still.playhead), (50, 150, None));
        assert_eq!(still.lanes[0].len(), 1, "only the sound in reach");
        let played = axis(&follow, Some((400, true))).sound();
        assert_eq!((played.from, played.to, played.playhead), (350, 450, Some(400)), "it moves with the player");
        let paused = axis(&follow, Some((420, false))).sound();
        assert_eq!(paused.playhead, Some(420), "it stays where the player stopped");
    }

    #[test]
    fn the_pictures_of_a_cut_are_marked_on_the_side_that_moves() {
        let picture = |frame| Some(StillPicture { clip_id: "c".into(), source_frame: frame });
        let stills = CutPictures { cut: 10, before: Some((picture(9), "a")), after: Some((picture(10), "b")), moving_after: false }.stills();
        assert_eq!(stills.iter().map(|still| (still.after, still.cut, still.label.as_str())).collect::<Vec<_>>(), vec![(false, true, "a"), (true, false, "b")]);
        let edge = CutPictures { cut: 0, before: None, after: Some((picture(0), "b")), moving_after: true }.stills();
        assert_eq!(edge.len(), 1);
    }
}
