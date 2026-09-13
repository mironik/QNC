use super::*;
use std::sync::Arc;

#[derive(Clone)]
struct IngestTimelineArtifactReader {
    content_target: qnc_ingest_store::content::ContentTarget,
    filmstrip_root_uri: String,
    filmstrip_dir: std::path::PathBuf,
}

impl qnc_timeline_assets::TimelineArtifactRead for IngestTimelineArtifactReader {
    fn read_filmstrip(
        &self,
        clip_id: &str,
    ) -> Result<Option<qnc_filmstrip::FilmstripArtifactRecord>, String> {
        Ok(self
            .content_target
            .open(qnc_ingest_store::content::Access::ReadOnly)?
            .read_filmstrip(clip_id)
            .map_err(|error| error.to_string())?
            .map(to_filmstrip_record))
    }

    fn read_wave(&self, clip_id: &str) -> Result<Option<qnc_wave::WaveArtifactRecord>, String> {
        self.content_target
            .open(qnc_ingest_store::content::Access::ReadOnly)?
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
    content_target: qnc_ingest_store::content::ContentTarget,
}

impl qnc_filmstrip_worker::FilmstripContentRead for IngestFilmstripContentReader {
    fn list_clips(
        &self,
        after: Option<String>,
    ) -> Result<Vec<qnc_filmstrip_worker::FilmstripClipRecord>, String> {
        self.content_target
            .open(qnc_ingest_store::content::Access::ReadOnly)?
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
        Ok(self
            .content_target
            .open(qnc_ingest_store::content::Access::ReadOnly)?
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
        Ok(self
            .content_target
            .open(qnc_ingest_store::content::Access::ReadOnly)?
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
    content_target: qnc_ingest_store::content::ContentTarget,
}

impl qnc_wave_worker::WaveContentRead for IngestWaveContentReader {
    fn list_clips(
        &self,
        after: Option<String>,
    ) -> Result<Vec<qnc_wave_worker::WaveClipRecord>, String> {
        self.content_target
            .open(qnc_ingest_store::content::Access::ReadOnly)?
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
        Ok(self
            .content_target
            .open(qnc_ingest_store::content::Access::ReadOnly)?
            .read(clip_id)?
            .map(|stored| qnc_wave_worker::WaveClipRecord {
                clip_id: stored.clip.id().to_string(),
                name: stored.clip.name,
                snapshot: stored.clip.snapshot,
            }))
    }

    fn read_wave(&self, clip_id: &str) -> Result<Option<qnc_wave::WaveArtifactRecord>, String> {
        self.content_target
            .open(qnc_ingest_store::content::Access::ReadOnly)?
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
    config: &selection_config::SelectionConfig,
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
    config: &selection_config::SelectionConfig,
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

impl IngestApplication {
    pub(super) fn reset_timeline_artifacts(&mut self) {
        self.filmstrip.reset();
        self.wave.reset();
        self.timeline_assets.reset();
        self.timeline_artifact_sync_deferred = false;
        self.view.timeline_assets = qnc_timeline_assets::SourceTimelineAssets::empty();
    }

    pub(super) fn refresh_timeline_artifact_context(&mut self) {
        let (Some(settings_reader), Some(work_plan), Some(content_target), Some(selection_config)) = (
            self.settings_reader.clone(),
            self.work_plan().cloned(),
            self.catalog_target.clone(),
            self.selection_config.clone(),
        ) else {
            return;
        };

        match settings_reader.local_workspace_dir(&work_plan.settings) {
            Ok(Some(project_dir)) => {
                self.timeline_assets
                    .configure(qnc_timeline_assets::TimelineAssetContext {
                        project_id: work_plan.settings.project_id.clone(),
                        reader: Arc::new(IngestTimelineArtifactReader {
                            content_target: content_target.clone(),
                            filmstrip_root_uri: work_plan.filmstrip_uri.clone(),
                            filmstrip_dir: project_dir.join("filmstrip"),
                        }),
                    });
            }
            Ok(None) => {
                self.view.message =
                    "Timeline artifact reader nema lokalni filmstrip binding.".into();
            }
            Err(error) => {
                self.view.message = error.to_string();
            }
        }
        let filmstrip_sources = match filmstrip_source_bindings(&selection_config) {
            Ok(sources) => sources,
            Err(error) => {
                self.view.message = error;
                return;
            }
        };
        self.filmstrip
            .configure(qnc_filmstrip_worker::FilmstripContext {
                project_id: work_plan.settings.project_id.clone(),
                filmstrip_root_uri: work_plan.filmstrip_uri.clone(),
                filmstrip_dir: match settings_reader.local_workspace_dir(&work_plan.settings) {
                    Ok(Some(project_dir)) => project_dir.join("filmstrip"),
                    Ok(None) => {
                        self.view.message =
                            "Filmstrip worker nema lokalni artifact binding.".into();
                        return;
                    }
                    Err(error) => {
                        self.view.message = error.to_string();
                        return;
                    }
                },
                content_reader: Arc::new(IngestFilmstripContentReader {
                    content_target: content_target.clone(),
                }),
                content_writer: Arc::new(IngestFilmstripContentWriteFactory {
                    content_target: content_target.clone(),
                }),
                source_bindings: filmstrip_sources,
            });
        let wave_sources = match wave_source_bindings(&selection_config) {
            Ok(sources) => sources,
            Err(error) => {
                self.view.message = error;
                return;
            }
        };
        let project_audio_channels = match work_plan.settings.audio_channels() {
            Ok(channels) => channels,
            Err(error) => {
                self.view.message = error.to_string();
                return;
            }
        };
        self.wave.configure(qnc_wave_worker::WaveContext {
            project_id: work_plan.settings.project_id.clone(),
            wave_root_uri: format!("{}/wave", content_target.uri().trim_end_matches('/')),
            project_audio_channels,
            content_reader: Arc::new(IngestWaveContentReader {
                content_target: content_target.clone(),
            }),
            content_writer: Arc::new(IngestWaveContentWriteFactory { content_target }),
            source_bindings: wave_sources,
        });
    }

    pub(super) fn sync_timeline_artifact_content_db(&mut self) {
        self.refresh_timeline_artifact_context();
        if self.playback_guard_active() {
            self.timeline_artifact_sync_deferred = true;
            self.set_timeline_artifact_playback_priority(true);
            return;
        }
        self.timeline_artifact_sync_deferred = false;
        if let Err(error) = self.filmstrip.sync_content_db() {
            self.view.message = error;
        }
        if let Err(error) = self.wave.sync_content_db() {
            self.view.message = error;
        }
    }

    pub(super) fn focus_timeline_assets(&mut self, clip_id: &str) {
        self.refresh_timeline_artifact_context();
        self.view.timeline_assets = self
            .timeline_assets
            .load_clip(clip_id)
            .unwrap_or_else(|_| qnc_timeline_assets::SourceTimelineAssets::empty_for(clip_id));
    }

    pub(super) fn remove_timeline_artifact_clips(&mut self, clip_ids: &[String]) {
        self.filmstrip.remove_clips(clip_ids);
        self.wave.remove_clips(clip_ids);
        self.timeline_assets.remove_clips(clip_ids);
        if self
            .view
            .preview_clip_id
            .as_ref()
            .is_some_and(|id| clip_ids.contains(id))
        {
            self.view.timeline_assets = qnc_timeline_assets::SourceTimelineAssets::empty();
        }
    }

    pub(super) fn poll_timeline_artifacts(&mut self) -> bool {
        let active_clip_id = self.view.preview_clip_id.as_deref();
        let filmstrip = self.filmstrip.poll(active_clip_id);
        let wave = self.wave.poll(active_clip_id);
        if let Some(error) = filmstrip.active_error.or(wave.active_error) {
            self.view.message = error;
        }

        let changed = filmstrip.changed || wave.changed;
        if changed {
            if let Some(clip_id) = self.view.preview_clip_id.as_deref() {
                if let Ok(assets) = self.timeline_assets.refresh_clip(clip_id) {
                    self.view.timeline_assets = assets;
                }
            }
        }
        changed
    }

    pub(super) fn set_timeline_artifact_playback_priority(&mut self, active: bool) {
        self.filmstrip.set_playback_priority(active);
        self.wave.set_playback_priority(active);
    }

    pub(super) fn update_timeline_artifact_playback_priority(&mut self) {
        self.set_timeline_artifact_playback_priority(self.playback_guard_active());
    }
}
