//! Timeline artifacts of a project (QNC v5 `filmstrips`/`filmstrip_frames`,
//! `audio_waveforms`): the filmstrip frames and wave peaks of each clip, as a table
//! module of the project database intermediary (`qnc-db-broker`). It owns only its
//! tables, reads only public views of others (the clip catalog through
//! `qnc-content-read`) and is written only through that intermediary. The frames
//! themselves are JPEG files in the project directory; the database keeps their
//! URIs. It knows no form, no application and no worker.

mod store;

use std::sync::Arc;

use qnc_db_broker::{
    Access, Pending, ProjectDbClient, ProjectDbTarget, ProjectDbWriter, TableModule,
    TableModuleFactory,
};
pub use qnc_filmstrip::{FilmstripArtifactRecord, FilmstripFrameRecord};
pub use qnc_wave::WaveArtifactRecord;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.artifact-db";

pub type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    PublishFilmstrip(Box<FilmstripArtifactRecord>),
    ReadFilmstrip { clip_id: String },
    PublishWave(Box<WaveArtifactRecord>),
    ReadWave { clip_id: String },
    /// The artifacts of clips about to leave the catalog (missing on their source):
    /// only of clips not imported; the others keep theirs. Their ids come back.
    ForgetClips { clip_ids: Vec<String> },
}

impl Operation {
    pub fn is_write(&self) -> bool {
        !matches!(self, Self::ReadFilmstrip { .. } | Self::ReadWave { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    Filmstrip(Option<FilmstripArtifactRecord>),
    Wave(Option<WaveArtifactRecord>),
    Forgotten(Vec<String>),
    Changed,
}

/// Joins the artifact tables to a project database.
pub struct ArtifactsModule;

impl ArtifactsModule {
    pub fn factory() -> Arc<dyn TableModuleFactory> {
        Arc::new(Self)
    }
}

impl TableModuleFactory for ArtifactsModule {
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
}

fn decode(value: Value) -> Result<Data> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

fn encode(operation: &Operation) -> Result<Value> {
    serde_json::to_value(operation).map_err(|e| e.to_string())
}

fn wrong() -> String {
    "Neispravan odgovor artefakata.".into()
}

/// Reads of the artifacts of the active project: a read-only client of its database.
pub struct ArtifactReader(ProjectDbClient);

impl ArtifactReader {
    pub fn open(target: &ProjectDbTarget) -> Result<Self> {
        Ok(Self(target.open(Access::ReadOnly, vec![ArtifactsModule::factory()])?))
    }

    fn call(&mut self, operation: &Operation) -> Result<Data> {
        decode(self.0.execute(MODULE_ID, encode(operation)?)?)
    }

    pub fn read_filmstrip(&mut self, clip_id: &str) -> Result<Option<FilmstripArtifactRecord>> {
        match self.call(&Operation::ReadFilmstrip {
            clip_id: clip_id.into(),
        })? {
            Data::Filmstrip(record) => Ok(record),
            _ => Err(wrong()),
        }
    }

    pub fn read_wave(&mut self, clip_id: &str) -> Result<Option<WaveArtifactRecord>> {
        match self.call(&Operation::ReadWave {
            clip_id: clip_id.into(),
        })? {
            Data::Wave(record) => Ok(record),
            _ => Err(wrong()),
        }
    }
}

/// Writes of the artifacts of the active project, through the one serial writer of
/// its database in this process.
#[derive(Debug, Clone)]
pub struct ArtifactWriter(ProjectDbWriter);

impl ArtifactWriter {
    pub fn start(target: ProjectDbTarget) -> Result<Self> {
        Ok(Self(ProjectDbWriter::start(
            target,
            vec![ArtifactsModule::factory()],
        )?))
    }

    /// Runs a request and waits for its reply.
    pub fn call(&self, operation: &Operation) -> Result<Data> {
        decode(self.0.call(MODULE_ID, encode(operation)?)?)
    }

    /// Sends a request without waiting; the reply is taken from the returned handle.
    pub fn submit(&self, operation: &Operation) -> Result<ArtifactPending> {
        Ok(ArtifactPending(self.0.submit(MODULE_ID, encode(operation)?)?))
    }

    /// The artifacts of clips about to leave the catalog, before it removes them.
    pub fn forget_clips(&self, clip_ids: Vec<String>) -> Result<Vec<String>> {
        match self.call(&Operation::ForgetClips { clip_ids })? {
            Data::Forgotten(ids) => Ok(ids),
            _ => Err(wrong()),
        }
    }
}

/// The reply of a submitted request, once it is there.
#[derive(Debug)]
pub struct ArtifactPending(Pending);

impl ArtifactPending {
    pub fn try_take(&self) -> Option<Result<Data>> {
        self.0.try_take().map(|result| result.and_then(decode))
    }
}

#[cfg(test)]
mod tests;
