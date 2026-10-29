//! What the script panel shows: the blocks with their marks, lengths and kinds, the lengths of
//! the story, its cues, and the script as text and as the export takes it. No state, no storage.

use std::collections::{BTreeMap, HashMap};

pub use qnc_script_db::{Block, Counts, KindSpec, Piece, Role, ScriptProfile};

/// One block as the panel shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockRow {
    pub block: Block,
    /// Its length in seconds: a clip's own, a recording's own, text's from its words.
    pub seconds: f64,
    /// Its mark: the numbered cue of its kind (`OFF 2`, `Take SOT 1`), else the kind's short mark.
    pub mark: String,
    /// What its kind is (none when the project's profile does not have it).
    pub role: Option<Role>,
    /// Whether its kind is recorded from the microphone.
    pub recorded: bool,
    /// Whether its kind shows its first and last words (in-cue, out-cue).
    pub cues: bool,
    /// Whether it is in the length of the story.
    pub in_story: bool,
}

impl BlockRow {
    pub fn is_clip(&self) -> bool {
        self.role == Some(Role::Clip)
    }
}

/// What the script panel shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScriptView {
    pub blocks: Vec<BlockRow>,
    /// The script profile of the project (none until it is read).
    pub profile: Option<ScriptProfile>,
    /// The chosen block: new blocks go after it.
    pub selected: Option<i64>,
    /// The editor's text.
    pub editor: String,
    /// The text block the editor's text was taken from (Replace puts it back there).
    pub editing: Option<i64>,
    /// Seconds the editor's text takes to read.
    pub editor_seconds: f64,
    /// Text and clips in the length of the story.
    pub read_seconds: f64,
    pub clip_seconds: f64,
    /// Text the anchor reads in the studio (not in the length of the story).
    pub anchor_seconds: f64,
    /// The target length of the story (the story sheet's first length field).
    pub target_seconds: Option<f64>,
    /// The first words of the story and its last words (the director's cue that it ends).
    pub in_cue: String,
    pub out_cue: String,
    /// Whether the shown clip has an IN/OUT (a clip block can take it).
    pub marks: bool,
    /// The block being recorded now, the seconds recorded and the level of the microphone.
    pub recording: Option<(i64, f64, f32)>,
    /// A recording is being started or finished.
    pub record_busy: bool,
    /// Field values typed and not yet in the script, by block and field code.
    pub field_drafts: BTreeMap<(i64, String), String>,
    /// The A/V script written last (its file), shown as a link.
    pub exported: Option<String>,
    /// An export is being written.
    pub exporting: bool,
    /// The SubSegment open in place of the source timeline.
    pub subsegment: Option<SubsegmentView>,
    pub message: String,
}

impl ScriptView {
    pub fn total_seconds(&self) -> f64 {
        self.read_seconds + self.clip_seconds
    }

    /// The kinds of the profile that are text written in the editor, in its order.
    pub fn text_kinds(&self) -> Vec<&KindSpec> {
        self.profile.iter().flat_map(|profile| &profile.kinds).filter(|kind| kind.role != Role::Clip).collect()
    }

    /// The kinds of the profile that are ranges of a clip, in its order.
    pub fn clip_kinds(&self) -> Vec<&KindSpec> {
        self.profile.iter().flat_map(|profile| &profile.kinds).filter(|kind| kind.role == Role::Clip).collect()
    }

    /// The kind of block `id`, if the profile has it.
    pub fn kind_of(&self, id: i64) -> Option<&KindSpec> {
        let block = self.blocks.iter().find(|row| row.block.id == id)?;
        self.profile.as_ref()?.kind(&block.block.kind)
    }

    /// The value of field `code` of block `id` as shown: the draft while it is typed, else the
    /// value in the script.
    pub fn field_value(&self, id: i64, code: &str) -> String {
        if let Some(draft) = self.field_drafts.get(&(id, code.to_string())) {
            return draft.clone();
        }
        self.blocks.iter().find(|row| row.block.id == id).and_then(|row| row.block.fields.get(code).cloned()).unwrap_or_default()
    }

    /// The fields of a block as lines of the pasted script (`Caption: name: Ivan Horvat`).
    fn fields_text(&self, row: &BlockRow) -> String {
        let Some(kind) = self.profile.as_ref().and_then(|profile| profile.kind(&row.block.kind)) else { return String::new() };
        kind.fields
            .iter()
            .filter_map(|field| row.block.fields.get(&field.path).map(|value| format!("\n{}: {}", field.label, value)))
            .collect()
    }

    /// The script as text to paste: each block in reading order with its mark, a clip block
    /// with its clip and timecodes.
    pub fn text(&self) -> String {
        self.blocks
            .iter()
            .map(|row| {
                if row.is_clip() {
                    format!("{} {} {} - {} ({})\n{}{}", row.mark, row.block.clip_name, row.block.start_tc, row.block.end_tc, row.block.length, row.block.text, self.fields_text(row))
                } else {
                    format!("{} ({})\n{}{}", row.mark, seconds_label(row.seconds), row.block.text, self.fields_text(row))
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// Seconds as `m:ss`.
pub fn seconds_label(seconds: f64) -> String {
    let whole = seconds.max(0.0).round() as u64;
    format!("{}:{:02}", whole / 60, whole % 60)
}

/// Seconds a block of kind `kind` lasts: a clip its own, a recording its own, a note none,
/// other text the time its words take to read.
pub fn block_seconds(block: &Block, kind: Option<&KindSpec>, words_per_second: f64) -> f64 {
    match kind.map(|kind| kind.role) {
        Some(Role::Clip) => block.seconds,
        Some(Role::Note) => 0.0,
        _ if !block.voice_uri.is_empty() => block.voice_seconds,
        Some(_) => reading_seconds(&block.text, words_per_second),
        // A kind the profile does not have: a clip by its range, text by its words.
        None if !block.clip_id.is_empty() => block.seconds,
        None => reading_seconds(&block.text, words_per_second),
    }
}

/// Words at the start of a text the director takes as its in-cue.
pub const IN_CUE_WORDS: usize = 3;
/// Words at the end of a text the director takes as its out-cue.
pub const OUT_CUE_WORDS: usize = 4;

/// The first `IN_CUE_WORDS` words of `text` (`...` after them when more follow).
pub fn in_cue(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let cue = words.iter().take(IN_CUE_WORDS).copied().collect::<Vec<_>>().join(" ");
    if words.len() > IN_CUE_WORDS { format!("{cue}...") } else { cue }
}

/// The last `OUT_CUE_WORDS` words of `text` (`...` before them when more come first).
pub fn out_cue(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let cue = words[words.len().saturating_sub(OUT_CUE_WORDS)..].join(" ");
    if words.len() > OUT_CUE_WORDS { format!("...{cue}") } else { cue }
}

/// The in-cue and out-cue of the story: the first words of its first block in the length of the
/// story that has words, the last words of its last one.
pub fn story_cues(rows: &[BlockRow]) -> (String, String) {
    let spoken = || rows.iter().filter(|row| row.in_story && !row.block.text.trim().is_empty());
    (spoken().next().map(|row| in_cue(&row.block.text)).unwrap_or_default(), spoken().last().map(|row| out_cue(&row.block.text)).unwrap_or_default())
}

/// Seconds `text` takes to read at `words_per_second`.
pub fn reading_seconds(text: &str, words_per_second: f64) -> f64 {
    if words_per_second <= 0.0 {
        return 0.0;
    }
    text.split_whitespace().count() as f64 / words_per_second
}

/// The rows of `blocks`: their lengths, marks (numbered among the blocks of their kind) and
/// what their kinds are.
pub fn rows(blocks: &[Block], profile: Option<&ScriptProfile>, words_per_second: f64) -> Vec<BlockRow> {
    let mut numbers: HashMap<&str, usize> = HashMap::new();
    blocks
        .iter()
        .map(|block| {
            let kind = profile.and_then(|profile| profile.kind(&block.kind));
            let number = numbers.entry(block.kind.as_str()).or_insert(0);
            *number += 1;
            let cue = profile.map(|profile| profile.cue_label_with(&block.kind, *number, &block.fields)).unwrap_or_default();
            let mark = if !cue.is_empty() {
                cue
            } else {
                kind.map_or_else(|| block.kind.to_uppercase(), |kind| kind.short.clone())
            };
            BlockRow { seconds: block_seconds(block, kind, words_per_second), mark, role: kind.map(|kind| kind.role), recorded: kind.is_some_and(|kind| kind.recorded), cues: kind.is_some_and(|kind| kind.cues), in_story: kind.is_some_and(|kind| kind.counts == Counts::Story), block: block.clone() }
        })
        .collect()
}

/// The script as `qnc-script-export` writes it out (its title and sheet are the project's,
/// filled in when it is written).
pub fn export_story(view: &ScriptView) -> qnc_script_export::Story {
    let items = view
        .blocks
        .iter()
        .map(|row| {
            let block = &row.block;
            let kind = view.profile.as_ref().and_then(|profile| profile.kind(&block.kind));
            let fields = kind
                .map(|kind| kind.fields.iter().filter_map(|field| block.fields.get(&field.path).map(|value| (field.label.clone(), value.clone()))).collect())
                .unwrap_or_default();
            qnc_script_export::Item {
                heading: view.profile.as_ref().map_or_else(|| row.mark.clone(), |profile| profile.picture_with(&block.kind, &row.mark, row.is_clip().then(|| (block.clip_name.as_str(), block.start_tc.as_str(), block.end_tc.as_str())), &block.fields)),
                role: row.role,
                text: block.text.clone(),
                speaker: view.profile.as_ref().map(|profile| profile.speaker_with(&block.kind, &block.fields)).unwrap_or_default(),
                length: if row.role == Some(Role::Note) { String::new() } else if row.is_clip() { block.length.clone() } else { seconds_label(row.seconds) },
                seconds: row.seconds,
                in_story: row.in_story,
                fields,
                cues: (row.cues && !block.text.trim().is_empty()).then(|| (in_cue(&block.text), out_cue(&block.text))),
                recorded_tc: (!block.voice_uri.is_empty()).then(|| block.voice_tc.clone()),
            }
        })
        .collect();
    qnc_script_export::Story {
        title: String::new(),
        header: Vec::new(),
        in_cue: view.in_cue.clone(),
        out_cue: view.out_cue.clone(),
        total: seconds_label(view.total_seconds()),
        target: view.target_seconds.map(seconds_label),
        anchor: seconds_label(view.anchor_seconds),
        labels: view.profile.as_ref().map(|profile| profile.labels.clone()).unwrap_or_default(),
        items,
    }
}

/// The target of `FIELD_TEXT` and `FIELD_SET`.
#[derive(serde::Deserialize, serde::Serialize)]
pub struct FieldEdit {
    pub block: i64,
    pub field: String,
    pub value: String,
}

impl FieldEdit {
    pub fn target(block: i64, field: &str, value: &str) -> String {
        serde_json::to_string(&Self { block, field: field.into(), value: value.into() }).unwrap_or_default()
    }
}

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
}

/// The parts of a clip block (its own range when it has no parts).
pub fn block_pieces(block: &Block) -> Vec<Piece> {
    if block.pieces.is_empty() { vec![Piece::of(block)] } else { block.pieces.clone() }
}

/// The parts as the player's program takes them.
pub fn piece_inputs(block: &Block) -> Vec<qnc_source_preview::PieceInput> {
    block_pieces(block)
        .into_iter()
        .enumerate()
        .map(|(index, piece)| qnc_source_preview::PieceInput {
            clip_id: piece.clip_id,
            from_frame: piece.from_frame,
            to_frame: piece.to_frame,
            gap_before: if index == 0 { 0 } else { piece.gap_before },
            crossfade: !piece.cut,
            // The part's own A1 once any of it is set; else its clip's kept sound.
            a1: (piece.a1_channel.is_some() || piece.a1_level_db != 0 || piece.a1_off).then_some((piece.a1_channel, piece.a1_level_db, piece.a1_off)),
        })
        .collect()
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
            let part = SubsegmentPart { start, end, clip_name: piece.clip_name, length: piece.length, gap_before: gap, cut: piece.cut, a1_channel: piece.a1_channel, a1_level_db: piece.a1_level_db, a1_off: piece.a1_off, channels: None };
            start = end;
            part
        })
        .collect();
    SubsegmentView { block_id: block.id, parts, total_frames: start, playhead, a1: Vec::new(), pictures: Vec::new(), trim: None, zoom: None, sound: None }
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
    rate: u64,
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
    let (from, to) = (cut.saturating_sub(half), (cut + half).min(view.total_frames.max(cut + 1)));
    let lane = subsegment_placements(view, pieces).into_iter().filter(|at| at.program_start < to && from < at.program_end).collect();
    Some(qnc_program_view::SegmentsView {
        source_edit_stills: vec![before, after],
        zoom_sound: Some(qnc_program_view::ZoomSound { from, to, cut, lanes: [lane, Vec::new()], ..Default::default() }),
        ..Default::default()
    })
}
