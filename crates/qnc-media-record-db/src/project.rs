//! This module as a table module of the project database intermediary
//! (`qnc-db-broker`) and its client: the media records of the active project.

use std::sync::Arc;

use qnc_db_broker::{Access, ProjectDbWriter, TableModule, TableModuleFactory};
use serde_json::Value;

use crate::contract::{
    Acquisition, AcquisitionClaim, BeginAcquisition, Data, Document, FinishAcquisition,
    Operation, Receipt, Request, Snapshot, Write, VERSION,
};
use crate::Store;

pub const MODULE_ID: &str = "qnc.module.media-record-db";
/// The public media record identity inside a project database.
pub const URI: &str = "qnc://local/db/media_records";

/// Joins the media record tables to a project database.
pub struct MediaRecordsModule;

impl MediaRecordsModule {
    pub fn factory() -> Arc<dyn TableModuleFactory> {
        Arc::new(Self)
    }
}

impl TableModuleFactory for MediaRecordsModule {
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
    ) -> Result<Box<dyn TableModule>, String> {
        Ok(Box::new(Attached(
            Store::attach(connection, access).map_err(|e| format!("Zapisi medija: {e:?}"))?,
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
            .map_err(|e| format!("Zapisi medija: {e:?}"))?;
        serde_json::to_value(data).map_err(|e| e.to_string())
    }
}

/// The media records of the active project, through its one intermediary.
#[derive(Debug, Clone)]
pub struct ProjectMediaRecords(ProjectDbWriter);

impl ProjectMediaRecords {
    pub fn new(writer: ProjectDbWriter) -> Self {
        Self(writer)
    }

    fn call(&self, operation: Operation) -> Result<Data, String> {
        self.0.request(MODULE_ID, &operation)
    }

    pub fn write(&self, write: Write) -> Result<Receipt, String> {
        match self.call(Operation::Write(Box::new(write)))? {
            Data::Written(receipt) => Ok(receipt),
            _ => Err("Neispravan odgovor zapisa medija.".into()),
        }
    }

    pub fn begin_acquisition(&self, begin: BeginAcquisition) -> Result<AcquisitionClaim, String> {
        match self.call(Operation::BeginAcquisition(begin))? {
            Data::AcquisitionClaim(claim) => Ok(*claim),
            _ => Err("Neispravan odgovor zapisa medija.".into()),
        }
    }

    pub fn finish_acquisition(&self, finish: FinishAcquisition) -> Result<Acquisition, String> {
        match self.call(Operation::FinishAcquisition(finish))? {
            Data::Acquisition(Some(attempt)) => Ok(*attempt),
            _ => Err("Neispravan odgovor zapisa medija.".into()),
        }
    }

    pub fn read(&self, clip_id: &str, revision: Option<u32>) -> Result<Option<Snapshot>, String> {
        match self.call(Operation::Read {
            clip_id: clip_id.into(),
            revision,
        })? {
            Data::Snapshot(snapshot) => Ok(snapshot.map(|s| *s)),
            _ => Err("Neispravan odgovor zapisa medija.".into()),
        }
    }

    pub fn document(&self, uri: &str) -> Result<Option<Document>, String> {
        match self.call(Operation::Document {
            document_uri: uri.into(),
        })? {
            Data::Document(document) => Ok(document),
            _ => Err("Neispravan odgovor zapisa medija.".into()),
        }
    }
}
