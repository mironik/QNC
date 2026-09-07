use crate::{
    contract::*,
    database::{check_document, db_error},
    Store,
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::time::{SystemTime, UNIX_EPOCH};

impl Store {
    pub(crate) fn begin_acquisition(
        &mut self,
        begin: &BeginAcquisition,
    ) -> Result<AcquisitionClaim> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(existing) = read(&tx, &begin.media_uri)? {
            return Ok(AcquisitionClaim {
                granted: false,
                acquisition: existing,
            });
        }
        let json: Option<String> = tx.query_row(
            "SELECT s.snapshot_json FROM media_heads h JOIN media_snapshots s ON s.clip_id=h.clip_id AND s.revision=h.revision WHERE h.clip_id=?1",
            [&begin.clip_id], |r| r.get(0)).optional().map_err(db_error)?;
        let snapshot: Snapshot = serde_json::from_str(&json.ok_or(Error::InvalidRequest)?)
            .map_err(|_| Error::Protocol)?;
        snapshot.validate()?;
        if snapshot.phase == Phase::Final {
            return Err(Error::Finalized);
        }
        if snapshot.revision != begin.expected_revision {
            return Err(Error::StaleRevision);
        }
        if snapshot.binding.original_uri != begin.media_uri
            && snapshot.binding.proxy_uri.as_deref() != Some(&begin.media_uri)
        {
            return Err(Error::InvalidRequest);
        }
        // A prior final record or stored probe document is not a new acquisition opportunity.
        let already_recorded: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM media_heads h JOIN media_snapshots s ON s.clip_id=h.clip_id AND s.revision=h.revision WHERE s.phase='final' AND (json_extract(h.binding_json,'$.original_uri')=?1 OR json_extract(h.binding_json,'$.proxy_uri')=?1)) OR EXISTS(SELECT 1 FROM evidence_documents WHERE document_uri=?2)",
            params![begin.media_uri, begin.document_uri], |r| r.get(0)).map_err(db_error)?;
        if already_recorded {
            return Err(Error::Finalized);
        }
        let attempt = Acquisition {
            request: begin.clone(),
            started_at_unix_ms: now()?,
            finished_at_unix_ms: None,
            outcome: None,
        };
        attempt.validate()?;
        tx.execute(
            "INSERT INTO media_acquisitions VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                begin.media_uri,
                begin.attempt_id,
                begin.document_uri,
                begin.clip_id,
                begin.expected_revision,
                serde_json::to_string(&attempt).map_err(|_| Error::Protocol)?
            ],
        )
        .map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(AcquisitionClaim {
            granted: true,
            acquisition: attempt,
        })
    }

    pub(crate) fn finish_acquisition(&mut self, finish: &FinishAcquisition) -> Result<Acquisition> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let uri: String = tx
            .query_row(
                "SELECT media_uri FROM media_acquisitions WHERE attempt_id=?1",
                [&finish.attempt_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_error)?
            .ok_or(Error::InvalidRequest)?;
        let mut attempt = read(&tx, &uri)?.ok_or(Error::Protocol)?;
        finish.validate_for(&attempt.request)?;
        if let Some(outcome) = &attempt.outcome {
            if outcome != &finish.outcome {
                return Err(Error::Conflict);
            }
            if let Some(document) = &finish.document {
                check_document(&tx, document, false)?;
            }
            return Ok(attempt);
        }
        if let Some(document) = &finish.document {
            check_document(&tx, document, true)?;
        }
        attempt.outcome = Some(finish.outcome.clone());
        attempt.finished_at_unix_ms = Some(now()?.max(attempt.started_at_unix_ms));
        attempt.validate()?;
        tx.execute(
            "UPDATE media_acquisitions SET acquisition_json=?1 WHERE attempt_id=?2",
            params![
                serde_json::to_string(&attempt).map_err(|_| Error::Protocol)?,
                finish.attempt_id
            ],
        )
        .map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(attempt)
    }

    pub(crate) fn acquisition(&self, media_uri: &str) -> Result<Option<Acquisition>> {
        read(&self.conn, media_uri)
    }
}

fn read(conn: &Connection, media_uri: &str) -> Result<Option<Acquisition>> {
    let json: Option<String> = conn
        .query_row(
            "SELECT acquisition_json FROM public_media_acquisitions WHERE media_uri=?1",
            [media_uri],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)?;
    json.map(|json| {
        let attempt: Acquisition = serde_json::from_str(&json).map_err(|_| Error::Protocol)?;
        attempt.validate()?;
        if attempt.request.media_uri != media_uri {
            return Err(Error::Protocol);
        }
        Ok(attempt)
    })
    .transpose()
}

fn now() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| u64::try_from(d.as_millis()).ok())
        .ok_or(Error::Unavailable)
}
