//! The S-Story skeleton (user 2026-10-31): the elements of a story's skeleton in order, each a
//! source clip with its own IN/OUT, A1/A2, J/L and the pause or cut before it, and where in
//! the script it came from. It is its own record, apart from the script and the Story, so it
//! is also the safe copy a broken Story is sent again from (all of it or a part).
//!
//! A table module of the project database intermediary (`qnc-db-broker`): it owns only its
//! table, is written only through that intermediary and knows no form, worker or
//! application.

mod store;

use std::sync::Arc;

use qnc_db_broker::{Access, ProjectDbTarget, ProjectDbWriter, TableModuleFactory};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.skeleton-db";

pub type Result<T> = std::result::Result<T, String>;

/// One lane of an element: the camera channel (zero based), its level in dB and whether it is off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lane {
    pub channel: u16,
    pub level_db: i8,
    pub off: bool,
}

/// What an element is: a statement (`ton`, picture and sound of its clip) or a voice over
/// (`off`, the sound of its voice record).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementKind {
    Ton,
    Off,
}

impl ElementKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ton => "ton",
            Self::Off => "off",
        }
    }
}

/// One element of the skeleton as it is added: a source clip and how it is heard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewElement {
    pub kind: ElementKind,
    pub clip_id: String,
    pub in_frame: u64,
    pub out_frame: u64,
    pub a1: Lane,
    pub a2: Lane,
    /// `script:<block>:<part>`: the block and part of the script it came from.
    pub origin: String,
}

/// One element as the skeleton keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Element {
    pub element_id: i64,
    pub sort_index: i64,
    pub kind: ElementKind,
    pub clip_id: String,
    pub in_frame: u64,
    pub out_frame: u64,
    pub a1: Lane,
    pub a2: Lane,
    /// The A1 of this element heard before its picture (J) and after it (L), in frames.
    pub a1_lead_frames: u64,
    pub a1_tail_frames: u64,
    /// Frames of pause before this element, and whether its join is a cut (else a crossfade).
    pub gap_before: u64,
    pub cut: bool,
    pub origin: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// The elements come at the end of the skeleton, in one step (all or nothing).
    Append { elements: Vec<NewElement> },
    /// The elements of the skeleton in order.
    Read,
}

impl Operation {
    pub fn is_write(&self) -> bool {
        matches!(self, Self::Append { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    Elements(Vec<Element>),
    Appended(usize),
}

/// Joins the skeleton table to a project database.
pub struct SkeletonModule;

impl SkeletonModule {
    pub fn factory() -> Arc<dyn TableModuleFactory> {
        qnc_db_broker::owned_tables::<store::Store>()
    }
}

fn encode(operation: &Operation) -> Result<Value> {
    serde_json::to_value(operation).map_err(|e| e.to_string())
}

fn reply() -> String {
    qnc_ui_text::text("db.skeleton_reply").into()
}

/// The elements of the skeleton of the project behind `target`, in order.
pub fn read(target: &ProjectDbTarget) -> Result<Vec<Element>> {
    let mut client = target.open(Access::ReadOnly, vec![SkeletonModule::factory()])?;
    let answer = client.execute(MODULE_ID, encode(&Operation::Read)?)?;
    match serde_json::from_value(answer).map_err(|e| e.to_string())? {
        Data::Elements(elements) => Ok(elements),
        Data::Appended(_) => Err(reply()),
    }
}

/// `elements` come at the end of the skeleton of the project behind `target`, in one step,
/// through the project's serial writer. Returns how many were added.
pub fn append(target: &ProjectDbTarget, elements: Vec<NewElement>) -> Result<usize> {
    let writer = ProjectDbWriter::start(target.clone(), vec![SkeletonModule::factory()])?;
    let answer = writer.call(MODULE_ID, encode(&Operation::Append { elements })?)?;
    match serde_json::from_value(answer).map_err(|e| e.to_string())? {
        Data::Appended(count) => Ok(count),
        Data::Elements(_) => Err(reply()),
    }
}

#[cfg(test)]
mod tests;
