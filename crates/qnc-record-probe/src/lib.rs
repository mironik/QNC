//! Completes one media record of a clip, once (QNC v5 `media_probe` job; user rule
//! 2026-09-26: "if it can go without a probe, no probe; if not, a probe").
//!
//! A camera record that already states everything playback reads becomes final
//! without a probe. A record that lacks some of it (a Sony FX6 XML states no
//! container stream facts, docs/26) gets one probe of each medium that lacks them;
//! the probe evidence is stored before it is interpreted and composed with the
//! camera facts, which stay a source. A medium probed or tried once is never probed
//! again. Records are written only through the media record module of the project
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

/// Makes the record of `camera` final: without a probe when it lacks nothing,
/// otherwise with one probe of each medium that lacks something. A final record is
/// returned as it is.
pub fn complete(
    records: &ProjectMediaRecords,
    source_record: &SourceRecord,
    camera: Snapshot,
    backend: &dyn ProbeBackend,
    cancel: &AtomicBool,
) -> Result<Snapshot> {
    if camera.phase == Phase::Final {
        return Ok(camera);
    }
    let needed =
        qnc_media_metadata_compose::required_probes(&camera).map_err(|e| format!("{e:?}"))?;
    let mut documents = camera_documents(records, &camera)?;
    let mut reports = Vec::new();
    for (i, media_uri) in needed.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err("Dopuna zapisa je prekinuta.".into());
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
        .ok_or_else(|| "Konacni zapis nedostaje.".to_string())
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
) -> Result<Document> {
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
                .ok_or_else(|| "Spremljeni probe dokaz nedostaje.".to_string()),
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
            let outcome = if matches!(error, qnc_media_probe::Error::TransportUncertain) {
                AcquisitionOutcome::Uncertain { code: code.clone() }
            } else {
                AcquisitionOutcome::Failed { code: code.clone() }
            };
            records.finish_acquisition(FinishAcquisition {
                attempt_id,
                outcome,
                document: None,
            })?;
            Err(format!("Probe nije uspio: {code}"))
        }
    }
}
