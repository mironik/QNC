//! Read-only adapters over the public content read port of the project DB
//! (AGENTS.md section 4.1, step 6). Every open is `Access::ReadOnly`.

use qnc_ingest_store::content::{Access, ContentTarget};
use qnc_player_input::{PlayerClipRecord, PlayerContentRead};
use qnc_timeline_assets::TimelineArtifactRead;

#[derive(Clone)]
pub(crate) struct PlayerContent {
    pub target: ContentTarget,
}

impl PlayerContentRead for PlayerContent {
    fn read_clip(&self, clip_id: &str) -> Result<Option<PlayerClipRecord>, String> {
        let stored = self.target.open(Access::ReadOnly)?.read(clip_id)?;
        Ok(stored.map(|stored| PlayerClipRecord {
            name: stored.clip.name,
            snapshot: stored.clip.snapshot,
            imported_media_uri: stored.imported_media_uri,
        }))
    }
}

#[derive(Clone)]
pub(crate) struct ArtifactReader {
    pub target: ContentTarget,
    pub filmstrip_root_uri: String,
    pub filmstrip_dir: std::path::PathBuf,
}

impl TimelineArtifactRead for ArtifactReader {
    fn read_filmstrip(
        &self,
        clip_id: &str,
    ) -> Result<Option<qnc_filmstrip::FilmstripArtifactRecord>, String> {
        Ok(self
            .target
            .open(Access::ReadOnly)?
            .read_filmstrip(clip_id)
            .map_err(|error| error.to_string())?
            .map(to_filmstrip_record))
    }

    fn read_wave(&self, clip_id: &str) -> Result<Option<qnc_wave::WaveArtifactRecord>, String> {
        self.target.open(Access::ReadOnly)?.read_wave(clip_id)
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
