//! Read-only adapters that feed the player and the timeline from public
//! contracts of the active project only: the saved clip with its media record
//! (`qnc-content-store`, read-only) and the content views (`qnc-content-read`).
//! Nothing is read from a host database, so a copied project plays anywhere.

use qnc_content_read::ContentReader;
use qnc_timeline_assets::TimelineArtifactRead;

/// The clip records of the active project (block `qnc-player-clip-content`).
pub(crate) use qnc_player_clip_content::PlayerClipContent as PlayerContent;

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
