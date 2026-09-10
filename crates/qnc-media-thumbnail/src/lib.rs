use qnc_source_reader::{SourceReader, SourceReference};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
        Arc,
    },
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
