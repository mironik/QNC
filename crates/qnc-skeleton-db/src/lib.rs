//! The S-Story versions of a project (user 2026-10-31): each version is the elements of a story
//! in order, each a SubClip (a source clip with its own IN/OUT, A1/A2, J/L and the pause or cut
//! before it, and where in the script it came from); they become Story segments only when sent
//! to the Story. Its own record, apart from the script and the Story.
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

/// One version of the story (user 2026-10-31: "from the same elements of MA Audio several
/// S-Stories can be put together: an announcement, a short package, a report full of detail").
/// Numbered 1, 2, ... as they are made; every send from the script is a new version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Version {
    pub version_id: i64,
    pub number: i64,
    pub elements: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// The elements are a new version, in one step (all or nothing).
    AppendVersion { elements: Vec<NewElement> },
    /// The versions, oldest first.
    ReadVersions,
    /// The elements of a version in order.
    Read { version_id: i64 },
    /// An element's IN/OUT, A1 and the pause and join before it change (edited as a source
    /// clip on the S-Story timeline).
    Update { element_id: i64, in_frame: u64, out_frame: u64, a1: Lane, gap_before: u64, cut: bool },
    /// An element changes places with the one before (`earlier`) or after it.
    Move { element_id: i64, earlier: bool },
    /// An element leaves the skeleton.
    Remove { element_id: i64 },
}

impl Operation {
    pub fn is_write(&self) -> bool {
        !matches!(self, Self::Read { .. } | Self::ReadVersions)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    Elements(Vec<Element>),
    Versions(Vec<Version>),
    /// The new version.
    Appended(Version),
    Changed,
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

fn ask(target: &ProjectDbTarget, operation: &Operation) -> Result<Data> {
    let mut client = target.open(Access::ReadOnly, vec![SkeletonModule::factory()])?;
    let answer = client.execute(MODULE_ID, encode(operation)?)?;
    serde_json::from_value(answer).map_err(|e| e.to_string())
}

/// The versions of the story of the project behind `target`, oldest first.
pub fn versions(target: &ProjectDbTarget) -> Result<Vec<Version>> {
    match ask(target, &Operation::ReadVersions)? {
        Data::Versions(versions) => Ok(versions),
        _ => Err(reply()),
    }
}

/// The elements of version `version_id`, in order.
pub fn read(target: &ProjectDbTarget, version_id: i64) -> Result<Vec<Element>> {
    match ask(target, &Operation::Read { version_id })? {
        Data::Elements(elements) => Ok(elements),
        _ => Err(reply()),
    }
}

/// A change of one element (`Update`, `Move` or `Remove`) through the project's serial writer.
pub fn change(target: &ProjectDbTarget, operation: Operation) -> Result<()> {
    if !matches!(operation, Operation::Update { .. } | Operation::Move { .. } | Operation::Remove { .. }) {
        return Err(reply());
    }
    let writer = ProjectDbWriter::start(target.clone(), vec![SkeletonModule::factory()])?;
    let answer = writer.call(MODULE_ID, encode(&operation)?)?;
    match serde_json::from_value(answer).map_err(|e| e.to_string())? {
        Data::Changed => Ok(()),
        _ => Err(reply()),
    }
}

/// `elements` are a new version of the story of the project behind `target`, in one step,
/// through the project's serial writer. Returns the version.
pub fn append_version(target: &ProjectDbTarget, elements: Vec<NewElement>) -> Result<Version> {
    let writer = ProjectDbWriter::start(target.clone(), vec![SkeletonModule::factory()])?;
    let answer = writer.call(MODULE_ID, encode(&Operation::AppendVersion { elements })?)?;
    match serde_json::from_value(answer).map_err(|e| e.to_string())? {
        Data::Appended(version) => Ok(version),
        _ => Err(reply()),
    }
}

#[cfg(test)]
mod tests;
