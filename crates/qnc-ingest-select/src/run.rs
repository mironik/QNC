use crate::*;

pub fn run(
    config: SelectionConfig,
    selected: SourceReference,
    target: SelectTarget,
    registry: Arc<CameraRegistry>,
    send: SyncSender<Event>,
    cancel: Arc<AtomicBool>,
) {
    let result = run_inner(
        &config,
        &selected,
        &send,
        &cancel,
        |s, media| s.backend(media),
        target,
        &registry,
    )
    .map_err(|e| e.to_string());
    let _ = send.send(Event::Finished(result));
}

/// The thin driver: scan the source, register the records, describe them and publish.
/// Each stage lives in its own module and knows nothing of the others.
pub(crate) fn run_inner(
    config: &SelectionConfig,
    selected: &SourceReference,
    send: &SyncSender<Event>,
    cancel: &AtomicBool,
    make_backend: impl FnOnce(
        &selection_config::SourceConfig,
        &[SourceReference],
    ) -> Result<Box<dyn ProbeBackend + Send>>,
    target: SelectTarget,
    registry: &CameraRegistry,
) -> Result<Summary> {
    let SelectTarget {
        content: content_target,
        records: records_target,
        posters,
    } = target;
    let started = std::time::Instant::now();
    if cancel.load(Ordering::Relaxed) {
        return Err(qnc_ui_text::text("select.select_interrupted").into());
    }
    if registry.is_empty() {
        return Err(qnc_ui_text::text("select.camera_adapter_registered").into());
    }
    selected.validate()?;
    let source_config = config
        .sources
        .iter()
        .find(|s| s.location.uri == selected.source_uri())
        .ok_or("unbound Select source")?;
    let source = source_config.reader()?;
    let scanned = scan::scan_source(
        config,
        selected,
        source_config,
        &source,
        &content_target,
        registry,
        send,
    )?;
    // One intermediary of the project database for every record of this Select
    // (v5: the ingest records live in the project database).
    let project = qnc_db_broker::ProjectDbWriter::start(
        records_target,
        vec![
            qnc_media_record_db::project::MediaRecordsModule::factory(),
            qnc_source_index_db::project::SourceIndexModule::factory(),
            qnc_artifact_db::ArtifactsModule::factory(),
        ],
    )?;
    let (records, replaced) = records::register_groups(
        &project,
        &source,
        &scanned.groups,
        &scanned.file_facts,
        &scanned.existing,
        cancel,
    )?;
    // No backend, thumbnail reads or metadata loads for an unchanged catalog.
    let backend = if records.is_empty() {
        None
    } else {
        Some(make_backend(
            source_config,
            &records::media_references(&records),
        )?)
    };
    if let Some(backend) = backend.as_deref() {
        describe_and_publish(
            &metadata::Worker {
                config,
                project: &project,
                source_config,
                source: &source,
                registry,
                backend,
                records: &records,
                existing: &scanned.existing,
                next: &AtomicUsize::new(0),
                send,
                cancel,
            },
            content_target.clone(),
        )?;
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(qnc_ui_text::text("select.select_interrupted").into());
    }
    let gone: Vec<_> = scanned.missing.iter().chain(&replaced).cloned().collect();
    let removed = publish::remove_missing(content_target, &project, &gone, send)?;
    publish::warn_kept_replaced(&replaced, &removed, send)?;
    let removed = removed.len();
    // Select runs the poster block (user 2026-10-18): the card's posters are copied into the
    // project and the missing ones made at once, on the block's own thread.
    if let Some(project) = posters {
        let sources = config.sources.clone();
        qnc_missing_posters::start(
            project,
            Box::new(move |uri: &str| {
                let reference = SourceReference::from_uri(uri).ok()?;
                let source = sources.iter().find(|source| source.location.uri == reference.source_uri())?;
                source.local_media_path(uri).ok().flatten()
            }),
        );
    }
    Ok(Summary {
        unchanged: scanned.groups.len() - records.len(),
        processed: records.len(),
        removed,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

/// Runs the metadata workers and the single publisher next to each other.
pub(crate) fn describe_and_publish(worker: &metadata::Worker, content_target: ContentTarget) -> Result<()> {
    let (send, cancel) = (worker.send, worker.cancel);
    std::thread::scope(|scope| {
        let (publish, publications) = std::sync::mpsc::sync_channel::<CatalogClip>(32);
        let writer = scope
            .spawn(move || publish::publish_batches(content_target, publications, send, cancel));
        let handles: Vec<_> = (0..worker.config.parallelism.min(worker.records.len()))
            .map(|_| {
                let publish = publish.clone();
                scope.spawn(move || worker.run(publish))
            })
            .collect();
        let mut worker_result = Ok(());
        for handle in handles {
            if let Err(error) = handle
                .join()
                .map_err(|_| "Select worker interrupted".into())
                .and_then(|r| r)
            {
                worker_result = Err(error);
            }
        }
        drop(publish);
        writer.join().map_err(|_| "Catalog writer interrupted")??;
        worker_result
    })
}

pub(crate) fn view(stored: &StoredClip) -> SelectedClip {
    let duration = stored
        .clip
        .snapshot
        .metadata
        .original
        .duration_seconds
        .as_ref()
        .map(|d| d.value.numerator as f64 / d.value.denominator as f64)
        .unwrap_or(0.0);
    SelectedClip {
        clip_id: stored.clip.id().into(),
        name: stored.clip.name.clone(),
        duration_seconds: duration,
        selected: stored.selected,
        imported: stored.import_status == ImportStatus::Imported,
        previously_seen: true,
        metadata_revision: stored.clip.snapshot.revision,
        thumb_uri: stored.clip.thumbnail_uri.clone(),
        thumb_status: if stored.clip.thumbnail_uri.is_some() {
            SelectThumbStatus::Pending
        } else {
            SelectThumbStatus::Missing
        },
        ..Default::default()
    }
}

