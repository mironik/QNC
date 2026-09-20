//! Import execution for a claimed clip.
//!
//! What is copied and where it goes is decided only by the project settings
//! (`storage.ingest_media`, `playback.input`) through [`IngestWorkPlan`]:
//!
//! * `link`: nothing is copied; the imported media is the source URI chosen by
//!   `playback.input`;
//! * `proxy`: the proxy of the card is copied into the project `proxy` folder;
//! * `original`: the original is copied into the project `original` folder.
//!
//! Source media is read through the media stream transport (local disk, LAN or
//! intranet behind one contract), never by a path. The copy is a plain copy. The outcome is
//! written through the content transport
//! (`finish_import`). Nothing here depends on an operating system or a drive.
//!
//! With the media the poster is copied too. Not done here: proxy generation (transcode).

mod config;

pub use config::ConfigMediaOpener;
mod process;

pub use process::{
    is_running, launch_worker, remove_lock, run_service, touch_lock, ImportSummary,
    WORKER_EXECUTABLE,
};

use qnc_ingest_store::content::{ContentClient, StoredClip};
use qnc_ingest_work_plan::{IngestMedia, IngestWorkPlan, PlaybackInput};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

pub const MODULE_ID: &str = "qnc.module.ingest-import-worker";
pub const VERSION: &str = "0.1.0";

const CHUNK: usize = 1024 * 1024;

/// One open source medium: bytes and the length the transport reports.
pub trait MediaRead: Read + Send {
    fn byte_len(&self) -> u64;
}

/// Opens source media by QNC URI: local disk, LAN or intranet.
pub trait MediaOpener: Send + Sync {
    fn open(&self, media_uri: &str) -> Result<Box<dyn MediaRead>, String>;
    /// The copy waits while this is true (for example while the player works).
    fn paused(&self) -> bool {
        false
    }
}

/// The queue of the content database seen by the executor.
pub trait ImportQueue {
    fn claim_next(&mut self) -> Result<Option<StoredClip>, String>;
    /// Tells the queue this clip is still being imported (renews its lease).
    fn heartbeat(&mut self, _clip_id: &str) -> Result<(), String> {
        Ok(())
    }
    fn finish_import(
        &mut self,
        clip_id: String,
        media_uri: Option<String>,
        thumbnail_uri: Option<String>,
        error: Option<String>,
    ) -> Result<(), String>;
}

impl ImportQueue for ContentClient {
    fn heartbeat(&mut self, clip_id: &str) -> Result<(), String> {
        ContentClient::heartbeat(self, clip_id.to_string()).map_err(|e| e.to_string())
    }

    fn claim_next(&mut self) -> Result<Option<StoredClip>, String> {
        ContentClient::claim_next(self).map_err(|e| e.to_string())
    }

    fn finish_import(
        &mut self,
        clip_id: String,
        media_uri: Option<String>,
        thumbnail_uri: Option<String>,
        error: Option<String>,
    ) -> Result<(), String> {
        ContentClient::finish_import(self, clip_id, media_uri, thumbnail_uri, error).map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Folder {
    Original,
    Proxy,
}

impl Folder {
    fn name(self) -> &'static str {
        match self {
            Folder::Original => "original",
            Folder::Proxy => "proxy",
        }
    }
}

/// What the project settings ask for one clip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Keep the media where it is; the imported media is this source URI.
    Link { media_uri: String },
    /// Copy this source medium into the project folder.
    Copy { source_uri: String, folder: Folder },
}

pub fn action_for(clip: &StoredClip, plan: &IngestWorkPlan) -> Result<Action, String> {
    let binding = &clip.clip.snapshot.binding;
    let original = binding.original_uri.clone();
    let proxy = binding.proxy_uri.clone();
    match plan.media {
        IngestMedia::Link => {
            let media_uri = match plan.playback_input {
                PlaybackInput::Original => original,
                PlaybackInput::Proxy => proxy
                    .ok_or_else(|| "playback.input=proxy, ali kamera nema proxy.".to_string())?,
                PlaybackInput::ProxyIfAvailable => proxy.unwrap_or(original),
            };
            Ok(Action::Link { media_uri })
        }
        IngestMedia::Proxy => match proxy {
            Some(source_uri) => Ok(Action::Copy {
                source_uri,
                folder: Folder::Proxy,
            }),
            None => Err("Klip nema proxy, a generiranje proxyja jos nije podrzano.".into()),
        },
        IngestMedia::Original => Ok(Action::Copy {
            source_uri: original,
            folder: Folder::Original,
        }),
    }
}

/// A file name that is safe on every operating system and inside a QNC URI.
fn safe_name(clip_id: &str, source_uri: &str) -> String {
    let last = source_uri.rsplit('/').next().unwrap_or("media");
    let mut name = format!("{clip_id}_{last}");
    name = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    name.truncate(120);
    name
}

/// Well inside the store lease, so a slow copy never loses its clip.
const HEARTBEAT_EVERY: std::time::Duration = std::time::Duration::from_secs(20);

fn copy_into(
    opener: &dyn MediaOpener,
    source_uri: &str,
    destination: &Path,
    cancel: &AtomicBool,
    beat: &mut dyn FnMut(),
) -> Result<(), String> {
    let mut source = opener.open(source_uri)?;
    let mut file = fs::File::create(destination).map_err(|e| e.to_string())?;
    let mut buffer = vec![0_u8; CHUNK];
    let mut last_beat = std::time::Instant::now();
    loop {
        while opener.paused() {
            if cancel.load(Ordering::Relaxed) {
                return Err("Uvoz je prekinut.".into());
            }
            if last_beat.elapsed() >= HEARTBEAT_EVERY {
                beat();
                last_beat = std::time::Instant::now();
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if cancel.load(Ordering::Relaxed) {
            return Err("Uvoz je prekinut.".into());
        }
        let n = source.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(());
        }
        file.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        if last_beat.elapsed() >= HEARTBEAT_EVERY {
            beat();
            last_beat = std::time::Instant::now();
        }
    }
}

/// Executes the action for one clip and returns the URI of the imported media.
pub fn import_clip(
    clip: &StoredClip,
    plan: &IngestWorkPlan,
    project_dir: &Path,
    opener: &dyn MediaOpener,
    cancel: &AtomicBool,
) -> Result<String, String> {
    import_clip_beating(clip, plan, project_dir, opener, cancel, &mut || {})
}

/// Like `import_clip`, and calls `beat` regularly while a long copy runs.
pub fn import_clip_beating(
    clip: &StoredClip,
    plan: &IngestWorkPlan,
    project_dir: &Path,
    opener: &dyn MediaOpener,
    cancel: &AtomicBool,
    beat: &mut dyn FnMut(),
) -> Result<String, String> {
    match action_for(clip, plan)? {
        Action::Link { media_uri } => Ok(media_uri),
        Action::Copy { source_uri, folder } => {
            let name = safe_name(clip.clip.id(), &source_uri);
            let directory = project_dir.join(folder.name());
            fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
            copy_into(opener, &source_uri, &directory.join(&name), cancel, beat)?;
            let root = match folder {
                Folder::Original => &plan.original_uri,
                Folder::Proxy => &plan.proxy_uri,
            };
            let uri = format!("{}/{name}", root.trim_end_matches('/'));
            qnc_contracts::parse_qnc_uri(&uri).map_err(|e| e.to_string())?;
            Ok(uri)
        }
    }
}

/// The poster of a clip whose media was copied goes into the project too, so the clip
/// keeps its picture when the card is gone. A missing or unreadable poster never fails
/// the import: the clip simply keeps the poster reference of the card.
pub fn import_poster(
    clip: &StoredClip,
    plan: &IngestWorkPlan,
    project_dir: &Path,
    opener: &dyn MediaOpener,
    cancel: &AtomicBool,
) -> Option<String> {
    if !matches!(action_for(clip, plan), Ok(Action::Copy { .. })) {
        return None;
    }
    let source_uri = clip.clip.thumbnail_uri.as_deref()?;
    let clip_id = clip.clip.id();
    let directory = project_dir.join("ingest").join("thumbnails").join(clip_id);
    fs::create_dir_all(&directory).ok()?;
    copy_into(opener, source_uri, &directory.join("poster.jpg"), cancel, &mut || {}).ok()?;
    Some(format!(
        "{}/{clip_id}/poster.jpg",
        plan.thumbnails_uri.trim_end_matches('/')
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub clip_id: String,
    pub result: Result<String, String>,
    /// The poster copied into the project together with the media, if any.
    pub thumbnail_uri: Option<String>,
}

/// Claims the next queued clip, imports it and records the outcome. `None` when
/// the queue is empty. A failed import is an outcome (recorded as failed), not an
/// error of the executor.
pub fn run_next(
    queue: &mut dyn ImportQueue,
    plan: &IngestWorkPlan,
    project_dir: &Path,
    opener: &dyn MediaOpener,
    cancel: &AtomicBool,
) -> Result<Option<Outcome>, String> {
    let Some(clip) = queue.claim_next()? else {
        return Ok(None);
    };
    let clip_id = clip.clip.id().to_string();
    let result = import_clip_beating(&clip, plan, project_dir, opener, cancel, &mut || {
        let _ = queue.heartbeat(&clip_id);
    });
    let mut thumbnail_uri = None;
    match &result {
        Ok(uri) => {
            thumbnail_uri = import_poster(&clip, plan, project_dir, opener, cancel);
            queue.finish_import(
                clip_id.clone(),
                Some(uri.clone()),
                thumbnail_uri.clone(),
                None,
            )?
        }
        Err(error) => {
            let message: String = error.chars().take(4000).collect();
            queue.finish_import(clip_id.clone(), None, None, Some(message))?
        }
    }
    Ok(Some(Outcome {
        clip_id,
        result,
        thumbnail_uri,
    }))
}

/// Runs until the queue is empty or the import is cancelled.
pub fn drain(
    queue: &mut dyn ImportQueue,
    plan: &IngestWorkPlan,
    project_dir: &Path,
    opener: &dyn MediaOpener,
    cancel: &AtomicBool,
) -> Result<Vec<Outcome>, String> {
    let mut outcomes = Vec::new();
    while !cancel.load(Ordering::Relaxed) {
        match run_next(queue, plan, project_dir, opener, cancel)? {
            Some(outcome) => outcomes.push(outcome),
            None => break,
        }
    }
    Ok(outcomes)
}

#[cfg(test)]
mod tests;

/// The import queue reached through the serialized content write transport: the
/// only way this module writes to the content database.
pub struct TransportQueue {
    transport: qnc_ingest_store::content::ContentWriteTransport,
    sequence: u64,
}

impl TransportQueue {
    pub fn start(target: qnc_ingest_store::content::ContentTarget) -> Result<Self, String> {
        Ok(Self {
            transport: qnc_ingest_store::content::ContentWriteTransport::start(target)
                .map_err(|e| e.to_string())?,
            sequence: 0,
        })
    }

    fn next_key(&mut self, what: &str) -> String {
        self.sequence += 1;
        format!("import-{what}-{}", self.sequence)
    }

    fn wait(
        &mut self,
        key: &str,
    ) -> Result<qnc_ingest_store::content::ContentWriteData, String> {
        loop {
            for completion in self.transport.poll() {
                if completion.key == key {
                    return completion
                        .result
                        .map(|r| r.data)
                        .map_err(|e| e.to_string());
                }
            }
            if !self.transport.has_pending() {
                return Err("Content write transport nije vratio rezultat.".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    /// Puts the selected, ready clips into the queue.
    pub fn queue_selected(&mut self) -> Result<(), String> {
        let key = self.next_key("queue");
        self.transport
            .queue_selected(key.clone())
            .map_err(|e| e.to_string())?;
        self.wait(&key).map(|_| ())
    }
}

impl ImportQueue for TransportQueue {
    fn heartbeat(&mut self, clip_id: &str) -> Result<(), String> {
        let key = self.next_key("beat");
        self.transport
            .heartbeat(key.clone(), clip_id.to_string())
            .map_err(|e| e.to_string())?;
        self.wait(&key).map(|_| ())
    }

    fn claim_next(&mut self) -> Result<Option<StoredClip>, String> {
        use qnc_ingest_store::content::ContentWriteData;
        let key = self.next_key("claim");
        self.transport
            .claim_next(key.clone())
            .map_err(|e| e.to_string())?;
        match self.wait(&key)? {
            ContentWriteData::Claimed(clip) => Ok(clip.map(|c| *c)),
            _ => Err("Neispravan claim odgovor.".into()),
        }
    }

    fn finish_import(
        &mut self,
        clip_id: String,
        media_uri: Option<String>,
        thumbnail_uri: Option<String>,
        error: Option<String>,
    ) -> Result<(), String> {
        let key = self.next_key("finish");
        self.transport
            .finish_import(key.clone(), clip_id, media_uri, thumbnail_uri, error)
            .map_err(|e| e.to_string())?;
        self.wait(&key).map(|_| ())
    }
}

/// Queues the selected clips through the write transport of `target`.
pub fn queue_selected(target: qnc_ingest_store::content::ContentTarget) -> Result<(), String> {
    TransportQueue::start(target)?.queue_selected()
}
