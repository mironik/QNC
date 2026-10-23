use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use qnc_db_contract::DatabaseContract;
use qnc_transport_resolver::{ResolvedEndpoint, ResolverConfig};
use rusqlite::{params, Connection};

pub mod content;
mod ids;
mod records;
mod store;
mod validate;

pub use records::*;
pub use store::IngestStore;
pub(crate) use ids::*;
pub(crate) use validate::*;

pub const INGEST_REGISTRY_DB_ID: &str = "qnc.db.ingest_registry";
pub const INGEST_CONTENT_DB_ID: &str = "qnc.db.ingest_content";
pub const INGEST_REGISTRY_DB_LOCAL_URI: &str = "qnc://local/db/ingest_registry";
pub const INGEST_CONTENT_DB_LOCAL_URI: &str = "qnc://local/db/ingest_content";
const INGEST_REGISTRY_DB_CONTRACT: &str =
    include_str!("../../../contracts/databases/ingest-registry.database.json");
const INGEST_CONTENT_DB_CONTRACT: &str =
    include_str!("../../../contracts/databases/ingest-content.database.json");
const APPLICATION_ID: &str = "qnc.ingest";
const PROJECT_DB_WRITER: &str = "qnc.module.content-store";

#[cfg(test)]
mod tests;
