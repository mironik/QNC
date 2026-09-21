//! Public background filmstrip worker.
//!
//! The worker reads already saved clip records, creates JPEG artifacts in the
//! project filmstrip directory and publishes their QNC URIs. It does not scan,
//! probe, own UI, own timeline state or choose a hardcoded decoder.

use qnc_filmstrip::{
    file_ready, FilmstripArtifactRecord, FilmstripExtractionMode, FilmstripFrameRecord,
    FilmstripFrameRequest, FilmstripPlan, LocalFilmstripArtifacts,
};
use qnc_frame_timebase::FrameTimebase;
use qnc_media_decode::{DecodeRequest, DecodedFormat, Decoder, DecoderConfig};
use qnc_media_metadata::{MediaRepresentation, Rational, StreamDetails, VideoMetadata};
use qnc_media_records::Snapshot;
use qnc_media_stream::{CodecEndpoint, LocalSource, MediaStream, SourceReference};
use qnc_pixel_convert::{ConversionSpec, Converter, RasterConverter};
use qnc_transport_resolver::ResolverConfig;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt, fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
        Arc,
    },
    thread::JoinHandle,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

pub const MODULE_ID: &str = "qnc.module.filmstrip-worker";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const THUMB_WIDTH: u32 = 112;
pub const THUMB_HEIGHT: u32 = 64;
const SEEK_PREROLL_FRAMES: u64 = 25;
const MAX_ACTIVE_FILMSTRIP_WORKERS: usize = 2;
const FRAME_PIPELINE_QUEUE: usize = 2;
const FILMSTRIP_PAUSED_FOR_PLAYBACK: &str = "Filmstrip je pauziran zbog Broadcast Playera.";

#[derive(Debug, Clone, PartialEq)]
pub struct FilmstripClipRecord {
    pub clip_id: String,
    pub name: String,
    pub snapshot: Snapshot,
    pub priority: bool,
}

pub trait FilmstripContentRead: Send + Sync {
    fn list_clips(&self, after: Option<String>) -> Result<Vec<FilmstripClipRecord>, String>;
    fn read_clip(&self, clip_id: &str) -> Result<Option<FilmstripClipRecord>, String>;
    fn read_filmstrip(&self, clip_id: &str) -> Result<Option<FilmstripArtifactRecord>, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilmstripWriteCompletion {
    pub key: String,
    pub result: Result<(), String>,
}

pub trait FilmstripContentWrite: Send {
    fn publish_filmstrip(
        &mut self,
        key: String,
        artifact: FilmstripArtifactRecord,
    ) -> Result<(), String>;
    fn poll(&mut self) -> Vec<FilmstripWriteCompletion>;
    fn has_pending(&self) -> bool;
}

pub trait FilmstripContentWriteFactory: Send + Sync {
    fn start(&self) -> Result<Box<dyn FilmstripContentWrite>, String>;
}

#[derive(Clone)]
pub struct FilmstripSourceBinding {
    pub source_uri: String,
    pub local_root: Option<PathBuf>,
    pub endpoint: Option<String>,
    pub token: Option<String>,
    pub serial_number: Option<String>,
}

impl fmt::Debug for FilmstripSourceBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilmstripSourceBinding")
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

impl FilmstripSourceBinding {
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
            _ => Err("invalid filmstrip source binding".into()),
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
pub struct FilmstripContext {
    pub project_id: String,
    pub filmstrip_root_uri: String,
    pub filmstrip_dir: PathBuf,
    pub content_reader: Arc<dyn FilmstripContentRead>,
    pub content_writer: Arc<dyn FilmstripContentWriteFactory>,
    pub source_bindings: Vec<FilmstripSourceBinding>,
}

impl fmt::Debug for FilmstripContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilmstripContext")
            .field("project_id", &self.project_id)
            .field("filmstrip_root_uri", &self.filmstrip_root_uri)
            .field("filmstrip_dir", &self.filmstrip_dir)
            .field("source_bindings", &self.source_bindings)
            .finish_non_exhaustive()
    }
}

impl FilmstripContext {
    fn project_id(&self) -> &str {
        &self.project_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilmstripRequest {
    pub project_id: String,
    pub clip_id: String,
}

impl FilmstripRequest {
    fn key(&self) -> String {
        filmstrip_cache_key(&self.project_id, &self.clip_id)
    }
}

#[derive(Debug)]
pub struct FilmstripPoll {
    pub changed: bool,
    pub active_error: Option<String>,
}

impl FilmstripPoll {
    fn empty() -> Self {
        Self {
            changed: false,
            active_error: None,
        }
    }
}

#[derive(Debug)]
struct FilmstripOutcome {
    request: FilmstripRequest,
    result: Result<GeneratedFilmstrip, String>,
}

#[derive(Debug)]
struct GeneratedFilmstrip {
    artifact: FilmstripArtifactRecord,
    source_kind: qnc_filmstrip::FilmstripSourceKind,
    extraction_mode: FilmstripExtractionMode,
    ready_before: bool,
    extract_ms: u128,
    total_ms: u128,
}

#[derive(Debug)]
struct ActiveFilmstripWorker {
    request: FilmstripRequest,
    result: Receiver<FilmstripOutcome>,
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

pub struct TimelineFilmstripService {
    context: Option<FilmstripContext>,
    ready: BTreeSet<String>,
    db_complete: bool,
    queue: VecDeque<FilmstripRequest>,
    queued: BTreeSet<String>,
    workers: Vec<ActiveFilmstripWorker>,
    publisher: Option<Box<dyn FilmstripContentWrite>>,
    playback_priority: bool,
}

impl Default for TimelineFilmstripService {
    fn default() -> Self {
        Self {
            context: None,
            ready: BTreeSet::new(),
            db_complete: true,
            queue: VecDeque::new(),
            queued: BTreeSet::new(),
            workers: Vec::new(),
            publisher: None,
            playback_priority: false,
        }
    }
}

impl fmt::Debug for TimelineFilmstripService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TimelineFilmstripService")
            .field("context", &self.context)
            .field("ready", &self.ready.len())
            .field("db_complete", &self.db_complete)
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

impl TimelineFilmstripService {
    pub fn configure(&mut self, context: FilmstripContext) {
        let project_changed =
            self.context.as_ref().map(FilmstripContext::project_id) != Some(context.project_id());
        if project_changed {
            self.cancel_and_join_workers();
            self.ready.clear();
            self.db_complete = false;
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
        self.db_complete = false;
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
            self.db_complete = true;
            return Ok(());
        };
        let project_id = context.project_id().to_string();
        let mut all_ready = true;
        let mut after = None;
        loop {
            let page = context.content_reader.list_clips(after.clone())?;
            if page.is_empty() {
                break;
            }
            let next_after = page
                .last()
                .map(|clip| clip.clip_id.clone())
                .ok_or_else(|| "Filmstrip content page je prazna.".to_string())?;
            for clip in page {
                let key = filmstrip_cache_key(&project_id, &clip.clip_id);
                if self.ready.contains(&key) {
                    continue;
                }
                if let Some(record) = context.content_reader.read_filmstrip(&clip.clip_id)? {
                    if filmstrip_artifact_ready(&context, &record) {
                        self.ready.insert(key);
                        continue;
                    }
                }
                all_ready = false;
                self.enqueue(
                    FilmstripRequest {
                        project_id: project_id.clone(),
                        clip_id: clip.clip_id,
                    },
                    clip.priority,
                );
            }
            if after.as_deref() == Some(next_after.as_str()) {
                return Err("Filmstrip content paging se nije pomaknuo.".into());
            }
            after = Some(next_after);
        }
        self.db_complete = all_ready;
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
                    .any(|clip_id| key == &filmstrip_cache_key(project, clip_id))
            })
        });
        if let Some(project) = project_id {
            for clip_id in clip_ids {
                self.ready.remove(&filmstrip_cache_key(&project, clip_id));
            }
        }
        self.db_complete = false;
    }

    pub fn poll(&mut self, active_clip_id: Option<&str>) -> FilmstripPoll {
        let mut poll = FilmstripPoll::empty();
        let current_project = self.context.as_ref().map(FilmstripContext::project_id);
        if !self.playback_priority {
            if let Some(publisher) = &mut self.publisher {
                for completion in publisher.poll() {
                    poll.changed = true;
                    match completion.result {
                        Ok(_) => {
                            self.ready.insert(completion.key);
                            self.db_complete = false;
                        }
                        Err(error) => {
                            self.db_complete = false;
                            eprintln!("Filmstrip content transport: {error}");
                            let matches_active = current_project.is_some_and(|project| {
                                completion.key.starts_with(&format!("{project}::"))
                            }) && active_clip_id.is_some_and(|clip_id| {
                                completion.key.ends_with(&format!("::{clip_id}"))
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
                Err(TryRecvError::Disconnected) => Some(FilmstripOutcome {
                    request: self.workers[index].request.clone(),
                    result: Err("Filmstrip worker je prekinut.".into()),
                }),
            };
            let Some(outcome) = outcome else {
                continue;
            };
            let worker = self.workers.swap_remove(index);
            let _ = worker.thread.join();
            poll.changed = true;
            let current_project = self.context.as_ref().map(FilmstripContext::project_id);
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
                    self.queue_publish(key, generated.artifact, matches_active, &mut poll);
                    filmstrip_log(|| {
                        format!(
                            "qnc-filmstrip-worker-total: clip={} source={:?} mode={:?} ready_before={} extract_ms={} publish=queued total_ms={}",
                            outcome.request.clip_id,
                            generated.source_kind,
                            generated.extraction_mode,
                            generated.ready_before,
                            generated.extract_ms,
                            generated.total_ms
                        )
                    });
                }
                Ok(_) => {}
                Err(error) if error == FILMSTRIP_PAUSED_FOR_PLAYBACK => {
                    if current_project == Some(outcome.request.project_id.as_str()) {
                        self.enqueue(outcome.request, true);
                    }
                }
                Err(error) if matches_active => {
                    self.db_complete = false;
                    poll.active_error = Some(error);
                }
                Err(_) => self.db_complete = false,
            }
        }
        if !self.playback_priority {
            self.start_next();
            if self.should_resync_content_db() {
                match self.sync_content_db() {
                    Ok(()) => poll.changed = true,
                    Err(error) => poll.active_error = Some(error),
                }
            }
        }
        poll
    }

    pub fn has_pending_work(&self) -> bool {
        !self.db_complete
            || !self.workers.is_empty()
            || !self.queue.is_empty()
            || self
                .publisher
                .as_ref()
                .is_some_and(|publisher| publisher.has_pending())
    }

    fn should_resync_content_db(&self) -> bool {
        !self.db_complete
            && self.workers.is_empty()
            && self.queue.is_empty()
            && self
                .publisher
                .as_ref()
                .is_none_or(|publisher| !publisher.has_pending())
    }

    fn enqueue(&mut self, request: FilmstripRequest, front: bool) {
        let key = request.key();
        if self.ready.contains(&key) || self.workers.iter().any(|worker| worker.request == request)
        {
            return;
        }
        self.db_complete = false;
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
        while self.workers.len() < MAX_ACTIVE_FILMSTRIP_WORKERS {
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
                .name("qnc-filmstrip-worker".into())
                .spawn({
                    let context = context.clone();
                    move || run_filmstrip_worker(worker_request, context, worker_cancel, send)
                }) {
                Ok(thread) => self.workers.push(ActiveFilmstripWorker {
                    request,
                    result: receive,
                    cancel,
                    thread,
                }),
                Err(error) => eprintln!("Filmstrip worker start: {error}"),
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
            Err(error) => eprintln!("Filmstrip content transport start: {error}"),
        }
    }

    fn queue_publish(
        &mut self,
        key: String,
        artifact: FilmstripArtifactRecord,
        matches_active: bool,
        poll: &mut FilmstripPoll,
    ) {
        self.ensure_publisher();
        let Some(publisher) = &mut self.publisher else {
            let error = "Filmstrip content transport nije dostupan.".to_string();
            eprintln!("{error}");
            if matches_active {
                poll.active_error = Some(error);
            }
            return;
        };
        if let Err(error) = publisher.publish_filmstrip(key, artifact) {
            eprintln!("Filmstrip content transport queue: {error}");
            if matches_active {
                poll.active_error = Some(error);
            }
        }
    }
}

impl Drop for TimelineFilmstripService {
    fn drop(&mut self) {
        self.cancel_and_join_workers();
    }
}

pub fn filmstrip_cache_key(project_id: &str, clip_id: &str) -> String {
    format!("{project_id}::{clip_id}")
}

fn filmstrip_artifact_ready(
    context: &FilmstripContext,
    artifact: &FilmstripArtifactRecord,
) -> bool {
    if artifact.status != "ready"
        || artifact.frames.is_empty()
        || artifact.frame_count != artifact.frames.len()
    {
        return false;
    }
    let Ok(artifacts) =
        LocalFilmstripArtifacts::new(&context.filmstrip_root_uri, context.filmstrip_dir.clone())
    else {
        return false;
    };
    artifact.frames.iter().all(|frame| {
        artifacts
            .frame_path(&frame.artifact_uri)
            .ok()
            .is_some_and(|path| file_ready(&path))
    })
}

fn filmstrip_scratch_dir(clip_id: &str) -> PathBuf {
    std::env::temp_dir()
        .join("qnc-filmstrip-worker")
        .join(format!(
            "{}_{}_{}",
            sanitize_scratch_part(clip_id),
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
}

fn sanitize_scratch_part(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "clip".into()
    } else {
        sanitized
    }
}

fn run_filmstrip_worker(
    request: FilmstripRequest,
    context: FilmstripContext,
    cancel: Arc<AtomicBool>,
    send: SyncSender<FilmstripOutcome>,
) {
    let result = build_timeline_background(&request, &context, &cancel);
    let _ = send.send(FilmstripOutcome { request, result });
}

fn build_timeline_background(
    request: &FilmstripRequest,
    context: &FilmstripContext,
    cancel: &AtomicBool,
) -> Result<GeneratedFilmstrip, String> {
    if cancel.load(Ordering::Acquire) {
        return Err(FILMSTRIP_PAUSED_FOR_PLAYBACK.into());
    }
    let started = Instant::now();
    let clip = context
        .content_reader
        .read_clip(&request.clip_id)?
        .ok_or_else(|| "Clip nije pronadjen u projektnoj bazi.".to_string())?;
    let plan = qnc_filmstrip::plan_from_snapshot_with_artifact_name(
        &clip.snapshot,
        &context.filmstrip_root_uri,
        Some(&clip.name),
    )?;
    let media = media_for_plan(&clip.snapshot.metadata, &plan)?;
    if cancel.load(Ordering::Acquire) {
        return Err(FILMSTRIP_PAUSED_FOR_PLAYBACK.into());
    }
    let artifacts =
        LocalFilmstripArtifacts::new(&context.filmstrip_root_uri, context.filmstrip_dir.clone())?;
    let ready_before = artifacts.ready(&plan);
    let extract_started = Instant::now();
    if !artifacts.ready(&plan) {
        DecoderFilmstripGenerator::from_installed_decoder_catalog()?.generate_from_sources(
            &context.source_bindings,
            media,
            &plan,
            &artifacts,
            cancel,
        )?;
    }
    let extract_ms = extract_started.elapsed().as_millis();
    if cancel.load(Ordering::Acquire) {
        return Err(FILMSTRIP_PAUSED_FOR_PLAYBACK.into());
    }
    let artifact = store_artifact_from_plan(&plan);
    Ok(GeneratedFilmstrip {
        artifact,
        source_kind: plan.source_kind,
        extraction_mode: plan.extraction_mode,
        ready_before,
        extract_ms,
        total_ms: started.elapsed().as_millis(),
    })
}

#[derive(Clone)]
pub struct DecoderFilmstripGenerator {
    decoder_config: DecoderConfig,
    fast_local_extractor: Option<qnc_decoder_catalog::LocalFilmstripExtractor>,
}

impl DecoderFilmstripGenerator {
    pub fn new(decoder_config: DecoderConfig) -> Self {
        Self {
            decoder_config,
            fast_local_extractor: None,
        }
    }

    pub fn from_installed_decoder_catalog() -> Result<Self, String> {
        let decoder_config =
            qnc_decoder_catalog::installed_config().map_err(|error| error.to_string())?;
        let fast_local_extractor = qnc_decoder_catalog::installed_filmstrip_extractor()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            decoder_config,
            fast_local_extractor,
        })
    }

    pub fn generate_from_sources(
        &self,
        sources: &[FilmstripSourceBinding],
        media: &MediaRepresentation,
        plan: &FilmstripPlan,
        artifacts: &LocalFilmstripArtifacts,
        cancel: &AtomicBool,
    ) -> Result<(), String> {
        if cancel.load(Ordering::Acquire) {
            return Err(FILMSTRIP_PAUSED_FOR_PLAYBACK.into());
        }
        if plan.frames.len() < 2 {
            return Err("filmstrip requires at least two frames".into());
        }
        let source = sources
            .iter()
            .find(|source| source.matches_media_uri(&plan.source_uri))
            .ok_or_else(|| "Filmstrip source binding nije pronadjen.".to_string())?;
        let output_paths = artifacts.output_paths(plan)?;
        if output_paths.iter().all(|path| file_ready(path)) {
            return Ok(());
        }
        if cancel.load(Ordering::Acquire) {
            return Err(FILMSTRIP_PAUSED_FOR_PLAYBACK.into());
        }
        let started = Instant::now();
        let result = match plan.extraction_mode {
            FilmstripExtractionMode::RandomSeek => self
                .extract_fast_local_to_outputs(source, plan, &output_paths, cancel)
                .unwrap_or_else(|| {
                    self.extract_random_seek(source, media, plan, &output_paths, cancel)
                }),
            FilmstripExtractionMode::KeyframeSeek => self
                .extract_fast_local_to_outputs(source, plan, &output_paths, cancel)
                .unwrap_or_else(|| {
                    Err(
                        "selected decoder does not provide local filmstrip keyframe extraction"
                            .into(),
                    )
                }),
        };
        if let Err(error) = result {
            return Err(error);
        }
        filmstrip_log(|| {
            format!(
                "qnc-filmstrip-worker: clip={} source={:?} mode={:?} frames={} build_ms={}",
                plan.clip_id,
                plan.source_kind,
                plan.extraction_mode,
                plan.frames.len(),
                started.elapsed().as_millis()
            )
        });
        Ok(())
    }

    fn extract_fast_local_to_outputs(
        &self,
        source: &FilmstripSourceBinding,
        plan: &FilmstripPlan,
        output_paths: &[PathBuf],
        cancel: &AtomicBool,
    ) -> Option<Result<(), String>> {
        let temp_dir = filmstrip_scratch_dir(&plan.clip_id);
        let result = (|| {
            if cancel.load(Ordering::Acquire) {
                return Err(FILMSTRIP_PAUSED_FOR_PLAYBACK.into());
            }
            fs::create_dir_all(&temp_dir)
                .map_err(|error| format!("filmstrip temp dir: {error}"))?;
            let Some(extract) = self.extract_fast_local(source, plan, &temp_dir, cancel) else {
                return Ok(false);
            };
            extract?;
            if cancel.load(Ordering::Acquire) {
                return Err(FILMSTRIP_PAUSED_FOR_PLAYBACK.into());
            }
            prepare_output_dirs(output_paths)?;
            for (index, target) in output_paths.iter().enumerate() {
                if cancel.load(Ordering::Acquire) {
                    return Err(FILMSTRIP_PAUSED_FOR_PLAYBACK.into());
                }
                let temp = temp_dir.join(format!("{index:03}.jpg"));
                if !file_ready(&temp) {
                    return Err("filmstrip frame missing".into());
                }
                move_or_copy(&temp, target)?;
            }
            Ok(true)
        })();
        let _ = fs::remove_dir_all(&temp_dir);
        match result {
            Ok(true) => Some(Ok(())),
            Ok(false) => None,
            Err(error) => Some(Err(error)),
        }
    }

    fn extract_fast_local(
        &self,
        source: &FilmstripSourceBinding,
        plan: &FilmstripPlan,
        temp_dir: &Path,
        cancel: &AtomicBool,
    ) -> Option<Result<(), String>> {
        let extractor = self.fast_local_extractor.as_ref()?;
        let source = match source.local_media_path(&plan.source_uri) {
            Ok(Some(path)) => path,
            Ok(None) => return None,
            Err(error) => return Some(Err(error.to_string())),
        };
        Some(
            extractor.extract_frames_with_cancel(
                &source,
                &plan
                    .frames
                    .iter()
                    .map(|frame| qnc_decoder_catalog::FilmstripExtractFrame {
                        seek_sec: frame.seek_sec,
                    })
                    .collect::<Vec<_>>(),
                plan.source_timebase,
                match plan.extraction_mode {
                    FilmstripExtractionMode::RandomSeek => {
                        qnc_decoder_catalog::FilmstripExtractMode::RandomSeek
                    }
                    FilmstripExtractionMode::KeyframeSeek => {
                        qnc_decoder_catalog::FilmstripExtractMode::KeyframeSeek
                    }
                },
                [THUMB_WIDTH, THUMB_HEIGHT],
                temp_dir,
                cancel,
            ),
        )
    }

    fn extract_random_seek(
        &self,
        source: &FilmstripSourceBinding,
        media: &MediaRepresentation,
        plan: &FilmstripPlan,
        output_paths: &[PathBuf],
        cancel: &AtomicBool,
    ) -> Result<(), String> {
        if cancel.load(Ordering::Acquire) {
            return Err(FILMSTRIP_PAUSED_FOR_PLAYBACK.into());
        }
        let (stream_index, video, origin) = video_stream(media)?;
        let mut pipeline = FrameWritePipeline::start(video, output_paths)?;
        for (target_frame, indices) in target_frame_map(plan) {
            if cancel.load(Ordering::Acquire) {
                let _ = pipeline.finish();
                return Err(FILMSTRIP_PAUSED_FOR_PLAYBACK.into());
            }
            let start = target_frame.saturating_sub(SEEK_PREROLL_FRAMES);
            let start = (start > 0).then(|| rational_from_frame(start, plan.source_timebase));
            let mut decoder = self.open_decoder(source, media, stream_index, start)?;
            loop {
                if cancel.load(Ordering::Acquire) {
                    let _ = pipeline.finish();
                    return Err(FILMSTRIP_PAUSED_FOR_PLAYBACK.into());
                }
                let Some(packet) = decoder.next_packet().map_err(|error| error.to_string())? else {
                    return Err("decoder ended before filmstrip frame".into());
                };
                validate_video_packet(&packet, media, stream_index, video)?;
                let frame = decoded_frame_number(&packet, origin, plan.source_timebase)?;
                if frame < target_frame {
                    continue;
                }
                if frame != target_frame {
                    return Err("decoder skipped requested filmstrip source frame".into());
                }
                pipeline.submit(packet, indices)?;
                break;
            }
        }
        pipeline.finish()
    }

    fn open_decoder(
        &self,
        source: &FilmstripSourceBinding,
        media: &MediaRepresentation,
        stream_index: u32,
        start: Option<Rational>,
    ) -> Result<Decoder, String> {
        let request = DecodeRequest {
            version: qnc_media_decode::VERSION.into(),
            media: media.clone(),
            stream_index,
            start,
        };
        request
            .validate(&self.decoder_config)
            .map_err(|error| error.to_string())?;
        if let Some(path) = source
            .local_media_path(&media.media_uri)
            .map_err(|error| error.to_string())?
        {
            let stream = open_media_stream(source, &media.media_uri)?;
            let stamp = stream.info().storage_stamp.clone();
            let endpoint = CodecEndpoint::for_local_file(path, &media.media_uri)
                .map_err(|error| error.to_string())?;
            return Decoder::open_endpoint(request, endpoint, stamp, self.decoder_config.clone())
                .map_err(|error| error.to_string());
        }
        Err("filmstrip decoder requires a seekable QNC decoder endpoint".into())
    }
}

fn prepare_output_dirs(output_paths: &[PathBuf]) -> Result<(), String> {
    let mut created = BTreeSet::new();
    for path in output_paths {
        let Some(parent) = path.parent() else {
            return Err("filmstrip output path has no parent".into());
        };
        if created.insert(parent.to_path_buf()) {
            fs::create_dir_all(parent).map_err(|error| format!("filmstrip output dir: {error}"))?;
        }
    }
    Ok(())
}

fn media_for_plan<'a>(
    metadata: &'a qnc_media_metadata::ClipMetadata,
    plan: &FilmstripPlan,
) -> Result<&'a MediaRepresentation, String> {
    if metadata.original.media_uri == plan.source_uri {
        return Ok(&metadata.original);
    }
    metadata
        .proxy
        .as_ref()
        .filter(|proxy| proxy.media_uri == plan.source_uri)
        .ok_or_else(|| "Filmstrip media zapis nije pronadjen u spremljenom klipu.".into())
}

fn video_stream(
    media: &MediaRepresentation,
) -> Result<(u32, &VideoMetadata, (i64, Rational)), String> {
    for stream in &media.streams {
        let StreamDetails::Video(video) = &stream.details else {
            continue;
        };
        let index = stream
            .index
            .as_ref()
            .ok_or_else(|| "Filmstrip video stream nema index.".to_string())?
            .value;
        let origin = (
            stream
                .start_pts
                .as_ref()
                .ok_or_else(|| "Filmstrip video stream nema start PTS.".to_string())?
                .value,
            stream
                .time_base
                .as_ref()
                .ok_or_else(|| "Filmstrip video stream nema time base.".to_string())?
                .value,
        );
        return Ok((index, video, origin));
    }
    Err("Filmstrip video stream nije pronadjen.".into())
}

fn converter_for(video: &VideoMetadata) -> Result<RasterConverter, String> {
    let spec = ConversionSpec::from_saved(video).map_err(|error| error.to_string())?;
    let scratch = spec.scratch_bytes().map_err(|error| error.to_string())?;
    let converter = Converter::prepare(spec, scratch).map_err(|error| error.to_string())?;
    RasterConverter::prepare(converter, Some([THUMB_WIDTH, THUMB_HEIGHT]))
        .map_err(|error| error.to_string())
}

fn target_frame_map(plan: &FilmstripPlan) -> BTreeMap<u64, Vec<usize>> {
    let mut targets: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    for frame in &plan.frames {
        targets
            .entry(target_frame_number(frame, plan))
            .or_default()
            .push(frame.index);
    }
    targets
}

fn target_frame_number(frame: &FilmstripFrameRequest, plan: &FilmstripPlan) -> u64 {
    let fps = plan.source_timebase;
    let source_frame =
        (frame.seek_sec.max(0.0) * fps.fps_num as f64 / fps.fps_den as f64).round() as u64;
    source_frame.min(plan.source_duration_frames.saturating_sub(1))
}

fn rational_from_frame(frame: u64, timebase: FrameTimebase) -> Rational {
    Rational {
        numerator: (frame as i128 * i128::from(timebase.fps_den)).min(i64::MAX as i128) as i64,
        denominator: timebase.fps_num,
    }
}

fn decoded_frame_number(
    packet: &qnc_media_decode::DecodedPacket,
    origin: (i64, Rational),
    timebase: FrameTimebase,
) -> Result<u64, String> {
    relative_position(
        packet.pts,
        packet.time_base,
        origin,
        timebase.fps_num,
        timebase.fps_den,
    )
}

fn relative_position(
    pts: i64,
    tb: Rational,
    origin: (i64, Rational),
    num: i64,
    den: i64,
) -> Result<u64, String> {
    let mul = |a: i128, b: i128| {
        a.checked_mul(b)
            .ok_or_else(|| "filmstrip timestamp overflow".to_string())
    };
    if tb.numerator <= 0
        || tb.denominator <= 0
        || origin.1.numerator <= 0
        || origin.1.denominator <= 0
        || num <= 0
        || den <= 0
    {
        return Err("invalid filmstrip timestamp scale".into());
    }
    let a = mul(
        mul(pts.into(), tb.numerator.into())?,
        origin.1.denominator.into(),
    )?;
    let b = mul(
        mul(origin.0.into(), origin.1.numerator.into())?,
        tb.denominator.into(),
    )?;
    let n = mul(
        a.checked_sub(b)
            .ok_or_else(|| "filmstrip timestamp overflow".to_string())?,
        num.into(),
    )?;
    let d = mul(
        mul(tb.denominator.into(), origin.1.denominator.into())?,
        den.into(),
    )?;
    if n < 0 || n % d != 0 {
        return Err("filmstrip timestamp is not on saved source grid".into());
    }
    u64::try_from(n / d).map_err(|error| error.to_string())
}

fn validate_video_packet(
    packet: &qnc_media_decode::DecodedPacket,
    media: &MediaRepresentation,
    stream_index: u32,
    video: &VideoMetadata,
) -> Result<(), String> {
    let spec = ConversionSpec::from_saved(video).map_err(|error| error.to_string())?;
    let expected = DecodedFormat::Video {
        width: spec.width,
        height: spec.height,
        pixel_format: spec.layout.name().into(),
    };
    if packet.media_uri != media.media_uri
        || packet.stream_index != stream_index
        || packet.format != expected
    {
        return Err("decoded filmstrip frame differs from saved input".into());
    }
    Ok(())
}

struct FrameEncodeJob {
    packet: qnc_media_decode::DecodedPacket,
    indices: Vec<usize>,
}

struct FrameWriteJob {
    index: usize,
    bytes: Vec<u8>,
}

struct FrameWritePipeline {
    encode_send: Option<SyncSender<FrameEncodeJob>>,
    encoder: JoinHandle<Result<(), String>>,
    writer: JoinHandle<Result<(), String>>,
}

impl FrameWritePipeline {
    fn start(video: &VideoMetadata, output_paths: &[PathBuf]) -> Result<Self, String> {
        prepare_output_dirs(output_paths)?;
        let (encode_send, encode_recv) = mpsc::sync_channel::<FrameEncodeJob>(FRAME_PIPELINE_QUEUE);
        let (write_send, write_recv) = mpsc::sync_channel::<FrameWriteJob>(FRAME_PIPELINE_QUEUE);
        let video = video.clone();
        let output_paths = output_paths.to_vec();

        let encoder = std::thread::Builder::new()
            .name("qnc-filmstrip-encode".into())
            .spawn(move || {
                let mut converter = converter_for(&video)?;
                let mut rgba = vec![0; converter.output_bytes()];
                while let Ok(job) = encode_recv.recv() {
                    converter
                        .convert(&job.packet.bytes, &mut rgba)
                        .map_err(|error| error.to_string())?;
                    let bytes = encode_padded_jpeg_bytes(converter.size(), &rgba)?;
                    for index in job.indices {
                        write_send
                            .send(FrameWriteJob {
                                index,
                                bytes: bytes.clone(),
                            })
                            .map_err(|_| "filmstrip writer stopped".to_string())?;
                    }
                }
                Ok(())
            })
            .map_err(|error| format!("filmstrip encoder start: {error}"))?;

        let writer = std::thread::Builder::new()
            .name("qnc-filmstrip-write".into())
            .spawn(move || {
                while let Ok(job) = write_recv.recv() {
                    let path = output_paths
                        .get(job.index)
                        .ok_or_else(|| "filmstrip frame output index missing".to_string())?;
                    write_jpeg_file(&job.bytes, path)?;
                }
                Ok(())
            })
            .map_err(|error| format!("filmstrip writer start: {error}"))?;

        Ok(Self {
            encode_send: Some(encode_send),
            encoder,
            writer,
        })
    }

    fn submit(
        &mut self,
        packet: qnc_media_decode::DecodedPacket,
        indices: Vec<usize>,
    ) -> Result<(), String> {
        self.encode_send
            .as_ref()
            .ok_or_else(|| "filmstrip frame pipeline already closed".to_string())?
            .send(FrameEncodeJob { packet, indices })
            .map_err(|_| "filmstrip encoder stopped".to_string())
    }

    fn finish(mut self) -> Result<(), String> {
        drop(self.encode_send.take());
        let encode_result = self
            .encoder
            .join()
            .map_err(|_| "filmstrip encoder panicked".to_string())?;
        let write_result = self
            .writer
            .join()
            .map_err(|_| "filmstrip writer panicked".to_string())?;
        encode_result?;
        write_result
    }
}

fn encode_padded_jpeg_bytes(size: [u32; 2], rgba: &[u8]) -> Result<Vec<u8>, String> {
    let source = image::RgbaImage::from_vec(size[0], size[1], rgba.to_vec())
        .ok_or_else(|| "filmstrip RGBA size mismatch".to_string())?;
    let mut canvas =
        image::RgbaImage::from_pixel(THUMB_WIDTH, THUMB_HEIGHT, image::Rgba([0, 0, 0, 255]));
    let x = i64::from(THUMB_WIDTH.saturating_sub(size[0]) / 2);
    let y = i64::from(THUMB_HEIGHT.saturating_sub(size[1]) / 2);
    image::imageops::overlay(&mut canvas, &source, x, y);
    let rgb = image::DynamicImage::ImageRgba8(canvas).to_rgb8();
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 88);
    encoder
        .encode_image(&rgb)
        .map_err(|error| format!("filmstrip jpg encode: {error}"))?;
    Ok(bytes)
}

fn write_jpeg_file(bytes: &[u8], path: &Path) -> Result<(), String> {
    let mut file =
        fs::File::create(path).map_err(|error| format!("filmstrip jpg create: {error}"))?;
    file.write_all(bytes)
        .map_err(|error| format!("filmstrip jpg write: {error}"))?;
    file.flush()
        .map_err(|error| format!("filmstrip jpg flush: {error}"))
}

fn open_media_stream(
    source: &FilmstripSourceBinding,
    media_uri: &str,
) -> Result<MediaStream, String> {
    source.open_media_stream(media_uri)
}

fn store_artifact_from_plan(plan: &FilmstripPlan) -> FilmstripArtifactRecord {
    FilmstripArtifactRecord {
        clip_id: plan.clip_id.clone(),
        status: "ready".into(),
        duration_sec: qnc_filmstrip::format_seconds(plan.duration_sec),
        frame_count: plan.frames.len(),
        artifact_uri: plan.artifact_root_uri.clone(),
        frames: plan
            .frames
            .iter()
            .map(|frame| FilmstripFrameRecord {
                index: frame.index,
                seek_sec: qnc_filmstrip::format_seconds(frame.seek_sec),
                artifact_uri: frame.artifact_uri.clone(),
            })
            .collect(),
    }
}

fn move_or_copy(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("filmstrip output dir: {error}"))?;
    }
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(from, to).map_err(|error| format!("filmstrip frame copy: {error}"))?;
            Ok(())
        }
    }
}

fn filmstrip_diagnostics_enabled() -> bool {
    qnc_dev_diagnostics::filmstrip_diagnostics_enabled()
}

fn filmstrip_log(message: impl FnOnce() -> String) {
    if filmstrip_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Filmstrip, message());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_is_project_and_clip_scoped() {
        assert_eq!(filmstrip_cache_key("p1", "c1"), "p1::c1");
        assert_ne!(
            filmstrip_cache_key("p1", "c1"),
            filmstrip_cache_key("p2", "c1")
        );
    }

    #[test]
    fn service_does_not_start_without_project_context() {
        let mut service = TimelineFilmstripService::default();
        service.sync_content_db().unwrap();

        assert!(!service.has_pending_work());
        assert!(!service.has_pending_work());
    }

    #[test]
    fn timestamp_conversion_rejects_off_grid_frames() {
        let tb = Rational {
            numerator: 1,
            denominator: 50_000,
        };
        let origin = (0, tb);
        assert_eq!(relative_position(1000, tb, origin, 50, 1).unwrap(), 1);
        assert!(relative_position(1001, tb, origin, 50, 1).is_err());
    }

    #[test]
    fn scratch_dir_is_not_inside_public_filmstrip_artifact_dir() {
        let public_dir = Path::new("project").join("filmstrip").join("clip-1");
        let scratch = filmstrip_scratch_dir("clip-1");

        assert!(!scratch.starts_with(&public_dir));
        assert!(scratch.to_string_lossy().contains("qnc-filmstrip-worker"));
    }

    #[test]
    fn filmstrip_diagnostics_query_is_shared_module_backed() {
        let _ = filmstrip_diagnostics_enabled();
    }

    #[test]
    fn playback_priority_holds_filmstrip_queue() {
        let mut service = TimelineFilmstripService::default();
        service.set_playback_priority(true);
        service.enqueue(
            FilmstripRequest {
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
    fn playback_priority_cancels_active_filmstrip_workers() {
        let mut service = TimelineFilmstripService::default();
        let (_send, receive) = mpsc::sync_channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        service.workers.push(ActiveFilmstripWorker {
            request: FilmstripRequest {
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
