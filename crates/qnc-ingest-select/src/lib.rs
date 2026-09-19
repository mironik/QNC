mod metadata;
mod publish;
mod records;
mod scan;
pub mod selection_config;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use selection_config::{Binding, ProbeConfig, SelectionConfig, SourceConfig};

use qnc_ingest_store::content::{
    Access, CatalogClip, ContentTarget, ContentWriteData, ContentWriteResult,
    ContentWriteTransport, ImportStatus, StoredClip,
};
use qnc_camera_adapter::{CameraRegistry, MetadataSufficiency};
use qnc_media_probe::{ProbeBackend, Request as ProbeRequest};
use qnc_media_record_db::{contract::*, Client};
use qnc_source_groups::IndexDocument;
use qnc_source_reader::{SourceReader, SourceReference, MAX_TEXT_BYTES};
use selection_config::Result;
use std::fmt;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
        Arc,
    },
    thread::JoinHandle,
};

#[derive(Debug)]
pub enum Event {
    Status(String),
    Clip(SelectedClip),
    Existing(BTreeSet<String>),
    Saved {
        revisions: Vec<(String, u32)>,
        error: Option<String>,
    },
    Removed(Vec<String>),
    Warning(String),
    Finished(std::result::Result<Summary, String>),
}

#[derive(Debug)]
pub struct Summary {
    pub unchanged: usize,
    pub processed: usize,
    pub removed: usize,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectSaveState {
    Pending,
    Failed,
    #[default]
    Saved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectThumbStatus {
    Ready,
    Pending,
    #[default]
    Missing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectedClip {
    pub clip_id: String,
    pub name: String,
    pub duration_seconds: f64,
    pub selected: bool,
    pub imported: bool,
    pub previously_seen: bool,
    pub metadata_revision: u32,
    pub save_state: SelectSaveState,
    pub thumb_uri: Option<String>,
    pub thumb_status: SelectThumbStatus,
    pub thumb_image: Option<Arc<qnc_image_assets::RgbaImage>>,
}

const CANCEL_WAIT: std::time::Duration = std::time::Duration::from_millis(500);

#[derive(Default)]
pub struct SelectSession {
    result: Option<Receiver<Event>>,
    cancel: Option<Arc<AtomicBool>>,
    thread: Option<JoinHandle<()>>,
}

impl SelectSession {
    pub fn start(
        &mut self,
        config: SelectionConfig,
        selected: SourceReference,
        target: ContentTarget,
        registry: Arc<CameraRegistry>,
    ) -> std::result::Result<(), String> {
        self.cancel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (send, receive) = mpsc::sync_channel(32);
        let thread = std::thread::Builder::new()
            .name("qnc-ingest-select".into())
            .spawn(move || run(config, selected, target, registry, send, worker_cancel))
            .map_err(|error| error.to_string())?;
        self.cancel = Some(cancel);
        self.result = Some(receive);
        self.thread = Some(thread);
        Ok(())
    }

    /// Stops the running Select. Late events are discarded (the receiver is gone) and
    /// the worker starts no new database write once it sees the flag. The thread is
    /// waited for a short, bounded time so that a normal cancel ends deterministically
    /// without freezing the caller behind a long media read.
    pub fn cancel(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.result = None;
        if let Some(thread) = self.thread.take() {
            let deadline = std::time::Instant::now() + CANCEL_WAIT;
            while !thread.is_finished() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            if thread.is_finished() {
                let _ = thread.join();
            }
        }
    }

    pub fn poll(&mut self, limit: usize) -> Vec<Event> {
        let mut events = Vec::new();
        for _ in 0..limit {
            let Some(receiver) = self.result.as_ref() else {
                break;
            };
            let event = match receiver.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    Event::Finished(Err("Select proces je prekinut.".into()))
                }
            };
            let finished = matches!(event, Event::Finished(_));
            events.push(event);
            if finished {
                self.result = None;
                self.cancel = None;
                drop(self.thread.take());
                break;
            }
        }
        events
    }

    pub fn has_pending_work(&self) -> bool {
        self.result.is_some()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn from_receiver_for_test(receiver: Receiver<Event>) -> Self {
        Self {
            result: Some(receiver),
            cancel: None,
            thread: None,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_cancel_for_test(receiver: Receiver<Event>, cancel: Arc<AtomicBool>) -> Self {
        Self {
            result: Some(receiver),
            cancel: Some(cancel),
            thread: None,
        }
    }
}

impl fmt::Debug for SelectSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectSession")
            .field("pending", &self.has_pending_work())
            .finish()
    }
}

impl Drop for SelectSession {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl Default for SelectedClip {
    fn default() -> Self {
        Self {
            clip_id: String::new(),
            name: String::new(),
            duration_seconds: 0.0,
            selected: false,
            imported: false,
            previously_seen: false,
            metadata_revision: 0,
            save_state: SelectSaveState::Saved,
            thumb_uri: None,
            thumb_status: SelectThumbStatus::Missing,
            thumb_image: None,
        }
    }
}

pub fn run(
    config: SelectionConfig,
    selected: SourceReference,
    target: ContentTarget,
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
fn run_inner(
    config: &SelectionConfig,
    selected: &SourceReference,
    send: &SyncSender<Event>,
    cancel: &AtomicBool,
    make_backend: impl FnOnce(
        &selection_config::SourceConfig,
        &[SourceReference],
    ) -> Result<Box<dyn ProbeBackend + Send>>,
    content_target: ContentTarget,
    registry: &CameraRegistry,
) -> Result<Summary> {
    let started = std::time::Instant::now();
    if cancel.load(Ordering::Relaxed) {
        return Err("Select je prekinut.".into());
    }
    if registry.is_empty() {
        return Err("Nema registriranog adaptera kamere.".into());
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
    let records = records::register_groups(
        config,
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
        return Err("Select je prekinut.".into());
    }
    let removed = publish::remove_missing(content_target, &scanned.missing, send)?;
    Ok(Summary {
        unchanged: scanned.groups.len() - records.len(),
        processed: records.len(),
        removed,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

/// Runs the metadata workers and the single publisher next to each other.
fn describe_and_publish(worker: &metadata::Worker, content_target: ContentTarget) -> Result<()> {
    let (send, cancel) = (worker.send, worker.cancel);
    std::thread::scope(|scope| {
        let (publish, publications) = std::sync::mpsc::sync_channel::<CatalogClip>(32);
        let writer =
            scope.spawn(move || publish::publish_batches(content_target, publications, send, cancel));
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

fn view(stored: &StoredClip) -> SelectedClip {
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

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
