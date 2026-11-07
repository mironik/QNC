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
    /// The sound of the join before this part taken (Ctrl+1 on a taken cut) and which side moves.
    pub join_sound: Option<(usize, crate::JoinSide)>,
    /// The part taken whole as a slot (Ctrl+S), moved as drafted.
    pub slide: Option<usize>,
}

/// The parts on the SubSegment's axis (a pause before a part moves it on).
pub fn subsegment_view(block: &Block, playhead: Option<u64>) -> SubsegmentView {
    let mut start = 0;
    let parts = block_pieces(block)
        .into_iter()
        .enumerate()
        .map(|(index, piece)| {
            let gap = if index == 0 { 0 } else { piece.gap_before };
            start += gap;
            let end = start + piece.to_frame.saturating_sub(piece.from_frame);
            let part = SubsegmentPart { start, end, clip_name: piece.clip_name, length: piece.length, gap_before: gap, cut: piece.cut, a1_channel: piece.a1_channel, a1_level_db: piece.a1_level_db, a1_off: piece.a1_off, channels: None, a1_lead: if index == 0 || gap > 0 { 0 } else { piece.a1_lead_frames }, a1_tail: if index == 0 || gap > 0 { 0 } else { piece.a1_tail_frames } };
            start = end;
            part
        })
        .collect();
    SubsegmentView { block_id: block.id, parts, total_frames: start, playhead, a1: Vec::new(), pictures: Vec::new(), trim: None, zoom: None, sound: None, sound_choice: None, rate: None, roll: false, chosen: None, chosen_slots: Vec::new(), join_sound: None, slide: None }
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
            curve: Vec::new(),
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
    let bounded = |still: EditStill, piece: &Piece| EditStill { clip_frames: clip_frames(&piece.clip_id), ..still };
    let (cut, before, after) = if out {
        // The next part's IN on the right, also after a pause (user 2026-10-29: left the OUT of
        // the part before, right the IN of the next); after the last part what lies past its OUT.
        let next = pieces.get(index + 1);
        let after = match next {
            Some(next) => bounded(EditStill::new(part.end, true, picture(next, next.from_frame)), next),
            None => bounded(EditStill::new(part.end, true, picture(piece, piece.to_frame)), piece),
        };
        let before = bounded(EditStill::new(part.end, false, picture(piece, piece.to_frame.saturating_sub(1))), piece).at_cut();
        (part.end, before.labelled(qnc_ui_text::text("story.still.out_last")), after.labelled(qnc_ui_text::text("story.still.after_out")))
    } else {
        // The previous part's OUT on the left; before the first part what lies before its IN.
        let previous = index.checked_sub(1).and_then(|i| pieces.get(i));
        let before = match previous {
            Some(previous) => bounded(EditStill::new(part.start, false, picture(previous, previous.to_frame.saturating_sub(1))), previous),
            None => bounded(EditStill::new(part.start, false, (piece.from_frame > 0).then(|| picture(piece, piece.from_frame - 1)).flatten()), piece),
        };
        let after = bounded(EditStill::new(part.start, true, picture(piece, piece.from_frame)), piece).at_cut();
        (part.start, before.labelled(qnc_ui_text::text("story.still.before_in")), after.labelled(qnc_ui_text::text("story.still.in_first")))
    };
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
    let half = rate.max(1);
    let playhead = follow.and_then(|(follow, position)| follow.playhead(cut, position));
    let (from, to) = qnc_program_view::zoom_window(cut, cut, half, view.total_frames, playhead);
    let reach = qnc_program_view::zoom_reach(from, to, playhead);
    let lane = subsegment_placements(view, pieces).into_iter().filter(|at| at.program_start < reach && from < at.program_end).collect();
    Some(qnc_program_view::SegmentsView {
        source_edit_stills: vec![before, after],
        zoom_sound: Some(qnc_program_view::ZoomSound { from, to, cut, lanes: [lane, Vec::new()], playhead, ..Default::default() }),
        ..Default::default()
    })
}
