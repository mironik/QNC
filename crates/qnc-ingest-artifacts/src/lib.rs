//! Ingest adapters for the timeline artifacts: they read and write the filmstrip
//! and wave content through the Ingest content transport (local, LAN or intranet) and
//! give the neutral component everything it needs. It names no Ingest type.

use qnc_ingest_select::selection_config::SelectionConfig;
use qnc_ingest_store::content::ContentTarget;
use qnc_ingest_work_plan::IngestWorkPlan;
use qnc_timeline_artifacts::ArtifactsContext;
use qnc_work_settings::SettingsReader;
use std::sync::Arc;

pub const MODULE_ID: &str = "qnc.module.ingest-artifacts";
pub const VERSION: &str = "0.1.0";

/// One read connection shared by every read of a reader. Opening a connection for each
/// clip made a sync of a hundred clips take seconds on the caller's thread.
#[derive(Clone)]
struct SharedRead {
    target: ContentTarget,
    client: Arc<std::sync::Mutex<Option<qnc_ingest_store::content::ContentClient>>>,
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
            .map_err(|_| "Veza prema bazi nije dostupna.".to_string())?;
        if guard.is_none() {
            *guard = Some(
                self.target
                    .open(qnc_ingest_store::content::Access::ReadOnly)?,
            );
        }
        Ok(ReadGuard(guard))
    }
}

struct ReadGuard<'a>(std::sync::MutexGuard<'a, Option<qnc_ingest_store::content::ContentClient>>);

impl std::ops::Deref for ReadGuard<'_> {
    type Target = qnc_ingest_store::content::ContentClient;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("opened by SharedRead::open")
    }
}

impl std::ops::DerefMut for ReadGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().expect("opened by SharedRead::open")
    }
}

#[derive(Clone)]
struct IngestTimelineArtifactReader {
    read: SharedRead,
    filmstrip_root_uri: String,
    filmstrip_dir: std::path::PathBuf,
}

impl qnc_timeline_assets::TimelineArtifactRead for IngestTimelineArtifactReader {
    fn read_filmstrip(
        &self,
        clip_id: &str,
    ) -> Result<Option<qnc_filmstrip::FilmstripArtifactRecord>, String> {
        Ok(self.read.open()?
            .read_filmstrip(clip_id)
            .map_err(|error| error.to_string())?
            .map(to_filmstrip_record))
    }

    fn read_wave(&self, clip_id: &str) -> Result<Option<qnc_wave::WaveArtifactRecord>, String> {
        self.read.open()?
            .read_wave(clip_id)
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
struct IngestFilmstripContentReader {
    read: SharedRead,
}

impl qnc_filmstrip_worker::FilmstripContentRead for IngestFilmstripContentReader {
    fn list_clips(
        &self,
        after: Option<String>,
    ) -> Result<Vec<qnc_filmstrip_worker::FilmstripClipRecord>, String> {
        self.read.open()?
            .list(after)?
            .into_iter()
            .map(|stored| {
                Ok(qnc_filmstrip_worker::FilmstripClipRecord {
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
        Ok(self.read.open()?
            .read(clip_id)?
            .map(|stored| qnc_filmstrip_worker::FilmstripClipRecord {
                clip_id: stored.clip.id().to_string(),
                name: stored.clip.name,
                snapshot: stored.clip.snapshot,
            }))
    }

    fn read_filmstrip(
        &self,
        clip_id: &str,
    ) -> Result<Option<qnc_filmstrip::FilmstripArtifactRecord>, String> {
        Ok(self.read.open()?
            .read_filmstrip(clip_id)?
            .map(to_filmstrip_record))
    }
}

#[derive(Clone)]
struct IngestFilmstripContentWriteFactory {
    content_target: qnc_ingest_store::content::ContentTarget,
}

impl qnc_filmstrip_worker::FilmstripContentWriteFactory for IngestFilmstripContentWriteFactory {
    fn start(&self) -> Result<Box<dyn qnc_filmstrip_worker::FilmstripContentWrite>, String> {
        Ok(Box::new(IngestFilmstripContentWriter {
            transport: qnc_ingest_store::content::ContentWriteTransport::start(
                self.content_target.clone(),
            )?,
        }))
    }
}

struct IngestFilmstripContentWriter {
    transport: qnc_ingest_store::content::ContentWriteTransport,
}

impl qnc_filmstrip_worker::FilmstripContentWrite for IngestFilmstripContentWriter {
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
struct IngestWaveContentReader {
    read: SharedRead,
}

impl qnc_wave_worker::WaveContentRead for IngestWaveContentReader {
    fn list_clips(
        &self,
        after: Option<String>,
    ) -> Result<Vec<qnc_wave_worker::WaveClipRecord>, String> {
        self.read.open()?
            .list(after)?
            .into_iter()
            .map(|stored| {
                Ok(qnc_wave_worker::WaveClipRecord {
                    clip_id: stored.clip.id().to_string(),
                    name: stored.clip.name,
                    snapshot: stored.clip.snapshot,
                })
            })
            .collect()
    }

    fn read_clip(&self, clip_id: &str) -> Result<Option<qnc_wave_worker::WaveClipRecord>, String> {
        Ok(self.read.open()?
            .read(clip_id)?
            .map(|stored| qnc_wave_worker::WaveClipRecord {
                clip_id: stored.clip.id().to_string(),
                name: stored.clip.name,
                snapshot: stored.clip.snapshot,
            }))
    }

    fn read_wave(&self, clip_id: &str) -> Result<Option<qnc_wave::WaveArtifactRecord>, String> {
        self.read.open()?
            .read_wave(clip_id)
    }
}

#[derive(Clone)]
struct IngestWaveContentWriteFactory {
    content_target: qnc_ingest_store::content::ContentTarget,
}

impl qnc_wave_worker::WaveContentWriteFactory for IngestWaveContentWriteFactory {
    fn start(&self) -> Result<Box<dyn qnc_wave_worker::WaveContentWrite>, String> {
        Ok(Box::new(IngestWaveContentWriter {
            transport: qnc_ingest_store::content::ContentWriteTransport::start(
                self.content_target.clone(),
            )?,
        }))
    }
}

struct IngestWaveContentWriter {
    transport: qnc_ingest_store::content::ContentWriteTransport,
}

impl qnc_wave_worker::WaveContentWrite for IngestWaveContentWriter {
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
    record: qnc_ingest_store::content::FilmstripArtifactRecord,
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
) -> qnc_ingest_store::content::FilmstripArtifactRecord {
    qnc_ingest_store::content::FilmstripArtifactRecord {
        clip_id: record.clip_id,
        status: record.status,
        duration_sec: record.duration_sec,
        frame_count: record.frame_count,
        artifact_uri: record.artifact_uri,
        frames: record
            .frames
            .into_iter()
            .map(|frame| qnc_ingest_store::content::FilmstripFrameRecord {
                index: frame.index,
                seek_sec: frame.seek_sec,
                artifact_uri: frame.artifact_uri,
            })
            .collect(),
    }
}

fn filmstrip_source_bindings(
    config: &SelectionConfig,
) -> Result<Vec<qnc_filmstrip_worker::FilmstripSourceBinding>, String> {
    config
        .sources
        .iter()
        .map(|source| {
            Ok(qnc_filmstrip_worker::FilmstripSourceBinding {
                source_uri: source.location.uri.clone(),
                local_root: source.location.file.clone(),
                endpoint: source.location.endpoint.clone(),
                token: source.location.token().map_err(|error| error.to_string())?,
                serial_number: (!source.serial_number.trim().is_empty())
                    .then(|| source.serial_number.clone()),
            })
        })
        .collect()
}

fn wave_source_bindings(
    config: &SelectionConfig,
) -> Result<Vec<qnc_wave_worker::WaveSourceBinding>, String> {
    config
        .sources
        .iter()
        .map(|source| {
            Ok(qnc_wave_worker::WaveSourceBinding {
                source_uri: source.location.uri.clone(),
                local_root: source.location.file.clone(),
                endpoint: source.location.endpoint.clone(),
                token: source.location.token().map_err(|error| error.to_string())?,
                serial_number: (!source.serial_number.trim().is_empty())
                    .then(|| source.serial_number.clone()),
            })
        })
        .collect()
}


/// The context of one project: readers and writers over the Ingest content
/// transport, the source bindings of the Select configuration and the project folder
/// of this machine. Without local access to the project folder there is no context.
pub fn artifacts_context(
    reader: &SettingsReader,
    plan: &IngestWorkPlan,
    content_target: ContentTarget,
    config: &SelectionConfig,
) -> Result<ArtifactsContext, String> {
    let project_dir = reader
        .local_workspace_dir(&plan.settings)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Artefakti timelinea nemaju lokalni binding projekta.".to_string())?;
    let filmstrip_dir = project_dir.join("filmstrip");
    let project_audio_channels = plan.settings.audio_channels().map_err(|e| e.to_string())?;
    Ok(ArtifactsContext {
        project_id: plan.settings.project_id.clone(),
        filmstrip_root_uri: plan.filmstrip_uri.clone(),
        filmstrip_dir: filmstrip_dir.clone(),
        wave_root_uri: format!("{}/wave", content_target.uri().trim_end_matches('/')),
        project_audio_channels,
        timeline_reader: Arc::new(IngestTimelineArtifactReader {
            read: SharedRead::new(content_target.clone()),
            filmstrip_root_uri: plan.filmstrip_uri.clone(),
            filmstrip_dir,
        }),
        filmstrip_reader: Arc::new(IngestFilmstripContentReader {
            read: SharedRead::new(content_target.clone()),
        }),
        filmstrip_writer: Arc::new(IngestFilmstripContentWriteFactory {
            content_target: content_target.clone(),
        }),
        filmstrip_sources: filmstrip_source_bindings(config)?,
        wave_reader: Arc::new(IngestWaveContentReader {
            read: SharedRead::new(content_target.clone()),
        }),
        wave_writer: Arc::new(IngestWaveContentWriteFactory { content_target }),
        wave_sources: wave_source_bindings(config)?,
    })
}
