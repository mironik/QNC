//! The pictures on both sides of an edit being moved (user 2026-10-10: a professional
//! trim shows the last picture before and the first after every moved marker). For a
//! slot that is four: the last of the slot before, its own first and last, the first of
//! the slot after. They come from the draft, so they show the program as it will be
//! after Enter: the moved slot's cover goes with it, a neighbouring slot shows its own
//! cover where it still has it, else the picture of the segment, Off is black.
//! Pure: the pictures are named (clip and source frame); someone else decodes them.

use std::sync::Arc;

use crate::{source_at, CoverSpan, SegmentsView, Slot};

/// A picture of the program: a frame of a clip.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StillPicture {
    pub clip_id: String,
    pub source_frame: u64,
}

/// One picture beside a moved marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditStill {
    /// Program frame of the marker in the draft.
    pub marker_frame: u64,
    /// The first picture from the marker; else the last one before it.
    pub after: bool,
    /// `None`: no picture there (an Off segment), drawn black.
    pub picture: Option<StillPicture>,
    /// The decoded picture, once it is there.
    pub image: Option<Arc<qnc_image_assets::RgbaImage>>,
}

/// The pictures beside markers moved from `old` to `new` (`moves`), with the stored
/// slots (not the drafted ones).
pub fn edit_stills(view: &SegmentsView, slots: &[Slot], moves: &[(u64, u64)]) -> Vec<EditStill> {
    let moved = |frame: u64| moves.iter().find(|(old, _)| *old == frame).map_or(frame, |(_, new)| *new);
    let mut stills = Vec::new();
    for &(_, new) in moves {
        if new > 0 {
            stills.push(still(view, slots, &moved, new, false));
        }
        if new < view.total_frames {
            stills.push(still(view, slots, &moved, new, true));
        }
    }
    stills
}

fn still(view: &SegmentsView, slots: &[Slot], moved: &dyn Fn(u64) -> u64, marker: u64, after: bool) -> EditStill {
    let frame = if after { marker } else { marker - 1 };
    EditStill { marker_frame: marker, after, picture: picture_at(view, slots, moved, frame), image: None }
}

/// The picture at a program frame of the draft: a cover there, else the segment.
fn picture_at(view: &SegmentsView, slots: &[Slot], moved: &dyn Fn(u64) -> u64, frame: u64) -> Option<StillPicture> {
    if let Some(picture) = view.covers.iter().find_map(|cover| cover_picture(cover, slots, moved, frame)) {
        return Some(picture);
    }
    let (segment, source_frame) = source_at(view, frame)?;
    segment.kind.has_base_video().then(|| StillPicture { clip_id: segment.clip_id.clone(), source_frame })
}

/// A cover starts at its slot's start marker and keeps its length within the slot.
fn cover_picture(cover: &CoverSpan, slots: &[Slot], moved: &dyn Fn(u64) -> u64, frame: u64) -> Option<StillPicture> {
    let slot = slots.iter().find(|slot| slot.slot_id == cover.slot_id);
    let (start, slot_end) = match slot {
        Some(slot) => (moved(slot.start_frame), moved(slot.end_frame)),
        None => (moved(cover.start_frame), moved(cover.end_frame)),
    };
    let length = cover.end_frame.saturating_sub(cover.start_frame).max(1);
    let end = (start + length).min(slot_end);
    if !(start..end).contains(&frame) {
        return None;
    }
    let source = cover.source_out_frame.saturating_sub(cover.source_in_frame);
    Some(StillPicture { clip_id: cover.clip_id.clone(), source_frame: cover.source_in_frame + (frame - start) * source / length })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SegmentKind, SegmentRow};

    fn view() -> (SegmentsView, Vec<Slot>) {
        let row = |id: &str, kind, (start, end): (u64, u64), source_in: u64| SegmentRow {
            segment_id: id.into(),
            kind,
            clip_id: format!("clip-{id}"),
            source_in_frame: source_in,
            source_out_frame: source_in + (end - start),
            a1_source_channel: 0,
            start_frame: start,
            end_frame: end,
            duration_label: String::new(),
            duration_color_key: "under_3",
            selected: false,
        };
        let slot = |id: &str, start, end| Slot { slot_id: id.into(), start_frame: start, end_frame: end, has_cover: id == "s2", selected: false };
        let slots = vec![slot("s1", 0, 15), slot("s2", 15, 33), slot("s3", 33, 40)];
        let view = SegmentsView {
            rows: vec![row("a", SegmentKind::Ton, (0, 20), 100), row("b", SegmentKind::Off, (20, 40), 0)],
            covers: vec![CoverSpan {
                cover_id: "c".into(),
                slot_id: "s2".into(),
                start_frame: 15,
                end_frame: 25,
                selected: false,
                clip_id: "clip-cover".into(),
                source_in_frame: 500,
                source_out_frame: 510,
                a2_source_channel: 1,
            }],
            total_frames: 40,
            slots: slots.clone(),
            ..Default::default()
        };
        (view, slots)
    }

    fn pictures(stills: &[EditStill]) -> Vec<Option<(String, u64)>> {
        stills.iter().map(|still| still.picture.clone().map(|p| (p.clip_id, p.source_frame))).collect()
    }

    #[test]
    fn a_moved_slot_shows_four_pictures_from_the_draft() {
        let (view, slots) = view();
        // The slot 15..33 moved two frames right: its cover goes with it.
        let stills = edit_stills(&view, &slots, &[(15, 17), (33, 35)]);
        assert_eq!(
            pictures(&stills),
            vec![
                Some(("clip-a".into(), 116)), // the slot before ends on the Ton segment
                Some(("clip-cover".into(), 500)), // the cover's first picture, at 17 now
                None,                             // the slot's last picture is on Off
                None,                             // and so is the first of the slot after
            ]
        );
        assert_eq!(stills.iter().map(|s| (s.marker_frame, s.after)).collect::<Vec<_>>(), vec![(17, false), (17, true), (35, false), (35, true)]);
    }

    #[test]
    fn a_moved_marker_shows_the_picture_before_and_after_it() {
        let (view, slots) = view();
        let stills = edit_stills(&view, &slots, &[(15, 12)]);
        assert_eq!(pictures(&stills), vec![Some(("clip-a".into(), 111)), Some(("clip-cover".into(), 500))]);
    }
}
