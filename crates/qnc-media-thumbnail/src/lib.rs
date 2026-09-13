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
        self.cancel();
        if sources.is_empty() || requests.is_empty() {
            return Ok(());
        }
        let (send, receive) = mpsc::sync_channel(32);
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let thread = std::thread::Builder::new()
            .name("qnc-media-thumbnail".into())
            .spawn(move || load_from_sources(sources, requests, send, worker_cancel))
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
    let readers = sources
        .into_iter()
        .map(|reader| (reader.source_uri().to_string(), reader))
        .collect::<BTreeMap<_, _>>();
    for request in requests {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let Ok(image) = thumbnail(&readers, &request.uri) else {
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
