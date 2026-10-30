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

/// The parts as the player's program takes them.
pub fn piece_inputs(block: &Block) -> Vec<PieceInput> {
    block_pieces(block)
        .into_iter()
        .enumerate()
        .map(|(index, piece)| PieceInput {
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
                    }
                    pieces.push(piece);
                }
                pause = 0;
            }
            Role::Spoken => match voices.iter().find(|voice| voice.block_id == block.id).and_then(|voice| Some((voice, voice.chosen_take()?))) {
                Some((voice, take)) => {
                    pieces.push(PieceInput {
                        clip_id: (rates.clip_id)(&voice.name),
                        from_frame: 0,
                        to_frame: frames(take.seconds).max(1),
                        gap_before: pause,
                        crossfade: false,
                        // The take used is its channel of the record (user 2026-10-30).
                        a1: Some((Some(take.take.saturating_sub(1)), 0, false)),
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

#[cfg(test)]
mod tests;
