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
    /// Texture identity of the picture.
    pub id: String,
    /// What the picture is, e.g. "Slot - prva" (shown under it).
    pub label: String,
    /// The picture at the cut being moved (user 2026-10-13: marked red in the zoom).
    pub cut: bool,
    /// The picture is the first or the last of its clip: the edit cannot go further
    /// that way (user 2026-10-13: a red line on that side in the zoom).
    pub clip_edge: Option<ClipEdge>,
    /// How many frames the picture's clip has, when known (nothing beyond is decoded).
    pub clip_frames: Option<u64>,
    /// The draft asked a picture past the end of its clip (it is shown at the last one).
    pub past_end: bool,
}

/// Which end of its clip a picture reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipEdge {
    Start,
    End,
}

/// Marks the pictures at the first or the last frame of their clip and brings a picture
/// beyond the end of its clip back to its last frame (a cover that runs past its clip
/// plays to its end; asking a decoder for a picture that is not there only waits).
/// `frames` gives a clip's length in its own frames.
pub fn mark_clip_edges(stills: &mut [EditStill], frames: impl Fn(&str) -> Option<u64>) {
    for still in stills {
        let Some(picture) = still.picture.as_mut() else { continue };
        let Some(length) = frames(&picture.clip_id).filter(|length| *length > 0) else { continue };
        still.clip_frames = Some(length);
        if picture.source_frame + 1 >= length {
            if picture.source_frame + 1 > length {
                picture.source_frame = length - 1;
                still.past_end = true;
                still.id = format!("qnc_edit_still:{}:{}", picture.clip_id, picture.source_frame);
            }
            still.clip_edge = Some(ClipEdge::End);
        } else if picture.source_frame == 0 {
            still.clip_edge = Some(ClipEdge::Start);
        }
    }
}

/// The pictures beside markers moved from `old` to `new` (`moves`), with the stored
/// slots (not the drafted ones).
pub fn edit_stills(view: &SegmentsView, slots: &[Slot], moves: &[(u64, u64)]) -> Vec<EditStill> {
    let moved = |frame: u64| moves.iter().find(|(old, _)| *old == frame).map_or(frame, |(_, new)| *new);
    // A slot moves two markers: the slot before, the slot itself, the slot after.
    let labels: &[[&str; 2]] = if moves.len() == 2 {
        &[[qnc_ui_text::text("story.still.previous_slot_last"), "Slot - prva"], ["Slot - zadnja", qnc_ui_text::text("story.still.next_slot_first")]]
    } else {
        &[[qnc_ui_text::text("story.still.before_marker_last"), qnc_ui_text::text("story.still.after_marker_first")]]
    };
    let mut stills = Vec::new();
    let slot = moves.len() == 2;
    for (index, (&(_, new), [before, after])) in moves.iter().zip(labels.iter().chain(std::iter::repeat(&labels[0]))).enumerate() {
        // The cut: the picture after a marker; of a slot its own first and last.
        let (cut_before, cut_after) = if slot { (index == 1, index == 0) } else { (false, true) };
        if new > 0 {
            let still = still(view, slots, &moved, new, false).labelled(before);
            stills.push(if cut_before { still.at_cut() } else { still });
        }
        if new < view.total_frames {
            let still = still(view, slots, &moved, new, true).labelled(after);
            stills.push(if cut_after { still.at_cut() } else { still });
        }
    }
    stills
}

/// The pictures of a slot trimmed at one edge by `frames` (user 2026-10-13): the slots
/// after it move with its end, so at its start the last picture of the slot before and
/// its new first one (its cover starts `frames` earlier in its source), at its end its
/// new last picture and the first of the slot after it.
pub fn trim_stills(view: &SegmentsView, slots: &[Slot], slot_id: &str, start: bool, frames: i64) -> Vec<EditStill> {
    let Some(slot) = slots.iter().find(|slot| slot.slot_id == slot_id) else {
        return Vec::new();
    };
    let total = view.total_frames;
    let new_end = slot.end_frame.saturating_add_signed(frames);
    let moved = |frame: u64| if frame >= slot.end_frame && frame < total { frame.saturating_add_signed(frames) } else { frame };
    let mut stills = Vec::new();
    if start {
        if slot.start_frame > 0 {
            stills.push(still(view, slots, &|frame| frame, slot.start_frame, false).labelled(qnc_ui_text::text("story.still.previous_slot_last")));
        }
        let cover = view.covers.iter().find(|cover| cover.slot_id == slot.slot_id);
        let first = match cover {
            Some(cover) => {
                let length = cover.end_frame.saturating_sub(cover.start_frame).max(1) as i64;
                let source = cover.source_out_frame.saturating_sub(cover.source_in_frame) as i64;
                let source_in = (cover.source_in_frame as i64 - frames * source / length).max(0);
                Some(StillPicture { clip_id: cover.clip_id.clone(), source_frame: source_in as u64 })
            }
            None => picture_at(view, &[], &|frame| frame, slot.start_frame),
        };
        stills.push(EditStill::new(slot.start_frame, true, first).labelled(qnc_ui_text::text("story.still.slot_new_first")).at_cut());
    } else {
        if new_end > 0 {
            stills.push(still(view, slots, &moved, new_end, false).labelled(qnc_ui_text::text("story.still.slot_new_last")).at_cut());
        }
        if new_end < total {
            stills.push(still(view, slots, &moved, new_end, true).labelled(qnc_ui_text::text("story.still.next_slot_first")));
        }
    }
    stills
}

impl EditStill {
    /// A picture beside the edit at `marker_frame`; `None` is drawn black.
    pub fn new(marker_frame: u64, after: bool, picture: Option<StillPicture>) -> Self {
        let id = picture
            .as_ref()
            .map_or_else(String::new, |p| format!("qnc_edit_still:{}:{}", p.clip_id, p.source_frame));
        Self { marker_frame, after, picture, image: None, id, label: String::new(), cut: false, clip_edge: None, clip_frames: None, past_end: false }
    }

    /// The same picture with what it is.
    pub fn labelled(mut self, label: &str) -> Self {
        self.label = label.to_string();
        self
    }

    /// The same picture marked as the one at the cut being moved.
    pub fn at_cut(mut self) -> Self {
        self.cut = true;
        self
    }
}

/// The picture the stored program shows at a program frame (a cover, else the segment;
/// `None` on Off or outside the program).
pub fn program_picture(view: &SegmentsView, frame: u64) -> Option<StillPicture> {
    (frame < view.total_frames).then(|| picture_at(view, &view.slots, &|frame| frame, frame)).flatten()
}

fn still(view: &SegmentsView, slots: &[Slot], moved: &dyn Fn(u64) -> u64, marker: u64, after: bool) -> EditStill {
    let frame = if after { marker } else { marker - 1 };
    EditStill::new(marker, after, picture_at(view, slots, moved, frame))
}

/// The picture at a program frame of the draft: a cover there, else the segment.
fn picture_at(view: &SegmentsView, slots: &[Slot], moved: &dyn Fn(u64) -> u64, frame: u64) -> Option<StillPicture> {
    if let Some(picture) = view.covers.iter().find_map(|cover| cover_picture(cover, slots, moved, frame)) {
        return Some(picture);
    }
    let (segment, source_frame) = source_at(view, frame)?;
    segment.kind.has_base_video().then(|| StillPicture { clip_id: segment.clip_id.clone(), source_frame })
}

/// A cover covers its whole slot, from its start marker to its end marker.
fn cover_picture(cover: &CoverSpan, slots: &[Slot], moved: &dyn Fn(u64) -> u64, frame: u64) -> Option<StillPicture> {
    let slot = slots.iter().find(|slot| slot.slot_id == cover.slot_id);
    let (old_start, old_end) = slot.map_or((cover.start_frame, cover.end_frame), |slot| (slot.start_frame, slot.end_frame));
    let (start, slot_end) = (moved(old_start), moved(old_end));
    // A cover fills its whole slot and plays its source from IN for the slot's length
    // (the store binds it so), at the story's ratio to its source (the stored span).
    let length = cover.end_frame.saturating_sub(cover.start_frame).max(1) as i64;
    if !(start..slot_end).contains(&frame) {
        return None;
    }
    // Roll edit (the store's `roll_cover`): only the start marker moved, so the cover
    // starts that much later or earlier in its source; a slot moved whole keeps it.
    let roll = if start != old_start && slot_end == old_end { start as i64 - old_start as i64 } else { 0 };
    let source = cover.source_out_frame.saturating_sub(cover.source_in_frame) as i64;
    // As far as its source allows: a cover never starts before its first picture.
    let source_in = (cover.source_in_frame as i64 + roll * source / length).max(0);
    let source_frame = source_in + (frame - start) as i64 * source / length;
    Some(StillPicture { clip_id: cover.clip_id.clone(), source_frame: source_frame as u64 })
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
                Some(("clip-cover".into(), 517)), // the cover fills the slot: its last picture
                None,                             // and so is the first of the slot after
            ]
        );
        assert_eq!(stills.iter().map(|s| (s.marker_frame, s.after)).collect::<Vec<_>>(), vec![(17, false), (17, true), (35, false), (35, true)]);
    }

    #[test]
    fn a_moved_marker_shows_the_picture_before_and_after_it() {
        let (view, slots) = view();
        let stills = edit_stills(&view, &slots, &[(15, 12)]);
        // Roll edit: the covered slot starts 3 earlier, so does its cover in its source.
        assert_eq!(pictures(&stills), vec![Some(("clip-a".into(), 111)), Some(("clip-cover".into(), 497))]);
        let stills = edit_stills(&view, &slots, &[(15, 18)]);
        assert_eq!(pictures(&stills), vec![Some(("clip-a".into(), 117)), Some(("clip-cover".into(), 503))]);
    }
}

#[cfg(test)]
mod clip_edge_tests {
    use super::*;

    #[test]
    fn a_picture_at_or_past_the_end_of_its_clip_is_marked_and_held_on_its_last_frame() {
        let still = |frame| EditStill::new(10, true, Some(StillPicture { clip_id: "c".into(), source_frame: frame }));
        let mut stills = vec![still(0), still(50), still(99), still(120)];
        mark_clip_edges(&mut stills, |_| Some(100));
        let edges: Vec<_> = stills.iter().map(|still| still.clip_edge).collect();
        assert_eq!(edges, vec![Some(ClipEdge::Start), None, Some(ClipEdge::End), Some(ClipEdge::End)]);
        assert_eq!(stills[3].picture.as_ref().unwrap().source_frame, 99, "past the end: the last frame");
    }
}
