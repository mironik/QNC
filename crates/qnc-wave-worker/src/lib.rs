//! Public background waveform worker.
//!
//! The worker reads already saved clip records, generates timeline peak arrays
//! from the saved media metadata and publishes the artifact through the public
//! content write adapter. It does not scan, probe, own UI, own timeline state
//! or choose a hardcoded decoder.

use qnc_media_decode::{DecodeRequest, DecodedFormat, Decoder, DecoderConfig};
use qnc_media_metadata::{MediaRepresentation, Rational, StreamDetails};
use qnc_media_records::Snapshot;
use qnc_media_stream::{CodecEndpoint, LocalSource, MediaStream, SourceReference};
use qnc_transport_resolver::ResolverConfig;
use qnc_wave::{StreamPeakCollector, WaveArtifactRecord, WaveLane, WavePlan};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    path::PathBuf,
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
const WAVE_PAUSED_FOR_PLAYBACK: &str = "Wave worker je pauziran zbog Broadcast Playera.";

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
    pub project_audio_channels: u16,
    pub content_reader: Arc<dyn WaveContentRead>,
    pub content_writer: Arc<dyn WaveContentWriteFactory>,
    pub source_bindings: Vec<WaveSourceBinding>,
}

impl fmt::Debug for WaveContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WaveContext")
            .field("project_id", &self.project_id)
            .field("wave_root_uri", &self.wave_root_uri)
            .field("project_audio_channels", &self.project_audio_channels)
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
                .ok_or_else(|| "Wave content page je prazna.".to_string())?;
            for clip in page {
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
                        continue;
                    }
                }
                self.enqueue(request, clip.priority);
            }
            if after.as_deref() == Some(next_after.as_str()) {
                return Err("Wave content paging se nije pomaknuo.".into());
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
                    result: Err("Wave worker je prekinut.".into()),
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
                Err(error) if error == WAVE_PAUSED_FOR_PLAYBACK => {
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
            let error = "Wave content transport nije dostupan.".to_string();
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
        return Err(WAVE_PAUSED_FOR_PLAYBACK.into());
    }
    let started = Instant::now();
    if let Some(record) = context.content_reader.read_wave(&request.clip_id)? {
        if wave_artifact_ready(&record) {
            return Ok(GeneratedWave {
                artifact: record,
                ready_before: true,
                total_ms: started.elapsed().as_millis(),
            });
        }
    }
    let clip = context
        .content_reader
        .read_clip(&request.clip_id)?
        .ok_or_else(|| "Clip nije pronadjen u projektnoj bazi.".to_string())?;
    let plan = qnc_wave::plan_from_snapshot_with_project_audio_channels(
        &clip.snapshot,
        &context.wave_root_uri,
        Some(&clip.name),
        context.project_audio_channels,
    )?;
    if cancel.load(Ordering::Acquire) {
        return Err(WAVE_PAUSED_FOR_PLAYBACK.into());
    }
    let source = source_for_uri(&context.source_bindings, &plan.source_uri)?;
    let media = qnc_wave::media_for_plan(&clip.snapshot, &plan)?;
    let lane_peaks = decode_wave(&plan, media, source, cancel)?;
    let has_a2 = lane_peaks.iter().any(|(lane, _)| *lane == WaveLane::A2);
    let warning = (!has_a2).then(|| "Izvor nema A2 audio kanal.".to_string());
    let artifact = qnc_wave::artifact_record_from_lane_peaks(&plan, lane_peaks, warning)?;
    if !wave_artifact_ready(&artifact) {
        return Err("Wave rezultat nije spreman.".to_string());
    }
    Ok(GeneratedWave {
        artifact,
        ready_before: false,
        total_ms: started.elapsed().as_millis(),
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
    cancel: &AtomicBool,
) -> Result<Vec<(WaveLane, Vec<f32>)>, String> {
    let mut by_lane = BTreeMap::new();
    let streams = plan
        .channels
        .iter()
        .map(|channel| channel.stream_index)
        .collect::<BTreeSet<_>>();
    let decoder_config =
        qnc_decoder_catalog::installed_config().map_err(|error| error.to_string())?;
    for stream_index in streams {
        if cancel.load(Ordering::Acquire) {
            return Err(WAVE_PAUSED_FOR_PLAYBACK.into());
        }
        let (source_channels, sample_rate_hz, origin) = audio_stream_facts(media, stream_index)?;
        let mut collector = StreamPeakCollector::new(plan, stream_index)?;
        let mut decoder = open_decoder(source, media, stream_index, decoder_config.clone())?;
        loop {
            if cancel.load(Ordering::Acquire) {
                decoder.cancel();
                return Err(WAVE_PAUSED_FOR_PLAYBACK.into());
            }
            let Some(packet) = decoder.next_packet().map_err(|error| error.to_string())? else {
                break;
            };
            validate_audio_packet(
                &packet,
                media,
                stream_index,
                sample_rate_hz,
                source_channels,
            )?;
            let first_sample = qnc_wave::relative_sample_position(
                packet.pts,
                packet.time_base,
                origin,
                sample_rate_hz,
            )?;
            collector.push_f32le_interleaved_at(first_sample, &packet.bytes)?;
        }
        for (lane, peaks) in collector.finish() {
            by_lane.insert(lane, peaks);
        }
    }
    if !by_lane.contains_key(&WaveLane::A1) {
        return Err("Wave nije dobio A1 kanal.".to_string());
    }
    Ok(by_lane.into_iter().collect())
}

fn open_decoder(
    source: &WaveSourceBinding,
    media: &MediaRepresentation,
    stream_index: u32,
    decoder_config: DecoderConfig,
) -> Result<Decoder, String> {
    let request = DecodeRequest {
        version: qnc_media_decode::VERSION.into(),
        media: media.clone(),
        stream_index,
        start: None,
    };
    request
        .validate(&decoder_config)
        .map_err(|error| error.to_string())?;
    if let Some(path) = source
        .local_media_path(&media.media_uri)
        .map_err(|error| error.to_string())?
    {
        let stream = open_media_stream(source, &media.media_uri)?;
        let stamp = stream.info().storage_stamp.clone();
        let endpoint = CodecEndpoint::for_local_file(path, &media.media_uri)
            .map_err(|error| error.to_string())?;
        return Decoder::open_endpoint(request, endpoint, stamp, decoder_config)
            .map_err(|error| error.to_string());
    }
    Err("wave decoder requires a seekable QNC decoder endpoint".into())
}

fn validate_audio_packet(
    packet: &qnc_media_decode::DecodedPacket,
    media: &MediaRepresentation,
    stream_index: u32,
    sample_rate_hz: u32,
    source_channels: u32,
) -> Result<(), String> {
    let expected = DecodedFormat::Audio {
        sample_rate_hz,
        channels: source_channels,
        sample_format: "f32le".into(),
    };
    if packet.media_uri != media.media_uri
        || packet.stream_index != stream_index
        || packet.format != expected
    {
        return Err("Decoded wave audio differs from saved input.".into());
    }
    Ok(())
}

fn audio_stream_facts(
    media: &MediaRepresentation,
    stream_index: u32,
) -> Result<(u32, u32, (i64, Rational)), String> {
    let stream = media
        .streams
        .iter()
        .find(|stream| {
            stream
                .index
                .as_ref()
                .is_some_and(|index| index.value == stream_index)
        })
        .ok_or_else(|| "Wave audio stream nije pronadjen.".to_string())?;
    let StreamDetails::Audio(audio) = &stream.details else {
        return Err("Wave stream nije audio.".into());
    };
    let source_channels = audio
        .channels
        .as_ref()
        .ok_or_else(|| "Wave audio stream nema broj kanala.".to_string())?
        .value;
    let sample_rate_hz = audio
        .sample_rate_hz
        .as_ref()
        .ok_or_else(|| "Wave audio stream nema sample rate.".to_string())?
        .value;
    let origin = (
        stream
            .start_pts
            .as_ref()
            .ok_or_else(|| "Wave audio stream nema start PTS.".to_string())?
            .value,
        stream
            .time_base
            .as_ref()
            .ok_or_else(|| "Wave audio stream nema time base.".to_string())?
            .value,
    );
    Ok((source_channels, sample_rate_hz, origin))
}

fn source_for_uri<'a>(
    sources: &'a [WaveSourceBinding],
    media_uri: &str,
) -> Result<&'a WaveSourceBinding, String> {
    sources
        .iter()
        .find(|source| source.matches_media_uri(media_uri))
        .ok_or_else(|| "Wave source transport nije registriran.".to_string())
}

fn open_media_stream(source: &WaveSourceBinding, media_uri: &str) -> Result<MediaStream, String> {
    source.open_media_stream(media_uri)
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
