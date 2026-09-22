//! Project content adapters for timeline artifacts.
//!
//! Filmstrip and wave workers are neutral. This module feeds them from the
//! active project content database and writes results back through the public
//! content write transport. It does not scan, probe, read UI state or know an
//! application workflow.

use qnc_content_store::{ContentTarget, ImportStatus, StoredClip};
use qnc_source_bindings::SourceBinding;
use qnc_timeline_artifacts::{Artifacts, ArtifactsContext};
use qnc_timeline_assets::{
    SourceTimelineAssets, TimelineArtifactRead, TimelineAssetContext, TimelineAssetReader,
};
use qnc_work_settings::{SettingsReader, WorkSettings};
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

fn artifact_priority(stored: &StoredClip) -> bool {
    stored.selected
        || matches!(
            stored.import_status,
            ImportStatus::Queued | ImportStatus::Processing | ImportStatus::Imported
        )
}

#[derive(Clone)]
struct ProjectTimelineArtifactReader {
    read: SharedRead,
    filmstrip_root_uri: String,
    filmstrip_dir: std::path::PathBuf,
}

impl TimelineArtifactRead for ProjectTimelineArtifactReader {
    fn read_filmstrip(
        &self,
        clip_id: &str,
    ) -> Result<Option<qnc_filmstrip::FilmstripArtifactRecord>, String> {
        Ok(self
            .read
            .open()?
            .read_filmstrip(clip_id)
            .map_err(|error| error.to_string())?
            .map(to_filmstrip_record))
    }

    fn read_wave(&self, clip_id: &str) -> Result<Option<qnc_wave::WaveArtifactRecord>, String> {
        self.read.open()?.read_wave(clip_id)
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
}

impl qnc_filmstrip_worker::FilmstripContentRead for ProjectFilmstripContentReader {
    fn list_clips(
        &self,
        after: Option<String>,
    ) -> Result<Vec<qnc_filmstrip_worker::FilmstripClipRecord>, String> {
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
        Ok(self
            .read
            .open()?
            .read_filmstrip(clip_id)?
            .map(to_filmstrip_record))
    }
}

#[derive(Clone)]
struct ProjectFilmstripContentWriteFactory {
    content_target: ContentTarget,
}

impl qnc_filmstrip_worker::FilmstripContentWriteFactory for ProjectFilmstripContentWriteFactory {
    fn start(&self) -> Result<Box<dyn qnc_filmstrip_worker::FilmstripContentWrite>, String> {
        Ok(Box::new(ProjectFilmstripContentWriter {
            transport: qnc_content_store::ContentWriteTransport::start(
                self.content_target.clone(),
            )?,
        }))
    }
}

struct ProjectFilmstripContentWriter {
    transport: qnc_content_store::ContentWriteTransport,
}

impl qnc_filmstrip_worker::FilmstripContentWrite for ProjectFilmstripContentWriter {
    fn publish_filmstrip(
        &mut self,
        key: String,
        artifact: qnc_filmstrip::FilmstripArtifactRecord,
    ) -> Result<(), String> {
        self.transport
            .publish_filmstrip(key, to_store_filmstrip_record(artifact))
    }

    fn poll(&mut self) -> Vec<qnc_filmstrip_worker::FilmstripWriteCompletion> {
        self.transport
            .poll()
            .into_iter()
            .map(
                |completion| qnc_filmstrip_worker::FilmstripWriteCompletion {
                    key: completion.key,
                    result: completion.result.map(|_| ()),
                },
            )
            .collect()
    }

    fn has_pending(&self) -> bool {
        self.transport.has_pending()
    }
}

#[derive(Clone)]
struct ProjectWaveContentReader {
    read: SharedRead,
}

impl qnc_wave_worker::WaveContentRead for ProjectWaveContentReader {
    fn list_clips(
        &self,
        after: Option<String>,
    ) -> Result<Vec<qnc_wave_worker::WaveClipRecord>, String> {
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
        self.read.open()?.read_wave(clip_id)
    }
}

#[derive(Clone)]
struct ProjectWaveContentWriteFactory {
    content_target: ContentTarget,
}

impl qnc_wave_worker::WaveContentWriteFactory for ProjectWaveContentWriteFactory {
    fn start(&self) -> Result<Box<dyn qnc_wave_worker::WaveContentWrite>, String> {
        Ok(Box::new(ProjectWaveContentWriter {
            transport: qnc_content_store::ContentWriteTransport::start(
                self.content_target.clone(),
            )?,
        }))
    }
}

struct ProjectWaveContentWriter {
    transport: qnc_content_store::ContentWriteTransport,
}

impl qnc_wave_worker::WaveContentWrite for ProjectWaveContentWriter {
    fn publish_wave(
        &mut self,
        key: String,
        artifact: qnc_wave::WaveArtifactRecord,
    ) -> Result<(), String> {
        self.transport.publish_wave(key, artifact)
    }

    fn poll(&mut self) -> Vec<qnc_wave_worker::WaveWriteCompletion> {
        self.transport
            .poll()
            .into_iter()
            .map(|completion| qnc_wave_worker::WaveWriteCompletion {
                key: completion.key,
                result: completion.result.map(|_| ()),
            })
            .collect()
    }

    fn has_pending(&self) -> bool {
        self.transport.has_pending()
    }
}

fn to_filmstrip_record(
    record: qnc_content_store::FilmstripArtifactRecord,
) -> qnc_filmstrip::FilmstripArtifactRecord {
    qnc_filmstrip::FilmstripArtifactRecord {
        clip_id: record.clip_id,
        status: record.status,
        duration_sec: record.duration_sec,
        frame_count: record.frame_count,
        artifact_uri: record.artifact_uri,
        frames: record
            .frames
            .into_iter()
            .map(|frame| qnc_filmstrip::FilmstripFrameRecord {
                index: frame.index,
                seek_sec: frame.seek_sec,
                artifact_uri: frame.artifact_uri,
            })
            .collect(),
    }
}

fn to_store_filmstrip_record(
    record: qnc_filmstrip::FilmstripArtifactRecord,
) -> qnc_content_store::FilmstripArtifactRecord {
    qnc_content_store::FilmstripArtifactRecord {
        clip_id: record.clip_id,
        status: record.status,
        duration_sec: record.duration_sec,
        frame_count: record.frame_count,
        artifact_uri: record.artifact_uri,
        frames: record
            .frames
            .into_iter()
            .map(|frame| qnc_content_store::FilmstripFrameRecord {
                index: frame.index,
                seek_sec: frame.seek_sec,
                artifact_uri: frame.artifact_uri,
            })
            .collect(),
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
    content_target: ContentTarget,
) -> Result<Arc<dyn TimelineArtifactRead>, String> {
    let project_dir = reader
        .local_workspace_dir(settings)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Artefakti timelinea nemaju lokalni binding projekta.".to_string())?;
    Ok(Arc::new(ProjectTimelineArtifactReader {
        read: SharedRead::new(content_target),
        filmstrip_root_uri: format!(
            "{}/filmstrip",
            settings.output_root_uri.trim_end_matches('/')
        ),
        filmstrip_dir: project_dir.join("filmstrip"),
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
    let filmstrip_dir = project_dir.join("filmstrip");
    let project_audio_channels = settings.audio_channels().map_err(|e| e.to_string())?;
    let filmstrip_root_uri = format!(
        "{}/filmstrip",
        settings.output_root_uri.trim_end_matches('/')
    );
    Ok(ArtifactsContext {
        project_id: settings.project_id.clone(),
        filmstrip_root_uri: filmstrip_root_uri.clone(),
        filmstrip_dir: filmstrip_dir.clone(),
        wave_root_uri: format!("{}/wave", content_target.uri().trim_end_matches('/')),
        project_audio_channels,
        timeline_reader: Arc::new(ProjectTimelineArtifactReader {
            read: SharedRead::new(content_target.clone()),
            filmstrip_root_uri,
            filmstrip_dir,
        }),
        filmstrip_reader: Arc::new(ProjectFilmstripContentReader {
            read: SharedRead::new(content_target.clone()),
        }),
        filmstrip_writer: Arc::new(ProjectFilmstripContentWriteFactory {
            content_target: content_target.clone(),
        }),
        filmstrip_sources: filmstrip_source_bindings(source_bindings)?,
        wave_reader: Arc::new(ProjectWaveContentReader {
            read: SharedRead::new(content_target.clone()),
        }),
        wave_writer: Arc::new(ProjectWaveContentWriteFactory { content_target }),
        wave_sources: wave_source_bindings(source_bindings)?,
    })
}

#[derive(Debug, Default)]
pub struct ProjectArtifacts {
    artifacts: Artifacts,
    assets: TimelineAssetReader,
    host_root: Option<PathBuf>,
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
        content_target: ContentTarget,
    ) -> Result<(), String> {
        self.assets.configure(TimelineAssetContext {
            project_id: settings.project_id.clone(),
            reader: timeline_artifact_reader(reader, settings, content_target)?,
        });
        Ok(())
    }

    pub fn configure(
        &mut self,
        reader: &SettingsReader,
        settings: &WorkSettings,
        content_target: ContentTarget,
    ) -> Result<(), String> {
        self.configure_assets(reader, settings, content_target.clone())?;
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
        content_target: ContentTarget,
        clip_id: &str,
    ) -> Result<SourceTimelineAssets, String> {
        self.configure_assets(reader, settings, content_target)?;
        self.assets.load_clip(clip_id)
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
