//! The Story of a project (QNC v5 `story_*` tables in the project database):
//! program segments, M markers, M-M slots, covers, the selection and undo. A table
//! module of the project database intermediary (`qnc-db-broker`): it owns only its
//! tables, reads only public views of others (`public_clips`,
//! `public_project_settings`) and is written only through that intermediary. It
//! knows no form, no application and no player.

mod store;
pub use store::story_per_source;
mod undo;
mod model;
pub use model::*;

use std::sync::Arc;

use qnc_db_broker::{
    Access, Pending, ProjectDbClient, ProjectDbTarget, ProjectDbWriter, TableModule,
    TableModuleFactory,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.program-db";

pub type Result<T> = std::result::Result<T, String>;


/// Joins the Story tables to a project database.
pub struct StoryModule;

impl StoryModule {
    pub fn factory() -> Arc<dyn TableModuleFactory> {
        Arc::new(Self)
    }
}

impl TableModuleFactory for StoryModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }
    fn is_write(&self, payload: &Value) -> bool {
        serde_json::from_value::<Operation>(payload.clone()).is_ok_and(|o| o.is_write())
    }
    fn attach(
        &self,
        connection: rusqlite::Connection,
        access: Access,
    ) -> Result<Box<dyn TableModule>> {
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

fn decode(value: Value) -> Result<Data> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

fn encode(operation: &Operation) -> Result<Value> {
    serde_json::to_value(operation).map_err(|e| e.to_string())
}

fn wrong() -> String {
    qnc_ui_text::text("story.invalid_reply_story").into()
}

/// The owner brings the Story tables of a project to the current schema (one short write
/// connection, one transaction).
pub fn prepare_schema(target: &ProjectDbTarget) -> Result<()> {
    // The module joins the base on its first request: one read makes it bring the schema up.
    let mut owner = target.open(Access::ReadWrite, vec![StoryModule::factory()])?;
    decode(owner.execute(MODULE_ID, encode(&Operation::ReadStorySelection)?)?).map(|_| ())
}

/// Reads of the Story of the active project: a read-only client of its database,
/// so a form reading never waits for a writer.
pub struct StoryReader {
    client: ProjectDbClient,
    target: ProjectDbTarget,
}

impl StoryReader {
    pub fn open(target: &ProjectDbTarget) -> Result<Self> {
        Ok(Self { client: target.open(Access::ReadOnly, vec![StoryModule::factory()])?, target: target.clone() })
    }

    /// A story of an older schema is first brought up by the owner (a short write
    /// connection of this module), then read once more; the reader itself never changes
    /// the base. The module joins the base on the first read, so this is where the older
    /// schema shows (live 2026-10-17: the segment panel stayed empty until a write).
    fn call(&mut self, operation: &Operation) -> Result<Data> {
        match self.client.execute(MODULE_ID, encode(operation)?) {
            Err(error) if error.to_string().contains(qnc_ui_text::text("story.schema_outdated")) => {
                prepare_schema(&self.target)?;
                decode(self.client.execute(MODULE_ID, encode(operation)?)?)
            }
            answer => decode(answer?),
        }
    }

    pub fn list_segments(&mut self) -> Result<Vec<ProgramSegment>> {
        match self.call(&Operation::ListSegments)? {
            Data::Segments(rows) => Ok(rows),
            _ => Err(wrong()),
        }
    }

    pub fn list_markers(&mut self) -> Result<Vec<ProgramMarker>> {
        match self.call(&Operation::ListMarkers)? {
            Data::Markers(rows) => Ok(rows),
            _ => Err(wrong()),
        }
    }

    pub fn list_slots(&mut self) -> Result<Vec<ProgramSlot>> {
        match self.call(&Operation::ListSlots)? {
            Data::Slots(rows) => Ok(rows),
            _ => Err(wrong()),
        }
    }

    pub fn list_covers(&mut self) -> Result<Vec<ProgramCover>> {
        match self.call(&Operation::ListCovers)? {
            Data::Covers(rows) => Ok(rows),
            _ => Err(wrong()),
        }
    }

    pub fn read_story_selection(&mut self) -> Result<StorySelection> {
        match self.call(&Operation::ReadStorySelection)? {
            Data::StorySelection(selection) => Ok(selection),
            _ => Err(wrong()),
        }
    }

    /// The program in one read (segments, covers, rate) with its revision.
    pub fn program_snapshot(&mut self) -> Result<ProgramSnapshot> {
        match self.call(&Operation::ReadProgramSnapshot)? {
            Data::Program(snapshot) => Ok(snapshot),
            _ => Err(wrong()),
        }
    }
}

/// Writes of the Story of the active project, through the one serial writer of its
/// database in this process.
#[derive(Debug, Clone)]
pub struct StoryWriter(ProjectDbWriter);

impl StoryWriter {
    pub fn start(target: ProjectDbTarget) -> Result<Self> {
        Ok(Self(ProjectDbWriter::start(target, vec![StoryModule::factory()])?))
    }

    /// Opens the database on the writer's thread: a form never waits for it; a
    /// database it cannot use is the error of the first reply.
    pub fn spawn(target: ProjectDbTarget) -> Result<Self> {
        Ok(Self(ProjectDbWriter::spawn(target, vec![StoryModule::factory()])?))
    }

    /// Runs a request and waits for its reply.
    pub fn call(&self, operation: &Operation) -> Result<Data> {
        decode(self.0.call(MODULE_ID, encode(operation)?)?)
    }

    /// Sends a request without waiting; the reply is taken from the returned handle.
    pub fn submit(&self, operation: &Operation) -> Result<StoryPending> {
        Ok(StoryPending(self.0.submit(MODULE_ID, encode(operation)?)?))
    }
}

/// The reply of a submitted Story request, once it is there.
#[derive(Debug)]
pub struct StoryPending(Pending);

impl StoryPending {
    pub fn try_take(&self) -> Option<Result<Data>> {
        self.0.try_take().map(|result| result.and_then(decode))
    }
}

#[cfg(test)]
mod tests;
