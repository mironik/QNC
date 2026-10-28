//! The script of a project's story (user 2026-10-28: Media Assist Audio, writing the offs and
//! putting the story together as text): its blocks in the order they are read. An OFF block
//! is the text the journalist writes and later records; a statement (SOT) is a range of a
//! clip with its timecodes, length and the words said there. Kept in the project database,
//! so Story builds the story from it and another computer reads it.
//!
//! A table module of the project database intermediary (`qnc-db-broker`): it owns only its
//! table, is written only through that intermediary and knows no form or application.

mod store;

use std::sync::Arc;

use qnc_db_broker::{Access, ProjectDbTarget, TableModuleFactory};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.script-db";

pub type Result<T> = std::result::Result<T, String>;

/// What a block of the script is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    /// Text the journalist writes and records (voice-over).
    Off,
    /// A statement: a range of a clip and what is said in it.
    Sot,
}

/// One block of the script. `id` is given by the table; the timecodes and length of a
/// statement are written as shown when it was added.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Block {
    #[serde(default)]
    pub id: i64,
    pub kind: BlockKind,
    pub text: String,
    /// The clip of a statement (empty for an OFF block).
    #[serde(default)]
    pub clip_id: String,
    #[serde(default)]
    pub clip_name: String,
    /// Clip frames `[from, to)` of a statement.
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
    /// Seconds of a statement (an OFF block's length comes from its words).
    #[serde(default)]
    pub seconds: f64,
}

impl Block {
    /// An OFF block of `text`.
    pub fn off(text: impl Into<String>) -> Self {
        Self { id: 0, kind: BlockKind::Off, text: text.into(), clip_id: String::new(), clip_name: String::new(), from_frame: 0, to_frame: 0, start_tc: String::new(), end_tc: String::new(), length: String::new(), seconds: 0.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// The blocks in reading order.
    Read,
    /// A block is added after block `after` (at the end without it).
    Add { after: Option<i64>, block: Block },
    /// The text of a block changes.
    SetText { id: i64, text: String },
    /// A block goes one place up or down.
    Move { id: i64, up: bool },
    /// A block leaves the script.
    Remove { id: i64 },
}

impl Operation {
    pub fn is_write(&self) -> bool {
        !matches!(self, Self::Read)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    Blocks(Vec<Block>),
    Added(i64),
    Changed,
}

/// Joins the script table to a project database.
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

/// The script of the project behind `target`, in reading order.
pub fn read(target: &ProjectDbTarget) -> Result<Vec<Block>> {
    let mut client = target.open(Access::ReadOnly, vec![ScriptModule::factory()])?;
    let answer = client.execute(MODULE_ID, encode(&Operation::Read)?)?;
    match serde_json::from_value(answer).map_err(|e| e.to_string())? {
        Data::Blocks(blocks) => Ok(blocks),
        _ => Err(reply()),
    }
}

/// Adds `block` after block `after` (at the end without it); the id it was given.
pub fn add(target: &ProjectDbTarget, after: Option<i64>, block: Block) -> Result<i64> {
    match write(target, Operation::Add { after, block })? {
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
