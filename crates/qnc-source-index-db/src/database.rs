use crate::contract::*;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use std::{
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const SCHEMA: &str = include_str!("schema.sql");
const APPLICATION_ID: i32 = 1_364_083_529;

pub use qnc_json_transport::Access;

pub struct Store {
    connection: Connection,
    access: Access,
}
impl Store {
    /// Storage-owner bootstrap only. The private path is never accepted on the wire.
    pub fn open_owner_binding(path: &Path, access: Access, initialize: bool) -> Result<Self> {
        if initialize && access == Access::ReadOnly {
            return Err(Error::AccessDenied);
        }
        let mut flags = if access == Access::ReadOnly {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        };
        if initialize {
            flags |= OpenFlags::SQLITE_OPEN_CREATE;
        }
        let mut conn = Connection::open_with_flags(path, flags).map_err(db_error)?;
        conn.set_limit(
            rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH,
            (MAX_BYTES * 2) as i32,
        );
        conn.busy_timeout(Duration::from_secs(5))
            .map_err(db_error)?;
        conn.pragma_update(None, "trusted_schema", "OFF")
            .map_err(db_error)?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(db_error)?;
        if initialize {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(db_error)?;
            let empty: bool = tx
                .query_row("SELECT NOT EXISTS(SELECT 1 FROM sqlite_schema)", [], |r| {
                    r.get(0)
                })
                .map_err(db_error)?;
            let app: i32 = tx
                .pragma_query_value(None, "application_id", |r| r.get(0))
                .map_err(db_error)?;
            let version: i32 = tx
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .map_err(db_error)?;
            if empty && app == 0 && version == 0 {
                tx.execute_batch(SCHEMA).map_err(db_error)?;
                tx.pragma_update(None, "application_id", APPLICATION_ID)
                    .map_err(db_error)?;
                tx.pragma_update(None, "user_version", 1)
                    .map_err(db_error)?;
            }
            validate_schema(&tx)?;
            tx.commit().map_err(db_error)?;
        } else {
            validate_schema(&conn)?;
        }
        if access == Access::ReadWrite {
            let mode: String = conn
                .pragma_query_value(None, "journal_mode", |r| r.get(0))
                .map_err(db_error)?;
            if mode != "wal" {
                conn.pragma_update(None, "journal_mode", "WAL")
                    .map_err(db_error)?;
            }
        }
        Ok(Self {
            connection: conn,
            access,
        })
    }

    pub fn execute(&mut self, request: &Request) -> Result<Data> {
        request.validate()?;
        match &request.operation {
            Operation::Write(batch) => {
                if self.access != Access::ReadWrite {
                    return Err(Error::AccessDenied);
                }
                self.write(batch).map(Data::Written)
            }
            Operation::Read { record_id } => {
                self.read(record_id).map(|r| Data::Record(r.map(Box::new)))
            }
        }
    }

    fn write(&mut self, batch: &Batch) -> Result<Receipt> {
        let groups = batch.validate()?;
        let payload = serde_json::to_vec(batch).map_err(|_| Error::InvalidRequest)?;
        let encoded: Vec<_> = groups
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<_, _>>()
            .map_err(|_| Error::InvalidRequest)?;
        let now = u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| Error::Unavailable)?
                .as_millis(),
        )
        .map_err(|_| Error::Unavailable)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let previous: Option<(Vec<u8>, String)> = tx
            .query_row(
                "SELECT payload, receipt_json FROM write_receipts WHERE batch_id=?1",
                [&batch.batch_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db_error)?;
        if let Some((previous, receipt)) = previous {
            if previous != payload {
                return Err(Error::Conflict);
            }
            return serde_json::from_str(&receipt).map_err(|_| Error::Protocol);
        }
        let mut record_ids = Vec::with_capacity(groups.len());
        for (group, json) in groups.iter().zip(encoded) {
            let p = &group.proposal;
            let previous: Option<(String, String)> = tx.query_row(
                "SELECT record_id, group_json FROM source_records WHERE recording_root_uri=?1 AND recording_identity=?2",
                params![p.root.uri(), p.recording_identity], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(db_error)?;
            if let Some((id, previous)) = previous {
                if previous != json {
                    return Err(Error::Conflict);
                }
                record_ids.push(id);
                continue;
            }
            // The transaction also protects conflicts with records from earlier batches.
            for media in std::iter::once(&p.original).chain(&p.proxies) {
                let used: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM media_references WHERE media_uri=?1 UNION ALL SELECT 1 FROM support_references WHERE media_uri=?1)",
                    [media.uri()], |r| r.get(0)).map_err(db_error)?;
                if used {
                    return Err(Error::Conflict);
                }
            }
            for support in p
                .related
                .iter()
                .map(|r| &r.reference)
                .chain(std::iter::once(&p.evidence.document))
            {
                let used: bool = tx
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM media_references WHERE media_uri=?1)",
                        [support.uri()],
                        |r| r.get(0),
                    )
                    .map_err(db_error)?;
                if used {
                    return Err(Error::Conflict);
                }
            }
            let id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO source_records VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    id,
                    batch.source_uri,
                    p.root.uri(),
                    p.recording_identity,
                    json,
                    now
                ],
            )
            .map_err(db_error)?;
            tx.execute(
                "INSERT INTO media_references VALUES (?1, ?2, 'original')",
                params![p.original.uri(), id],
            )
            .map_err(db_error)?;
            for proxy in &p.proxies {
                tx.execute(
                    "INSERT INTO media_references VALUES (?1, ?2, 'proxy')",
                    params![proxy.uri(), id],
                )
                .map_err(db_error)?;
            }
            tx.execute(
                "INSERT INTO support_references VALUES (?1, ?2, 'evidence', 'index', 'file')",
                params![p.evidence.document.uri(), id],
            )
            .map_err(db_error)?;
            for (related, fact) in p.related.iter().zip(&group.related_states) {
                let state = match fact.state {
                    FileState::File => "file",
                    FileState::Missing => "missing",
                    FileState::Unavailable => "unavailable",
                };
                tx.execute(
                    "INSERT INTO support_references VALUES (?1, ?2, 'related', ?3, ?4)",
                    params![related.reference.uri(), id, related.kind, state],
                )
                .map_err(db_error)?;
            }
            record_ids.push(id);
        }
        let receipt = Receipt {
            batch_id: batch.batch_id.clone(),
            record_ids,
        };
        tx.execute(
            "INSERT INTO write_receipts VALUES (?1, ?2, ?3)",
            params![
                batch.batch_id,
                payload,
                serde_json::to_string(&receipt).map_err(|_| Error::Protocol)?
            ],
        )
        .map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(receipt)
    }

    fn read(&self, id: &str) -> Result<Option<Record>> {
        let row: Option<(String, String, String, u64)> = self.connection.query_row(
            "SELECT record_id, source_uri, group_json, recorded_at_unix_ms FROM public_source_records WHERE record_id=?1",
            [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional().map_err(db_error)?;
        row.map(|(record_id, source_uri, json, recorded_at_unix_ms)| {
            if json.len() > MAX_BYTES {
                return Err(Error::TooLarge);
            }
            let record = Record {
                record_id,
                source_uri,
                group: serde_json::from_str(&json).map_err(|_| Error::Protocol)?,
                recorded_at_unix_ms,
            };
            record.validate()?;
            Ok(record)
        })
        .transpose()
    }
}

fn validate_schema(conn: &Connection) -> Result<()> {
    let app: i32 = conn
        .pragma_query_value(None, "application_id", |r| r.get(0))
        .map_err(db_error)?;
    let version: i32 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(db_error)?;
    let expected = Connection::open_in_memory().map_err(db_error)?;
    expected.execute_batch(SCHEMA).map_err(db_error)?;
    if app != APPLICATION_ID || version != 1 || schema_rows(conn)? != schema_rows(&expected)? {
        return Err(Error::IncompatibleSchema);
    }
    Ok(())
}
fn schema_rows(conn: &Connection) -> Result<Vec<(String, String, Option<String>)>> {
    let mut statement = conn
        .prepare("SELECT type, name, sql FROM sqlite_schema ORDER BY type, name")
        .map_err(db_error)?;
    let rows = statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(db_error)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(db_error)
}
fn db_error(error: rusqlite::Error) -> Error {
    if let rusqlite::Error::SqliteFailure(ref code, _) = error {
        if code.code == rusqlite::ErrorCode::ConstraintViolation {
            return Error::Conflict;
        }
    }
    Error::Unavailable
}
