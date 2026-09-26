//! This module as a table module of the project database intermediary
//! (`qnc-db-broker`) and its client: the source index of the active project.

use std::sync::Arc;

use qnc_db_broker::{Access, ProjectDbWriter, TableModule, TableModuleFactory};
use serde_json::Value;

use crate::contract::{Batch, Data, Operation, Receipt, Record, Request, VERSION};
use crate::Store;

pub const MODULE_ID: &str = "qnc.module.source-index-db";
/// The public source index identity inside a project database.
pub const URI: &str = "qnc://local/db/source_index";

/// Joins the source index tables to a project database.
pub struct SourceIndexModule;

impl SourceIndexModule {
    pub fn factory() -> Arc<dyn TableModuleFactory> {
        Arc::new(Self)
    }
}

impl TableModuleFactory for SourceIndexModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }
    fn is_write(&self, payload: &Value) -> bool {
        matches!(
            serde_json::from_value::<Operation>(payload.clone()),
            Ok(Operation::Write(_))
        )
    }
    fn attach(
        &self,
        connection: rusqlite::Connection,
        access: Access,
    ) -> Result<Box<dyn TableModule>, String> {
        Ok(Box::new(Attached(
            Store::attach(connection, access).map_err(|e| format!("Izvorni indeks: {e:?}"))?,
        )))
    }
}

struct Attached(Store);

impl TableModule for Attached {
    fn execute(&mut self, payload: Value) -> Result<Value, String> {
        let operation: Operation = serde_json::from_value(payload).map_err(|e| e.to_string())?;
        let data = self
            .0
            .execute(&Request {
                version: VERSION.into(),
                db_uri: URI.into(),
                operation,
            })
            .map_err(|e| format!("Izvorni indeks: {e:?}"))?;
        serde_json::to_value(data).map_err(|e| e.to_string())
    }
}

/// The source index of the active project, through its one intermediary.
#[derive(Debug, Clone)]
pub struct ProjectSourceIndex(ProjectDbWriter);

impl ProjectSourceIndex {
    pub fn new(writer: ProjectDbWriter) -> Self {
        Self(writer)
    }

    pub fn write(&self, batch: Batch) -> Result<Receipt, String> {
        match self.0.request(MODULE_ID, &Operation::Write(batch))? {
            Data::Written(receipt) => Ok(receipt),
            _ => Err("Neispravan odgovor izvornog indeksa.".into()),
        }
    }

    pub fn read(&self, record_id: &str) -> Result<Option<Record>, String> {
        match self.0.request(
            MODULE_ID,
            &Operation::Read {
                record_id: record_id.into(),
            },
        )? {
            Data::Record(record) => Ok(record.map(|r| *r)),
            _ => Err("Neispravan odgovor izvornog indeksa.".into()),
        }
    }
}
