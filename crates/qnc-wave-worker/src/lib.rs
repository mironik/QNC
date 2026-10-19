//! Public background waveform worker.
//!
//! The worker reads already saved clip records, generates timeline peak arrays
//! from the saved media metadata and publishes the artifact through the public
//! content write adapter. It does not scan, probe, own UI, own timeline state
//! or choose a hardcoded decoder.

use qnc_media_decode::{DecodeRequest, DecoderConfig};
use qnc_media_metadata::{MediaRepresentation, StreamDetails};
use qnc_media_records::Snapshot;
use qnc_media_stream::{LocalSource, MediaStream, SourceReference};
use qnc_transport_resolver::ResolverConfig;
use qnc_wave::{StreamPeakCollector, WaveArtifactRecord, WaveLane, WavePlan};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
        Arc,
    },
    thread::JoinHandle,
    time::Instant,
};

pub const MODULE_ID: &str = "qnc.module.wave-worker";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The most one-pass readers at once; how many really run, one short test of the medium
/// decides (`qnc-read-pace`; user 2026-10-18). Never more than the processor's threads.
const MOST_WAVE_READERS: usize = 8;
/// From this many readers on the medium serves the waves and the filmstrips at once.
const PARALLEL_MEDIUM_READERS: usize = 4;

/// The readers the medium's test found: `None` until the first pass has tested it; then one
/// pass runs at a time. Shared with the passes, kept for the run (no more testing).
type Readers = Arc<std::sync::Mutex<Option<usize>>>;

fn most_readers() -> usize {
    std::thread::available_parallelism().map_or(1, |threads| threads.get()).min(MOST_WAVE_READERS)
}
fn wave_paused_for_playback() -> &'static str {
    qnc_ui_text::text("wave.wave_worker_paused_broadcast_player")
}

#[derive(Debug, Clone, PartialEq)]
pub struct WaveClipRecord {
    pub clip_id: String,
    pub name: String,
    pub snapshot: Snapshot,
    pub priority: bool,
}

pub trait WaveContentRead: Send + Sync {
    fn list_clips(&self, after: Option<String>) -> Result<Vec<WaveClipRecord>, String>;
    fn read_clip(&self, clip_id: &str) -> Result<Option<WaveClipRecord>, String>;
    fn read_wave(&self, clip_id: &str) -> Result<Option<WaveArtifactRecord>, String>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct WaveWriteCompletion {
    pub key: String,
    pub result: Result<(), String>,
}

pub trait WaveContentWrite: Send {
    fn publish_wave(&mut self, key: String, artifact: WaveArtifactRecord) -> Result<(), String>;
    fn poll(&mut self) -> Vec<WaveWriteCompletion>;
    fn has_pending(&self) -> bool;
}

pub trait WaveContentWriteFactory: Send + Sync {
    fn start(&self) -> Result<Box<dyn WaveContentWrite>, String>;
}

#[derive(Clone)]
pub struct WaveSourceBinding {
    pub source_uri: String,
    pub local_root: Option<PathBuf>,
    pub endpoint: Option<String>,
    pub token: Option<String>,
    pub serial_number: Option<String>,
}

impl fmt::Debug for WaveSourceBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WaveSourceBinding")
            .field("source_uri", &self.source_uri)
            .field("local_root", &self.local_root)
            .field("endpoint", &self.endpoint)
            .field(
                "has_token",
                &self.token.as_ref().is_some_and(|t| !t.is_empty()),
            )
            .field("serial_number", &self.serial_number)
            .finish()
    }
}

impl WaveSourceBinding {
    fn matches_media_uri(&self, media_uri: &str) -> bool {
        SourceReference::from_uri(media_uri)
            .ok()
            .is_some_and(|reference| reference.source_uri() == self.source_uri)
    }

    fn resolver(&self) -> Result<ResolverConfig, String> {
        let parsed = qnc_contracts::parse_qnc_uri(&self.source_uri).map_err(|e| e.to_string())?;
        let resolver = ResolverConfig::new(PathBuf::new());
        match (
            parsed.environment.as_str(),
            &self.local_root,
            &self.endpoint,
        ) {
            ("local" | "lan" | "intranet", Some(root), None) => {
                Ok(resolver.with_local_binding(&self.source_uri, root))
            }
            ("lan", None, Some(endpoint)) => Ok(resolver
                .with_lan_authority(parsed.authority.ok_or("missing LAN authority")?, endpoint)),
            ("intranet", None, Some(endpoint)) => Ok(resolver.with_intranet_authority(
                parsed.authority.ok_or("missing intranet authority")?,
                endpoint,
            )),
            _ => Err("invalid wave source binding".into()),
        }
    }

    fn local_media_path(&self, media_uri: &str) -> Result<Option<PathBuf>, String> {
        let Some(root) = &self.local_root else {
            return Ok(None);
        };
        if let Some(serial) = &self.serial_number {
            qnc_dir_browser::verify_local_volume_serial(root, serial)
                .map_err(|error| error.to_string())?;
        }
        let reference = SourceReference::from_uri(media_uri).map_err(|error| error.to_string())?;
        if reference.source_uri() != self.source_uri {
            return Err("media source mismatch".into());
        }
        let root = root.canonicalize().map_err(|error| error.to_string())?;
        let path = root
            .join(reference.relative_path())
            .canonicalize()
            .map_err(|error| error.to_string())?;
        if !path.starts_with(&root) || !path.is_file() {
            return Err("media escapes source binding".into());
        }
        Ok(Some(path))
    }

    fn open_media_stream(&self, media_uri: &str) -> Result<MediaStream, String> {
        if let Some(root) = &self.local_root {
            if let Some(serial) = &self.serial_number {
                qnc_dir_browser::verify_local_volume_serial(root, serial)
                    .map_err(|error| error.to_string())?;
            }
            let local =
                LocalSource::new(&self.source_uri, root).map_err(|error| error.to_string())?;
            MediaStream::local(&local, media_uri).map_err(|error| error.to_string())
        } else {
            let resolver = self.resolver()?;
            let token = self
                .token
                .as_deref()
                .ok_or_else(|| "source credential missing".to_string())?;
            MediaStream::remote(&resolver, media_uri, token).map_err(|error| error.to_string())
        }
    }
}

#[derive(Clone)]
pub struct WaveContext {
    pub project_id: String,
    pub wave_root_uri: String,
    pub content_reader: Arc<dyn WaveContentRead>,
    pub content_writer: Arc<dyn WaveContentWriteFactory>,
    pub source_bindings: Vec<WaveSourceBinding>,
}

impl fmt::Debug for WaveContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WaveContext")
            .field("project_id", &self.project_id)
            .field("wave_root_uri", &self.wave_root_uri)
            .field("source_bindings", &self.source_bindings)
            .finish_non_exhaustive()
    }
}

impl WaveContext {
    fn project_id(&self) -> &str {
        &self.project_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveRequest {
    pub project_id: String,
    pub clip_id: String,
}

impl WaveRequest {
    fn key(&self) -> String {
        wave_cache_key(&self.project_id, &self.clip_id)
    }
}

#[derive(Debug)]
pub struct WavePoll {
    pub changed: bool,
    pub active_error: Option<String>,
}

impl WavePoll {
    fn empty() -> Self {
        Self {
            changed: false,
            active_error: None,
        }
    }
}

#[derive(Debug)]
struct WaveOutcome {
    request: WaveRequest,
    result: Result<GeneratedWave, String>,
}

#[derive(Debug)]
struct GeneratedWave {
    artifact: WaveArtifactRecord,
    ready_before: bool,
    total_ms: u128,
    /// The coarse wave of the intra frames: the clip comes again for every frame.
    coarse: bool,
}

#[derive(Debug)]
struct ActiveWaveWorker {
    request: WaveRequest,
    result: Receiver<WaveOutcome>,
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

#[derive(Default)]
pub struct TimelineWaveService {
    context: Option<WaveContext>,
    ready: BTreeSet<String>,
    queue: VecDeque<WaveRequest>,
    queued: BTreeSet<String>,
    workers: Vec<ActiveWaveWorker>,
    /// Held: only the clip open in a preview gets its wave (the filmstrips go first).
    held: bool,
    /// Coarse waves on their way to the database: their clips are not ready.
    coarse: BTreeSet<String>,
    /// Clips queued with no wave stored at all: until they have one (coarse or full) the
    /// filmstrips wait (user 2026-10-19: every clip's approximate sound first).
    without_wave: BTreeSet<String>,
    publisher: Option<Box<dyn WaveContentWrite>>,
    playback_priority: bool,
    /// The clip open in a preview: its pass reads the card alone (user 2026-10-18: two long
    /// passes side by side on one card took 45-55 s each, its filmstrip waited for it).
    preferred: Option<String>,
    /// How many passes read at once, found by one short test of the medium (user 2026-10-18).
    readers: Readers,
}

impl fmt::Debug for TimelineWaveService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TimelineWaveService")
            .field("context", &self.context)
            .field("ready", &self.ready.len())
            .field("queue", &self.queue.len())
            .field("queued", &self.queued.len())
            .field("workers", &self.workers.len())
            .field(
                "publisher_pending",
                &self.publisher.as_ref().map(|p| p.has_pending()),
            )
            .field("playback_priority", &self.playback_priority)
            .finish()
    }
}

impl TimelineWaveService {
    pub fn configure(&mut self, context: WaveContext) {
        let project_changed = self
            .context
            .as_ref()
            .is_some_and(|old| old.project_id() != context.project_id());
        if project_changed {
            self.cancel_and_join_workers();
            self.ready.clear();
            self.queue.clear();
            self.queued.clear();
            self.publisher = None;
            self.readers = Readers::default();
            self.without_wave.clear();
        }
        self.context = Some(context);
        self.ensure_publisher();
        self.start_next();
    }

    pub fn reset(&mut self) {
        self.cancel_and_join_workers();
        self.context = None;
        self.ready.clear();
        self.queue.clear();
        self.queued.clear();
        self.publisher = None;
        self.without_wave.clear();
    }

    pub fn set_playback_priority(&mut self, active: bool) {
        if self.playback_priority == active {
            return;
        }
        self.playback_priority = active;
        if active {
            self.cancel_active_workers();
        } else {
            self.start_next();
        }
    }

    pub fn sync_content_db(&mut self) -> Result<(), String> {
        let Some(context) = self.context.clone() else {
            return Ok(());
        };
        let project_id = context.project_id().to_string();
        let mut after = None;
        loop {
            let page = context.content_reader.list_clips(after.clone())?;
            if page.is_empty() {
                break;
            }
            let next_after = page
                .last()
                .map(|clip| clip.clip_id.clone())
                .ok_or_else(|| qnc_ui_text::text("wave.wave_content_page_empty").to_string())?;
            for clip in page {
                // A wave needs the final record; a clip still being completed is taken by
                // a later sync (user 2026-10-15: each wave as soon as its own record is).
                if clip.snapshot.phase != qnc_media_records::Phase::Final {
                    continue;
                }
                let request = WaveRequest {
                    project_id: project_id.clone(),
                    clip_id: clip.clip_id,
                };
                let key = request.key();
                if self.ready.contains(&key) {
                    continue;
                }
                match context.content_reader.read_wave(&request.clip_id)? {
                    Some(record) if wave_artifact_ready(&record) => {
                        self.ready.insert(key);
                        continue;
                    }
                    Some(_) => {}
                    None => _ = self.without_wave.insert(key),
                }
                self.enqueue(request, clip.priority);
            }
            if after.as_deref() == Some(next_after.as_str()) {
                return Err(qnc_ui_text::text("wave.wave_content_paging_did_move").into());
            }
            after = Some(next_after);
        }
        self.start_next();
        Ok(())
    }

    pub fn remove_clips(&mut self, clip_ids: &[String]) {
        let ids = clip_ids.iter().collect::<BTreeSet<_>>();
        let project_id = self
            .context
            .as_ref()
            .map(|ctx| ctx.project_id().to_string());
        self.queue.retain(|request| !ids.contains(&request.clip_id));
        self.without_wave.retain(|key| !clip_ids.iter().any(|clip_id| key.ends_with(&format!("::{clip_id}"))));
        self.queued.retain(|key| {
            project_id.as_ref().is_none_or(|project| {
                !clip_ids
                    .iter()
                    .any(|clip_id| key == &wave_cache_key(project, clip_id))
            })
        });
        if let Some(project) = project_id {
            for clip_id in clip_ids {
                self.ready.remove(&wave_cache_key(&project, clip_id));
            }
        }
    }

    pub fn poll(&mut self, active_clip_id: Option<&str>) -> WavePoll {
        let mut poll = WavePoll::empty();
        let current_project = self.context.as_ref().map(WaveContext::project_id);
        if !self.playback_priority {
            if let Some(publisher) = &mut self.publisher {
                for completion in publisher.poll() {
                    poll.changed = true;
                    match completion.result {
                        Ok(_) => {
                            wave_log(|| {
                                format!(
                                    "qnc-wave-worker-publish: key={} status=ready",
                                    completion.key
                                )
                            });
                            self.without_wave.remove(&completion.key);
                            // A coarse wave stored: its clip waits for the wave of every frame.
                            if !self.coarse.remove(&completion.key) {
                                self.ready.insert(completion.key);
                            }
                        }
                        Err(error) => {
                            let matches_active = current_project.is_some_and(|project| {
                                completion.key.starts_with(&format!("{project}::"))
                            }) && active_clip_id.is_some_and(|clip_id| {
                                completion.key.ends_with(&format!("::{clip_id}"))
                            });
                            wave_log(|| {
                                format!(
                                    "qnc-wave-worker-error: key={} stage=publish error={}",
                                    completion.key, error
                                )
                            });
                            if matches_active {
                                poll.active_error = Some(error);
                            }
                        }
                    }
                }
            }
        }
        let mut index = 0;
        while index < self.workers.len() {
            let outcome = match self.workers[index].result.try_recv() {
                Ok(outcome) => Some(outcome),
                Err(TryRecvError::Empty) => {
                    index += 1;
                    None
                }
                Err(TryRecvError::Disconnected) => Some(WaveOutcome {
                    request: self.workers[index].request.clone(),
                    result: Err(qnc_ui_text::text("wave.wave_worker_interrupted").into()),
                }),
            };
            let Some(outcome) = outcome else {
                continue;
            };
            let worker = self.workers.swap_remove(index);
            let _ = worker.thread.join();
            poll.changed = true;
            let current_project = self.context.as_ref().map(WaveContext::project_id);
            let matches_active = current_project == Some(outcome.request.project_id.as_str())
                && active_clip_id == Some(outcome.request.clip_id.as_str());
            if self.playback_priority {
                if current_project == Some(outcome.request.project_id.as_str()) {
                    self.enqueue(outcome.request, true);
                }
                continue;
            }
            match outcome.result {
                Ok(generated) if current_project == Some(outcome.request.project_id.as_str()) => {
                    let key = outcome.request.key();
                    let ready_before = generated.ready_before;
                    let total_ms = generated.total_ms;
                    let peak_count = generated.artifact.peak_count;
                    let a1_count = generated.artifact.a1_peaks.len();
                    let a2_count = generated.artifact.a2_peaks.len();
                    let a3_count = generated.artifact.a3_peaks.len();
                    let a4_count = generated.artifact.a4_peaks.len();
                    let source_uri = generated.artifact.source_uri.clone();
                    let publish_state = if ready_before { "ready" } else { "queued" };
                    let coarse = generated.coarse;
                    if generated.ready_before {
                        self.without_wave.remove(&key);
                        self.ready.insert(key);
                    } else {
                        if coarse {
                            self.coarse.insert(key.clone());
                        }
                        self.queue_publish(key, generated.artifact, matches_active, &mut poll);
                    }
                    if coarse {
                        // The wave of every frame comes after every clip's coarse one; the
                        // open clip's comes first.
                        let first = self.preferred.as_deref() == Some(outcome.request.clip_id.as_str());
                        self.enqueue(outcome.request.clone(), first);
                    }
                    wave_log(|| {
                        format!(
                            "qnc-wave-worker: clip={} source={} coarse={coarse} ready_before={} peaks={} a1={} a2={} a3={} a4={} total_ms={} publish={}",
                            outcome.request.clip_id,
                            source_uri,
                            ready_before,
                            peak_count,
                            a1_count,
                            a2_count,
                            a3_count,
                            a4_count,
                            total_ms,
                            publish_state
                        )
                    });
                }
                Ok(_) => {}
                Err(error) if error == wave_paused_for_playback() => {
                    wave_log(|| {
                        format!(
                            "qnc-wave-worker-paused: clip={} reason=playback_priority",
                            outcome.request.clip_id
                        )
                    });
                    if current_project == Some(outcome.request.project_id.as_str()) {
                        self.enqueue(outcome.request, true);
                    }
                }
                Err(error) => {
                    wave_log(|| {
                        format!(
                            "qnc-wave-worker-error: clip={} stage=generate active={} error={}",
                            outcome.request.clip_id, matches_active, error
                        )
                    });
                    // A clip whose wave cannot be made holds back no filmstrip.
                    self.without_wave.remove(&outcome.request.key());
                    if matches_active {
                        poll.active_error = Some(error);
                    }
                }
            }
        }
        if !self.playback_priority {
            self.start_next();
        }
        poll
    }

    /// The clip the user has open goes first (user 2026-10-15: editing starts on it while
    /// the card's other waves are still made). A clip not waiting is left as it is.
    /// Its pass reads the card alone: the other passes give way (they are cancelled and
    /// queued again, as for a player) and wait until it is done.
    pub fn prefer(&mut self, clip_id: &str) {
        self.preferred = Some(clip_id.to_string());
        if let Some(index) = self.queue.iter().position(|queued| queued.clip_id == clip_id) {
            let request = self.queue.remove(index).expect("index exists");
            self.queue.push_front(request);
            for worker in self.workers.iter().filter(|worker| worker.request.clip_id != clip_id) {
                worker.cancel.store(true, Ordering::Release);
            }
        }
    }

    /// Whether the preferred clip waits in the queue or runs: then nothing else starts.
    fn preferred_alone(&mut self) -> Option<bool> {
        let preferred = self.preferred.as_deref()?;
        if self.workers.iter().any(|worker| worker.request.clip_id == preferred) {
            return Some(true); // it runs: the card is its own
        }
        let index = self.queue.iter().position(|queued| queued.clip_id == preferred)?;
        let request = self.queue.remove(index).expect("index exists");
        self.queue.push_front(request);
        Some(!self.workers.is_empty()) // the others still stop: it starts once they have
    }

    /// Some clip has no wave at all yet (its coarse one is still to come).
    pub fn coarse_pending(&self) -> bool {
        !self.without_wave.is_empty()
    }

    /// The medium's test found that it serves several readers at once (a CFexpress or
    /// SSD reader): the full waves may read while the filmstrips decode. `false` before
    /// the test and for a slow medium.
    pub fn medium_parallel(&self) -> bool {
        self.readers
            .try_lock()
            .ok()
            .and_then(|readers| *readers)
            .is_some_and(|readers| readers >= PARALLEL_MEDIUM_READERS)
    }

    /// While held only the clip open in a preview gets its wave; the others wait
    /// (user 2026-10-18: records, then filmstrips, then waves).
    pub fn set_held(&mut self, held: bool) {
        self.held = held;
        if !held {
            self.start_next();
        }
    }

    pub fn has_pending_work(&self) -> bool {
        !self.workers.is_empty()
            || !self.queue.is_empty()
            || self
                .publisher
                .as_ref()
                .is_some_and(|publisher| publisher.has_pending())
    }

    fn enqueue(&mut self, request: WaveRequest, front: bool) {
        let key = request.key();
        if self.ready.contains(&key) || self.workers.iter().any(|worker| worker.request == request)
        {
            return;
        }
        if !self.queued.insert(key.clone()) {
            if front {
                if let Some(index) = self.queue.iter().position(|queued| queued == &request) {
                    let queued = self.queue.remove(index).expect("index exists");
                    self.queue.push_front(queued);
                }
            }
            return;
        }
        if front {
            self.queue.push_front(request);
        } else {
            self.queue.push_back(request);
        }
    }

    fn start_next(&mut self) {
        if self.playback_priority {
            return;
        }
        let Some(context) = self.context.clone() else {
            return;
        };
        match self.preferred_alone() {
            Some(true) => return,
            // The preferred clip is first in the queue and starts alone.
            Some(false) => {
                self.start_one(&context);
                return;
            }
            None if self.held => return,
            None => {}
        }
        // Until the medium is tested one pass runs (it tests first).
        let readers = self.readers.try_lock().ok().and_then(|readers| *readers).unwrap_or(1);
        while self.workers.len() < readers {
            if !self.start_one(&context) {
                return;
            }
        }
    }

    /// Starts the first request of the queue that still needs a wave; false when none.
    fn start_one(&mut self, context: &WaveContext) -> bool {
        loop {
            let Some(request) = self.queue.pop_front() else {
                return false;
            };
            self.queued.remove(&request.key());
            if request.project_id != context.project_id() || self.ready.contains(&request.key()) {
                continue;
            }
            let (send, receive) = mpsc::sync_channel(1);
            let worker_request = request.clone();
            let cancel = Arc::new(AtomicBool::new(false));
            let worker_cancel = cancel.clone();
            let readers = self.readers.clone();
            // Its coarse wave is made (maybe still on its way to the database): the wave of
            // every frame now (live 2026-10-19: the open clip's coarse wave was made 95 times
            // over, read again before its own write had landed).
            let coarse_made = self.coarse.contains(&request.key());
            match std::thread::Builder::new()
                .name("qnc-wave-worker".into())
                .spawn({
                    let context = context.clone();
                    move || run_wave_worker(worker_request, context, readers, coarse_made, worker_cancel, send)
                }) {
                Ok(thread) => self.workers.push(ActiveWaveWorker {
                    request,
                    result: receive,
                    cancel,
                    thread,
                }),
                Err(error) => {
                    wave_log(|| format!("qnc-wave-worker-error: stage=start error={error}"))
                }
            }
            return true;
        }
    }

    fn cancel_active_workers(&self) {
        for worker in &self.workers {
            worker.cancel.store(true, Ordering::Release);
        }
    }

    fn cancel_and_join_workers(&mut self) {
        self.cancel_active_workers();
        for worker in self.workers.drain(..) {
            let _ = worker.thread.join();
        }
    }

    fn ensure_publisher(&mut self) {
        if self.publisher.is_some() {
            return;
        }
        let Some(context) = &self.context else {
            return;
        };
        match context.content_writer.start() {
            Ok(publisher) => self.publisher = Some(publisher),
            Err(error) => {
                wave_log(|| format!("qnc-wave-worker-error: stage=transport_start error={error}"))
            }
        }
    }

    fn queue_publish(
        &mut self,
        key: String,
        artifact: WaveArtifactRecord,
        matches_active: bool,
        poll: &mut WavePoll,
    ) {
        self.ensure_publisher();
        let Some(publisher) = &mut self.publisher else {
            let error = qnc_ui_text::text("wave.wave_content_transport_available").to_string();
            wave_log(|| format!("qnc-wave-worker-error: stage=publish_queue error={error}"));
            if matches_active {
                poll.active_error = Some(error);
            }
            return;
        };
        if let Err(error) = publisher.publish_wave(key, artifact) {
            wave_log(|| format!("qnc-wave-worker-error: stage=publish_queue error={error}"));
            if matches_active {
                poll.active_error = Some(error);
            }
        }
    }
}

impl Drop for TimelineWaveService {
    fn drop(&mut self) {
        self.cancel_and_join_workers();
    }
}

pub fn wave_cache_key(project_id: &str, clip_id: &str) -> String {
    format!("{project_id}::{clip_id}")
}

fn run_wave_worker(
    request: WaveRequest,
    context: WaveContext,
    readers: Readers,
    coarse_made: bool,
    cancel: Arc<AtomicBool>,
    send: SyncSender<WaveOutcome>,
) {
    let result = build_timeline_wave(&request, &context, &readers, coarse_made, &cancel);
    let _ = send.send(WaveOutcome { request, result });
}

fn build_timeline_wave(
    request: &WaveRequest,
    context: &WaveContext,
    readers: &Readers,
    coarse_made: bool,
    cancel: &AtomicBool,
) -> Result<GeneratedWave, String> {
    if cancel.load(Ordering::Acquire) {
        return Err(wave_paused_for_playback().into());
    }
    let started = Instant::now();
    let stored = context.content_reader.read_wave(&request.clip_id)?;
    if let Some(record) = stored.as_ref().filter(|record| wave_artifact_ready(record)) {
        return Ok(GeneratedWave {
            artifact: record.clone(),
            ready_before: true,
            total_ms: started.elapsed().as_millis(),
            coarse: false,
        });
    }
    // A clip without any wave gets the coarse one first (user 2026-10-18: every clip's
    // approximate sound quickly, the complete one later).
    let coarse_first = stored.is_none() && !coarse_made;
    let clip = context
        .content_reader
        .read_clip(&request.clip_id)?
        .ok_or_else(|| qnc_ui_text::text("db.clip_project_database").to_string())?;
    let plan = qnc_wave::plan_from_snapshot_with_artifact_name(
        &clip.snapshot,
        &context.wave_root_uri,
        Some(&clip.name),
    )?;
    if cancel.load(Ordering::Acquire) {
        return Err(wave_paused_for_playback().into());
    }
    let source = source_for_uri(&context.source_bindings, &plan.source_uri)?;
    let media = qnc_wave::media_for_plan(&clip.snapshot, &plan)?;
    test_medium(readers, source, media, cancel);
    // The filmstrip reads its own pictures before the waves (user 2026-10-18: up to 10 s a
    // clip is read through, a longer one gives the intra picture nearest each of 13 places),
    // so the pass reads only the sound.
    let (lane_peaks, coarse) = match coarse_first.then(|| decode_wave(&plan, media, source, true, cancel)).transpose()?.flatten() {
        Some(lanes) => (lanes, true),
        None => (decode_wave(&plan, media, source, false, cancel)?.ok_or("wave: no full wave")?, false),
    };
    let has_a2 = lane_peaks.iter().any(|(lane, _)| *lane == WaveLane::A2);
    let warning = (!has_a2).then(|| qnc_ui_text::text("wave.source_a2_audio_channel").to_string());
    let mut artifact = qnc_wave::artifact_record_from_lane_peaks(&plan, lane_peaks, warning)?;
    artifact.coarse = coarse;
    if !wave_artifact_valid(&artifact) {
        return Err(qnc_ui_text::text("wave.wave_result_ready").to_string());
    }
    Ok(GeneratedWave {
        artifact,
        ready_before: false,
        total_ms: started.elapsed().as_millis(),
        coarse,
    })
}

/// The first pass of a run tests the medium it reads, once (user 2026-10-18); a file too
/// small for the test leaves it to the next pass. A failed test keeps one reader.
fn test_medium(readers: &Readers, source: &WaveSourceBinding, media: &MediaRepresentation, cancel: &AtomicBool) {
    let Ok(mut found) = readers.lock() else {
        return;
    };
    if found.is_some() {
        return;
    }
    let Ok(Some(path)) = source.local_media_path(&media.media_uri) else {
        return;
    };
    match qnc_read_pace::measure(&path, most_readers(), cancel) {
        Ok(Some(measured)) => {
            wave_log(|| {
                let rates: Vec<String> =
                    measured.rates.iter().map(|(level, rate)| format!("{level}:{:.0}MB/s", rate / 1e6)).collect();
                format!("qnc-wave-worker-pace: readers={} test={}", measured.readers, rates.join(" "))
            });
            *found = Some(measured.readers);
        }
        Ok(None) => {}
        Err(error) => {
            wave_log(|| format!("qnc-wave-worker-pace: test failed, one reader: {error}"));
            *found = Some(1);
        }
    }
}

/// A stored wave that needs no more work: the wave of every frame.
fn wave_artifact_ready(record: &WaveArtifactRecord) -> bool {
    !record.coarse && wave_artifact_valid(record)
}

/// A wave that can be stored and shown, coarse or of every frame (live 2026-10-19: every
/// coarse wave was refused as "not ready" and no clip got a wave).
fn wave_artifact_valid(record: &WaveArtifactRecord) -> bool {
    qnc_wave::validate_artifact(record).is_ok()
        && record.render_version == qnc_wave::WAVE_RENDER_VERSION
        && record.peaks().is_some()
}

fn decode_wave(
    plan: &WavePlan,
    media: &MediaRepresentation,
    source: &WaveSourceBinding,
    coarse: bool,
    cancel: &AtomicBool,
) -> Result<Option<Vec<(WaveLane, Vec<f32>)>>, String> {
    let streams = plan
        .channels
        .iter()
        .map(|channel| channel.stream_index)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    // An original keeps each channel in a stream of its own; one decoder per stream read
    // the whole file from the card once per channel. Read in one pass: every stream merged
    // in order, each one's channels handed to its own collector (user 2026-10-14).
    let decoder_config =
        qnc_decoder_catalog::installed_config().map_err(|error| error.to_string())?;
    let mut parts = Vec::new();
    for &stream_index in &streams {
        check_stream_supported(media, stream_index, &decoder_config)?;
        let source_channels = audio_stream_channels(media, stream_index)?;
        parts.push((source_channels, StreamPeakCollector::new(plan, stream_index)?));
    }
    let channels = parts.iter().map(|(count, _)| *count).sum::<usize>();
    // The source is the one this machine has bound (the card's volume serial is checked).
    source.open_media_stream(&media.media_uri)?;
    let path = source
        .local_media_path(&media.media_uri)?
        .ok_or("wave decoder requires a seekable QNC decoder endpoint")?;
    if media.container.is_none() {
        return Err(qnc_ui_text::text("wave.wave_needs_stored_container").to_string());
    }
    let mut bytes: Vec<Vec<u8>> = vec![Vec::new(); parts.len()];
    let mut on_samples = |samples: &[f32]| -> Result<(), String> {
        for buffer in &mut bytes {
            buffer.clear();
        }
        for frame in samples.chunks_exact(channels) {
            let mut at = 0;
            for (buffer, (count, _)) in bytes.iter_mut().zip(&parts) {
                for sample in &frame[at..at + count] {
                    buffer.extend_from_slice(&sample.to_le_bytes());
                }
                at += count;
            }
        }
        for (buffer, (_, collector)) in bytes.iter().zip(&mut parts) {
            collector.push_f32le_interleaved(buffer)?;
        }
        Ok(())
    };
    // A camera original stored as uncompressed 24-bit mono sound elements is read without
    // its pictures (`qnc-mxf-sound`; about twice as fast from a card), decided from the
    // saved format before any reading; every other source through the selected decoder.
    let mut pictures = None;
    let read = match (mxf_sound_elements(media, &streams), mp4_sound(media, &streams)) {
        // The coarse wave: the sound of the intra frames through the original's own index,
        // each held over its group of pictures; without an index there is none.
        (Some((elements, bytes)), _) if coarse => match mxf_intra_sound_samples(&path, &elements, bytes, cancel, &mut on_samples) {
            Ok(true) => Ok(()),
            Ok(false) => return Ok(None),
            Err(error) => Err(error),
        },
        _ if coarse => return Ok(None),
        (Some((elements, bytes)), _) => mxf_sound_samples(&path, &elements, bytes, None, &mut pictures, cancel, &mut on_samples),
        // An MP4 with one AAC sound track: its sound chunks and key pictures through its
        // index (user 2026-10-16), decoded from what was read.
        (None, Some(stream)) => mp4_sound_samples(&path, stream, channels as u32, None, &mut pictures, cancel, &mut on_samples),
        (None, None) => qnc_decoder_catalog::installed_filmstrip_extractor()
            .map_err(|error| error.to_string())?
            .ok_or("no local decoder for the wave")?
            .audio_streams_with_cancel(&path, media, &streams, channels as u32, cancel, &mut on_samples),
    };
    read.map_err(|error| if cancel.load(Ordering::Acquire) { wave_paused_for_playback().into() } else { error })?;
    let by_lane = parts
        .into_iter()
        .flat_map(|(_, collector)| collector.finish())
        .collect::<BTreeMap<_, _>>();
    if !by_lane.contains_key(&WaveLane::A1) {
        return Err(qnc_ui_text::text("wave.wave_got_a1_channel").to_string());
    }
    Ok(Some(by_lane.into_iter().collect()))
}

/// The selected decoder must declare the saved format of every stream it reads (no silent
/// fallback): the same check a decode request makes.
/// The sound element number of each requested stream, when the saved record is an MXF
/// whose requested streams are all uncompressed mono of one sample size (24-bit, or 16-bit
/// as XDCAM HD's LPCM16): the k-th audio stream of the file is sound element k (checked
/// byte for byte against FFmpeg on a camera original), with the sample size in bytes.
fn mxf_sound_elements(media: &qnc_media_metadata::MediaRepresentation, streams: &[u32]) -> Option<(Vec<u8>, usize)> {
    if media.container.as_ref()?.value != "mxf" {
        return None;
    }
    let mut audio = media
        .streams
        .iter()
        .filter_map(|stream| match &stream.details {
            qnc_media_metadata::StreamDetails::Audio(details) => Some((stream, details)),
            _ => None,
        })
        .collect::<Vec<_>>();
    audio.sort_by_key(|(stream, _)| stream.index.as_ref().map(|index| index.value));
    let mut size = None;
    let elements = streams
        .iter()
        .map(|wanted| {
            let position = audio
                .iter()
                .position(|(stream, _)| stream.index.as_ref().is_some_and(|index| index.value == *wanted))?;
            let (stream, details) = audio[position];
            let codec = stream.codec.as_ref().map(|codec| &codec.value);
            let bytes = match codec {
                Some(qnc_media_metadata::Signal::Known(name)) if name == "pcm_s24le" => 3,
                Some(qnc_media_metadata::Signal::Known(name)) if name == "pcm_s16le" => 2,
                _ => return None,
            };
            let mono = details.channels.as_ref().is_some_and(|channels| channels.value == 1);
            let same = *size.get_or_insert(bytes) == bytes;
            (mono && same).then(|| u8::try_from(position).ok()).flatten()
        })
        .collect::<Option<Vec<u8>>>()?;
    Some((elements, size?))
}

/// The track of the one requested sound stream when the saved record is an MP4 or
/// QuickTime file and that stream is AAC (the k-th stream is the file's k-th track).
fn mp4_sound(media: &qnc_media_metadata::MediaRepresentation, streams: &[u32]) -> Option<u32> {
    let container = media.container.as_ref()?.value.as_str();
    if !(container == "mp4" || container == "mov" || container.split(',').any(|name| name == "mp4" || name == "mov")) {
        return None;
    }
    let [stream] = streams else { return None };
    let found = media.streams.iter().find(|candidate| candidate.index.as_ref().is_some_and(|index| index.value == *stream))?;
    let aac = matches!(found.codec.as_ref().map(|codec| &codec.value), Some(qnc_media_metadata::Signal::Known(name)) if name == "aac");
    aac.then_some(*stream)
}

/// The sound of an MP4 through its index: AAC frames read from the card in a few hundred
/// reads, decoded by the selected decoder; with `picture_targets` the key pictures too.
fn mp4_sound_samples(
    source: &Path,
    stream: u32,
    channels: u32,
    picture_targets: Option<(&[u64], &'static str)>,
    pictures: &mut Option<qnc_filmstrip::EncodedPictures>,
    cancel: &AtomicBool,
    on_samples: &mut dyn FnMut(&[f32]) -> std::result::Result<(), String>,
) -> std::result::Result<(), String> {
    let tracks = qnc_mp4_sound::read_index(source)?;
    let sound = stream as usize;
    let config = tracks
        .get(sound)
        .filter(|track| track.is_sound() && &track.codec == b"mp4a")
        .and_then(|track| track.aac_config.clone())
        .ok_or("mp4: the stream is not an AAC track of the file")?;
    // H.264 pictures only: an MP4 sample becomes a picture that decodes on its own.
    let picture = picture_targets.filter(|(_, codec)| *codec == "h264").and_then(|(targets, _)| {
        let index = tracks.iter().position(|track| track.is_picture() && track.avc.is_some())?;
        Some((index, targets))
    });
    let avc = picture.and_then(|(index, _)| tracks[index].avc.clone());
    let mut adts = Vec::new();
    let mut found: Vec<Option<Vec<u8>>> = vec![None; picture.map_or(0, |(_, targets)| targets.len())];
    let mut error = None;
    qnc_mp4_sound::read_sound_and_pictures(
        source,
        cancel,
        &tracks,
        sound,
        picture,
        &mut |frame| qnc_mp4_sound::adts_frame(&config, frame, &mut adts),
        &mut |target, _, sample| {
            if let Some((nal_length, sets)) = &avc {
                match qnc_mp4_sound::avc_annexb(sample, *nal_length, sets) {
                    Ok(picture) => found[target] = Some(picture),
                    Err(problem) => error = Some(problem),
                }
            }
            Ok(())
        },
    )?;
    let decoder = qnc_decoder_catalog::installed_filmstrip_extractor()
        .map_err(|error| error.to_string())?
        .ok_or("no local decoder for the wave")?;
    decoder.audio_bytes_with_cancel("aac", adts, channels, cancel, on_samples)?;
    if error.is_none() && !found.is_empty() && found.iter().all(Option::is_some) {
        *pictures = Some(qnc_filmstrip::EncodedPictures { codec: "h264".into(), pictures: found.into_iter().flatten().collect() });
    }
    Ok(())
}

/// Sound elements to whole interleaved f32 frames of the requested streams, in their order.
/// The coarse sound of an MXF original: the sound elements of its intra frames only,
/// read through its own index, each intra frame's sound held over the frames up to the next
/// one, so the wave has every frame's place (a GOP of 24 reads one frame in 24). `false`
/// when the file has no usable index.
fn mxf_intra_sound_samples(
    source: &Path,
    elements: &[u8],
    sample_bytes: usize,
    cancel: &AtomicBool,
    on_samples: &mut dyn FnMut(&[f32]) -> std::result::Result<(), String>,
) -> std::result::Result<bool, String> {
    let Some(index) = qnc_mxf_index::MxfIndex::read(source)? else {
        return Ok(false);
    };
    let intra = index.intra_frames();
    if intra.is_empty() {
        return Ok(false);
    }
    let mut file = std::fs::File::open(source).map_err(|error| format!("wave original open: {error}"))?;
    let mut held: Vec<f32> = Vec::new();
    let mut next = 0usize;
    for frame in 0..index.entries.len() {
        if cancel.load(Ordering::Acquire) {
            return Err(wave_paused_for_playback().into());
        }
        if next < intra.len() && intra[next] == frame || held.is_empty() {
            let at = intra[next.min(intra.len() - 1)];
            let package = qnc_mxf_index::read_package(&mut file, &index, at, false, true)?;
            let mut channels = vec![Vec::<f32>::new(); elements.len()];
            for (number, value) in &package.sound {
                if let Some(slot) = elements.iter().position(|element| element == number) {
                    if sample_bytes == 2 {
                        qnc_mxf_sound::s16le_to_f32(value, &mut channels[slot]);
                    } else {
                        qnc_mxf_sound::s24le_to_f32(value, &mut channels[slot]);
                    }
                }
            }
            let samples = channels.iter().map(Vec::len).min().unwrap_or(0);
            if samples == 0 {
                return Ok(false);
            }
            held.clear();
            for sample in 0..samples {
                held.extend(channels.iter().map(|channel| channel[sample]));
            }
            if next < intra.len() && intra[next] <= frame {
                next += 1;
            }
        }
        on_samples(&held)?;
    }
    Ok(true)
}

fn mxf_sound_samples(
    source: &Path,
    elements: &[u8],
    sample_bytes: usize,
    picture_targets: Option<(&[u64], &'static str)>,
    pictures: &mut Option<qnc_filmstrip::EncodedPictures>,
    cancel: &AtomicBool,
    on_samples: &mut dyn FnMut(&[f32]) -> std::result::Result<(), String>,
) -> std::result::Result<(), String> {
    let mut queues = vec![Vec::<f32>::new(); elements.len()];
    let mut frames = Vec::new();
    let (targets, codec) = picture_targets.unwrap_or((&[], ""));
    let is_key = qnc_mxf_sound::key_picture_test(codec).unwrap_or(|_| false);
    let mut found: Vec<Option<Vec<u8>>> = vec![None; targets.len()];
    let mut on_picture = |target: usize, _number: u64, value: &[u8]| -> std::result::Result<(), String> {
        found[target] = Some(value.to_vec());
        Ok(())
    };
    let read = qnc_mxf_sound::read_sound_and_pictures(source, cancel, targets, &is_key, &mut |number, value| {
        let Some(slot) = elements.iter().position(|element| *element == number) else {
            return Ok(());
        };
        if sample_bytes == 2 {
            qnc_mxf_sound::s16le_to_f32(value, &mut queues[slot]);
        } else {
            qnc_mxf_sound::s24le_to_f32(value, &mut queues[slot]);
        }
        let ready = queues.iter().map(Vec::len).min().unwrap_or(0);
        if ready == 0 {
            return Ok(());
        }
        frames.clear();
        for at in 0..ready {
            frames.extend(queues.iter().map(|queue| queue[at]));
        }
        for queue in &mut queues {
            queue.drain(..ready);
        }
        on_samples(&frames)
    }, &mut on_picture);
    read?;
    // Every filmstrip frame needs its picture; otherwise the filmstrip reads the media itself.
    if !targets.is_empty() && found.iter().all(Option::is_some) {
        *pictures = Some(qnc_filmstrip::EncodedPictures { codec: codec.into(), pictures: found.into_iter().flatten().collect() });
    }
    Ok(())
}

fn check_stream_supported(
    media: &MediaRepresentation,
    stream_index: u32,
    decoder_config: &DecoderConfig,
) -> Result<(), String> {
    DecodeRequest {
        version: qnc_media_decode::VERSION.into(),
        media: media.clone(),
        stream_index,
        start: None,
        output_pixel_format: None,
        progressive: false,
        delivered_rate: None,
    }
    .validate(decoder_config)
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn audio_stream_channels(media: &MediaRepresentation, stream_index: u32) -> Result<usize, String> {
    let stream = media
        .streams
        .iter()
        .find(|stream| stream.index.as_ref().is_some_and(|index| index.value == stream_index))
        .ok_or_else(|| qnc_ui_text::text("wave.wave_audio_stream_found").to_string())?;
    let StreamDetails::Audio(audio) = &stream.details else {
        return Err(qnc_ui_text::text("wave.wave_stream_audio").into());
    };
    let channels = audio
        .channels
        .as_ref()
        .ok_or_else(|| qnc_ui_text::text("wave.wave_audio_stream_channel_count").to_string())?
        .value;
    Ok(channels as usize)
}

fn source_for_uri<'a>(
    sources: &'a [WaveSourceBinding],
    media_uri: &str,
) -> Result<&'a WaveSourceBinding, String> {
    sources
        .iter()
        .find(|source| source.matches_media_uri(media_uri))
        .ok_or_else(|| qnc_ui_text::text("wave.wave_source_transport_registered").to_string())
}

fn wave_diagnostics_enabled() -> bool {
    qnc_dev_diagnostics::wave_diagnostics_enabled()
}

fn wave_log(message: impl FnOnce() -> String) {
    if wave_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Wave, message());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A camera MP4 (QNC_MP4_SAMPLE, read only): the sound read through its index gives the
    /// same samples as the decoder reading the whole file, and its 13 key pictures come.
    #[test]
    #[ignore]
    fn mp4_sound_through_the_index_is_the_same_as_the_whole_file() {
        let Ok(path) = std::env::var("QNC_MP4_SAMPLE") else { return };
        let path = Path::new(&path);
        let cancel = AtomicBool::new(false);
        let mut through_index = Vec::new();
        let mut pictures = None;
        let targets: Vec<u64> = (0..13).map(|index| index * 1000).collect();
        let started = Instant::now();
        mp4_sound_samples(path, 1, 2, Some((&targets, "h264")), &mut pictures, &cancel, &mut |samples| {
            through_index.extend_from_slice(samples);
            Ok(())
        })
        .unwrap();
        let index_ms = started.elapsed().as_millis();
        let mut whole = Vec::new();
        let started = Instant::now();
        qnc_ffmpeg_decode::audio_streams_with_cancel(Path::new("ffmpeg"), path, "mp4", &[1], 2, &cancel, &mut |samples| {
            whole.extend_from_slice(samples);
            Ok(())
        })
        .unwrap();
        let whole_ms = started.elapsed().as_millis();
        eprintln!("through the index {index_ms} ms, whole file {whole_ms} ms, {} samples", whole.len());
        assert_eq!(through_index.len(), whole.len());
        assert!(through_index == whole, "the same samples");
        assert_eq!(pictures.map(|pictures| pictures.pictures.len()), Some(13));
    }

    #[test]
    fn cache_key_is_project_and_clip_scoped() {
        assert_eq!(wave_cache_key("p1", "c1"), "p1::c1");
        assert_ne!(wave_cache_key("p1", "c1"), wave_cache_key("p2", "c1"));
    }

    #[test]
    fn service_does_not_start_without_project_context() {
        let mut service = TimelineWaveService::default();
        service.sync_content_db().unwrap();

        assert!(!service.has_pending_work());
    }

    #[test]
    fn playback_priority_holds_wave_queue() {
        let mut service = TimelineWaveService::default();
        service.set_playback_priority(true);
        service.enqueue(
            WaveRequest {
                project_id: "project-a".into(),
                clip_id: "clip-a".into(),
            },
            false,
        );

        service.start_next();

        assert_eq!(service.queue.len(), 1);
        assert!(service.workers.is_empty());
    }

    #[test]
    fn the_preferred_clip_reads_the_card_alone() {
        let request = |clip: &str| WaveRequest { project_id: "project-a".into(), clip_id: clip.into() };
        let mut service = TimelineWaveService::default();
        let (_send, receive) = mpsc::sync_channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        service.workers.push(ActiveWaveWorker {
            request: request("long"),
            result: receive,
            cancel: cancel.clone(),
            thread: std::thread::spawn(|| {}),
        });
        service.enqueue(request("other"), false);
        service.enqueue(request("open"), false);
        service.prefer("open");
        assert!(cancel.load(Ordering::Acquire), "the other pass gives way");
        assert_eq!(service.queue.front().map(|queued| queued.clip_id.as_str()), Some("open"));
        assert_eq!(service.preferred_alone(), Some(true), "nothing starts while the other one stops");
        // Once it has stopped, the preferred clip starts first and alone.
        service.workers.clear();
        assert_eq!(service.preferred_alone(), Some(false));
        assert_eq!(service.queue.front().map(|queued| queued.clip_id.as_str()), Some("open"));
    }

    #[test]
    fn playback_priority_cancels_active_wave_workers() {
        let mut service = TimelineWaveService::default();
        let (_send, receive) = mpsc::sync_channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        service.workers.push(ActiveWaveWorker {
            request: WaveRequest {
                project_id: "project-a".into(),
                clip_id: "clip-a".into(),
            },
            result: receive,
            cancel: cancel.clone(),
            thread: std::thread::spawn(|| {}),
        });

        service.set_playback_priority(true);

        assert!(cancel.load(Ordering::Acquire));
    }
}
