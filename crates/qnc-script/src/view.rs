//! What the script panel shows: the blocks with their marks, lengths and kinds, the lengths of
//! the story, its cues, and the script as text and as the export takes it. No state, no storage.

use std::collections::{BTreeMap, HashMap};

pub use qnc_script_db::{Block, Counts, KindSpec, Piece, Role, ScriptProfile, Voice, VoiceTake};

/// What the voice record of a recorded block is (user 2026-10-30: the application knows
/// whether an OFF is recorded and where).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceStatus {
    /// The take used is in the project folder and was recorded with the block's text.
    Recorded,
    /// The block's text was changed after the take used was recorded.
    TextChanged,
    /// The clip or the take used is not in the project folder.
    Missing,
}

/// A recorded block's voice record as the panel shows it: its takes (the channels of its clip),
/// the one used and its status.
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceRow {
    pub name: String,
    pub takes: Vec<VoiceTake>,
    pub chosen: u16,
    pub status: VoiceStatus,
}

impl VoiceRow {
    /// The record as it stands against its block's text and the files on this computer.
    pub fn of(voice: &Voice, present: bool, text: &str) -> Self {
        let status = if !present {
            VoiceStatus::Missing
        } else if voice.chosen_take().is_some_and(|take| take.text.trim() != text.trim()) {
            VoiceStatus::TextChanged
        } else {
            VoiceStatus::Recorded
        };
        Self { name: voice.name.clone(), takes: voice.takes.clone(), chosen: voice.chosen, status }
    }
}

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
    /// Its voice record (a recorded block that has none is not recorded yet).
    pub voice: Option<VoiceRow>,
    /// For a clip block, the words said in each of its parts with their times (user
    /// 2026-11-04: the transcript in the script has its timecodes, to edit by its text); empty
    /// for a part whose clip has no transcript there.
    pub words: Vec<Vec<TimedWord>>,
}

/// A word of a statement: its clip frames `[from, to)`, its text, the source timecode of its
/// first frame and whether a line of speech starts with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedWord {
    pub from: u64,
    pub to: u64,
    pub text: String,
    pub tc: String,
    pub line: bool,
    /// Its letters with the clip frames `[from, to)` each is said in (user 2026-11-05: cutting
    /// by letters), once the letter aligner laid them; empty before, `None` for a sign the
    /// model has not.
    pub letters: Vec<(char, Option<(u64, u64)>)>,
}

/// The words said in `[piece.from_frame, piece.to_frame)` of its clip, in order, from every
/// transcript of the clip (where two overlap, the word that starts first is kept). `tc` gives
/// the timecode of a clip frame.
pub fn timed_words(transcripts: &[qnc_transcript_db::ClipTranscript], piece: &Piece, tc: impl Fn(u64) -> String) -> Vec<TimedWord> {
    let mut words: Vec<(u64, u64, String, bool)> = Vec::new();
    for line in transcripts.iter().flat_map(|transcript| &transcript.lines) {
        for (index, word) in line.words.iter().enumerate() {
            let middle = (word.from + word.to) / 2;
            if (piece.from_frame..piece.to_frame).contains(&middle) && !word.text.trim().is_empty() {
                words.push((word.from, word.to, word.text.trim().to_string(), index == 0));
            }
        }
    }
    words.sort_by_key(|word| word.0);
    let mut kept: Vec<TimedWord> = Vec::new();
    for (from, to, text, line) in words {
        if kept.last().is_some_and(|last| from < last.to) {
            continue;
        }
        let line = line || kept.is_empty();
        kept.push(TimedWord { from, to, tc: tc(from), text, line, letters: Vec::new() });
    }
    kept
}

/// Puts on every word the letters kept for it (its start frame and text), as `qnc-letter-db`
/// says each lasts.
pub fn put_letters(words: &mut [TimedWord], kept: &[qnc_letter_db::WordLetters]) {
    for word in words {
        if let Some(found) = qnc_letter_db::letters_of(kept, word.from, &word.text) {
            word.letters = found.spans();
        }
    }
}

/// The words the script shows without letters, clip by clip in the order shown.
pub fn letter_needs(rows: &[BlockRow], names: &std::collections::HashMap<String, (String, u64, f64)>) -> Vec<qnc_letter_times::Need> {
    let mut needs = Vec::new();
    for row in rows.iter().filter(|row| row.is_clip()) {
        for (piece, words) in block_pieces(&row.block).iter().zip(&row.words) {
            let missing: Vec<qnc_letter_times::Word> = words.iter().filter(|word| word.letters.is_empty()).map(|word| qnc_letter_times::Word { from: word.from, to: word.to, text: word.text.clone() }).collect();
            if missing.is_empty() {
                continue;
            }
            // A clip whose length is not known yet waits (its rate says how long a run is).
            let Some(fps) = names.get(&piece.clip_id).filter(|(_, frames, seconds)| *frames > 0 && *seconds > 0.0).map(|(_, frames, seconds)| *frames as f64 / seconds) else { continue };
            needs.push(qnc_letter_times::Need { clip_id: piece.clip_id.clone(), fps, words: missing });
        }
    }
    needs
}

impl BlockRow {
    pub fn is_clip(&self) -> bool {
        self.role == Some(Role::Clip)
    }

    /// Whether it is a SubSegment: a clip block put together from several parts.
    pub fn is_subsegment(&self) -> bool {
        self.is_clip() && self.block.pieces.len() > 1
    }
}

/// A SubSegment as a card of the pool, a virtual clip (user 2026-10-29): its block id, its mark
/// with its first clip and number of parts, its length, and the poster of its first clip.
pub use qnc_clip_row::TabCard as SubsegmentCard;

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
    /// The story's working title (the story sheet's slug).
    pub title: Option<String>,
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
    /// Words picked in the text of a statement (Delete cuts them out of its video).
    pub picked: Option<crate::WordPick>,
    pub message: String,
}

impl ScriptView {
    /// The SubSegments of the script as cards of the pool, in reading order; `clips` gives each
    /// its first clip's poster.
    pub fn pool_cards<'a>(&self, clips: &'a [qnc_clip_row::ClipRow]) -> Vec<SubsegmentCard<'a>> {
        self.blocks
            .iter()
            .filter(|row| row.is_subsegment())
            .map(|row| SubsegmentCard {
                id: row.block.id.to_string(),
                title: qnc_ui_text::fill("script.subsegment_card", &[("mark", &row.mark), ("clip", &row.block.clip_name), ("n", &row.block.pieces.len().to_string())]).into(),
                seconds: row.seconds,
                clip: clips.iter().find(|clip| clip.clip_id == row.block.clip_id),
            })
            .collect()
    }

    /// The voice records of the script as cards of the pool's Voice tab (user 2026-10-30): the
    /// block's mark, the take used and how many there are; the card's id is `block:take`, so a
    /// click plays the take used (`VOICE_TAKE`).
    pub fn voice_cards(&self) -> Vec<SubsegmentCard<'static>> {
        self.blocks
            .iter()
            .filter_map(|row| Some((row, row.voice.as_ref()?)))
            .map(|(row, voice)| SubsegmentCard {
                id: format!("{}:{}", row.block.id, voice.chosen),
                title: qnc_ui_text::fill("script.voice_card", &[("mark", &row.mark), ("take", &voice.chosen.to_string()), ("n", &voice.takes.len().to_string())]).into(),
                seconds: voice.takes.iter().find(|take| take.take == voice.chosen).map_or(0.0, |take| take.seconds),
                clip: None,
            })
            .collect()
    }

    /// The cards of the pool's SubSegment tab or Voice tab (`voice`) and the action a click
    /// sends with the card's id.
    pub fn tab_cards<'a>(&self, voice: bool, clips: &'a [qnc_clip_row::ClipRow]) -> (Vec<SubsegmentCard<'a>>, &'static str) {
        if voice {
            (self.voice_cards(), crate::VOICE_TAKE)
        } else {
            (self.pool_cards(clips), crate::OPEN)
        }
    }

    /// How many SubSegments the script has (the pool's SubSegment tab is grey without one).
    pub fn subsegment_count(&self) -> usize {
        self.blocks.iter().filter(|row| row.is_subsegment()).count()
    }

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

pub use qnc_script_draft::{block_pieces, piece_inputs, reading_seconds};

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
            BlockRow { seconds: block_seconds(block, kind, words_per_second), mark, role: kind.map(|kind| kind.role), recorded: kind.is_some_and(|kind| kind.recorded), cues: kind.is_some_and(|kind| kind.cues), in_story: kind.is_some_and(|kind| kind.counts == Counts::Story), voice: None, words: Vec::new(), block: block.clone() }
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
    /// The frame rate of the parts (of the first part's clip), once known: a part's length in
    /// seconds and its colour as a cover's (user 2026-11-03).
    pub rate: Option<(u32, u32)>,
    /// The taken edge is the whole cut of a join: the trimmed part's OUT and the next part's IN.
    pub roll: bool,
    /// The part chosen by a click on its length strip, as a slot is.
    pub chosen: Option<usize>,
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
    SubsegmentView { block_id: block.id, parts, total_frames: start, playhead, a1: Vec::new(), pictures: Vec::new(), trim: None, zoom: None, sound: None, rate: None, roll: false, chosen: None, join_sound: None, slide: None }
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
    // A click on a picture chooses the slot it shows (user 2026-11-04: "a click on the picture
    // in the zoom selects the slot"): the left one the part before the cut, the right one the
    // part after it; a right click on the sound then takes that slot's sound at the cut.
    let slot = |index: usize| (crate::SUBSEGMENT_SELECT.to_string(), index.to_string());
    let (left, right) = if out { (Some(index), (index + 1 < pieces.len()).then_some(index + 1)) } else { (index.checked_sub(1), Some(index)) };
    // Under each picture its slot's length strip, as on the timeline (user 2026-11-05: a left
    // click on the strip selects the slot, a right click takes it whole); a right click on the
    // picture outside the strip takes the marker of this cut, as on the timeline's filmstrip.
    let strip = |index: usize| {
        let part = view.parts.get(index)?;
        let frames = part.end.saturating_sub(part.start);
        let label = view.rate.map_or_else(|| part.length.clone(), |(num, den)| qnc_program_view::duration_label(frames, num, den));
        Some(qnc_program_view::StillStrip {
            label,
            frames,
            fill: None,
            chosen: view.chosen == Some(index),
            select: slot(index),
            take: (crate::SUBSEGMENT_TAKE.to_string(), index.to_string()),
        })
    };
    let marker = crate::nearest_edge(view, cut).map(|(index, side)| (crate::TAKE_EDGE.to_string(), format!("{index}:{side}")));
    let before = EditStill { click: left.map(slot), strip: left.and_then(strip), right_click: marker.clone(), ..before };
    let after = EditStill { click: right.map(slot), strip: right.and_then(strip), right_click: marker, ..after };
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
