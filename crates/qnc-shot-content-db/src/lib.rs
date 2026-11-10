//! What the usable parts of the clips of a project show (user 2026-11-10: sorting the B-roll by
//! content): for each usable part of a clip's shot analysis one vector of what its pictures show,
//! the frames it was seen in and the model that saw it, or why the clip was not looked at. The
//! vectors are kept, not the labels: which label a part gets is worked out when it is read, so a
//! newsroom's own list of labels needs no new look at the pictures. Kept in the project database,
//! so a clip is looked at once and every form, and another computer, reads it.
//!
//! A table module of the project database intermediary (`qnc-db-broker`): it owns only its
//! table, is written only through that intermediary and knows no form, worker or application.

mod store;

use std::sync::Arc;

use qnc_db_broker::{Access, ProjectDbTarget, TableModuleFactory, WriteLease};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.shot-content-db";

pub type Result<T> = std::result::Result<T, String>;

/// How the content is made; a row made another way is looked at again.
pub const CONTENT_VERSION: u32 = 1;

/// What one usable part of a clip shows: clip frames `[from, to)` (as the shot analysis has
/// them), the frames its pictures were taken at and their vector (unit length).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartContent {
    pub from: u64,
    pub to: u64,
    pub frames: Vec<u64>,
    pub vector: Vec<f32>,
}

/// What the parts of one clip show, by which model; or why the clip was not looked at
/// (`error` not empty, no parts).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipContent {
    pub clip_id: String,
    /// The model the vectors come from (vectors of different models are not compared).
    pub model: String,
    pub parts: Vec<PartContent>,
    pub error: String,
    /// How long the look took (milliseconds).
    pub took_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// The content of a clip is kept (it replaces the clip's earlier one).
    Save(ClipContent),
    /// The content of a clip.
    ReadClip { clip_id: String },
    /// The content of every clip looked at by `model` (done or failed).
    ReadAll { model: String },
}

impl Operation {
    pub fn is_write(&self) -> bool {
        matches!(self, Self::Save(_))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    Clip(Option<ClipContent>),
    All(Vec<ClipContent>),
    Changed,
}

/// Joins the content table to a project database.
pub struct ShotContentModule;

impl ShotContentModule {
    pub fn factory() -> Arc<dyn TableModuleFactory> {
        qnc_db_broker::owned_tables::<store::Store>()
    }
}

fn encode(operation: &Operation) -> Result<Value> {
    serde_json::to_value(operation).map_err(|e| e.to_string())
}

fn read(target: &ProjectDbTarget, operation: Operation) -> Result<Data> {
    let mut client = target.open(Access::ReadOnly, vec![ShotContentModule::factory()])?;
    let answer = client.execute(MODULE_ID, encode(&operation)?)?;
    serde_json::from_value(answer).map_err(|e| e.to_string())
}

/// The content of `clip_id` in the project behind `target`, if it was looked at.
pub fn read_clip(target: &ProjectDbTarget, clip_id: &str) -> Result<Option<ClipContent>> {
    match read(target, Operation::ReadClip { clip_id: clip_id.into() })? {
        Data::Clip(content) => Ok(content),
        _ => Err(qnc_ui_text::text("db.shot_content_reply").into()),
    }
}

/// The content of every clip of the project behind `target` looked at by `model`.
pub fn read_all(target: &ProjectDbTarget, model: &str) -> Result<Vec<ClipContent>> {
    match read(target, Operation::ReadAll { model: model.into() })? {
        Data::All(clips) => Ok(clips),
        _ => Err(qnc_ui_text::text("db.shot_content_reply").into()),
    }
}

/// Keeps `content` in the project behind `target` through its serial writer, under `lease`
/// when given (checked in the same transaction as the write).
pub fn save(target: &ProjectDbTarget, content: ClipContent, lease: Option<WriteLease>) -> Result<()> {
    let writer = qnc_db_broker::ProjectDbWriter::spawn(target.clone(), vec![ShotContentModule::factory()])?;
    let writer = match lease {
        Some(lease) => writer.leased(lease),
        None => writer,
    };
    writer.call(MODULE_ID, encode(&Operation::Save(content))?).map(drop)
}

#[cfg(test)]
mod tests;
