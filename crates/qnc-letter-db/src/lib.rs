//! The fine times of the letters of a transcript's words (user 2026-11-05: "a finer timecode
//! that allows cutting by letters in the script, made in the background only for the text the
//! script shows"). A word of a clip (its start frame and text) has its letters with the clip
//! frames they are said in, fractions of a frame (a letter is 20 ms, a frame 20 or 40 ms).
//!
//! A table module of the project database intermediary (`qnc-db-broker`): it owns only its
//! table, is written only through that intermediary and knows no form, worker or application.

mod store;

use std::sync::Arc;

use qnc_db_broker::{Access, ProjectDbTarget, TableModuleFactory};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.letter-db";

pub type Result<T> = std::result::Result<T, String>;

/// One letter of a word and the clip frames `[from, to)` it is said in (none for a sign the
/// model has not, a comma).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Letter {
    pub letter: char,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<f64>,
}

/// The letters of one word of a clip: the word by its start frame and text, as the transcript has it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WordLetters {
    pub word_from: u64,
    pub text: String,
    pub letters: Vec<Letter>,
}

impl WordLetters {
    /// Every letter with the whole clip frames `[from, to)` it lasts: the aligner gives a
    /// letter where its sound starts (a short peak), so a letter lasts until the next one
    /// starts and the last until the end of its sound. `None` for a sign without frames.
    pub fn spans(&self) -> Vec<(char, Option<(u64, u64)>)> {
        let starts: Vec<Option<u64>> = self.letters.iter().map(|letter| letter.from.map(|from| from.round() as u64)).collect();
        let end = self.letters.iter().filter_map(|letter| letter.to).fold(None, |end: Option<f64>, to| Some(end.map_or(to, |end| end.max(to))));
        self.letters
            .iter()
            .enumerate()
            .map(|(index, letter)| {
                let span = starts[index].map(|from| {
                    let next = starts[index + 1..].iter().flatten().next().copied();
                    let to = next.or(end.map(|end| end.ceil() as u64)).unwrap_or(from + 1);
                    (from, to.max(from + 1))
                });
                (letter.letter, span)
            })
            .collect()
    }
}

/// The letters kept for the word of a clip that starts at `from` with `text`.
pub fn letters_of<'a>(kept: &'a [WordLetters], from: u64, text: &str) -> Option<&'a WordLetters> {
    let text = text.trim();
    kept.iter().find(|kept| kept.word_from == from && kept.text == text)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// The letters of words of a clip are kept (each replaces what its word had).
    Save { clip_id: String, engine: String, words: Vec<WordLetters> },
    /// Every word of a clip with its letters.
    ReadClip { clip_id: String },
}

impl Operation {
    pub fn is_write(&self) -> bool {
        matches!(self, Self::Save { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    Words(Vec<WordLetters>),
    Changed,
}

/// Joins the letter table to a project database.
pub struct LetterModule;

impl LetterModule {
    pub fn factory() -> Arc<dyn TableModuleFactory> {
        qnc_db_broker::owned_tables::<store::Store>()
    }
}

fn encode(operation: &Operation) -> Result<Value> {
    serde_json::to_value(operation).map_err(|e| e.to_string())
}

/// Every word of `clip_id` with its letters in the project behind `target`.
pub fn read_clip(target: &ProjectDbTarget, clip_id: &str) -> Result<Vec<WordLetters>> {
    let mut client = target.open(Access::ReadOnly, vec![LetterModule::factory()])?;
    match serde_json::from_value(client.execute(MODULE_ID, encode(&Operation::ReadClip { clip_id: clip_id.into() })?)?).map_err(|e| e.to_string())? {
        Data::Words(words) => Ok(words),
        Data::Changed => Err(qnc_ui_text::text("db.letters_reply").into()),
    }
}

/// Keeps the letters of `words` of `clip_id` (through the project's intermediary).
pub fn save(target: &ProjectDbTarget, clip_id: &str, engine: &str, words: Vec<WordLetters>) -> Result<()> {
    let mut client = target.open(Access::ReadWrite, vec![LetterModule::factory()])?;
    client.execute(MODULE_ID, encode(&Operation::Save { clip_id: clip_id.into(), engine: engine.into(), words })?)?;
    Ok(())
}

#[cfg(test)]
mod tests;
