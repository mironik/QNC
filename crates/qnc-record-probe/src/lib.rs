//! Completes one media record of a clip, once (QNC v5 `media_probe` job; user rule
//! 2026-09-26: "if it can go without a probe, no probe; if not, a probe").
//!
//! A camera record that already states everything playback reads becomes final
//! without a probe. A record that lacks some of it (a Sony FX6 XML states no
//! container stream facts, docs/26) gets one probe of each medium that lacks them;
//! the probe evidence is stored before it is interpreted and composed with the
//! camera facts, which stay a source. A medium probed or tried once is never probed
//! again, except when the probe never read it (it did not start or ran out of time,
//! v5: a media probe job that fails that way goes back to the queue). Records are written only through the media record module of the project
//! database intermediary. This module knows no application, form or scan.

use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicBool, Ordering},
};

use qnc_media_probe::{ProbeBackend, Request as ProbeRequest};
use qnc_media_record_db::contract::{
    AcquisitionOutcome, BeginAcquisition, Completeness, Document, DocumentType, FinishAcquisition,
    Phase, Snapshot, SourceRecord, Write,
};
use qnc_media_record_db::project::ProjectMediaRecords;

pub const MODULE_ID: &str = "qnc.module.record-probe";

pub type Result<T> = std::result::Result<T, String>;

/// Why a record was not completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A probe never read its medium (did not start or ran out of time): nothing was
    /// learned, and the record may be completed again later.
    Interrupted(String),
    /// The probe ran and could not read its medium (a container the probe does not
    /// allow, a broken file): the result is final, the medium is not playable.
    Unreadable(String),
    /// Anything else: the record stays as it is.
    Failed(String),
}

impl Error {
    pub fn is_interrupted(&self) -> bool {
        matches!(self, Self::Interrupted(_))
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Interrupted(message) | Self::Unreadable(message) | Self::Failed(message) => {
                f.write_str(message)
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}

impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Self::Failed(message.into())
    }
}

impl From<Error> for String {
    fn from(error: Error) -> Self {
        error.to_string()
    }
}

/// Makes the record of `camera` final: without a probe when it lacks nothing,
/// otherwise with one probe of each medium that lacks something. A final record is
/// returned as it is.
pub fn complete(
    records: &ProjectMediaRecords,
    source_record: &SourceRecord,
    camera: Snapshot,
    backend: &dyn ProbeBackend,
    cancel: &AtomicBool,
) -> std::result::Result<Snapshot, Error> {
    if camera.phase == Phase::Final {
        return Ok(camera);
    }
    let needed =
        qnc_media_metadata_compose::required_probes(&camera).map_err(|e| format!("{e:?}"))?;
    let mut documents = camera_documents(records, &camera)?;
    let mut reports = Vec::new();
    for (i, media_uri) in needed.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Interrupted("Dopuna zapisa je prekinuta.".into()));
        }
        let document = probe_once(records, &camera, &media_uri, backend)?;
        // Raw evidence is durable before any semantic interpretation or merge.
        reports.push(
            qnc_ffprobe_metadata::read(
                &document.text,
                &media_uri,
                &document.document_uri,
                &format!("ffprobe-{i}"),
            )
            .map_err(|e| format!("{e:?}"))?,
        );
        documents.push(document);
    }
    let metadata = if reports.is_empty() {
        camera.metadata.clone()
    } else {
        qnc_media_metadata_compose::compose(&camera.metadata, &reports)
            .map_err(|e| format!("{e:?}"))?
    };
    records.write(Write {
        request_id: uuid::Uuid::new_v4().to_string(),
        expected_revision: camera.revision,
        phase: Phase::Final,
        source_index_uri: qnc_source_index_db::project::URI.into(),
        source_record: source_record.clone(),
        metadata,
        documents,
    })?;
    records
        .read(&camera.metadata.clip_id, None)?
        .ok_or_else(|| "Konacni zapis nedostaje.".into())
}

/// Whether a final record still lacks something playback reads.
pub fn is_partial(snapshot: &Snapshot) -> bool {
    snapshot.completeness == Completeness::Partial
}

/// The camera evidence documents the final record carries again.
fn camera_documents(records: &ProjectMediaRecords, camera: &Snapshot) -> Result<Vec<Document>> {
    let uris: BTreeSet<_> = camera
        .metadata
        .evidence
        .iter()
        .map(|e| &e.document_uri)
        .collect();
    uris.into_iter()
        .map(|uri| records.document(uri)?.ok_or_else(|| "Dokaz kamere nedostaje.".to_string()))
        .collect()
}

/// One probe of a medium, claimed in the database first: a medium probed or tried
/// before is never probed again.
fn probe_once(
    records: &ProjectMediaRecords,
    camera: &Snapshot,
    media_uri: &str,
    backend: &dyn ProbeBackend,
) -> std::result::Result<Document, Error> {
    let attempt_id = uuid::Uuid::new_v4().to_string();
    let document_uri = format!("qnc://local/artifact/probe-{attempt_id}");
    let claim = records.begin_acquisition(BeginAcquisition {
        attempt_id: attempt_id.clone(),
        clip_id: camera.metadata.clip_id.clone(),
        expected_revision: 1,
        media_uri: media_uri.into(),
        document_uri: document_uri.clone(),
    })?;
    if !claim.granted {
        return match claim.acquisition.outcome {
            Some(AcquisitionOutcome::Stored { document_uri }) => records
                .document(&document_uri)?
                .ok_or_else(|| "Spremljeni probe dokaz nedostaje.".into()),
            // The medium was probed before and could not be read: the same answer again.
            Some(AcquisitionOutcome::Failed { code }) => {
                Err(Error::Unreadable(format!("Probe nije uspio: {code}")))
            }
            _ => Err("Probe je vec pokusan ili je u tijeku. Nema ponovnog pokretanja.".into()),
        };
    }
    let request = ProbeRequest {
        version: qnc_media_probe::VERSION.into(),
        request_id: attempt_id.clone(),
        media_uri: media_uri.into(),
        document_uri: document_uri.clone(),
    };
    match backend.execute(&request).and_then(|report| {
        report.validate(&request)?;
        Ok(report)
    }) {
        Ok(report) => {
            let document = Document {
                document_uri: report.document_uri,
                media_type: DocumentType::Json,
                text: report.json,
            };
            records.finish_acquisition(FinishAcquisition {
                attempt_id,
                outcome: AcquisitionOutcome::Stored { document_uri },
                document: Some(document.clone()),
            })?;
            Ok(document)
        }
        Err(error) => {
            let code = format!("{error:?}");
            let interrupted = matches!(
                error,
                qnc_media_probe::Error::Timeout | qnc_media_probe::Error::Spawn
            );
            let outcome = if interrupted {
                AcquisitionOutcome::Interrupted { code: code.clone() }
            } else if matches!(error, qnc_media_probe::Error::TransportUncertain) {
                AcquisitionOutcome::Uncertain { code: code.clone() }
            } else {
                AcquisitionOutcome::Failed { code: code.clone() }
            };
            records.finish_acquisition(FinishAcquisition {
                attempt_id,
                outcome,
                document: None,
            })?;
            let message = format!("Probe nije uspio: {code}");
            Err(if interrupted {
                Error::Interrupted(message)
            } else if matches!(error, qnc_media_probe::Error::TransportUncertain) {
                Error::Failed(message)
            } else {
                Error::Unreadable(message)
            })
        }
    }
}
