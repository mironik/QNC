use crate::{contract::*, Access};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use std::{
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
const SCHEMA: &str = include_str!("schema.sql");
const APPLICATION_ID: i32 = 1_364_083_530;

pub struct Store {
    pub(crate) conn: Connection,
    access: Access,
}
impl Store {
    /// Private storage-owner binding only; never exposed in a wire request.
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
                tx.pragma_update(None, "user_version", 2)
                    .map_err(db_error)?;
            }
            validate_schema(&tx)?;
            tx.commit().map_err(db_error)?;
        } else {
            validate_schema(&conn)?;
        }
        if access == Access::ReadWrite {
            conn.pragma_update(None, "synchronous", "FULL")
                .map_err(db_error)?;
            let mode: String = conn
                .pragma_query_value(None, "journal_mode", |r| r.get(0))
                .map_err(db_error)?;
            if mode != "wal" {
                conn.pragma_update(None, "journal_mode", "WAL")
                    .map_err(db_error)?;
            }
        }
        Ok(Self { conn, access })
    }
    pub fn execute(&mut self, request: &Request) -> Result<Data> {
        request.validate()?;
        if request.operation.is_write() && self.access != Access::ReadWrite {
            return Err(Error::AccessDenied);
        }
        match &request.operation {
            Operation::Write(write) => self.write(write).map(Data::Written),
            Operation::BeginAcquisition(begin) => self
                .begin_acquisition(begin)
                .map(|claim| Data::AcquisitionClaim(Box::new(claim))),
            Operation::FinishAcquisition(finish) => self
                .finish_acquisition(finish)
                .map(|attempt| Data::Acquisition(Some(Box::new(attempt)))),
            Operation::Acquisition { media_uri } => self
                .acquisition(media_uri)
                .map(|attempt| Data::Acquisition(attempt.map(Box::new))),
            Operation::Read { clip_id, revision } => self
                .read(clip_id, *revision)
                .map(|s| Data::Snapshot(s.map(Box::new))),
            Operation::Document { document_uri } => self.document(document_uri).map(Data::Document),
        }
    }
    fn write(&mut self, write: &Write) -> Result<Receipt> {
        let (binding, report) = write.validate()?;
        let binding_json = serde_json::to_string(&binding).map_err(|_| Error::Protocol)?;
        // Receipt descriptors omit document bodies; immutable document rows are compared exactly.
        let mut descriptor = serde_json::to_value(write).map_err(|_| Error::Protocol)?;
        for doc in descriptor["documents"]
            .as_array_mut()
            .ok_or(Error::Protocol)?
        {
            doc.as_object_mut().ok_or(Error::Protocol)?.remove("text");
        }
        let descriptor = serde_json::to_string(&descriptor).map_err(|_| Error::Protocol)?;
        let now = u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| Error::Unavailable)?
                .as_millis(),
        )
        .map_err(|_| Error::Unavailable)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let previous: Option<(String, String)> = tx
            .query_row(
                "SELECT descriptor_json, receipt_json FROM write_receipts WHERE request_id=?1",
                [&write.request_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db_error)?;
        if let Some((previous, receipt)) = previous {
            if previous != descriptor {
                return Err(Error::Conflict);
            }
            for doc in &write.documents {
                check_document(&tx, doc, false)?;
            }
            return serde_json::from_str(&receipt).map_err(|_| Error::Protocol);
        }
        let head: Option<(String, u32, String)> = tx.query_row(
            "SELECT h.binding_json, h.revision, s.phase FROM media_heads h JOIN media_snapshots s ON s.clip_id=h.clip_id AND s.revision=h.revision WHERE h.clip_id=?1",
            [&write.metadata.clip_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional().map_err(db_error)?;
        if head.as_ref().is_some_and(|h| h.0 != binding_json) {
            return Err(Error::Conflict);
        }
        let previous = head
            .map(|(_, revision, phase)| Ok((revision, parse_phase(&phase)?)))
            .transpose()?;
        let revision = next_revision(previous, write.expected_revision, write.phase)?;
        for doc in &write.documents {
            check_document(&tx, doc, true)?;
        }
        let snapshot = Snapshot {
            binding: binding.clone(),
            revision,
            phase: write.phase,
            completeness: completeness(&report),
            metadata: write.metadata.clone(),
            report,
            recorded_at_unix_ms: now,
        };
        let phase = phase_text(write.phase);
        let complete = match snapshot.completeness {
            Completeness::Partial => "partial",
            Completeness::Complete => "complete",
        };
        tx.execute(
            "INSERT INTO media_snapshots VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                write.metadata.clip_id,
                revision,
                phase,
                complete,
                serde_json::to_string(&snapshot).map_err(|_| Error::Protocol)?
            ],
        )
        .map_err(db_error)?;
        for doc in &write.documents {
            tx.execute(
                "INSERT INTO snapshot_documents VALUES (?1, ?2, ?3)",
                params![write.metadata.clip_id, revision, doc.document_uri],
            )
            .map_err(db_error)?;
        }
        tx.execute("INSERT INTO media_heads VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(clip_id) DO UPDATE SET revision=excluded.revision", params![write.metadata.clip_id, binding.source_index_uri, binding.source_record_id, binding_json, revision]).map_err(db_error)?;
        let receipt = Receipt {
            request_id: write.request_id.clone(),
            clip_id: write.metadata.clip_id.clone(),
            revision,
            phase: write.phase,
            completeness: snapshot.completeness,
        };
        tx.execute(
            "INSERT INTO write_receipts VALUES (?1, ?2, ?3)",
            params![
                write.request_id,
                descriptor,
                serde_json::to_string(&receipt).map_err(|_| Error::Protocol)?
            ],
        )
        .map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(receipt)
    }
    fn read(&self, clip_id: &str, revision: Option<u32>) -> Result<Option<Snapshot>> {
        let json: Option<String> = self.conn.query_row(
            "SELECT s.snapshot_json FROM public_media_snapshots s JOIN public_media_heads h ON h.clip_id=s.clip_id WHERE s.clip_id=?1 AND s.revision=COALESCE(?2,h.revision)",
            params![clip_id, revision], |r| r.get(0)).optional().map_err(db_error)?;
        json.map(|json| {
            if json.len() > MAX_BYTES {
                return Err(Error::TooLarge);
            }
            let snapshot: Snapshot = serde_json::from_str(&json).map_err(|_| Error::Protocol)?;
            snapshot.validate()?;
            Ok(snapshot)
        })
        .transpose()
    }
    fn document(&self, uri: &str) -> Result<Option<Document>> {
        let row: Option<(String, String)> = self.conn.query_row("SELECT media_type, document_text FROM public_media_documents WHERE document_uri=?1", [uri], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(db_error)?;
        row.map(|(kind, text)| {
            let media_type = match kind.as_str() {
                "xml" => DocumentType::Xml,
                "json" => DocumentType::Json,
                _ => return Err(Error::Protocol),
            };
            let doc = Document {
                document_uri: uri.into(),
                media_type,
                text,
            };
            doc.validate()?;
            Ok(doc)
        })
        .transpose()
    }
}
pub(crate) fn check_document(conn: &Connection, doc: &Document, insert: bool) -> Result<()> {
    let kind = match doc.media_type {
        DocumentType::Xml => "xml",
        DocumentType::Json => "json",
    };
    let existing: Option<(String, String)> = conn
        .query_row(
            "SELECT media_type, document_text FROM evidence_documents WHERE document_uri=?1",
            [&doc.document_uri],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(db_error)?;
    match existing {
        Some((old_kind, text)) if old_kind == kind && text == doc.text => Ok(()),
        Some(_) => Err(Error::Conflict),
        None if insert => {
            conn.execute(
                "INSERT INTO evidence_documents VALUES (?1, ?2, ?3)",
                params![doc.document_uri, kind, doc.text],
            )
            .map_err(db_error)?;
            Ok(())
        }
        None => Err(Error::Conflict),
    }
}
fn phase_text(phase: Phase) -> &'static str {
    match phase {
        Phase::Camera => "camera",
        Phase::Final => "final",
    }
}
fn parse_phase(phase: &str) -> Result<Phase> {
    match phase {
        "camera" => Ok(Phase::Camera),
        "final" => Ok(Phase::Final),
        _ => Err(Error::Protocol),
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
    if app != APPLICATION_ID || version != 2 || schema_rows(conn)? != schema_rows(&expected)? {
        return Err(Error::IncompatibleSchema);
    }
    Ok(())
}
fn schema_rows(conn: &Connection) -> Result<Vec<(String, String, Option<String>)>> {
    let mut stmt = conn
        .prepare("SELECT type, name, sql FROM sqlite_schema ORDER BY type, name")
        .map_err(db_error)?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(db_error)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(db_error)
}
pub(crate) fn db_error(error: rusqlite::Error) -> Error {
    if let rusqlite::Error::SqliteFailure(ref code, _) = error {
        if code.code == rusqlite::ErrorCode::ConstraintViolation {
            return Error::Conflict;
        }
    }
    Error::Unavailable
}
