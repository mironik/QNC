//! The script of a project's story (user 2026-10-28: Media Assist Audio, writing the offs and
//! putting the story together as text): its blocks in the order they are read, and the script
//! profile the project is written with (user 2026-10-28: the form of the script is
//! programmable, not fixed). A block's kind is a code of that profile (OFF, SOT, lead-in, or a
//! kind QNC does not foresee); the profile says what a kind is: text read by the reporter or the
//! anchor, a range of a clip with its timecodes, length and the words said there, or a note.
//! Kept in the project database, so Story builds the story from it and another computer reads
//! it as it was written, without that computer's profiles.
//!
//! A table module of the project database intermediary (`qnc-db-broker`): it owns only its
//! tables, is written only through that intermediary and knows no form or application.

mod store;

use std::{collections::BTreeMap, sync::Arc};

use qnc_db_broker::{Access, ProjectDbTarget, TableModuleFactory};
pub use qnc_script_profile::{Counts, KindSpec, Role, ScriptProfile};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.script-db";

pub type Result<T> = std::result::Result<T, String>;

/// One block of the script. `id` is given by the table; `kind` is a code of the project's
/// script profile; the timecodes and length of a clip block are written as shown when it was
/// added.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Block {
    #[serde(default)]
    pub id: i64,
    pub kind: String,
    pub text: String,
    /// The clip of a clip block (empty for text).
    #[serde(default)]
    pub clip_id: String,
    #[serde(default)]
    pub clip_name: String,
    /// Clip frames `[from, to)` of a clip block.
    #[serde(default)]
    pub from_frame: u64,
    #[serde(default)]
    pub to_frame: u64,
    #[serde(default)]
    pub start_tc: String,
    #[serde(default)]
    pub end_tc: String,
    #[serde(default)]
    pub length: String,
    /// Seconds of a clip block (a text block's length comes from its words).
    #[serde(default)]
    pub seconds: f64,
    /// The recording of a block of a recorded kind (its media URI in the project; empty until
    /// recorded).
    #[serde(default)]
    pub voice_uri: String,
    /// Seconds of the recording.
    #[serde(default)]
    pub voice_seconds: f64,
    /// The timecode the recording starts at (the time of day it was recorded).
    #[serde(default)]
    pub voice_tc: String,
    /// The values of its kind's fields, by field code (for example the caption of a statement or
    /// the lines of a graphic); a field without a value is not kept.
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
    /// The parts of a clip block put together from several ranges (empty: the block is its one
    /// range).
    #[serde(default)]
    pub pieces: Vec<Piece>,
}

impl Block {
    /// A text block of kind `kind`.
    pub fn written(kind: impl Into<String>, text: impl Into<String>) -> Self {
        Self { id: 0, kind: kind.into(), text: text.into(), clip_id: String::new(), clip_name: String::new(), from_frame: 0, to_frame: 0, start_tc: String::new(), end_tc: String::new(), length: String::new(), seconds: 0.0, voice_uri: String::new(), voice_seconds: 0.0, voice_tc: String::new(), fields: BTreeMap::new(), pieces: Vec::new() }
    }

    /// A clip block of kind `kind` made of `pieces` (at least one): its clip, IN and timecodes
    /// the first part's, its length all of them, its words theirs one under another.
    pub fn of_pieces(kind: impl Into<String>, pieces: Vec<Piece>) -> Self {
        let mut block = Self::written(kind, "");
        block.pieces = pieces;
        block.from_pieces();
        block
    }

    /// The block's range, length and words from its parts (nothing when it has none).
    pub fn from_pieces(&mut self) {
        let (Some(first), Some(last)) = (self.pieces.first().cloned(), self.pieces.last().cloned()) else { return };
        self.clip_id = first.clip_id;
        self.clip_name = first.clip_name;
        self.from_frame = first.from_frame;
        self.to_frame = first.to_frame;
        self.start_tc = first.start_tc;
        self.end_tc = last.end_tc;
        self.seconds = self.pieces.iter().map(|piece| piece.seconds).sum();
        self.length = if self.pieces.len() == 1 { first.length } else { qnc_ui_text::fill("script.pieces_length", &[("n", &self.pieces.len().to_string())]).into() };
        self.text = self.pieces.iter().map(|piece| piece.text.trim()).filter(|text| !text.is_empty()).collect::<Vec<_>>().join("\n");
    }
}

/// One part of a clip block put together from several ranges (user 2026-10-29: a statement is
/// cut to leave out pauses or repeats; the parts follow one another, their words one under
/// another).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Piece {
    pub clip_id: String,
    pub clip_name: String,
    /// Clip frames `[from, to)`.
    pub from_frame: u64,
    pub to_frame: u64,
    pub start_tc: String,
    pub end_tc: String,
    pub length: String,
    pub seconds: f64,
    /// The words said in it.
    #[serde(default)]
    pub text: String,
    /// Frames of silence before the part (user 2026-10-29: "space the sounds out so the speech
    /// sounds natural"), none before the first part.
    #[serde(default)]
    pub gap_before: u64,
    /// The part's sound joins the part before with a cut, not the project's crossfade.
    #[serde(default)]
    pub cut: bool,
    /// The camera channel heard on A1 for this part (zero based; user 2026-10-29: channel and
    /// level per part); `None`: the channel kept for its clip on the source timeline.
    #[serde(default)]
    pub a1_channel: Option<u16>,
    /// Its A1 level in dB (0 as recorded) and whether its A1 is off.
    #[serde(default)]
    pub a1_level_db: i8,
    #[serde(default)]
    pub a1_off: bool,
}

impl Piece {
    /// The block's own range as its first part (a clip block that had one range).
    pub fn of(block: &Block) -> Self {
        Self { clip_id: block.clip_id.clone(), clip_name: block.clip_name.clone(), from_frame: block.from_frame, to_frame: block.to_frame, start_tc: block.start_tc.clone(), end_tc: block.end_tc.clone(), length: block.length.clone(), seconds: block.seconds, text: block.text.clone(), gap_before: 0, cut: false, a1_channel: None, a1_level_db: 0, a1_off: false }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// The blocks in reading order.
    Read,
    /// The profile the project's script is written with (none yet: the script was never
    /// written, or was written before profiles).
    ReadProfile,
    /// The project's script is written with `profile` from now on; refused when it cannot be
    /// used or lacks a kind a block of the script has.
    SetProfile { profile: ScriptProfile },
    /// A block is added after block `after` (at the end without it, or first).
    Add { after: Option<i64>, #[serde(default)] first: bool, block: Block },
    /// The text of a block changes.
    SetText { id: i64, text: String },
    /// A block goes one place up or down.
    Move { id: i64, up: bool },
    /// A block leaves the script.
    Remove { id: i64 },
    /// A block of a recorded kind got its recording (an empty URI takes it away).
    SetVoice { id: i64, uri: String, seconds: f64, tc: String },
    /// The values of a block's fields (each one a field of its kind; an empty value is none).
    SetFields { id: i64, fields: BTreeMap<String, String> },
    /// Clip block `id` gets `piece` as its next part (a block of one range keeps that range as
    /// its first part).
    AddPiece { id: i64, piece: Piece },
    /// The pause before part `index` (frames) and whether its sound is cut (else crossed).
    SetJoin { id: i64, index: usize, gap_before: u64, cut: bool },
    /// Part `index` with a new range (its pause and join stay).
    SetPiece { id: i64, index: usize, piece: Piece },
}

impl Operation {
    pub fn is_write(&self) -> bool {
        !matches!(self, Self::Read | Self::ReadProfile)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    Blocks(Vec<Block>),
    Profile(Option<ScriptProfile>),
    Added(i64),
    Changed,
}

/// Joins the script tables to a project database.
pub struct ScriptModule;

impl ScriptModule {
    pub fn factory() -> Arc<dyn TableModuleFactory> {
        qnc_db_broker::owned_tables::<store::Store>()
    }
}

fn encode(operation: &Operation) -> Result<Value> {
    serde_json::to_value(operation).map_err(|e| e.to_string())
}

fn reply() -> String {
    qnc_ui_text::text("db.script_reply").into()
}

fn read_only(target: &ProjectDbTarget, operation: &Operation) -> Result<Data> {
    let mut client = target.open(Access::ReadOnly, vec![ScriptModule::factory()])?;
    let answer = client.execute(MODULE_ID, encode(operation)?)?;
    serde_json::from_value(answer).map_err(|e| e.to_string())
}

/// The script of the project behind `target`, in reading order.
pub fn read(target: &ProjectDbTarget) -> Result<Vec<Block>> {
    match read_only(target, &Operation::Read)? {
        Data::Blocks(blocks) => Ok(blocks),
        _ => Err(reply()),
    }
}

/// The profile the project's script is written with.
pub fn read_profile(target: &ProjectDbTarget) -> Result<Option<ScriptProfile>> {
    match read_only(target, &Operation::ReadProfile)? {
        Data::Profile(profile) => Ok(profile),
        _ => Err(reply()),
    }
}

/// The project's script is written with `profile` from now on.
pub fn set_profile(target: &ProjectDbTarget, profile: ScriptProfile) -> Result<()> {
    write(target, Operation::SetProfile { profile }).map(drop)
}

/// Adds `block` after block `after` (at the end without it); the id it was given.
pub fn add(target: &ProjectDbTarget, after: Option<i64>, block: Block) -> Result<i64> {
    add_at(target, after.map_or(Place::Last, Place::After), block)
}

/// Where a new block goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    First,
    Last,
    After(i64),
}

/// Adds `block` at `place`; the id it was given.
pub fn add_at(target: &ProjectDbTarget, place: Place, block: Block) -> Result<i64> {
    let (after, first) = match place {
        Place::First => (None, true),
        Place::Last => (None, false),
        Place::After(id) => (Some(id), false),
    };
    match write(target, Operation::Add { after, first, block })? {
        Data::Added(id) => Ok(id),
        _ => Err(reply()),
    }
}

/// The text of block `id` becomes `text`.
pub fn set_text(target: &ProjectDbTarget, id: i64, text: String) -> Result<()> {
    write(target, Operation::SetText { id, text }).map(drop)
}

/// Block `id` goes one place up (or down).
pub fn move_block(target: &ProjectDbTarget, id: i64, up: bool) -> Result<()> {
    write(target, Operation::Move { id, up }).map(drop)
}

/// Block `id` of a recorded kind got the recording at `uri`, `seconds` long, starting at `tc`.
pub fn set_voice(target: &ProjectDbTarget, id: i64, uri: String, seconds: f64, tc: String) -> Result<()> {
    write(target, Operation::SetVoice { id, uri, seconds, tc }).map(drop)
}

/// The fields of block `id` become `fields`.
pub fn set_fields(target: &ProjectDbTarget, id: i64, fields: BTreeMap<String, String>) -> Result<()> {
    write(target, Operation::SetFields { id, fields }).map(drop)
}

/// Clip block `id` gets `piece` as its next part.
pub fn add_piece(target: &ProjectDbTarget, id: i64, piece: Piece) -> Result<()> {
    write(target, Operation::AddPiece { id, piece }).map(drop)
}

/// The pause before part `index` of block `id` and its join (cut or crossfade).
pub fn set_join(target: &ProjectDbTarget, id: i64, index: usize, gap_before: u64, cut: bool) -> Result<()> {
    write(target, Operation::SetJoin { id, index, gap_before, cut }).map(drop)
}

/// Block `id` leaves the script.
pub fn remove(target: &ProjectDbTarget, id: i64) -> Result<()> {
    write(target, Operation::Remove { id }).map(drop)
}

fn write(target: &ProjectDbTarget, operation: Operation) -> Result<Data> {
    let writer = qnc_db_broker::ProjectDbWriter::spawn(target.clone(), vec![ScriptModule::factory()])?;
    let answer = writer.call(MODULE_ID, encode(&operation)?)?;
    serde_json::from_value(answer).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;

/// Part `index` of block `id` with a new range (an edge trimmed on the SubSegment timeline).
pub fn set_piece(target: &ProjectDbTarget, id: i64, index: usize, piece: Piece) -> Result<()> {
    write(target, Operation::SetPiece { id, index, piece }).map(drop)
}
