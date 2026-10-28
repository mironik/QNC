//! The shots of the clips of a project (user 2026-10-27: Video AI, shot analysis in AI Media
//! Assist): for each analysed clip its shots (clip frames, what each looks like, its sharpest
//! picture), or why it could not be analysed. Kept in the project database, so a clip is
//! analysed once and every form, and another computer, reads it.
//!
//! A table module of the project database intermediary (`qnc-db-broker`): it owns only its
//! table, is written only through that intermediary and knows no form, worker or application.

mod store;

use std::sync::Arc;

use qnc_db_broker::{Access, ProjectDbTarget, TableModuleFactory, WriteLease};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.shot-db";

pub type Result<T> = std::result::Result<T, String>;

/// One shot of a clip: clip frames `[from, to)`, its looks (`black`, `soft`, `moving`,
/// `still`) and the frame of its sharpest picture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShotRecord {
    pub from: u64,
    pub to: u64,
    pub looks: Vec<String>,
    pub key_frame: u64,
    pub brightness: f32,
    pub sharpness: f32,
    pub motion: f32,
}

/// One usable part of a clip (a cutter can take it as B-roll): clip frames `[from, to)`, how
/// the camera holds it (`steady`, `pan`) and its sharpest frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartRecord {
    pub from: u64,
    pub to: u64,
    pub camera: String,
    pub key_frame: u64,
}

/// How the analysis is made; a row made another way is analysed again.
pub const ANALYSIS_VERSION: u32 = 2;

/// The analysis of one clip: its shots, from how many pictures, of which medium; or why it
/// failed (`error` not empty, no shots).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipShots {
    pub clip_id: String,
    /// The medium the pictures were taken from (a QNC URI).
    pub media_uri: String,
    pub pictures: u32,
    pub shots: Vec<ShotRecord>,
    /// The usable parts inside the shots.
    #[serde(default)]
    pub parts: Vec<PartRecord>,
    pub error: String,
    /// How long the analysis took (milliseconds).
    pub took_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// The analysis of a clip is kept (it replaces the clip's earlier one).
    Save(ClipShots),
    /// The analysis of a clip.
    ReadClip { clip_id: String },
    /// The clips that have an analysis of this `ANALYSIS_VERSION` (done or failed).
    ReadAnalysed,
}

impl Operation {
    pub fn is_write(&self) -> bool {
        matches!(self, Self::Save(_))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    Clip(Option<ClipShots>),
    Analysed(Vec<String>),
    Changed,
}

/// Joins the shot table to a project database.
pub struct ShotModule;

impl ShotModule {
    pub fn factory() -> Arc<dyn TableModuleFactory> {
        qnc_db_broker::owned_tables::<store::Store>()
    }
}

fn encode(operation: &Operation) -> Result<Value> {
    serde_json::to_value(operation).map_err(|e| e.to_string())
}

fn read(target: &ProjectDbTarget, operation: Operation) -> Result<Data> {
    let mut client = target.open(Access::ReadOnly, vec![ShotModule::factory()])?;
    let answer = client.execute(MODULE_ID, encode(&operation)?)?;
    serde_json::from_value(answer).map_err(|e| e.to_string())
}

/// The analysis of `clip_id` in the project behind `target`, if it was analysed.
pub fn read_clip(target: &ProjectDbTarget, clip_id: &str) -> Result<Option<ClipShots>> {
    match read(target, Operation::ReadClip { clip_id: clip_id.into() })? {
        Data::Clip(shots) => Ok(shots),
        _ => Err(qnc_ui_text::text("db.shot_reply").into()),
    }
}

/// The clips of the project behind `target` that were analysed (done or failed).
pub fn read_analysed(target: &ProjectDbTarget) -> Result<Vec<String>> {
    match read(target, Operation::ReadAnalysed)? {
        Data::Analysed(clips) => Ok(clips),
        _ => Err(qnc_ui_text::text("db.shot_reply").into()),
    }
}

/// Keeps `shots` in the project behind `target` through its serial writer, under `lease`
/// when given (checked in the same transaction as the write).
pub fn save(target: &ProjectDbTarget, shots: ClipShots, lease: Option<WriteLease>) -> Result<()> {
    let writer = qnc_db_broker::ProjectDbWriter::spawn(target.clone(), vec![ShotModule::factory()])?;
    let writer = match lease {
        Some(lease) => writer.leased(lease),
        None => writer,
    };
    writer.call(MODULE_ID, encode(&Operation::Save(shots))?).map(drop)
}

#[cfg(test)]
mod tests;
