//! Virtual shots of a project (QNC v5 `virtual_shots` in the project database):
//! shorts (Virtual tab) and the B-roll shots of covers (B-roll tab), with the IN/OUT
//! stills of shorts. A table module of the project database intermediary
//! (`qnc-db-broker`): it owns only its table, reads only public views of others
//! (`public_project_settings`; the clip catalog through `qnc-content-read`) and is
//! written only through that intermediary. It knows no form and no application.

mod store;

use std::sync::Arc;

use qnc_db_broker::{
    Access, Pending, ProjectDbClient, ProjectDbTarget, ProjectDbWriter, TableModule,
    TableModuleFactory,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.virtual-shots";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedShort {
    pub shot_id: String,
    pub in_frame: u64,
    pub out_frame: u64,
}

/// One virtual shot stored in the project DB, ordered by creation: a short
/// (Virtual tab) or, with `b_roll`, the shot of a cover (B-roll tab).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShortClip {
    pub shot_id: String,
    pub clip_id: String,
    pub in_frame: u64,
    pub out_frame: u64,
    pub name: String,
    pub in_still_uri: Option<String>,
    pub out_still_uri: Option<String>,
    pub still_status: String,
    #[serde(default)]
    pub b_roll: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// A short of an imported clip between IN and OUT (Add virtual clip).
    SaveShort {
        project_id: String,
        clip_id: String,
        clip_name: String,
        in_frame: u64,
        out_frame: u64,
    },
    /// v5 `add_virtual_shot_from_frames` for a cover: the B-roll virtual shot of the
    /// source IN/OUT (its id comes back); the story puts it in a slot.
    CreateCoverShot {
        project_id: String,
        clip_id: String,
        clip_name: String,
        in_frame: u64,
        out_frame: u64,
    },
    ListShorts,
    /// The B-roll virtual shots (covers), oldest first.
    ListBroll,
    MarkShortStills {
        shot_id: String,
        status: String,
        in_uri: Option<String>,
        out_uri: Option<String>,
        error: Option<String>,
    },
}

impl Operation {
    pub fn is_write(&self) -> bool {
        !matches!(self, Self::ListShorts | Self::ListBroll)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    SavedShort(SavedShort),
    Created(String),
    ShortClips(Vec<ShortClip>),
    Changed,
}

/// Joins the virtual shot table to a project database.
pub struct VirtualShotsModule;

impl VirtualShotsModule {
    pub fn factory() -> Arc<dyn TableModuleFactory> {
        Arc::new(Self)
    }
}

impl TableModuleFactory for VirtualShotsModule {
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
    "Neispravan odgovor virtualnih kadrova.".into()
}

fn read(target: &ProjectDbTarget, operation: &Operation) -> Result<Vec<ShortClip>> {
    let mut client: ProjectDbClient =
        target.open(Access::ReadOnly, vec![VirtualShotsModule::factory()])?;
    match decode(client.execute(MODULE_ID, encode(operation)?)?)? {
        Data::ShortClips(rows) => Ok(rows),
        _ => Err(wrong()),
    }
}

/// The shorts of the active project, oldest first.
pub fn list_shorts(target: &ProjectDbTarget) -> Result<Vec<ShortClip>> {
    read(target, &Operation::ListShorts)
}

/// The virtual shots of the pool: shorts (Virtual tab), then the B-roll shots of
/// the covers (B-roll tab, `b_roll`), each oldest first.
pub fn list_pool_shots(target: &ProjectDbTarget) -> Result<Vec<ShortClip>> {
    let mut shots = list_shorts(target)?;
    shots.extend(read(target, &Operation::ListBroll)?);
    Ok(shots)
}

/// Writes of the virtual shots of the active project, through the one serial
/// writer of its database in this process.
#[derive(Debug, Clone)]
pub struct VirtualShotsWriter(ProjectDbWriter);

impl VirtualShotsWriter {
    pub fn start(target: ProjectDbTarget) -> Result<Self> {
        Ok(Self(ProjectDbWriter::start(
            target,
            vec![VirtualShotsModule::factory()],
        )?))
    }

    /// Runs a request and waits for its reply.
    pub fn call(&self, operation: &Operation) -> Result<Data> {
        decode(self.0.call(MODULE_ID, encode(operation)?)?)
    }

    /// Sends a request without waiting; the reply is taken from the returned handle.
    pub fn submit(&self, operation: &Operation) -> Result<VirtualShotsPending> {
        Ok(VirtualShotsPending(self.0.submit(MODULE_ID, encode(operation)?)?))
    }
}

/// The reply of a submitted request, once it is there.
#[derive(Debug)]
pub struct VirtualShotsPending(Pending);

impl VirtualShotsPending {
    pub fn try_take(&self) -> Option<Result<Data>> {
        self.0.try_take().map(|result| result.and_then(decode))
    }
}

/// Saves a short and waits for its id.
pub fn save_short_now(
    target: &ProjectDbTarget,
    project_id: &str,
    clip_id: &str,
    name: &str,
    in_frame: u64,
    out_frame: u64,
) -> Result<SavedShort> {
    let writer = VirtualShotsWriter::start(target.clone())?;
    match writer.call(&Operation::SaveShort {
        project_id: project_id.into(),
        clip_id: clip_id.into(),
        clip_name: name.into(),
        in_frame,
        out_frame,
    })? {
        Data::SavedShort(shot) => Ok(shot),
        _ => Err(wrong()),
    }
}

/// The outcome of storing a short's IN/OUT stills, `(in_uri, out_uri)` or the
/// error, written as ready or failed.
pub fn publish_stills_now(
    target: &ProjectDbTarget,
    shot_id: &str,
    stills: std::result::Result<(String, String), String>,
) -> Result<()> {
    let operation = match stills {
        Ok((in_uri, out_uri)) => Operation::MarkShortStills {
            shot_id: shot_id.into(),
            status: "ready".into(),
            in_uri: Some(in_uri),
            out_uri: Some(out_uri),
            error: None,
        },
        Err(error) => Operation::MarkShortStills {
            shot_id: shot_id.into(),
            status: "failed".into(),
            in_uri: None,
            out_uri: None,
            error: Some(error),
        },
    };
    match VirtualShotsWriter::start(target.clone())?.call(&operation)? {
        Data::Changed => Ok(()),
        _ => Err(wrong()),
    }
}


/// The poster of a B-roll shot (its IN still) published in the database, or why it is missing.
pub fn publish_poster_now(
    target: &ProjectDbTarget,
    shot_id: &str,
    poster: std::result::Result<String, String>,
) -> Result<()> {
    let (status, in_uri, error) = match poster {
        Ok(uri) => ("ready", Some(uri), None),
        Err(error) => ("failed", None, Some(error)),
    };
    let operation = Operation::MarkShortStills {
        shot_id: shot_id.into(),
        status: status.into(),
        in_uri,
        out_uri: None,
        error,
    };
    match VirtualShotsWriter::start(target.clone())?.call(&operation)? {
        Data::Changed => Ok(()),
        _ => Err(wrong()),
    }
}
#[cfg(test)]
mod tests;
