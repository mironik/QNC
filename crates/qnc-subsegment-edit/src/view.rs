//! What the open SubSegment shows: its parts on its own axis (a pause before a part moves it
//! on), the A1 wave and filmstrip placed on that axis, and the trim zoom of a taken edge. No
//! state, no storage.

use qnc_script_db::{Block, Piece};
pub use qnc_script_draft::{block_pieces, piece_inputs};


/// One part of the SubSegment open on the source timeline's place: its frames on the
/// SubSegment's axis, its clip and length, and the pause and join before it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubsegmentPart {
    pub start: u64,
    pub end: u64,
    pub clip_name: String,
    pub length: String,
    pub gap_before: u64,
    pub cut: bool,
    /// Its own A1: the camera channel (`None`: the one kept for its clip), level in dB, off.
    pub a1_channel: Option<u16>,
    pub a1_level_db: i8,
    pub a1_off: bool,
    /// How many channels its clip has (from its stored wave), once read.
    pub channels: Option<u16>,
    /// The J and L of the join before it (frames), as drafted.
    pub a1_lead: i64,
    pub a1_tail: i64,
    /// The pictures the smooth cut of the join before it replaces (0: none) and its state:
    /// `Some(true)` playing, `Some(false)` being made, `None` it cannot be at this join.
    pub smooth_frames: u64,
    pub smooth_ready: Option<bool>,
}

/// The SubSegment open in place of the source timeline (user 2026-10-29): its parts one after
/// another on V1 and A1 as the player plays them, the playhead of that player.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SubsegmentView {
    pub block_id: i64,
    pub parts: Vec<SubsegmentPart>,
    pub total_frames: u64,
    pub playhead: Option<u64>,
    /// The A1 wave of the parts on the SubSegment's axis (peaks over the whole of it).
    pub a1: Vec<f32>,
    /// The filmstrip of the parts on the SubSegment's axis (user 2026-10-29: as a Wrap row
    /// shows its segments), from `qnc-program-filmstrip`.
    pub pictures: Vec<qnc_program_filmstrip::ProgramPicture>,
    /// The edge being trimmed: its part, whether it is the OUT, and its frame on the axis.
    pub trim: Option<(usize, bool, u64)>,
    /// The trim zoom of the taken edge (user 2026-10-29: as a cover's): the pictures either
    /// side of the cut, the fine A1 wave and the words around it.
    pub zoom: Option<Box<qnc_program_view::SegmentsView>>,
    /// The part whose A1 is taken (Ctrl+1), as drafted.
    pub sound: Option<usize>,
    /// The channel column and level of the taken A1 (as the source timeline shows its lane,
    /// AGENTS 0.12): drafted, open while it is taken.
    pub sound_choice: Option<qnc_lane_choice::ChannelChoice>,
    /// The frame rate of the parts (of the first part's clip), once known: a part's length in
    /// seconds and its colour as a cover's (user 2026-11-03).
    pub rate: Option<(u32, u32)>,
    /// The taken edge is the whole cut of a join: the trimmed part's OUT and the next part's IN.
    pub roll: bool,
    /// The part chosen by a click on its length strip, as a slot is.
    pub chosen: Option<usize>,
    /// Every chosen slot (several after a drag over their strips) and every slot whose A1 is
    /// changed with the taken one (user 2026-11-07).
    pub chosen_slots: Vec<usize>,
    /// The level curves of the parts' A1 on the axis (the taken one as drafted; user 2026-11-07,
    /// as a segment's on a Wrap row).
    pub level_curves: Vec<qnc_program_view::LevelCurve>,
    /// The sound of the join before this part taken (Ctrl+1 on a taken cut) and which side moves.
    pub join_sound: Option<(usize, crate::JoinSide)>,
    /// The part taken whole as a slot (Ctrl+S), moved as drafted.
    pub slide: Option<usize>,
    /// The narrow video row instead of the filmstrip (Ctrl+click on V, as on a Wrap row).
    pub video_compact: bool,
    /// The window of the part enlarged by a double click (70/15/15, as a story's slot).
    pub enlarged: Option<(u64, u64)>,
}

/// The parts on the SubSegment's axis (a pause before a part moves it on).
pub fn subsegment_view(block: &Block, playhead: Option<u64>) -> SubsegmentView {
    let mut start = 0;
    let pieces = block_pieces(block);
    let smooth: Vec<Option<bool>> = (0..pieces.len()).map(|index| index.checked_sub(1).and_then(|before| qnc_script_draft::smooth_of(&pieces[before], &pieces[index])).map(|(id, _)| id == pieces[index].smooth_clip)).collect();
    let parts = pieces
        .into_iter()
        .zip(smooth)
        .enumerate()
        .map(|(index, (piece, smooth_ready))| {
            let gap = if index == 0 { 0 } else { piece.gap_before };
            start += gap;
            let end = start + piece.to_frame.saturating_sub(piece.from_frame);
            let part = SubsegmentPart { start, end, clip_name: piece.clip_name, length: piece.length, gap_before: gap, cut: piece.cut, a1_channel: piece.a1_channel, a1_level_db: piece.a1_level_db, a1_off: piece.a1_off, channels: None, a1_lead: if index == 0 || gap > 0 { 0 } else { piece.a1_lead_frames }, a1_tail: if index == 0 || gap > 0 { 0 } else { piece.a1_tail_frames }, smooth_frames: if index == 0 { 0 } else { piece.smooth_frames }, smooth_ready };
            start = end;
            part
        })
        .collect();
    SubsegmentView { block_id: block.id, parts, total_frames: start, playhead, a1: Vec::new(), pictures: Vec::new(), trim: None, zoom: None, sound: None, sound_choice: None, rate: None, roll: false, chosen: None, chosen_slots: Vec::new(), level_curves: Vec::new(), join_sound: None, slide: None, video_compact: false, enlarged: None }
}

/// Where the parts play on the SubSegment's axis, for its wave (channel 1 of each clip: the
/// wave of A1 as recorded).
pub fn subsegment_placements(view: &SubsegmentView, pieces: &[Piece]) -> Vec<qnc_program_waveform::Placement> {
    view.parts
        .iter()
        .zip(pieces)
        .map(|(part, piece)| qnc_program_waveform::Placement {
            clip_id: piece.clip_id.clone(),
            channel: piece.a1_channel.unwrap_or(0),
            program_start: part.start,
            program_end: part.end,
            source_in: piece.from_frame,
            source_out: piece.to_frame,
            level_db: piece.a1_level_db,
            off: piece.a1_off,
            // The wave follows the part's level curve, as a segment's does.
            curve: piece.level_keys.clone(),
        })
        .collect()
}

/// Where the parts' pictures are on the SubSegment's axis, for its filmstrip.
pub fn subsegment_pictures(view: &SubsegmentView, pieces: &[Piece]) -> Vec<qnc_program_filmstrip::PicturePlacement> {
    view.parts
        .iter()
        .zip(pieces)
        .map(|(part, piece)| qnc_program_filmstrip::PicturePlacement {
            clip_id: piece.clip_id.clone(),
            program_start: part.start,
            program_end: part.end,
            source_in: piece.from_frame,
            source_out: piece.to_frame,
        })
        .collect()
}

/// The trim zoom of the edge taken on part `index` (`out`: its OUT), as a cover's edge
/// shows it: the last picture before the cut and the first after it (black where a pause or
/// nothing is), the cut's picture marked, and A1 a second either side on the SubSegment's
/// axis. `rate` frames a second, `clip_frames` the length of a clip.
pub fn subsegment_zoom(
    view: &SubsegmentView,
    pieces: &[Piece],
    (index, out): (usize, bool),
    (rate, follow): (u64, Option<(&qnc_program_view::ZoomFollow, Option<(u64, bool)>)>),
    clip_frames: impl Fn(&str) -> Option<u64>,
) -> Option<qnc_program_view::SegmentsView> {
    use qnc_program_view::{EditStill, StillPicture};
    let (part, piece) = (view.parts.get(index)?, pieces.get(index)?);
    let picture = |piece: &Piece, frame: u64| Some(StillPicture { clip_id: piece.clip_id.clone(), source_frame: frame });
    let pictures = if out {
        // The next part's IN on the right, also after a pause (user 2026-10-29: left the OUT of
        // the part before, right the IN of the next); after the last part what lies past its OUT.
        let after = pieces.get(index + 1).map_or_else(|| picture(piece, piece.to_frame), |next| picture(next, next.from_frame));
        qnc_program_view::CutPictures { cut: part.end, before: Some((picture(piece, piece.to_frame.saturating_sub(1)), qnc_ui_text::text("story.still.out_last"))), after: Some((after, qnc_ui_text::text("story.still.after_out"))), moving_after: false }
    } else {
        // The previous part's OUT on the left; before the first part what lies before its IN.
        let before = match index.checked_sub(1).and_then(|i| pieces.get(i)) {
            Some(previous) => picture(previous, previous.to_frame.saturating_sub(1)),
            None => (piece.from_frame > 0).then(|| picture(piece, piece.from_frame - 1)).flatten(),
        };
        qnc_program_view::CutPictures { cut: part.start, before: Some((before, qnc_ui_text::text("story.still.before_in"))), after: Some((picture(piece, piece.from_frame), qnc_ui_text::text("story.still.in_first"))), moving_after: true }
    };
    let cut = pictures.cut;
    let mut sides = pictures.stills().into_iter().map(|still| EditStill { clip_frames: still.picture.as_ref().and_then(|picture| clip_frames(&picture.clip_id)), ..still });
    let (before, after) = (sides.next()?, sides.next()?);
    // The zoom is the timeline enlarged (AGENTS 0.12, user 2026-11-05: "one action works the same
    // in every situation, a right click is Ctrl+"): each picture stands on a frame of the
    // SubSegment's axis and a right click on it, or on its slot's length strip under it, sends
    // what the timeline's mouse map gives the same gesture at that frame (the marker there; the
    // slot and its nearer edge). A left click on the picture or the strip picks that side's slot
    // and never closes the zoom (user 2026-11-05: "mark what is chosen").
    let (left, right) = if out { (Some(index), (index + 1 < pieces.len()).then_some(index + 1)) } else { (index.checked_sub(1), Some(index)) };
    let on_axis = |side: Option<usize>, last: bool| {
        side.and_then(|index| view.parts.get(index)).map_or(cut, |part| if last { part.end.saturating_sub(1).max(part.start) } else { part.start })
    };
    let action = |action: &str, target: Option<String>, frame: u64| qnc_program_view::StillAction::new(action, target, Some(frame));
    let strip = |index: usize, frame: u64| {
        let part = view.parts.get(index)?;
        let frames = part.end.saturating_sub(part.start);
        let label = view.rate.map_or_else(|| part.length.clone(), |(num, den)| qnc_program_view::duration_label(frames, num, den));
        let id = Some(format!("part-{index}"));
        Some(qnc_program_view::StillStrip {
            label,
            frames,
            fill: None,
            chosen: view.chosen == Some(index),
            // A left click picks this side's slot (user 2026-11-05: "a click in the zoom must not
            // close it, it marks what is chosen"), the edge held stays.
            select: Some(qnc_program_view::StillAction::new(crate::SUBSEGMENT_SELECT, Some(index.to_string()), None)),
            take: action("timeline_take_cover", id, frame),
        })
    };
    let still = |still: EditStill, side: Option<usize>, last: bool| {
        let frame = on_axis(side, last);
        let edge = crate::nearest_edge(view, frame).map(|(index, side)| format!("edge-{index}-{}", if side == "in" { "in" } else { "out" }));
        EditStill {
            click: None,
            strip: side.and_then(|index| strip(index, frame)),
            right_click: edge.map(|edge| action("select_marker", Some(edge), frame)),
            ..still
        }
    };
    let (before, after) = (still(before, left, true), still(after, right, false));
    let idle = qnc_program_view::ZoomFollow::default();
    let (follow, position) = follow.unwrap_or((&idle, None));
    let axis = qnc_program_view::ZoomAxis { cut, holds: (cut, cut), half: rate.max(1), total: view.total_frames, position, follow, lanes: [subsegment_placements(view, pieces), Vec::new()] };
    Some(qnc_program_view::SegmentsView {
        source_edit_stills: vec![before, after],
        zoom_sound: Some(axis.sound()),
        ..Default::default()
    })
}
