//! The transcripts of a project (user 2026-10-26: asked in Media Assist Audio AI for a clip's
//! IN/OUT, or its whole length): what was said in a range of a clip's frames, line by line
//! with every word and the frames it is said in, the language and the engine that heard it.
//! Kept in the project database so a range is heard once and every form, and another
//! computer, reads it.
//!
//! A table module of the project database intermediary (`qnc-db-broker`): it owns only its
//! table, is written only through that intermediary and knows no form, worker or application.

mod store;

use std::sync::Arc;

use qnc_db_broker::{Access, ProjectDbTarget, TableModuleFactory};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.transcript-db";

pub type Result<T> = std::result::Result<T, String>;

/// One word and the clip frames it is said in (`[from, to)`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Word {
    pub from: u64,
    pub to: u64,
    pub text: String,
}

/// One line of speech with its words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Line {
    pub from: u64,
    pub to: u64,
    pub text: String,
    pub words: Vec<Word>,
}

/// What was said in clip frames `[from_frame, to_frame)` of a clip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipTranscript {
    pub clip_id: String,
    pub from_frame: u64,
    pub to_frame: u64,
    /// The language the text is in (a code).
    pub language: String,
    /// The engine and model that heard it.
    pub engine: String,
    pub lines: Vec<Line>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// A transcript of a range is kept (it replaces one of the same range).
    Save(ClipTranscript),
    /// The transcripts of a clip, in the order of their ranges.
    ReadClip { clip_id: String },
}

impl Operation {
    pub fn is_write(&self) -> bool {
        matches!(self, Self::Save(_))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    Transcripts(Vec<ClipTranscript>),
    Changed,
}

/// Joins the transcript table to a project database.
pub struct TranscriptModule;

impl TranscriptModule {
    pub fn factory() -> Arc<dyn TableModuleFactory> {
        qnc_db_broker::owned_tables::<store::Store>()
    }
}

fn encode(operation: &Operation) -> Result<Value> {
    serde_json::to_value(operation).map_err(|e| e.to_string())
}

/// The transcripts kept for `clip_id` in the project behind `target`.
pub fn read_clip(target: &ProjectDbTarget, clip_id: &str) -> Result<Vec<ClipTranscript>> {
    let mut client = target.open(Access::ReadOnly, vec![TranscriptModule::factory()])?;
    let answer = client.execute(MODULE_ID, encode(&Operation::ReadClip { clip_id: clip_id.into() })?)?;
    match serde_json::from_value(answer).map_err(|e| e.to_string())? {
        Data::Transcripts(transcripts) => Ok(transcripts),
        Data::Changed => Err(qnc_ui_text::text("db.transcript_reply").into()),
    }
}

/// Keeps `transcript` in the project behind `target` through its serial writer.
pub fn save(target: &ProjectDbTarget, transcript: ClipTranscript) -> Result<()> {
    let writer = qnc_db_broker::ProjectDbWriter::spawn(target.clone(), vec![TranscriptModule::factory()])?;
    writer.call(MODULE_ID, encode(&Operation::Save(transcript))?).map(drop)
}

#[cfg(test)]
mod tests;
