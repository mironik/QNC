use qnc_source_reader::{SourceReader, SourceReference};
use std::fmt;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
        Arc,
    },
    thread::JoinHandle,
};

pub const MODULE_ID: &str = "qnc.module.media-thumbnail";
pub const VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailRequest {
    pub item_id: String,
    pub uri: String,
}

#[derive(Debug)]
pub enum ThumbnailEvent {
    Ready {
        item_id: String,
        uri: String,
        image: Arc<qnc_image_assets::RgbaImage>,
    },
    Finished,
}

pub trait ThumbnailItem {
    fn item_id(&self) -> &str;
    fn thumbnail_uri(&self) -> Option<&str>;
    fn set_thumbnail_ready(&mut self, image: Arc<qnc_image_assets::RgbaImage>);
}

/// A project folder on this machine: posters copied into it are addressed by project
/// URIs (`<root_uri>/<relative path>`), which are not source references.
#[derive(Debug, Clone)]
pub struct ProjectFolder {
    pub root_uri: String,
    pub dir: std::path::PathBuf,
}

impl ProjectFolder {
    fn read(&self, uri: &str) -> Option<Result<Arc<qnc_image_assets::RgbaImage>, String>> {
        let relative = uri
            .strip_prefix(self.root_uri.trim_end_matches('/'))?
            .strip_prefix('/')?;
        Some(self.read_relative(relative))
    }

    fn read_relative(&self, relative: &str) -> Result<Arc<qnc_image_assets::RgbaImage>, String> {
        let data = std::fs::read(self.dir.join(relative)).map_err(|e| e.to_string())?;
        qnc_image_assets::decode_thumbnail(&data)
            .map(Arc::new)
            .map_err(|e| e.to_string())
    }
}

#[derive(Default)]
pub struct ThumbnailBatchService {
    result: Option<Receiver<ThumbnailEvent>>,
    cancel: Option<Arc<AtomicBool>>,
    thread: Option<JoinHandle<()>>,
}

impl ThumbnailBatchService {
    pub fn start(
        &mut self,
        sources: Vec<SourceReader>,
        requests: Vec<ThumbnailRequest>,
    ) -> Result<(), String> {
        self.start_with_project(sources, None, requests)
    }

    /// Like `start`, and posters that live in the project folder of this machine are
    /// read from it as well.
    pub fn start_with_project(
        &mut self,
        sources: Vec<SourceReader>,
        project: Option<ProjectFolder>,
        requests: Vec<ThumbnailRequest>,
    ) -> Result<(), String> {
        self.cancel();
        if (sources.is_empty() && project.is_none()) || requests.is_empty() {
            return Ok(());
        }
        let (send, receive) = mpsc::sync_channel(32);
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let thread = std::thread::Builder::new()
            .name("qnc-media-thumbnail".into())
            .spawn(move || load_thumbnails(sources, project, requests, send, worker_cancel))
            .map_err(|error| format!("thumbnail worker start: {error}"))?;
        self.result = Some(receive);
        self.cancel = Some(cancel);
        self.thread = Some(thread);
        Ok(())
    }

    pub fn cancel(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.result = None;
        drop(self.thread.take());
    }

    pub fn poll(&mut self, limit: usize) -> Vec<ThumbnailEvent> {
        let mut events = Vec::new();
        for _ in 0..limit {
            let Some(receiver) = self.result.as_ref() else {
                break;
            };
            let event = match receiver.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => ThumbnailEvent::Finished,
            };
            let finished = matches!(event, ThumbnailEvent::Finished);
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

    pub fn poll_into<T: ThumbnailItem>(&mut self, items: &mut [T], limit: usize) -> bool {
        let mut changed = false;
        for event in self.poll(limit) {
            match event {
                ThumbnailEvent::Ready {
                    item_id,
                    uri,
                    image,
                } => {
                    if let Some(item) = items.iter_mut().find(|item| {
                        item.item_id() == item_id && item.thumbnail_uri() == Some(uri.as_str())
                    }) {
                        item.set_thumbnail_ready(image);
                        changed = true;
                    }
                }
                ThumbnailEvent::Finished => break,
            }
        }
        changed
    }
}

impl fmt::Debug for ThumbnailBatchService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThumbnailBatchService")
            .field("pending", &self.has_pending_work())
            .finish()
    }
}

impl Drop for ThumbnailBatchService {
    fn drop(&mut self) {
        self.cancel();
    }
}

pub fn load_from_sources(
    sources: Vec<SourceReader>,
    requests: Vec<ThumbnailRequest>,
    send: SyncSender<ThumbnailEvent>,
    cancel: Arc<AtomicBool>,
) {
    load_thumbnails(sources, None, requests, send, cancel)
}

pub fn load_thumbnails(
    sources: Vec<SourceReader>,
    project: Option<ProjectFolder>,
    requests: Vec<ThumbnailRequest>,
    send: SyncSender<ThumbnailEvent>,
    cancel: Arc<AtomicBool>,
) {
    let readers = sources
        .into_iter()
        .map(|reader| (reader.source_uri().to_string(), reader))
        .collect::<BTreeMap<_, _>>();
    for request in requests {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let image = match project.as_ref().and_then(|p| p.read(&request.uri)) {
            Some(result) => result,
            None => thumbnail(&readers, &request.uri),
        };
        let Ok(image) = image else {
            continue;
        };
        if send
            .send(ThumbnailEvent::Ready {
                item_id: request.item_id,
                uri: request.uri,
                image,
            })
            .is_err()
        {
            return;
        }
    }
    let _ = send.send(ThumbnailEvent::Finished);
}

fn thumbnail(
    readers: &BTreeMap<String, SourceReader>,
    uri: &str,
) -> Result<Arc<qnc_image_assets::RgbaImage>, String> {
    let reference = SourceReference::from_uri(uri).map_err(|e| e.to_string())?;
    let source = readers
        .get(reference.source_uri())
        .ok_or("Izvor postera nije dostupan.")?;
    let data = source
        .read_bytes(&reference, qnc_image_assets::MAX_BYTES as u64)
        .map_err(|e| e.to_string())?;
    qnc_image_assets::decode_thumbnail(&data.bytes)
        .map(Arc::new)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod project_read_tests {
    use super::*;

    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8,
        0xCF, 0xC0, 0xF0, 0x1F, 0x00, 0x05, 0x00, 0x01, 0xFF, 0x89, 0x99, 0x3D, 0x1D, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60,
    ];

    #[test]
    fn a_poster_copied_into_the_project_is_read_from_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let posters = dir.path().join("ingest").join("thumbnails");
        std::fs::create_dir_all(&posters).unwrap();
        std::fs::write(posters.join("c1_poster.png"), PNG_1X1).unwrap();
        let folder = ProjectFolder {
            root_uri: "qnc://local/project/p1".into(),
            dir: dir.path().to_path_buf(),
        };
        let image = folder
            .read("qnc://local/project/p1/ingest/thumbnails/c1_poster.png")
            .unwrap();
        assert!(image.is_ok(), "{:?}", image.err());
    }

    #[derive(Default)]
    struct Row {
        id: String,
        uri: Option<String>,
        ready: bool,
    }

    impl ThumbnailItem for Row {
        fn item_id(&self) -> &str {
            &self.id
        }

        fn thumbnail_uri(&self) -> Option<&str> {
            self.uri.as_deref()
        }

        fn set_thumbnail_ready(&mut self, _image: Arc<qnc_image_assets::RgbaImage>) {
            self.ready = true;
        }
    }

    #[test]
    fn ready_event_updates_matching_row_only() {
        let dir = tempfile::tempdir().unwrap();
        let posters = dir.path().join("ingest").join("thumbnails");
        std::fs::create_dir_all(&posters).unwrap();
        std::fs::write(posters.join("poster.png"), PNG_1X1).unwrap();
        let mut service = ThumbnailBatchService::default();
        service
            .start_with_project(
                Vec::new(),
                Some(ProjectFolder {
                    root_uri: "qnc://local/project/p1".into(),
                    dir: dir.path().to_path_buf(),
                }),
                vec![ThumbnailRequest {
                    item_id: "c1".into(),
                    uri: "qnc://local/project/p1/ingest/thumbnails/poster.png".into(),
                }],
            )
            .unwrap();
        let mut rows = vec![Row {
            id: "c1".into(),
            uri: Some("qnc://local/project/p1/ingest/thumbnails/poster.png".into()),
            ready: false,
        }];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while service.has_pending_work() {
            service.poll_into(&mut rows, 16);
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(rows[0].ready);
    }
}
