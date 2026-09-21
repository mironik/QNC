//! Stage 3: metadata of one record.
//!
//! The camera adapter reads what the card declares; only a record that declares too
//! little is probed, once. Every step is stored through the media record DB. This stage
//! publishes snapshots to the caller and never writes the content DB itself.

use crate::*;
use qnc_ingest_store::content::InventoryClip;

/// What one metadata worker needs; shared by all workers of a Select run.
pub(crate) struct Worker<'a> {
    pub config: &'a SelectionConfig,
    pub source_config: &'a SourceConfig,
    pub source: &'a SourceReader,
    pub registry: &'a CameraRegistry,
    pub backend: &'a dyn ProbeBackend,
    pub records: &'a [SourceRecord],
    pub existing: &'a BTreeMap<String, InventoryClip>,
    pub next: &'a AtomicUsize,
    pub send: &'a SyncSender<Event>,
    pub cancel: &'a AtomicBool,
}

impl Worker<'_> {
    /// Takes records one by one until none are left or Select is cancelled.
    pub fn run(&self, publish: SyncSender<CatalogClip>) -> Result<()> {
        let mut db = self.config.media_records.media_db()?;
        while !self.cancel.load(Ordering::Relaxed) {
            let i = self.next.fetch_add(1, Ordering::Relaxed);
            let Some(record) = self.records.get(i) else {
                break;
            };
            self.describe(&mut db, record, &publish)?;
        }
        Ok(())
    }

    fn describe(
        &self,
        db: &mut Client,
        record: &SourceRecord,
        publish: &SyncSender<CatalogClip>,
    ) -> Result<()> {
        let (send, source, registry) = (self.send, self.source, self.registry);
        let name = record
            .group
            .proposal
            .original
            .relative_path()
            .rsplit('/')
            .next()
            .unwrap_or("Clip");
        let thumbnail = read_thumbnail(record, source, registry);
        if let Err(error) = &thumbnail {
            send.send(Event::Warning(format!("{name}: {error}")))?;
        }
        let thumbnail = thumbnail.ok().flatten();
        let clip_id = format!("clip-{}", record.record_id);
        let mut preview = SelectedClip {
            clip_id: clip_id.clone(),
            name: name.into(),
            previously_seen: self.existing.contains_key(&clip_id),
            save_state: SelectSaveState::Pending,
            thumb_uri: thumbnail_reference(record, registry).map(|r| r.uri()),
            ..Default::default()
        };
        if let Some((_, image)) = &thumbnail {
            preview.thumb_image = Some(image.clone());
            preview.thumb_status = SelectThumbStatus::Ready;
        }
        send.send(Event::Clip(preview))?;
        let result = process_record(
            db,
            &self.config.source_index.uri,
            record,
            source,
            self.backend,
            self.cancel,
            registry,
            |snapshot| {
                let catalog_clip = CatalogClip {
                    name: name.into(),
                    source_uri: source.source_uri().into(),
                    source_name: self.source_config.name.clone(),
                    serial_number: self.source_config.serial_number.clone(),
                    volume_name: self.source_config.volume_name.clone(),
                    thumbnail_uri: thumbnail_reference(record, registry).map(|r| r.uri()),
                    media_records_uri: self.config.media_records.uri.clone(),
                    snapshot: snapshot.clone(),
                };
                let mut clip = view(&StoredClip {
                    clip: catalog_clip.clone(),
                    selected: false,
                    import_status: ImportStatus::Detected,
                    import_error: None,
                    imported_media_uri: None,
                });
                clip.previously_seen = self.existing.contains_key(&clip.clip_id);
                clip.save_state = SelectSaveState::Pending;
                if let Some((uri, image)) = &thumbnail {
                    clip.thumb_uri = Some(uri.clone());
                    clip.thumb_image = Some(image.clone());
                    clip.thumb_status = SelectThumbStatus::Ready;
                }
                send.send(Event::Clip(clip))?;
                publish.send(catalog_clip)?;
                Ok(())
            },
        );
        if let Err(error) = result {
            // Revision zero exists only in the immediate, uncommitted preview.
            send.send(Event::Saved {
                revisions: vec![(clip_id, 0)],
                error: Some(error.to_string()),
            })?;
            send.send(Event::Warning(format!("{name}: {error}")))?;
        }
        Ok(())
    }
}

type Thumbnail = (String, Arc<qnc_image_assets::RgbaImage>);
pub(crate) fn read_thumbnail(
    record: &SourceRecord,
    source: &SourceReader,
    registry: &CameraRegistry,
) -> Result<Option<Thumbnail>> {
    let Some(reference) = thumbnail_reference(record, registry) else {
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

pub(crate) fn thumbnail_reference(
    record: &SourceRecord,
    registry: &CameraRegistry,
) -> Option<SourceReference> {
    let p = &record.group.proposal;
    registry.for_reader(&p.evidence.reader_id)?.thumbnail(p)
}

fn process_record(
    db: &mut Client,
    source_index_uri: &str,
    record: &SourceRecord,
    source: &SourceReader,
    backend: &dyn ProbeBackend,
    cancel: &AtomicBool,
    registry: &CameraRegistry,
    mut publish: impl FnMut(&Snapshot) -> Result<()>,
) -> Result<()> {
    let clip_id = format!("clip-{}", record.record_id);
    let adapter = registry
        .for_reader(&record.group.proposal.evidence.reader_id)
        .ok_or("camera metadata reader unavailable")?;
    let snapshot = if let Some(snapshot) = db.read(&clip_id, None)? {
        snapshot
    } else {
        let p = &record.group.proposal;
        let mut documents = Vec::new();
        for reference in adapter.documents(p) {
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
        let mut metadata = adapter.metadata(&clip_id, p, &documents)?;
        let evidence_uris: BTreeSet<_> = metadata
            .evidence
            .iter()
            .map(|e| e.document_uri.as_str())
            .collect();
        let mut documents: Vec<Document> = documents
            .iter()
            .filter(|d| evidence_uris.contains(d.reference.uri().as_str()))
            .map(|d| Document {
                document_uri: d.reference.uri(),
                media_type: DocumentType::Xml,
                text: d.text.clone(),
            })
            .collect();
        freeze_camera_documents(&mut metadata, &mut documents)?;
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
    publish(&snapshot)?;
    let declared = adapter.sufficiency(&snapshot.metadata) == MetadataSufficiency::Declared;
    if snapshot.phase == Phase::Final {
        // A declared record is final by itself; an incomplete one is complete
        // enough by definition and is never probed.
        if snapshot.completeness == Completeness::Partial && !declared {
            return Err("Baza sadrzi nepotpune metapodatke. Nema ponovnog probea.".into());
        }
        return Ok(());
    }
    if declared {
        // The card records are the final metadata: no probe, ever.
        let snapshot = finalize_declared(db, source_index_uri, record, snapshot)?;
        publish(&snapshot)?;
        return Ok(());
    }
    let snapshot = complete_record(db, source_index_uri, record, snapshot, backend, cancel)?;
    publish(&snapshot)?;
    if snapshot.completeness == Completeness::Partial {
        return Err("Nepotpuni metapodaci spremljeni; ponovni probe nije dopusten.".into());
    }
    Ok(())
}

/// Camera records that declare enough become the final snapshot without any probe.
fn finalize_declared(
    db: &mut Client,
    source_index_uri: &str,
    record: &SourceRecord,
    camera: Snapshot,
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
    db.write(Write {
        request_id: uuid::Uuid::new_v4().to_string(),
        expected_revision: camera.revision,
        phase: Phase::Final,
        source_index_uri: source_index_uri.into(),
        source_record: record.clone(),
        metadata: camera.metadata.clone(),
        documents,
    })?;
    Ok(db
        .read(&camera.metadata.clip_id, None)?
        .ok_or("final snapshot missing")?)
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
                    );
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
