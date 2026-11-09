//! The draft of a story from its script (user 2026-10-30: "the OFFs we recorded and the SubClips
//! of statements exported as a draft story"). The blocks that count in the length of the story,
//! in reading order, become parts of one program: a recorded OFF is its voice record's clip
//! with the take used on A1, an OFF not recorded is a pause as long as its words take to read,
//! a clip block (a statement, a stand-up, natural sound) is its clip's range or its SubSegment's
//! parts. The anchor's blocks and notes are not in it. It reads and writes nothing; the caller
//! gives the script, the voice records and the rates.

use qnc_program_input::PieceInput;
use qnc_script_db::{Block, Piece, Voice};
use qnc_script_profile::{Counts, Role, ScriptProfile};

pub const MODULE_ID: &str = "qnc.module.script-draft";

/// Seconds `text` takes to read at `words_per_second`.
pub fn reading_seconds(text: &str, words_per_second: f64) -> f64 {
    if words_per_second <= 0.0 {
        return 0.0;
    }
    text.split_whitespace().count() as f64 / words_per_second
}

/// The parts of a clip block (its own range when it has no parts).
pub fn block_pieces(block: &Block) -> Vec<Piece> {
    if block.pieces.is_empty() { vec![Piece::of(block)] } else { block.pieces.clone() }
}

/// The smooth cut of the join between `before` and `after` as they are now: the id of the clip
/// of its in-between pictures and their count (those that replace the pictures around the cut
/// and one for every frame of the pause at the join: user 2026-11-09, a pause moves the parts
/// apart and the smooth cut fills it, no hole in the picture). `None` without a smooth cut or
/// where one cannot be: a J/L of the picture at the join, or parts too short for the pictures
/// it replaces.
pub fn smooth_of(before: &Piece, after: &Piece) -> Option<(String, u64)> {
    let n = after.smooth_frames;
    if n == 0 || after.v_lead_frames > 0 || after.v_tail_frames > 0 {
        return None;
    }
    let out_last = before.to_frame.checked_sub(1)?;
    let span = qnc_frame_interp::span(out_last, after.from_frame, n);
    (out_last >= span.before && span.from_picture >= before.from_frame && span.to_picture < after.to_frame).then(|| (qnc_frame_interp::clip_id(&before.clip_id, &after.clip_id, out_last, after.from_frame, n, after.gap_before), n + after.gap_before))
}

/// The parts as the player's program takes them.
pub fn piece_inputs(block: &Block) -> Vec<PieceInput> {
    let pieces = block_pieces(block);
    // A smooth cut plays once its in-between pictures are made for the join as it is now.
    let smooth: Vec<Option<(String, u64)>> = (0..pieces.len()).map(|index| index.checked_sub(1).and_then(|before| smooth_of(&pieces[before], &pieces[index])).filter(|(id, _)| *id == pieces[index].smooth_clip)).collect();
    pieces
        .into_iter()
        .zip(smooth)
        .enumerate()
        .map(|(index, (piece, smooth))| PieceInput {
            smooth,
            clip_id: piece.clip_id,
            from_frame: piece.from_frame,
            to_frame: piece.to_frame,
            gap_before: if index == 0 { 0 } else { piece.gap_before },
            crossfade: !piece.cut,
            // The part's own A1 once any of it is set; else its clip's kept sound.
            a1: (piece.a1_channel.is_some() || piece.a1_level_db != 0 || piece.a1_off).then_some((piece.a1_channel, piece.a1_level_db, piece.a1_off)),
            sound_lead: if index == 0 { 0 } else { piece.a1_lead_frames },
            sound_tail: if index == 0 { 0 } else { piece.a1_tail_frames },
            picture_lead: if index == 0 { 0 } else { piece.v_lead_frames },
            picture_tail: if index == 0 { 0 } else { piece.v_tail_frames },
            level_keys: piece.level_keys,
        })
        .collect()
}

/// What the draft is made of besides the script itself.
pub struct DraftRates<'a> {
    pub words_per_second: f64,
    /// The project's frame rate (a pause is counted in it; a voice record is made at it).
    pub fps: f64,
    /// The catalog id of a voice record's clip by its name.
    pub clip_id: &'a dyn Fn(&str) -> String,
}

/// The parts of the draft of `blocks` in reading order; an error names the block that cannot be
/// in it.
pub fn draft_pieces(blocks: &[Block], voices: &[Voice], profile: &ScriptProfile, rates: &DraftRates<'_>) -> Result<Vec<PieceInput>, String> {
    if !(rates.fps.is_finite() && rates.fps > 0.0) {
        return Err(qnc_ui_text::text("voice.clip_rate").into());
    }
    let frames = |seconds: f64| (seconds * rates.fps).round().max(0.0) as u64;
    let (mut pieces, mut pause) = (Vec::new(), 0u64);
    for block in blocks {
        let Some(kind) = profile.kind(&block.kind) else { continue };
        if kind.counts != Counts::Story {
            continue;
        }
        match kind.role {
            Role::Clip => {
                for (index, mut piece) in piece_inputs(block).into_iter().enumerate() {
                    if index == 0 {
                        piece.gap_before = pause;
                        // A cut where the script goes from one block to the next.
                        piece.crossfade = false;
                        (piece.sound_lead, piece.sound_tail, piece.picture_lead, piece.picture_tail) = (0, 0, 0, 0);
                    }
                    pieces.push(piece);
                }
                pause = 0;
            }
            Role::Spoken => match voices.iter().find(|voice| voice.block_id == block.id).and_then(|voice| Some((voice, voice.chosen_take()?))) {
                Some((voice, take)) => {
                    let (from_frame, to_frame) = take_sound_range(&block.text, take, rates.fps).unwrap_or((0, frames(take.seconds).max(1)));
                    pieces.push(PieceInput {
                        smooth: None,
                        clip_id: (rates.clip_id)(&qnc_script_db::take_name(&voice.name, take.take)),
                        from_frame,
                        to_frame,
                        gap_before: pause,
                        crossfade: false,
                        // Every reading is its own mono clip (user 2026-11-09).
                        a1: Some((Some(0), 0, false)),
                        sound_lead: 0,
                        sound_tail: 0,
                        picture_lead: 0,
                        picture_tail: 0,
                        level_keys: Vec::new(),
                    });
                    pause = 0;
                }
                None => pause += frames(reading_seconds(&block.text, rates.words_per_second)),
            },
            Role::Anchor | Role::Note => {}
        }
    }
    if pieces.is_empty() {
        return Err(qnc_ui_text::text("script.draft_empty").into());
    }
    Ok(pieces)
}

/// One segment of the skeleton of a story (user 2026-10-31: "Send to Story"): a recorded OFF is
/// an Off of its voice record, a part of a clip block a Ton.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletonPart {
    /// `tonovi` or `offovi`, as the story keeps them.
    pub kind: &'static str,
    pub clip_id: String,
    pub from_frame: u64,
    pub to_frame: u64,
    /// The part's own A1 (as `PieceInput::a1`); `None`: the sound kept for its clip.
    pub a1: Option<(Option<u16>, i8, bool)>,
    /// `script:<block>:<part>`: the block and part of the script it came from.
    pub origin: String,
    /// The M markers set in the text of an OFF, as frames of its clip.
    pub marks: Vec<u64>,
    /// The parts of a compound statement, which is one segment (user 2026-11-09); else empty.
    pub parts: Vec<PieceInput>,
}

/// The seconds of a recording `seconds` long at which the M markers in the text of `block` fall;
/// `check` is what was read and the second each heard word starts at, when it is known.
pub fn mark_seconds(block: &Block, check: Option<(&qnc_read_check::ReadCheck, &[f64])>, seconds: f64) -> Vec<f64> {
    block.marks.iter().map(|word| qnc_read_check::word_second(&block.text, check, seconds, *word as usize)).collect()
}

/// A reading this good has its IN and OUT where its sound is (user 2026-11-09: "for a correct off,
/// over 90%"); a worse one is listened to and cut by hand.
const SOUND_MARKS_PERCENT: u32 = 90;
/// Sound kept before the first word and after the last, seconds: breath of the word, not silence.
const SOUND_MARGIN: f64 = 0.1;

/// IN and OUT of the reading `take` of `text`, frames `[in, out)` of its clip, from where its first
/// word starts and its last ends; none until it is heard or when it is under 90 %.
pub fn take_sound_range(text: &str, take: &qnc_script_db::VoiceTake, fps: f64) -> Option<(u64, u64)> {
    let heard = take.heard.as_ref()?;
    let words: Vec<&str> = heard.iter().map(|word| word.text.as_str()).collect();
    if qnc_read_check::check(text, &words).percent < SOUND_MARKS_PERCENT {
        return None;
    }
    let frames = (take.seconds * fps).round().max(1.0) as u64;
    let from = (((heard.first()?.start - SOUND_MARGIN) * fps).floor().max(0.0)) as u64;
    let to = (((heard.last()?.end + SOUND_MARGIN) * fps).ceil() as u64).min(frames);
    (to > from).then_some((from, to))
}

/// The frames of the clip of `take` at which the M markers in the text of `block` fall, strictly
/// inside `[0, frames)`, each once.
fn mark_frames(block: &Block, take: &qnc_script_db::VoiceTake, fps: f64, frames: u64) -> Vec<u64> {
    let words: Vec<&str> = take.heard.iter().flatten().map(|word| word.text.as_str()).collect();
    let starts: Vec<f64> = take.heard.iter().flatten().map(|word| word.start).collect();
    let check = take.heard.is_some().then(|| qnc_read_check::check(&block.text, &words));
    let mut marks: Vec<u64> = mark_seconds(block, check.as_ref().map(|check| (check, starts.as_slice())), take.seconds).into_iter().map(|second| (second * fps).round().max(0.0) as u64).filter(|frame| (1..frames).contains(frame)).collect();
    marks.sort_unstable();
    marks.dedup();
    marks
}

/// The skeleton of `blocks`: the parts of the draft as story segments, in reading order. An OFF
/// not recorded yet is left out (user 2026-10-31), as are blocks that do not count in the story.
pub fn skeleton(blocks: &[Block], voices: &[Voice], profile: &ScriptProfile, rates: &DraftRates<'_>) -> Result<Vec<SkeletonPart>, String> {
    if !(rates.fps.is_finite() && rates.fps > 0.0) {
        return Err(qnc_ui_text::text("voice.clip_rate").into());
    }
    let mut parts = Vec::new();
    for block in blocks {
        let Some(kind) = profile.kind(&block.kind) else { continue };
        if kind.counts != Counts::Story {
            continue;
        }
        match kind.role {
            Role::Clip => {
                let pieces = piece_inputs(block);
                let Some(first) = pieces.first() else { continue };
                parts.push(SkeletonPart {
                    kind: "tonovi",
                    clip_id: first.clip_id.clone(),
                    from_frame: first.from_frame,
                    to_frame: first.to_frame,
                    a1: first.a1,
                    origin: format!("script:{}:0", block.id),
                    marks: Vec::new(),
                    parts: if pieces.len() > 1 { pieces } else { Vec::new() },
                });
            }
            Role::Spoken => {
                if let Some((voice, take)) = voices.iter().find(|voice| voice.block_id == block.id).and_then(|voice| Some((voice, voice.chosen_take()?))) {
                    let whole = ((take.seconds * rates.fps).round().max(1.0)) as u64;
                    let (from_frame, to_frame) = take_sound_range(&block.text, take, rates.fps).unwrap_or((0, whole));
                    parts.push(SkeletonPart {
                        kind: "offovi",
                        clip_id: (rates.clip_id)(&qnc_script_db::take_name(&voice.name, take.take)),
                        from_frame,
                        to_frame,
                        a1: Some((Some(0), 0, false)),
                        origin: format!("script:{}:0", block.id),
                        marks: mark_frames(block, take, rates.fps, whole),
                        parts: Vec::new(),
                    });
                }
            }
            Role::Anchor | Role::Note => {}
        }
    }
    if parts.is_empty() {
        return Err(qnc_ui_text::text("script.draft_empty").into());
    }
    Ok(parts)
}

#[cfg(test)]
mod tests;
