//! The one serial intermediary of a project database (QNC v5 `ProjectDbBroker`).
//!
//! The project owns its database; no application owns it (user rules 2026-09-25/26).
//! This module opens the database of the active project, checks that it belongs to
//! that project, keeps its journal policy and runs every request in order. It knows
//! no domain and holds no SQL of one: each table module (a lego piece) owns its
//! tables, their schema and their requests, and is handed a connection here. A table
//! module never opens a database file itself. Local, LAN and intranet use the same
//! requests.

mod database;
mod transport;

pub use database::ProjectDb;
pub use qnc_json_transport::{Access, Credentials};
pub use transport::{respond, ProjectDbClient, ProjectDbTarget, ProjectDbWriter, ENDPOINT};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.db-broker";
pub const VERSION: &str = "0.1.0";
pub const MAX_BYTES: usize = 16 * 1024 * 1024;

pub type Result<T> = std::result::Result<T, String>;

/// A table module: owns its tables in the project database and serves its requests
/// on the connection the intermediary gives it.
pub trait TableModule: Send {
    fn execute(&mut self, payload: Value) -> Result<Value>;
}

/// How a table module joins a project database. The process composing the
/// intermediary lists the modules it serves; the intermediary names none.
pub trait TableModuleFactory: Send + Sync {
    /// Stable id of the module, e.g. `qnc.module.media-record-db`.
    fn id(&self) -> &'static str;
    /// Whether a request of this module writes (checked before it is sent).
    fn is_write(&self, payload: &Value) -> bool;
    /// Creates the module's tables if missing (read-write) and checks only them.
    fn attach(
        &self,
        connection: rusqlite::Connection,
        access: Access,
    ) -> Result<Box<dyn TableModule>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: String,
    pub db_uri: String,
    pub module: String,
    pub payload: Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub version: String,
    pub db_uri: String,
    pub result: Result<Value>,
}

/// The public identity of the project database behind this intermediary, from the
/// workspace URI of the active project settings.
pub fn project_db_uri(workspace_uri: &str) -> Result<String> {
    let uri = qnc_contracts::parse_qnc_uri(workspace_uri).map_err(|e| e.to_string())?;
    if uri.resource_kind != "db" || !uri.resource_id.starts_with("project_workspace/") {
        return Err("Nedostaje projektna baza iz radnih postavki.".into());
    }
    Ok(workspace_uri.replacen("/db/project_workspace/", "/db/project_db/", 1))
}

/// The project id of a project database URI.
pub fn project_id(uri: &str) -> Result<String> {
    let parsed = qnc_contracts::parse_qnc_uri(uri).map_err(|e| e.to_string())?;
    let id = parsed
        .resource_id
        .strip_prefix("project_db/")
        .filter(|_| parsed.resource_kind == "db")
        .ok_or("Neispravan URI baze projekta.")?;
    if id.is_empty() || id.contains(['/', '\\', ':']) || matches!(id, "." | "..") {
        return Err("Neispravan projektni identitet.".into());
    }
    Ok(id.into())
}

#[cfg(test)]
mod tests;
