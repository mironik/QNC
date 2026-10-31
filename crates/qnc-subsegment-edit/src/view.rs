//! What the flat timeline of the parts shows: the parts on its axis, their waves, filmstrip and
//! the trim zoom of a taken edge.

use qnc_script_db::Piece;

/// One part on the flat timeline: its frames on the axis, its clip and length, and the pause
/// and join before it.
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
}

/// The parts open in place of the source timeline (user 2026-10-29): one after another on V1
/// and A1 as the player plays them, the playhead of that player.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SubsegmentView {
    /// What the parts are of: a script block's id, or the skeleton's.
    pub block_id: i64,
    pub parts: Vec<SubsegmentPart>,
    pub total_frames: u64,
    pub playhead: Option<u64>,
    /// The A1 wave of the parts on the axis (peaks over the whole of it).
    pub a1: Vec<f32>,
    /// The filmstrip of the parts on the axis, from `qnc-program-filmstrip`.
    pub pictures: Vec<qnc_program_filmstrip::ProgramPicture>,
    /// The edge being trimmed: its part, whether it is the OUT, and its frame on the axis.
    pub trim: Option<(usize, bool, u64)>,
    /// The trim zoom of the taken edge: the pictures either side of the cut, the fine A1 wave
    /// and the words around it.
    pub zoom: Option<Box<qnc_program_view::SegmentsView>>,
    /// The part whose A1 is taken (Ctrl+1), as drafted.
    pub sound: Option<usize>,
}

/// The parts on the axis (a pause before a part moves it on).
pub fn subsegment_view(key: i64, pieces: &[Piece], playhead: Option<u64>) -> SubsegmentView {
    let mut start = 0;
    let parts = pieces
        .iter()
        .enumerate()
        .map(|(index, piece)| {
            let gap = if index == 0 { 0 } else { piece.gap_before };
            start += gap;
            let end = start + piece.to_frame.saturating_sub(piece.from_frame);
            let part = SubsegmentPart {
                start,
                end,
                clip_name: piece.clip_name.clone(),
                length: piece.length.clone(),
                gap_before: gap,
                cut: piece.cut,
                a1_channel: piece.a1_channel,
                a1_level_db: piece.a1_level_db,
                a1_off: piece.a1_off,
                channels: None,
            };
            start = end;
            part
        })
        .collect();
    SubsegmentView { block_id: key, parts, total_frames: start, playhead, ..Default::default() }
}

/// Where the parts play on the axis, for its wave.
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
        })
        .collect()
}

/// Where the parts' pictures are on the axis, for its filmstrip.
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
/// nothing is), the cut's picture marked, and A1 a second either side on the axis. `rate`
/// frames a second, `clip_frames` the length of a clip.
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
    // A click on the left picture takes the OUT before the cut, on the right one the IN after it.
    let edge = |index: usize, out: bool| (crate::TAKE_EDGE.to_string(), format!("{index}:{}", if out { "out" } else { "in" }));
    let (left, right) = if out { (Some((index, true)), (index + 1 < pieces.len()).then_some((index + 1, false))) } else { (index.checked_sub(1).map(|i| (i, true)), Some((index, false))) };
    let before = EditStill { click: left.map(|(i, out)| edge(i, out)), ..before };
    let after = EditStill { click: right.map(|(i, out)| edge(i, out)), ..after };
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
