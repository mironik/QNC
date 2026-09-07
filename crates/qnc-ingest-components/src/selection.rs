use super::{
    selection_config::{Result, SelectionConfig},
    ClipView,
};
use qnc_media_probe::{ProbeBackend, Request as ProbeRequest};
use qnc_media_record_db::{contract::*, Client};
use qnc_source_groups::{GroupProposal, IndexDocument, IndexReader};
use qnc_source_reader::{SourceReader, SourceReference, MAX_TEXT_BYTES};
use std::{
    collections::BTreeSet,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::SyncSender,
        Arc,
    },
};

#[derive(Debug)]
pub(super) enum Event {
    Status(String),
    Clip(ClipView),
    Warning(String),
    Finished(std::result::Result<(), String>),
}

struct CameraAdapter {
    index: &'static dyn IndexReader,
    documents: fn(&GroupProposal) -> Vec<SourceReference>,
    thumbnail: fn(&GroupProposal) -> Option<SourceReference>,
    metadata:
        fn(&str, &GroupProposal, &[IndexDocument]) -> std::result::Result<ClipMetadata, String>,
}
fn adapters() -> Vec<CameraAdapter> {
    vec![CameraAdapter {
        index: &qnc_sony_metadata::SonyIndexReader,
        documents: qnc_sony_metadata::metadata_references,
        thumbnail: qnc_sony_metadata::thumbnail_reference,
        metadata: qnc_sony_metadata::read_group_metadata,
    }]
}

pub(super) fn run(
    config: SelectionConfig,
    selected: SourceReference,
    send: SyncSender<Event>,
    cancel: Arc<AtomicBool>,
) {
    let result = run_inner(&config, &selected, &send, &cancel, |s, media| {
        s.backend(media)
    })
    .map_err(|e| e.to_string());
    let _ = send.send(Event::Finished(result));
}

fn run_inner(
    config: &SelectionConfig,
    selected: &SourceReference,
    send: &SyncSender<Event>,
    cancel: &AtomicBool,
    make_backend: impl FnOnce(
        &super::selection_config::SourceConfig,
        &[SourceReference],
    ) -> Result<Box<dyn ProbeBackend + Send>>,
) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Select je prekinut.".into());
    }
    selected.validate()?;
    let source_config = config
        .sources
        .iter()
        .find(|s| s.location.uri == selected.source_uri())
        .ok_or("unbound Select source")?;
    let source = source_config.reader()?;
    let catalog = qnc_camera_patterns::read_uri(
        &config.catalog.resolver()?,
        &config.catalog.uri,
        config.catalog.token()?.as_deref(),
    )?;
    let readers = adapters();
    let indexes: Vec<_> = readers.iter().map(|r| r.index).collect();
    send.send(Event::Status("Prepoznavanje izvora...".into()))?;
    let scan = qnc_scanner::scan_roles(
        &catalog,
        &source,
        source_config.scope,
        &indexes,
        Default::default(),
    )?;
    if !scan.relationships_resolved() {
        send.send(Event::Warning(format!(
            "Nerazrijeseni zapisi: {}; greske skeniranja: {}; blokirane grupe: {}.",
            scan.unresolved_files.len(),
            scan.issues.len(),
            scan.grouping.blocked.len()
        )))?;
    }
    let mut source_db = config.source_index.source_db()?;
    // Initialize once before opening the bounded worker connections.
    drop(config.media_records.media_db()?);
    let groups: Vec<_> = scan
        .grouping
        .groups
        .into_iter()
        .filter(|g| g.proposal.original.is_within(selected) || g.proposal.root.is_within(selected))
        .map(|g| g.proposal)
        .collect();
    if groups.is_empty() {
        return Err("Nema potvrdenih originalnih klipova u odabranom izvoru. Neprepoznati zapisi nisu pogadani po ekstenziji.".into());
    }
    let mut records = Vec::new();
    for chunk in groups.chunks(32) {
        if cancel.load(Ordering::Relaxed) {
            return Err("Select je prekinut.".into());
        }
        let references: BTreeSet<_> = chunk
            .iter()
            .flat_map(|g| g.references())
            .map(|r| r.uri())
            .collect();
        let receipt = source_db.write(qnc_source_index_db::contract::Batch {
            batch_id: uuid::Uuid::new_v4().to_string(),
            source_uri: source.source_uri().into(),
            proposals: chunk.to_vec(),
            file_facts: scan
                .file_facts
                .iter()
                .filter(|f| references.contains(&f.reference.uri()))
                .cloned()
                .collect(),
        })?;
        for id in receipt.record_ids {
            records.push(source_db.read(&id)?.ok_or("source DB receipt missing")?);
        }
    }
    let mut media = Vec::new();
    let mut ids = BTreeSet::new();
    for p in &groups {
        for r in std::iter::once(&p.original).chain(&p.proxies) {
            if ids.insert(r.uri()) {
                media.push(r.clone());
            }
        }
    }
    let backend = make_backend(source_config, &media)?;
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..config.parallelism.min(records.len()) {
            let (records, next, source, backend) = (&records, &next, &source, backend.as_ref());
            handles.push(scope.spawn(move || -> Result<()> {
                let mut db = config.media_records.media_db()?;
                while !cancel.load(Ordering::Relaxed) {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(record) = records.get(i) else {
                        break;
                    };
                    let name = record
                        .group
                        .proposal
                        .original
                        .relative_path()
                        .rsplit('/')
                        .next()
                        .unwrap_or("Clip");
                    let thumbnail = read_thumbnail(record, source);
                    if let Err(error) = &thumbnail {
                        send.send(Event::Warning(format!("{name}: {error}")))?;
                    }
                    let thumbnail = thumbnail.ok().flatten();
                    let result = process_record(
                        &mut db,
                        &config.source_index.uri,
                        record,
                        source,
                        backend,
                        cancel,
                        |snapshot| {
                            let mut clip = clip_view(snapshot, name);
                            if let Some((uri, image)) = &thumbnail {
                                clip.thumb_uri = Some(uri.clone());
                                clip.thumb_image = Some(image.clone());
                                clip.thumb_status = crate::ThumbStatus::Ready;
                            }
                            let _ = send.send(Event::Clip(clip));
                        },
                    );
                    if let Err(error) = result {
                        send.send(Event::Warning(format!("{name}: {error}")))?;
                    }
                }
                Ok(())
            }));
        }
        for handle in handles {
            handle.join().map_err(|_| "Select worker interrupted")??;
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    })?;
    Ok(())
}

type Thumbnail = (String, Arc<qnc_image_assets::RgbaImage>);
fn read_thumbnail(record: &SourceRecord, source: &SourceReader) -> Result<Option<Thumbnail>> {
    let readers = adapters();
    let p = &record.group.proposal;
    let Some(reader) = readers
        .iter()
        .find(|r| r.index.reader_id() == p.evidence.reader_id)
    else {
        return Ok(None);
    };
    let Some(reference) = (reader.thumbnail)(p) else {
        return Ok(None);
    };
    let data = match source.read_bytes(&reference, qnc_image_assets::MAX_BYTES as u64) {
        Ok(data) => data,
        Err(qnc_source_reader::ReadError::NotFound) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let decoded = qnc_image_assets::decode_thumbnail(&data.bytes)?;
    Ok(Some((reference.uri(), Arc::new(decoded))))
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;

fn process_record(
    db: &mut Client,
    source_index_uri: &str,
    record: &SourceRecord,
    source: &SourceReader,
    backend: &dyn ProbeBackend,
    cancel: &AtomicBool,
    mut publish: impl FnMut(&Snapshot),
) -> Result<()> {
    let clip_id = format!("clip-{}", record.record_id);
    let snapshot = if let Some(snapshot) = db.read(&clip_id, None)? {
        snapshot
    } else {
        let p = &record.group.proposal;
        let readers = adapters();
        let reader = readers
            .iter()
            .find(|r| r.index.reader_id() == p.evidence.reader_id)
            .ok_or("camera metadata reader unavailable")?;
        let mut documents = Vec::new();
        for reference in (reader.documents)(p) {
            match source.read_text(&reference, MAX_TEXT_BYTES) {
                Ok(text) => documents.push(IndexDocument {
                    reference,
                    text: text.text,
                }),
                Err(qnc_source_reader::ReadError::NotFound) if reference != p.evidence.document => {
                }
                Err(error) => return Err(error.into()),
            }
        }
        let metadata = (reader.metadata)(&clip_id, p, &documents)?;
        let evidence_uris: BTreeSet<_> = metadata
            .evidence
            .iter()
            .map(|e| e.document_uri.as_str())
            .collect();
        let documents = documents
            .iter()
            .filter(|d| evidence_uris.contains(d.reference.uri().as_str()))
            .map(|d| Document {
                document_uri: d.reference.uri(),
                media_type: DocumentType::Xml,
                text: d.text.clone(),
            })
            .collect();
        db.write(Write {
            request_id: uuid::Uuid::new_v4().to_string(),
            expected_revision: 0,
            phase: Phase::Camera,
            source_index_uri: source_index_uri.into(),
            source_record: record.clone(),
            metadata,
            documents,
        })?;
        db.read(&clip_id, None)?
            .ok_or("camera snapshot missing after commit")?
    };
    publish(&snapshot);
    if snapshot.phase == Phase::Final {
        if snapshot.completeness == Completeness::Partial {
            return Err("Baza sadrzi nepotpune metapodatke. Nema ponovnog probea.".into());
        }
        return Ok(());
    }
    let snapshot = complete_record(db, source_index_uri, record, snapshot, backend, cancel)?;
    publish(&snapshot);
    if snapshot.completeness == Completeness::Partial {
        return Err("Nepotpuni metapodaci spremljeni; ponovni probe nije dopusten.".into());
    }
    Ok(())
}

fn complete_record(
    db: &mut Client,
    source_index_uri: &str,
    record: &SourceRecord,
    camera: Snapshot,
    backend: &dyn ProbeBackend,
    cancel: &AtomicBool,
) -> Result<Snapshot> {
    let mut documents = Vec::new();
    let uris: BTreeSet<_> = camera
        .metadata
        .evidence
        .iter()
        .map(|e| &e.document_uri)
        .collect();
    for uri in uris {
        documents.push(db.document(uri)?.ok_or("camera evidence missing")?);
    }
    let mut reports = Vec::new();
    for (i, media_uri) in qnc_media_metadata_compose::required_probes(&camera)?
        .into_iter()
        .enumerate()
    {
        if cancel.load(Ordering::Relaxed) {
            return Err("Select je prekinut.".into());
        }
        let attempt_id = uuid::Uuid::new_v4().to_string();
        let document_uri = format!("qnc://local/artifact/probe-{attempt_id}");
        let claim = db.begin_acquisition(BeginAcquisition {
            attempt_id: attempt_id.clone(),
            clip_id: camera.metadata.clip_id.clone(),
            expected_revision: 1,
            media_uri: media_uri.clone(),
            document_uri: document_uri.clone(),
        })?;
        let document = if claim.granted {
            let request = ProbeRequest {
                version: qnc_media_probe::VERSION.into(),
                request_id: attempt_id.clone(),
                media_uri: media_uri.clone(),
                document_uri: document_uri.clone(),
            };
            match backend.execute(&request).and_then(|r| {
                r.validate(&request)?;
                Ok(r)
            }) {
                Ok(report) => {
                    let document = Document {
                        document_uri: report.document_uri,
                        media_type: DocumentType::Json,
                        text: report.json,
                    };
                    db.finish_acquisition(FinishAcquisition {
                        attempt_id,
                        outcome: AcquisitionOutcome::Stored { document_uri },
                        document: Some(document.clone()),
                    })?;
                    document
                }
                Err(error) => {
                    let code = format!("{error:?}");
                    let outcome = if matches!(error, qnc_media_probe::Error::TransportUncertain) {
                        AcquisitionOutcome::Uncertain { code }
                    } else {
                        AcquisitionOutcome::Failed { code }
                    };
                    db.finish_acquisition(FinishAcquisition {
                        attempt_id,
                        outcome,
                        document: None,
                    })?;
                    return Err(error.into());
                }
            }
        } else {
            match claim.acquisition.outcome {
                Some(AcquisitionOutcome::Stored { document_uri }) => db
                    .document(&document_uri)?
                    .ok_or("stored probe evidence missing")?,
                _ => {
                    return Err(
                        "Probe je vec pokusan ili je u tijeku. Nema ponovnog pokretanja.".into(),
                    )
                }
            }
        };
        // Raw evidence is durable before any semantic interpretation or merge.
        reports.push(qnc_ffprobe_metadata::read(
            &document.text,
            &media_uri,
            &document.document_uri,
            &format!("ffprobe-{i}"),
        )?);
        documents.push(document);
    }
    let metadata = qnc_media_metadata_compose::compose(&camera.metadata, &reports)?;
    db.write(Write {
        request_id: uuid::Uuid::new_v4().to_string(),
        expected_revision: camera.revision,
        phase: Phase::Final,
        source_index_uri: source_index_uri.into(),
        source_record: record.clone(),
        metadata,
        documents,
    })?;
    Ok(db
        .read(&camera.metadata.clip_id, None)?
        .ok_or("final snapshot missing")?)
}

fn clip_view(snapshot: &Snapshot, name: &str) -> ClipView {
    let duration = snapshot
        .metadata
        .original
        .duration_seconds
        .as_ref()
        .map(|f| f.value.numerator as f64 / f.value.denominator as f64)
        .unwrap_or(0.0);
    ClipView {
        clip_id: snapshot.metadata.clip_id.clone(),
        name: name.into(),
        duration_seconds: duration,
        ..ClipView::default()
    }
}
