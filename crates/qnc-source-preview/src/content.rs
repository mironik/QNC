//! Read-only adapters that feed the player and the timeline from public
//! contracts: the project content views (`qnc-content-read`) and the media
//! records database (`qnc-media-record-db`, read-only).

use qnc_content_read::ContentReader;
use qnc_media_record_db::{Access, Client};
use qnc_player_input::{PlayerClipRecord, PlayerContentRead};
use qnc_source_bindings::SourceBinding;
use qnc_timeline_assets::TimelineArtifactRead;

#[derive(Clone)]
pub(crate) struct PlayerContent {
    pub content: ContentReader,
    pub records: Option<SourceBinding>,
}

impl PlayerContentRead for PlayerContent {
    fn read_clip(&self, clip_id: &str) -> Result<Option<PlayerClipRecord>, String> {
        let Some(head) = self.content.clip_head(clip_id)? else {
            return Ok(None);
        };
        let binding = self
            .records
            .as_ref()
            .ok_or("Nedostaje veza na bazu zapisa medija.")?;
        if binding.uri != head.record_db_uri {
            return Err("Zapis medija je u bazi koja nije konfigurirana.".into());
        }
        let resolver = binding.resolver()?;
        let token = binding.token()?;
        let mut client = Client::open(
            &resolver,
            &head.record_db_uri,
            Access::ReadOnly,
            token.as_deref(),
        )
        .map_err(|error| error.to_string())?;
        let snapshot = client
            .read(clip_id, Some(head.record_revision))
            .map_err(|error| error.to_string())?
            .ok_or("Zapis medija nije pronadjen.")?;
        Ok(Some(PlayerClipRecord {
            name: head.name,
            snapshot,
            imported_media_uri: head.imported_media_uri,
        }))
    }
}

#[derive(Clone)]
pub(crate) struct ArtifactReader {
    pub content: ContentReader,
    pub filmstrip_root_uri: String,
    pub filmstrip_dir: std::path::PathBuf,
}

impl TimelineArtifactRead for ArtifactReader {
    fn read_filmstrip(
        &self,
        clip_id: &str,
    ) -> Result<Option<qnc_filmstrip::FilmstripArtifactRecord>, String> {
        self.content.filmstrip(clip_id)
    }

    fn read_wave(&self, clip_id: &str) -> Result<Option<qnc_wave::WaveArtifactRecord>, String> {
        self.content.wave(clip_id)
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
