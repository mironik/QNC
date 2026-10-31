//! The list of the S-Story versions of a project (user 2026-10-31: "from the same elements of
//! MA Audio several S-Stories can be put together; every send is a new version"). Each version
//! is a copy of the Story of its own (`story-draft-<n>.db`, the story tables of
//! `qnc-program-db` through `ProjectDbTarget::draft_story`); this record only says which
//! versions there are, numbered 1, 2, ... and never numbered again.
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

/// One version of the story: its number is the number of its story file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Version {
    pub version_id: i64,
    pub number: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// A new version, numbered after the highest number a version ever had.
    NewVersion,
    /// The versions, oldest first.
    ReadVersions,
}

impl Operation {
    pub fn is_write(&self) -> bool {
        matches!(self, Self::NewVersion)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    Versions(Vec<Version>),
    Created(Version),
}

/// Joins the versions table to a project database.
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

/// The versions of the S-Story of the project behind `target`, oldest first.
pub fn versions(target: &ProjectDbTarget) -> Result<Vec<Version>> {
    let mut client = target.main_story().open(Access::ReadOnly, vec![SkeletonModule::factory()])?;
    let answer = client.execute(MODULE_ID, encode(&Operation::ReadVersions)?)?;
    match serde_json::from_value(answer).map_err(|e| e.to_string())? {
        Data::Versions(versions) => Ok(versions),
        _ => Err(reply()),
    }
}

/// A new version through the project's serial writer; returns its number (its story file is
/// `target.draft_story(number)`).
pub fn new_version(target: &ProjectDbTarget) -> Result<u32> {
    let writer = ProjectDbWriter::start(target.main_story(), vec![SkeletonModule::factory()])?;
    let answer = writer.call(MODULE_ID, encode(&Operation::NewVersion)?)?;
    match serde_json::from_value(answer).map_err(|e| e.to_string())? {
        Data::Created(version) => Ok(version.number),
        _ => Err(reply()),
    }
}

#[cfg(test)]
mod tests;
