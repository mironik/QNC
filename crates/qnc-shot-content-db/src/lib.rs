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

/// How the content is made; a row made another way is looked at again (2: every shot of the
/// analysis, with its poster, user 2026-11-10; 3: a statement is kept whole, not cut by its
/// pictures).
pub const CONTENT_VERSION: u32 = 3;

/// What one shot of a clip shows (user 2026-11-10: "the agent split the clips into shots, steady,
/// moving, panning; each gets a poster and a quick split by content"): clip frames `[from, to)`,
/// how the shot looks (`steady`, `pan` for a usable part; `moving`, `soft`, `still`, `shot` for a
/// shot without one), the frames its pictures were taken at, their vector (unit length), its
/// poster (a QNC URI in the project) and whether it was looked at finely (more pictures).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartContent {
    pub from: u64,
    pub to: u64,
    #[serde(default)]
    pub kind: String,
    pub frames: Vec<u64>,
    pub vector: Vec<f32>,
    #[serde(default)]
    pub poster: String,
    #[serde(default)]
    pub fine: bool,
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

/// The vector of one label's prompt, by one model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelVector {
    pub code: String,
    pub prompt: String,
    pub vector: Vec<f32>,
}

/// The vectors of a list of labels by one model and how the model scores a picture against a
/// text (the sigmoid of `cosine * scale + bias`): kept so a form sorts the B-roll by content
/// without loading the model (user 2026-11-10).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelSet {
    pub model: String,
    pub scale: f32,
    pub bias: f32,
    pub labels: Vec<LabelVector>,
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
    /// The label vectors of a model are kept (they replace its earlier ones).
    SaveLabels(LabelSet),
    /// The label vectors of a model.
    ReadLabels { model: String },
}

impl Operation {
    pub fn is_write(&self) -> bool {
        matches!(self, Self::Save(_) | Self::SaveLabels(_))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    Clip(Option<ClipContent>),
    All(Vec<ClipContent>),
    Labels(Option<LabelSet>),
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

/// The label vectors of `model` in the project behind `target`, if kept.
pub fn read_labels(target: &ProjectDbTarget, model: &str) -> Result<Option<LabelSet>> {
    match read(target, Operation::ReadLabels { model: model.into() })? {
        Data::Labels(labels) => Ok(labels),
        _ => Err(qnc_ui_text::text("db.shot_content_reply").into()),
    }
}

/// Keeps `content` in the project behind `target` through its serial writer, under `lease`
/// when given (checked in the same transaction as the write).
pub fn save(target: &ProjectDbTarget, content: ClipContent, lease: Option<WriteLease>) -> Result<()> {
    write(target, Operation::Save(content), lease)
}

/// Keeps the label vectors of a model in the project behind `target`, under `lease` when given.
pub fn save_labels(target: &ProjectDbTarget, labels: LabelSet, lease: Option<WriteLease>) -> Result<()> {
    write(target, Operation::SaveLabels(labels), lease)
}

fn write(target: &ProjectDbTarget, operation: Operation, lease: Option<WriteLease>) -> Result<()> {
    let writer = qnc_db_broker::ProjectDbWriter::spawn(target.clone(), vec![ShotContentModule::factory()])?;
    let writer = match lease {
        Some(lease) => writer.leased(lease),
        None => writer,
    };
    writer.call(MODULE_ID, encode(&operation)?).map(drop)
}

#[cfg(test)]
mod tests;
