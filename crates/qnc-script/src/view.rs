//! What the script panel shows: the blocks with their marks, lengths and kinds, the lengths of
//! the story, its cues, and the script as text and as the export takes it. No state, no storage.

use std::collections::{BTreeMap, HashMap};

use qnc_subsegment_edit::SubsegmentView;

pub use qnc_read_check::{ReadCheck, Verdict};
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
    /// What was read in each take already heard, against the block's text.
    pub checks: Vec<(u16, std::sync::Arc<ReadCheck>)>,
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
        Self { name: voice.name.clone(), takes: voice.takes.clone(), chosen: voice.chosen, status, checks: Vec::new() }
    }

    /// What was read in take `take`, once it is heard.
    pub fn check_of(&self, take: u16) -> Option<&ReadCheck> {
        self.checks.iter().find(|(number, _)| *number == take).map(|(_, check)| check.as_ref())
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
/// the timecode of a clip frame. A word whose letters are laid (`letters`, as `qnc-letter-db` says
/// each lasts) shows what of it the part still says, letter by letter (user 2026-11-08: of "pa"
/// the p was cut and the a stayed, heard but not shown); one without shows whole if its middle is
/// in the part.
pub fn timed_words(transcripts: &[qnc_transcript_db::ClipTranscript], piece: &Piece, letters: &[qnc_letter_db::WordLetters], tc: impl Fn(u64) -> String) -> Vec<TimedWord> {
    let range = piece.from_frame..piece.to_frame;
    let mut words: Vec<(u64, u64, String, bool, Vec<(char, Option<(u64, u64)>)>)> = Vec::new();
    for line in transcripts.iter().flat_map(|transcript| &transcript.lines) {
        for (index, word) in line.words.iter().enumerate() {
            let text = word.text.trim();
            if text.is_empty() {
                continue;
            }
            let spans = qnc_letter_db::letters_of(letters, word.from, text).map_or_else(Vec::new, |found| found.spans());
            if spans.is_empty() {
                if range.contains(&((word.from + word.to) / 2)) {
                    words.push((word.from, word.to, text.to_string(), index == 0, Vec::new()));
                }
                continue;
            }
            // A letter counts by its own middle; a sign without a time goes with the letter before.
            let mut inside = false;
            let flags: Vec<bool> = spans
                .iter()
                .map(|(_, span)| {
                    if let Some((from, to)) = span {
                        inside = range.contains(&((from + to) / 2));
                    }
                    inside
                })
                .collect();
            if flags.iter().all(|flag| *flag) {
                words.push((word.from, word.to, text.to_string(), index == 0, spans));
            } else if flags.iter().any(|flag| *flag) {
                let said: Vec<(char, Option<(u64, u64)>)> = spans.into_iter().zip(flags).filter_map(|(letter, flag)| flag.then_some(letter)).collect();
                let times: Vec<(u64, u64)> = said.iter().filter_map(|(_, span)| *span).collect();
                let (Some(first), Some(last)) = (times.first(), times.last()) else { continue };
                words.push((first.0, last.1, said.iter().map(|(letter, _)| *letter).collect(), index == 0, said));
            }
        }
    }
    words.sort_by_key(|word| word.0);
    let mut kept: Vec<TimedWord> = Vec::new();
    for (from, to, text, line, letters) in words {
        if kept.last().is_some_and(|last| from < last.to) {
            continue;
        }
        let line = line || kept.is_empty();
        kept.push(TimedWord { from, to, tc: tc(from), text, line, letters });
    }
    kept
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
    /// The text of a statement's part as the user edits it before Enter (docs/99 step 4: the
    /// script holds it, not egui memory), by block and part: the text as said and as edited.
    pub text_drafts: BTreeMap<(i64, usize), (String, String)>,
    /// The A/V script written last (its file), shown as a link.
    pub exported: Option<String>,
    /// An export is being written.
    pub exporting: bool,
    /// Why the takes are not being checked against their text, if they are not.
    pub read_note: Option<String>,
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

    /// The edited text of part `index` of block `id` while its words are still `said` (the
    /// draft of a text that was said differently is gone).
    pub fn text_draft(&self, id: i64, index: usize, said: &str) -> Option<&str> {
        self.text_drafts.get(&(id, index)).filter(|(was, _)| was == said).map(|(_, edited)| edited.as_str())
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
