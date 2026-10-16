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
const MAX_ACTIVE_WAVE_WORKERS: usize = 2;
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
    /// The key pictures of the filmstrip, read in the same pass over the original.
    pictures: Option<qnc_filmstrip::EncodedPictures>,
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
    /// Clips whose wave is done (made, already stored, or impossible) since the last
    /// `take_finished`, with the key pictures read in the same pass when there are.
    finished: Vec<(String, Option<Arc<qnc_filmstrip::EncodedPictures>>)>,
    publisher: Option<Box<dyn WaveContentWrite>>,
    playback_priority: bool,
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
                if let Some(record) = context.content_reader.read_wave(&request.clip_id)? {
                    if wave_artifact_ready(&record) {
                        self.ready.insert(key);
                        self.finished.push((request.clip_id, None));
                        continue;
                    }
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
                            self.ready.insert(completion.key);
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
                    // The wave is made: its clip's filmstrip may come now, from the key pictures
                    // read with it when there are.
                    self.finished.push((outcome.request.clip_id.clone(), generated.pictures.map(Arc::new)));
                    if generated.ready_before {
                        self.ready.insert(key);
                    } else {
                        self.queue_publish(key, generated.artifact, matches_active, &mut poll);
                    }
                    wave_log(|| {
                        format!(
                            "qnc-wave-worker: clip={} source={} ready_before={} peaks={} a1={} a2={} a3={} a4={} total_ms={} publish={}",
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
                    self.finished.push((outcome.request.clip_id.clone(), None)); // no wave to wait for
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
    pub fn prefer(&mut self, clip_id: &str) {
        if let Some(index) = self.queue.iter().position(|queued| queued.clip_id == clip_id) {
            let request = self.queue.remove(index).expect("index exists");
            self.queue.push_front(request);
        }
    }

    /// The clips whose wave is done since the last call (made, already stored, or
    /// impossible).
    pub fn take_finished(&mut self) -> Vec<(String, Option<Arc<qnc_filmstrip::EncodedPictures>>)> {
        std::mem::take(&mut self.finished)
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
        while self.workers.len() < MAX_ACTIVE_WAVE_WORKERS {
            let Some(request) = self.queue.pop_front() else {
                return;
            };
            self.queued.remove(&request.key());
            if request.project_id != context.project_id() || self.ready.contains(&request.key()) {
                continue;
            }
            let (send, receive) = mpsc::sync_channel(1);
            let worker_request = request.clone();
            let cancel = Arc::new(AtomicBool::new(false));
            let worker_cancel = cancel.clone();
            match std::thread::Builder::new()
                .name("qnc-wave-worker".into())
                .spawn({
                    let context = context.clone();
                    move || run_wave_worker(worker_request, context, worker_cancel, send)
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
    cancel: Arc<AtomicBool>,
    send: SyncSender<WaveOutcome>,
) {
    let result = build_timeline_wave(&request, &context, &cancel);
    let _ = send.send(WaveOutcome { request, result });
}

fn build_timeline_wave(
    request: &WaveRequest,
    context: &WaveContext,
    cancel: &AtomicBool,
) -> Result<GeneratedWave, String> {
    if cancel.load(Ordering::Acquire) {
        return Err(wave_paused_for_playback().into());
    }
    let started = Instant::now();
    if let Some(record) = context.content_reader.read_wave(&request.clip_id)? {
        if wave_artifact_ready(&record) {
            return Ok(GeneratedWave {
                artifact: record,
                ready_before: true,
                total_ms: started.elapsed().as_millis(),
                pictures: None,
            });
        }
    }
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
    // An original whose key pictures the reader knows (H.264, MPEG-2) gives its
    // filmstrip's key pictures in the same pass (user 2026-10-15: one reading of the card
    // for the wave and the filmstrip).
    let codec = picture_codec(&clip.snapshot.metadata.original);
    let targets = codec
        .and_then(|_| qnc_filmstrip::original_picture_frames(&clip.snapshot))
        .filter(|_| media.media_uri == clip.snapshot.metadata.original.media_uri)
        .zip(codec);
    let targets = targets.as_ref().map(|(frames, codec)| (frames.as_slice(), *codec));
    let (lane_peaks, pictures) = decode_wave(&plan, media, source, targets, cancel)?;
    let has_a2 = lane_peaks.iter().any(|(lane, _)| *lane == WaveLane::A2);
    let warning = (!has_a2).then(|| qnc_ui_text::text("wave.source_a2_audio_channel").to_string());
    let artifact = qnc_wave::artifact_record_from_lane_peaks(&plan, lane_peaks, warning)?;
    if !wave_artifact_ready(&artifact) {
        return Err(qnc_ui_text::text("wave.wave_result_ready").to_string());
    }
    Ok(GeneratedWave {
        artifact,
        ready_before: false,
        total_ms: started.elapsed().as_millis(),
        pictures,
    })
}

/// The stored picture codec of a representation when the MXF reader knows its key
/// pictures (`h264`, `mpeg2video`).
fn picture_codec(media: &MediaRepresentation) -> Option<&'static str> {
    media.streams.iter().find_map(|stream| {
        if !matches!(stream.details, StreamDetails::Video(_)) {
            return None;
        }
        match stream.codec.as_ref().map(|codec| &codec.value) {
            Some(qnc_media_metadata::Signal::Known(name)) => ["h264", "mpeg2video"].into_iter().find(|known| known == name),
            _ => None,
        }
    })
}

fn wave_artifact_ready(record: &WaveArtifactRecord) -> bool {
    qnc_wave::validate_artifact(record).is_ok()
        && record.render_version == qnc_wave::WAVE_RENDER_VERSION
        && record.peaks().is_some()
}

fn decode_wave(
    plan: &WavePlan,
    media: &MediaRepresentation,
    source: &WaveSourceBinding,
    picture_targets: Option<(&[u64], &'static str)>,
    cancel: &AtomicBool,
) -> Result<(Vec<(WaveLane, Vec<f32>)>, Option<qnc_filmstrip::EncodedPictures>), String> {
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
        (Some(elements), _) => mxf_sound_samples(&path, &elements, picture_targets, &mut pictures, cancel, &mut on_samples),
        // An MP4 with one AAC sound track: its sound chunks and key pictures through its
        // index (user 2026-10-16), decoded from what was read.
        (None, Some(stream)) => mp4_sound_samples(&path, stream, channels as u32, picture_targets, &mut pictures, cancel, &mut on_samples),
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
    Ok((by_lane.into_iter().collect(), pictures))
}

/// The selected decoder must declare the saved format of every stream it reads (no silent
/// fallback): the same check a decode request makes.
/// The sound element number of each requested stream, when the saved record is an MXF
/// whose requested streams are all uncompressed 24-bit mono: the k-th audio stream of the
/// file is sound element k (checked byte for byte against FFmpeg on a camera original).
fn mxf_sound_elements(media: &qnc_media_metadata::MediaRepresentation, streams: &[u32]) -> Option<Vec<u8>> {
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
    streams
        .iter()
        .map(|wanted| {
            let position = audio
                .iter()
                .position(|(stream, _)| stream.index.as_ref().is_some_and(|index| index.value == *wanted))?;
            let (stream, details) = audio[position];
            let codec = stream.codec.as_ref().map(|codec| &codec.value);
            let pcm = matches!(codec, Some(qnc_media_metadata::Signal::Known(name)) if name == "pcm_s24le");
            let mono = details.channels.as_ref().is_some_and(|channels| channels.value == 1);
            (pcm && mono).then(|| u8::try_from(position).ok()).flatten()
        })
        .collect()
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
fn mxf_sound_samples(
    source: &Path,
    elements: &[u8],
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
        qnc_mxf_sound::s24le_to_f32(value, &mut queues[slot]);
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
