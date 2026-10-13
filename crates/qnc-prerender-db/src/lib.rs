//! The rendered units of the story's prerender (user 2026-10-12), as a table module of the
//! project database intermediary (`qnc-db-broker`): which unit (by fingerprint) is
//! rendered, where its file is (a QNC URI of the project folder), how many frames and
//! samples it holds and which encoder made it. A unit counts as rendered only when it is
//! here: its file alone proves nothing (the project folder denies deleting, so a file of
//! an interrupted render can stay). It owns only its table, is written only through that
//! intermediary and knows no form, worker or application.

mod store;

use std::sync::Arc;

use qnc_db_broker::{Access, ProjectDbClient, ProjectDbTarget, ProjectDbWriter, TableModule, TableModuleFactory};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.prerender-db";

pub type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderedUnit {
    pub fingerprint: String,
    pub file_uri: String,
    pub frames: u64,
    pub samples: u64,
    pub encoder: String,
    /// What the unit was fingerprinted with (the delivery key of its render).
    pub key: Value,
    /// The description of the unit file as its render wrote it (never probed).
    pub media: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// A unit whose file is complete.
    Publish(RenderedUnit),
    /// The rendered units among `fingerprints`.
    Read { fingerprints: Vec<String> },
    /// Every rendered unit of the project (the player matches its story against them).
    ReadAll,
    /// The units among `fingerprints` are no longer rendered (the story no longer has
    /// them, user 2026-10-13: old units are removed).
    Forget { fingerprints: Vec<String> },
}

impl Operation {
    pub fn is_write(&self) -> bool {
        matches!(self, Self::Publish(_) | Self::Forget { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    Units(Vec<RenderedUnit>),
    Changed,
}

/// Joins the prerender table to a project database.
pub struct PrerenderModule;

impl PrerenderModule {
    pub fn factory() -> Arc<dyn TableModuleFactory> {
        Arc::new(Self)
    }
}

impl TableModuleFactory for PrerenderModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }
    fn is_write(&self, payload: &Value) -> bool {
        serde_json::from_value::<Operation>(payload.clone()).is_ok_and(|o| o.is_write())
    }
    fn attach(&self, connection: rusqlite::Connection, access: Access) -> Result<Box<dyn TableModule>> {
        Ok(Box::new(Attached(store::Store::attach(connection, access)?)))
    }
}

struct Attached(store::Store);

impl TableModule for Attached {
    fn execute(&mut self, payload: Value) -> Result<Value> {
        let operation: Operation = serde_json::from_value(payload).map_err(|e| e.to_string())?;
        let data = self.0.execute(&operation)?;
        serde_json::to_value(data).map_err(|e| e.to_string())
    }
}

fn decode(value: Value) -> Result<Data> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

fn encode(operation: &Operation) -> Result<Value> {
    serde_json::to_value(operation).map_err(|e| e.to_string())
}

fn wrong() -> String {
    "Neispravan odgovor prerendera.".into()
}

/// Reads of the rendered units of the active project: a read-only client of its database.
pub struct PrerenderReader(ProjectDbClient);

impl PrerenderReader {
    pub fn open(target: &ProjectDbTarget) -> Result<Self> {
        Ok(Self(target.open(Access::ReadOnly, vec![PrerenderModule::factory()])?))
    }

    /// The rendered units among `fingerprints` (in no particular order).
    pub fn rendered(&mut self, fingerprints: Vec<String>) -> Result<Vec<RenderedUnit>> {
        match decode(self.0.execute(MODULE_ID, encode(&Operation::Read { fingerprints })?)?)? {
            Data::Units(units) => Ok(units),
            _ => Err(wrong()),
        }
    }

    /// Every rendered unit of the project.
    pub fn all(&mut self) -> Result<Vec<RenderedUnit>> {
        match decode(self.0.execute(MODULE_ID, encode(&Operation::ReadAll)?)?)? {
            Data::Units(units) => Ok(units),
            _ => Err(wrong()),
        }
    }
}

/// Writes of the rendered units of the active project, through the one serial writer of
/// its database in this process.
#[derive(Debug, Clone)]
pub struct PrerenderWriter(ProjectDbWriter);

impl PrerenderWriter {
    pub fn start(target: ProjectDbTarget) -> Result<Self> {
        Ok(Self(ProjectDbWriter::start(target, vec![PrerenderModule::factory()])?))
    }

    pub fn publish(&self, unit: RenderedUnit) -> Result<()> {
        match decode(self.0.call(MODULE_ID, encode(&Operation::Publish(unit))?)?)? {
            Data::Changed => Ok(()),
            _ => Err(wrong()),
        }
    }

    /// The units among `fingerprints` are no longer rendered.
    pub fn forget(&self, fingerprints: Vec<String>) -> Result<()> {
        if fingerprints.is_empty() {
            return Ok(());
        }
        match decode(self.0.call(MODULE_ID, encode(&Operation::Forget { fingerprints })?)?)? {
            Data::Changed => Ok(()),
            _ => Err(wrong()),
        }
    }
}

#[cfg(test)]
mod tests;
