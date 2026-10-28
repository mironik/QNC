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

/// Which engine a queued range is transcribed with (user 2026-10-26: Fast or Quality).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueQuality {
    Fast,
    Best,
}

/// Where a queued range is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueState {
    Waiting,
    Running,
    Done,
    Failed,
}

/// A range of a clip asked to be transcribed in the background (user 2026-10-26: clips
/// ticked in the pool are queued; the queue is in the project database, so it outlives the
/// application and another one sees it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Queued {
    pub clip_id: String,
    pub from_frame: u64,
    pub to_frame: u64,
    pub quality: QueueQuality,
    pub state: QueueState,
    /// Why it failed.
    pub error: String,
    /// How long its transcription took, when done (milliseconds).
    #[serde(default)]
    pub took_ms: u64,
}


/// A statement chosen from a transcript (user 2026-10-26: the text between IN and OUT kept in
/// a list, with its start and end timecode and length, for the journalist's script). The
/// timecodes and length are written as shown when it was chosen; `id` is given by the table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    #[serde(default)]
    pub id: i64,
    pub clip_id: String,
    pub clip_name: String,
    pub from_frame: u64,
    pub to_frame: u64,
    pub start_tc: String,
    pub end_tc: String,
    pub length: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// A transcript of a range is kept (it replaces one of the same range).
    Save(ClipTranscript),
    /// The transcripts of a clip, in the order of their ranges.
    ReadClip { clip_id: String },
    /// Ranges are queued, waiting (a clip's earlier entry is replaced).
    Enqueue(Vec<Queued>),
    /// A queued clip is now in `state` (a running entry stays one, by its clip), with how
    /// long it took when done.
    SetQueued { clip_id: String, state: QueueState, error: String, #[serde(default)] took_ms: u64 },
    /// The queue, in the order it was asked.
    ReadQueue,
    /// A statement is added to the list (user 2026-10-26).
    AddQuote(Quote),
    /// A statement leaves the list.
    DeleteQuote { id: i64 },
    /// The statements, in the order they were added.
    ReadQuotes,
    /// The clips that have a kept transcript (user 2026-10-28: the pool's Transcript tab).
    ReadTranscribed,
    /// The entries that are done leave the queue (user 2026-10-28: once all asked are
    /// transcribed, their list goes); failed ones stay, with why.
    ClearDone,
}

impl Operation {
    pub fn is_write(&self) -> bool {
        matches!(self, Self::Save(_) | Self::Enqueue(_) | Self::SetQueued { .. } | Self::AddQuote(_) | Self::DeleteQuote { .. } | Self::ClearDone)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    Transcripts(Vec<ClipTranscript>),
    Queue(Vec<Queued>),
    Quotes(Vec<Quote>),
    Clips(Vec<String>),
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
        _ => Err(qnc_ui_text::text("db.transcript_reply").into()),
    }
}

/// The transcription queue of the project behind `target`.
pub fn read_queue(target: &ProjectDbTarget) -> Result<Vec<Queued>> {
    let mut client = target.open(Access::ReadOnly, vec![TranscriptModule::factory()])?;
    let answer = client.execute(MODULE_ID, encode(&Operation::ReadQueue)?)?;
    match serde_json::from_value(answer).map_err(|e| e.to_string())? {
        Data::Queue(queue) => Ok(queue),
        _ => Err(qnc_ui_text::text("db.transcript_reply").into()),
    }
}

/// Keeps `transcript` in the project behind `target` through its serial writer.
pub fn save(target: &ProjectDbTarget, transcript: ClipTranscript) -> Result<()> {
    write(target, Operation::Save(transcript))
}

/// Queues `ranges` (waiting) in the project behind `target`.
pub fn enqueue(target: &ProjectDbTarget, ranges: Vec<Queued>) -> Result<()> {
    write(target, Operation::Enqueue(ranges))
}

/// Puts the queued `clip_id` in `state`; `took_ms` is how long it took when done.
pub fn set_queued(target: &ProjectDbTarget, clip_id: &str, state: QueueState, error: &str, took_ms: u64) -> Result<()> {
    write(target, Operation::SetQueued { clip_id: clip_id.into(), state, error: error.into(), took_ms })
}

/// The statements of the project behind `target`, in the order they were added.
pub fn read_quotes(target: &ProjectDbTarget) -> Result<Vec<Quote>> {
    let mut client = target.open(Access::ReadOnly, vec![TranscriptModule::factory()])?;
    let answer = client.execute(MODULE_ID, encode(&Operation::ReadQuotes)?)?;
    match serde_json::from_value(answer).map_err(|e| e.to_string())? {
        Data::Quotes(quotes) => Ok(quotes),
        _ => Err(qnc_ui_text::text("db.transcript_reply").into()),
    }
}

/// Adds `quote` to the statements of the project behind `target`.
pub fn add_quote(target: &ProjectDbTarget, quote: Quote) -> Result<()> {
    write(target, Operation::AddQuote(quote))
}

/// The clips of the project behind `target` that have a kept transcript, by their id.
pub fn read_transcribed(target: &ProjectDbTarget) -> Result<Vec<String>> {
    let mut client = target.open(Access::ReadOnly, vec![TranscriptModule::factory()])?;
    let answer = client.execute(MODULE_ID, encode(&Operation::ReadTranscribed)?)?;
    match serde_json::from_value(answer).map_err(|e| e.to_string())? {
        Data::Clips(clips) => Ok(clips),
        _ => Err(qnc_ui_text::text("db.transcript_reply").into()),
    }
}

/// The entries of the queue that are done leave it; failed ones stay.
pub fn clear_done(target: &ProjectDbTarget) -> Result<()> {
    write(target, Operation::ClearDone)
}

/// Takes statement `id` off the list.
pub fn delete_quote(target: &ProjectDbTarget, id: i64) -> Result<()> {
    write(target, Operation::DeleteQuote { id })
}

fn write(target: &ProjectDbTarget, operation: Operation) -> Result<()> {
    let writer = qnc_db_broker::ProjectDbWriter::spawn(target.clone(), vec![TranscriptModule::factory()])?;
    writer.call(MODULE_ID, encode(&operation)?).map(drop)
}

#[cfg(test)]
mod tests;
