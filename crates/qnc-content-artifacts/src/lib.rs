//! Project content adapters for timeline artifacts.
//!
//! Filmstrip and wave workers are neutral. This module feeds them the clips of the
//! active project (its content) and reads and writes their results through the
//! artifact tables of the project database (`qnc-artifact-db`, through the one
//! intermediary). It does not scan, probe, read UI state or know an application
//! workflow.

use qnc_artifact_db::{ArtifactPending, ArtifactReader, ArtifactWriter, Operation as ArtifactOperation};
use qnc_content_store::{ContentTarget, ImportStatus, StoredClip};
use qnc_db_broker::ProjectDbTarget;
use qnc_source_bindings::SourceBinding;
use qnc_timeline_artifacts::{Artifacts, ArtifactsContext};
use qnc_timeline_assets::{
    SourceTimelineAssets, TimelineArtifactRead, TimelineAssetContext, TimelineAssetReader,
};
use qnc_work_settings::{ArtifactKind, ArtifactMode, ProductArea, SettingsReader, WorkSettings};
use std::{path::PathBuf, sync::Arc};

pub const MODULE_ID: &str = "qnc.module.content-artifacts";
pub const VERSION: &str = "0.1.0";

#[derive(Clone)]
struct SharedRead {
    target: ContentTarget,
    client: Arc<std::sync::Mutex<Option<qnc_content_store::ContentClient>>>,
}

impl SharedRead {
    fn new(target: ContentTarget) -> Self {
        Self {
            target,
            client: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    fn open(&self) -> Result<ReadGuard<'_>, String> {
        let mut guard = self
            .client
            .lock()
            .map_err(|_| "Veza prema projektnoj bazi nije dostupna.".to_string())?;
        if guard.is_none() {
            *guard = Some(self.target.open(qnc_content_store::Access::ReadOnly)?);
        }
        Ok(ReadGuard(guard))
    }
}

struct ReadGuard<'a>(std::sync::MutexGuard<'a, Option<qnc_content_store::ContentClient>>);

impl std::ops::Deref for ReadGuard<'_> {
    type Target = qnc_content_store::ContentClient;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("opened by SharedRead::open")
    }
}

impl std::ops::DerefMut for ReadGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().expect("opened by SharedRead::open")
    }
}

/// One read-only client of the artifact tables, opened on first use and shared.
#[derive(Clone)]
struct SharedArtifacts {
    target: ProjectDbTarget,
    client: Arc<std::sync::Mutex<Option<ArtifactReader>>>,
}

impl SharedArtifacts {
    fn new(target: ProjectDbTarget) -> Self {
        Self {
            target,
            client: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    fn read<T>(&self, read: impl FnOnce(&mut ArtifactReader) -> Result<T, String>) -> Result<T, String> {
        let mut guard = self
            .client
            .lock()
            .map_err(|_| "Veza prema projektnoj bazi nije dostupna.".to_string())?;
        if guard.is_none() {
            *guard = Some(ArtifactReader::open(&self.target)?);
        }
        read(guard.as_mut().expect("opened above"))
    }
}

/// Publications of one worker through the artifact writer, answered later.
struct ArtifactPublisher {
    writer: ArtifactWriter,
    pending: Vec<(String, ArtifactPending)>,
}

impl ArtifactPublisher {
    fn start(target: &ProjectDbTarget) -> Result<Self, String> {
        Ok(Self {
            writer: ArtifactWriter::start(target.clone())?,
            pending: Vec::new(),
        })
    }

    fn send(&mut self, key: String, operation: ArtifactOperation) -> Result<(), String> {
        let pending = self.writer.submit(&operation)?;
        self.pending.push((key, pending));
        Ok(())
    }

    fn poll(&mut self) -> Vec<(String, Result<(), String>)> {
        let mut done = Vec::new();
        let mut waiting = Vec::new();
        for (key, pending) in std::mem::take(&mut self.pending) {
            match pending.try_take() {
                Some(result) => done.push((key, result.map(|_| ()))),
                None => waiting.push((key, pending)),
            }
        }
        self.pending = waiting;
        done
    }
}

fn artifact_priority(stored: &StoredClip) -> bool {
    stored.selected
        || matches!(
            stored.import_status,
            ImportStatus::Queued | ImportStatus::Processing | ImportStatus::Imported
        )
}

#[derive(Clone)]
struct ProjectTimelineArtifactReader {
    artifacts: SharedArtifacts,
    filmstrip_root_uri: String,
    filmstrip_dir: std::path::PathBuf,
}

impl TimelineArtifactRead for ProjectTimelineArtifactReader {
    fn read_filmstrip(
        &self,
        clip_id: &str,
    ) -> Result<Option<qnc_filmstrip::FilmstripArtifactRecord>, String> {
        self.artifacts.read(|reader| reader.read_filmstrip(clip_id))
    }

    fn read_wave(&self, clip_id: &str) -> Result<Option<qnc_wave::WaveArtifactRecord>, String> {
        self.artifacts.read(|reader| reader.read_wave(clip_id))
    }

    fn read_image_bytes(&self, artifact_uri: &str) -> Result<Vec<u8>, String> {
        let artifacts = qnc_filmstrip::LocalFilmstripArtifacts::new(
            &self.filmstrip_root_uri,
            &self.filmstrip_dir,
        )?;
        let path = artifacts.frame_path(artifact_uri)?;
        std::fs::read(&path).map_err(|error| format!("filmstrip frame read failed: {error}"))
    }
}

#[derive(Clone)]
struct ProjectFilmstripContentReader {
    read: SharedRead,
    artifacts: SharedArtifacts,
    /// `artifacts.filmstrip` of the project: `off` gives the generator no clip.
    enabled: bool,
}

impl qnc_filmstrip_worker::FilmstripContentRead for ProjectFilmstripContentReader {
    fn list_clips(
        &self,
        after: Option<String>,
    ) -> Result<Vec<qnc_filmstrip_worker::FilmstripClipRecord>, String> {
        if !self.enabled {
            return Ok(Vec::new());
        }
        self.read
            .open()?
            .list(after)?
            .into_iter()
            .map(|stored| {
                Ok(qnc_filmstrip_worker::FilmstripClipRecord {
                    priority: artifact_priority(&stored),
                    clip_id: stored.clip.id().to_string(),
                    name: stored.clip.name,
                    snapshot: stored.clip.snapshot,
                })
            })
            .collect()
    }

    fn read_clip(
        &self,
        clip_id: &str,
    ) -> Result<Option<qnc_filmstrip_worker::FilmstripClipRecord>, String> {
        if !self.enabled {
            return Ok(None);
        }
        Ok(self.read.open()?.read(clip_id)?.map(|stored| {
            qnc_filmstrip_worker::FilmstripClipRecord {
                priority: artifact_priority(&stored),
                clip_id: stored.clip.id().to_string(),
                name: stored.clip.name,
                snapshot: stored.clip.snapshot,
            }
        }))
    }

    fn read_filmstrip(
        &self,
        clip_id: &str,
    ) -> Result<Option<qnc_filmstrip::FilmstripArtifactRecord>, String> {
        self.artifacts.read(|reader| reader.read_filmstrip(clip_id))
    }
}

#[derive(Clone)]
struct ProjectFilmstripContentWriteFactory {
    target: ProjectDbTarget,
}

impl qnc_filmstrip_worker::FilmstripContentWriteFactory for ProjectFilmstripContentWriteFactory {
    fn start(&self) -> Result<Box<dyn qnc_filmstrip_worker::FilmstripContentWrite>, String> {
        Ok(Box::new(ProjectFilmstripContentWriter(ArtifactPublisher::start(&self.target)?)))
    }
}

struct ProjectFilmstripContentWriter(ArtifactPublisher);

impl qnc_filmstrip_worker::FilmstripContentWrite for ProjectFilmstripContentWriter {
    fn publish_filmstrip(
        &mut self,
        key: String,
        artifact: qnc_filmstrip::FilmstripArtifactRecord,
    ) -> Result<(), String> {
        self.0
            .send(key, ArtifactOperation::PublishFilmstrip(Box::new(artifact)))
    }

    fn poll(&mut self) -> Vec<qnc_filmstrip_worker::FilmstripWriteCompletion> {
        self.0
            .poll()
            .into_iter()
            .map(|(key, result)| qnc_filmstrip_worker::FilmstripWriteCompletion { key, result })
            .collect()
    }

    fn has_pending(&self) -> bool {
        !self.0.pending.is_empty()
    }
}

#[derive(Clone)]
struct ProjectWaveContentReader {
    read: SharedRead,
    artifacts: SharedArtifacts,
    /// `artifacts.wave` of the project: `off` gives the generator no clip.
    enabled: bool,
}

impl qnc_wave_worker::WaveContentRead for ProjectWaveContentReader {
    fn list_clips(
        &self,
        after: Option<String>,
    ) -> Result<Vec<qnc_wave_worker::WaveClipRecord>, String> {
        if !self.enabled {
            return Ok(Vec::new());
        }
        self.read
            .open()?
            .list(after)?
            .into_iter()
            .map(|stored| {
                Ok(qnc_wave_worker::WaveClipRecord {
                    priority: artifact_priority(&stored),
                    clip_id: stored.clip.id().to_string(),
                    name: stored.clip.name,
                    snapshot: stored.clip.snapshot,
                })
            })
            .collect()
    }

    fn read_clip(&self, clip_id: &str) -> Result<Option<qnc_wave_worker::WaveClipRecord>, String> {
        if !self.enabled {
            return Ok(None);
        }
        Ok(self
            .read
            .open()?
            .read(clip_id)?
            .map(|stored| qnc_wave_worker::WaveClipRecord {
                priority: artifact_priority(&stored),
                clip_id: stored.clip.id().to_string(),
                name: stored.clip.name,
                snapshot: stored.clip.snapshot,
            }))
    }

    fn read_wave(&self, clip_id: &str) -> Result<Option<qnc_wave::WaveArtifactRecord>, String> {
        self.artifacts.read(|reader| reader.read_wave(clip_id))
    }
}

#[derive(Clone)]
struct ProjectWaveContentWriteFactory {
    target: ProjectDbTarget,
}

impl qnc_wave_worker::WaveContentWriteFactory for ProjectWaveContentWriteFactory {
    fn start(&self) -> Result<Box<dyn qnc_wave_worker::WaveContentWrite>, String> {
        Ok(Box::new(ProjectWaveContentWriter(ArtifactPublisher::start(&self.target)?)))
    }
}

struct ProjectWaveContentWriter(ArtifactPublisher);

impl qnc_wave_worker::WaveContentWrite for ProjectWaveContentWriter {
    fn publish_wave(
        &mut self,
        key: String,
        artifact: qnc_wave::WaveArtifactRecord,
    ) -> Result<(), String> {
        self.0
            .send(key, ArtifactOperation::PublishWave(Box::new(artifact)))
    }

    fn poll(&mut self) -> Vec<qnc_wave_worker::WaveWriteCompletion> {
        self.0
            .poll()
            .into_iter()
            .map(|(key, result)| qnc_wave_worker::WaveWriteCompletion { key, result })
            .collect()
    }

    fn has_pending(&self) -> bool {
        !self.0.pending.is_empty()
    }
}

fn filmstrip_source_bindings(
    sources: &[SourceBinding],
) -> Result<Vec<qnc_filmstrip_worker::FilmstripSourceBinding>, String> {
    sources
        .iter()
        .map(|source| {
            Ok(qnc_filmstrip_worker::FilmstripSourceBinding {
                source_uri: source.uri.clone(),
                local_root: source.file.clone(),
                endpoint: source.endpoint.clone(),
                token: source.token()?,
                serial_number: None,
            })
        })
        .collect()
}

fn wave_source_bindings(
    sources: &[SourceBinding],
) -> Result<Vec<qnc_wave_worker::WaveSourceBinding>, String> {
    sources
        .iter()
        .map(|source| {
            Ok(qnc_wave_worker::WaveSourceBinding {
                source_uri: source.uri.clone(),
                local_root: source.file.clone(),
                endpoint: source.endpoint.clone(),
                token: source.token()?,
                serial_number: None,
            })
        })
        .collect()
}

pub fn timeline_artifact_reader(
    reader: &SettingsReader,
    settings: &WorkSettings,
) -> Result<Arc<dyn TimelineArtifactRead>, String> {
    let project_dir = reader
        .local_workspace_dir(settings)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Artefakti timelinea nemaju lokalni binding projekta.".to_string())?;
    Ok(Arc::new(ProjectTimelineArtifactReader {
        artifacts: SharedArtifacts::new(ProjectDbTarget::for_project(reader, settings)?),
        filmstrip_root_uri: settings.product_uri(ProductArea::Filmstrip),
        filmstrip_dir: settings.product_local_dir(&project_dir, ProductArea::Filmstrip),
    }))
}

pub fn artifacts_context(
    reader: &SettingsReader,
    settings: &WorkSettings,
    content_target: ContentTarget,
    source_bindings: &[SourceBinding],
) -> Result<ArtifactsContext, String> {
    let project_dir = reader
        .local_workspace_dir(settings)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Artefakti timelinea nemaju lokalni binding projekta.".to_string())?;
    let filmstrip_dir = settings.product_local_dir(&project_dir, ProductArea::Filmstrip);
    let project_audio_channels = settings.audio_channels().map_err(|e| e.to_string())?;
    let filmstrip_root_uri = settings.product_uri(ProductArea::Filmstrip);
    let project_db = ProjectDbTarget::for_project(reader, settings)?;
    let artifacts = SharedArtifacts::new(project_db.clone());
    let made = |kind| -> Result<bool, String> {
        Ok(settings.artifact_mode(kind).map_err(|e| e.message)? == ArtifactMode::Auto)
    };
    let (filmstrip_enabled, wave_enabled) = (made(ArtifactKind::Filmstrip)?, made(ArtifactKind::Wave)?);
    Ok(ArtifactsContext {
        project_id: settings.project_id.clone(),
        filmstrip_root_uri: filmstrip_root_uri.clone(),
        filmstrip_dir: filmstrip_dir.clone(),
        wave_root_uri: format!("{}/wave", content_target.uri().trim_end_matches('/')),
        project_audio_channels,
        timeline_reader: Arc::new(ProjectTimelineArtifactReader {
            artifacts: artifacts.clone(),
            filmstrip_root_uri,
            filmstrip_dir,
        }),
        filmstrip_reader: Arc::new(ProjectFilmstripContentReader {
            read: SharedRead::new(content_target.clone()),
            artifacts: artifacts.clone(),
            enabled: filmstrip_enabled,
        }),
        filmstrip_writer: Arc::new(ProjectFilmstripContentWriteFactory {
            target: project_db.clone(),
        }),
        filmstrip_sources: filmstrip_source_bindings(source_bindings)?,
        wave_reader: Arc::new(ProjectWaveContentReader {
            read: SharedRead::new(content_target),
            artifacts,
            enabled: wave_enabled,
        }),
        wave_writer: Arc::new(ProjectWaveContentWriteFactory { target: project_db }),
        wave_sources: wave_source_bindings(source_bindings)?,
    })
}

#[derive(Debug, Default)]
pub struct ProjectArtifacts {
    artifacts: Artifacts,
    assets: TimelineAssetReader,
    host_root: Option<PathBuf>,
    /// When the clip on screen may be read again while its artifacts are made elsewhere.
    refresh_at: Option<std::time::Instant>,
}

#[derive(Debug, Default)]
pub struct ProjectArtifactsPoll {
    pub changed: bool,
    pub error: Option<String>,
    pub assets: Option<SourceTimelineAssets>,
}

impl ProjectArtifacts {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.artifacts.reset();
        self.assets.reset();
    }

    pub fn set_host_root(&mut self, root: impl Into<PathBuf>) {
        self.host_root = Some(root.into());
    }

    fn source_bindings(&self) -> Result<Vec<SourceBinding>, String> {
        let root = self
            .host_root
            .as_deref()
            .ok_or_else(|| "Nedostaje host konfiguracija izvora medija.".to_string())?;
        Ok(qnc_source_bindings::load(root)?.sources)
    }

    fn configure_assets(
        &mut self,
        reader: &SettingsReader,
        settings: &WorkSettings,
    ) -> Result<(), String> {
        self.assets.configure(TimelineAssetContext {
            project_id: settings.project_id.clone(),
            reader: timeline_artifact_reader(reader, settings)?,
        });
        Ok(())
    }

    pub fn configure(
        &mut self,
        reader: &SettingsReader,
        settings: &WorkSettings,
        content_target: ContentTarget,
    ) -> Result<(), String> {
        self.configure_assets(reader, settings)?;
        let source_bindings = self.source_bindings()?;
        if source_bindings.is_empty() {
            self.artifacts.reset();
            return Ok(());
        }
        self.artifacts.configure(artifacts_context(
            reader,
            settings,
            content_target,
            &source_bindings,
        )?);
        Ok(())
    }

    pub fn sync(
        &mut self,
        reader: &SettingsReader,
        settings: &WorkSettings,
        content_target: ContentTarget,
        defer: bool,
    ) -> Result<(), String> {
        self.configure(reader, settings, content_target)?;
        self.artifacts.sync(defer)
    }

    pub fn sync_deferred(&self) -> bool {
        self.artifacts.sync_deferred()
    }

    pub fn defer_sync(&mut self) {
        let _ = self.artifacts.sync(true);
    }

    pub fn focus(
        &mut self,
        reader: &SettingsReader,
        settings: &WorkSettings,
        clip_id: &str,
    ) -> Result<SourceTimelineAssets, String> {
        self.configure_assets(reader, settings)?;
        self.assets.load_clip(clip_id)
    }

    /// Whether the clip on screen should be read again: its filmstrip or wave is still
    /// missing (the background worker makes them), at most once a second.
    pub fn refresh_due(&mut self, shown: &SourceTimelineAssets) -> bool {
        if shown.clip_id.is_empty() || (shown.filmstrip_background.is_some() && shown.wave.is_some()) {
            return false;
        }
        let now = std::time::Instant::now();
        if self.refresh_at.is_some_and(|at| now < at) {
            return false;
        }
        self.refresh_at = Some(now + std::time::Duration::from_secs(1));
        true
    }

    pub fn remove_clips(&mut self, clip_ids: &[String]) {
        self.artifacts.remove_clips(clip_ids);
        self.assets.remove_clips(clip_ids);
    }

    pub fn poll(&mut self, active_clip_id: Option<&str>) -> ProjectArtifactsPoll {
        let polled = self.artifacts.poll(active_clip_id);
        let assets = if polled.changed {
            active_clip_id.and_then(|clip_id| self.assets.refresh_clip(clip_id).ok())
        } else {
            None
        };
        ProjectArtifactsPoll {
            changed: polled.changed,
            error: polled.error,
            assets: assets.or(polled.assets),
        }
    }

    pub fn set_playback_priority(&mut self, active: bool) {
        self.artifacts.set_playback_priority(active);
    }

    pub fn has_pending_work(&self) -> bool {
        self.artifacts.has_pending_work()
    }
}
