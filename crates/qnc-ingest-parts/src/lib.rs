//! The parts of source clips marked in Ingest (user 2026-10-23: a long clip on the card is
//! often needed only in part; one or more parts are marked with IN/OUT and only they are
//! copied at import). A part is a record of the project database, a table module of the
//! project database intermediary (`qnc-db-broker`): which clip, from which frame to which.
//! The parts are not the SubClips of Story (`qnc-virtual-shots` keeps those, of imported
//! clips only); a part is the instruction of what to bring in. The module owns only its
//! tables, is written only through that intermediary and knows no form, worker or application.

mod piece;
mod store;

pub use piece::{PartActions, PartsPiece};

use std::sync::Arc;

use qnc_db_broker::{Access, Pending, ProjectDbTarget, ProjectDbWriter, TableModule, TableModuleFactory};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.ingest-parts";

pub type Result<T> = std::result::Result<T, String>;

/// One marked part of a source clip: frames `in_frame..out_frame` of it (OUT is the bound
/// after the last picture of the part).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub part_id: String,
    pub clip_id: String,
    pub name: String,
    pub in_frame: u64,
    pub out_frame: u64,
    /// The name of its file on the parts card in the project once it is cut (the original
    /// name and its number, user 2026-10-23); `None` until Uvezi cuts it.
    #[serde(default)]
    pub cut_name: Option<String>,
    /// Ticked for Uvezi (user 2026-10-23: a new part is ticked; the user may untick it).
    #[serde(default = "ticked")]
    pub selected: bool,
}

fn ticked() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// A new part of a clip of the catalog.
    Save { clip_id: String, clip_name: String, in_frame: u64, out_frame: u64 },
    /// The part is no longer wanted.
    Delete { part_id: String },
    /// Every part of the project, oldest first.
    List,
    /// The part is cut: its file on the parts card is `cut_name`.
    MarkCut { part_id: String, cut_name: String },
    /// The part is ticked for Uvezi or not.
    Select { part_id: String, selected: bool },
}

impl Operation {
    pub fn is_write(&self) -> bool {
        !matches!(self, Self::List)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    Saved(Part),
    Parts(Vec<Part>),
    Changed,
}

/// Joins the parts tables to a project database.
pub struct PartsModule;

impl PartsModule {
    pub fn factory() -> Arc<dyn TableModuleFactory> {
        Arc::new(Self)
    }
}

impl TableModuleFactory for PartsModule {
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
    fn connection(&self) -> &rusqlite::Connection {
        self.0.connection()
    }
}

/// The parts of clip `clip_id`, read once (the import reads them for the clip it takes).
pub fn read_parts(target: &ProjectDbTarget, clip_id: &str) -> Result<Vec<Part>> {
    let mut client = target.open(Access::ReadOnly, vec![PartsModule::factory()])?;
    let value = client.execute(MODULE_ID, serde_json::to_value(Operation::List).map_err(|e| e.to_string())?)?;
    match serde_json::from_value::<Data>(value).map_err(|e| e.to_string())? {
        Data::Parts(parts) => Ok(parts.into_iter().filter(|part| part.clip_id == clip_id).collect()),
        _ => Err(qnc_ui_text::text("parts.not_ready").into()),
    }
}

/// The parts ticked and not cut yet, of every clip (Uvezi cuts them).
pub fn read_uncut(target: &ProjectDbTarget) -> Result<Vec<Part>> {
    let mut client = target.open(Access::ReadOnly, vec![PartsModule::factory()])?;
    let value = client.execute(MODULE_ID, serde_json::to_value(Operation::List).map_err(|e| e.to_string())?)?;
    match serde_json::from_value::<Data>(value).map_err(|e| e.to_string())? {
        Data::Parts(parts) => Ok(parts.into_iter().filter(|part| part.cut_name.is_none() && part.selected).collect()),
        _ => Err(qnc_ui_text::text("parts.not_ready").into()),
    }
}

/// Writes of the parts by the import, through the one serial writer of the project database.
#[derive(Debug, Clone)]
pub struct PartsWriter(ProjectDbWriter);

impl PartsWriter {
    pub fn start(target: ProjectDbTarget) -> Result<Self> {
        Ok(Self(ProjectDbWriter::start(target, vec![PartsModule::factory()])?))
    }

    /// The same writer whose writes are made under `lease` (a worker that lost it writes nothing).
    pub fn leased(self, lease: qnc_db_broker::WriteLease) -> Self {
        Self(self.0.leased(lease))
    }

    pub fn mark_cut(&self, part_id: &str, cut_name: &str) -> Result<()> {
        let operation = Operation::MarkCut { part_id: part_id.into(), cut_name: cut_name.into() };
        match serde_json::from_value::<Data>(self.0.call(MODULE_ID, serde_json::to_value(operation).map_err(|e| e.to_string())?)?) {
            Ok(Data::Changed) => Ok(()),
            Ok(_) => Err(qnc_ui_text::text("parts.not_ready").into()),
            Err(error) => Err(error.to_string()),
        }
    }
}

/// The parts of the active project as a surface shows them: read and written through one
/// serial writer of the project database, never waited for on the caller's thread.
#[derive(Debug, Default)]
pub struct IngestParts {
    writer: Option<(String, ProjectDbWriter)>,
    pending: Vec<(Pending, bool)>,
    parts: Vec<Part>,
    message: Option<String>,
}

impl IngestParts {
    /// The project whose parts are shown; another project starts again with none.
    pub fn configure(&mut self, target: Option<&ProjectDbTarget>) {
        let Some(target) = target else {
            *self = Self::default();
            return;
        };
        if self.writer.as_ref().is_some_and(|(uri, _)| uri == target.uri()) {
            return;
        }
        *self = Self::default();
        match ProjectDbWriter::spawn(target.clone(), vec![PartsModule::factory()]) {
            Ok(writer) => {
                self.writer = Some((target.uri().to_string(), writer));
                if let Err(error) = self.ask(Operation::List, false) {
                    self.message = Some(error);
                }
            }
            Err(error) => self.message = Some(error),
        }
    }

    /// Every part of the project as last read.
    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    /// Saves frames `in_frame..out_frame` of the clip as a new part.
    pub fn save(&mut self, clip_id: &str, clip_name: &str, (in_frame, out_frame): (u64, u64)) -> Result<()> {
        if out_frame <= in_frame {
            return Err(qnc_ui_text::text("parts.needs_in_and_out").into());
        }
        let operation = Operation::Save { clip_id: clip_id.into(), clip_name: clip_name.into(), in_frame, out_frame };
        self.ask(operation, true)
    }

    /// Ticks or unticks a part for Uvezi.
    pub fn select(&mut self, part_id: &str, selected: bool) -> Result<()> {
        self.ask(Operation::Select { part_id: part_id.into(), selected }, true)
    }

    pub fn delete(&mut self, part_id: &str) -> Result<()> {
        self.ask(Operation::Delete { part_id: part_id.into() }, true)
    }

    /// The replies that arrived: whether the parts changed; an error is the message.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        let mut read_again = false;
        let mut waiting = Vec::new();
        for (pending, write) in std::mem::take(&mut self.pending) {
            match pending.try_take() {
                None => waiting.push((pending, write)),
                Some(Ok(value)) => match serde_json::from_value::<Data>(value) {
                    Ok(Data::Parts(parts)) => {
                        changed |= parts != self.parts;
                        self.parts = parts;
                    }
                    Ok(_) => read_again |= write,
                    Err(error) => self.message = Some(error.to_string()),
                },
                Some(Err(error)) => {
                    self.message = Some(error);
                    changed = true;
                }
            }
        }
        self.pending = waiting;
        if read_again {
            let _ = self.ask(Operation::List, false);
        }
        changed
    }

    pub fn is_busy(&self) -> bool {
        !self.pending.is_empty()
    }

    /// The message of a failed read or write, handed over once.
    pub fn take_message(&mut self) -> Option<String> {
        self.message.take()
    }

    fn ask(&mut self, operation: Operation, write: bool) -> Result<()> {
        let (_, writer) = self.writer.as_ref().ok_or(qnc_ui_text::text("ingest.msg.catalog_unavailable"))?;
        let payload = serde_json::to_value(&operation).map_err(|e| e.to_string())?;
        self.pending.push((writer.submit(MODULE_ID, payload)?, write));
        Ok(())
    }
}

#[cfg(test)]
mod tests;
