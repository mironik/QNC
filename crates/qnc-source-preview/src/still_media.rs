//! Where this machine reads the picture a clip plays from, for a still of one frame (the
//! edit pictures of a trim, user 2026-10-10). The same choice the player makes: the
//! saved record through the same reader and the project's `playback.input` (original,
//! proxy or optimized copy), the file through the same source bindings, the project
//! folder included. No probe; a source that is not local to this machine gives an error.

use std::{path::PathBuf, sync::Arc};

use qnc_media_metadata::StreamDetails;
use qnc_player_input::{InputReader, PlayerContentRead};
use qnc_source_bindings::SourceBinding;
use qnc_work_settings::{SettingsReader, WorkSettings};

use crate::{PreviewContext, SourcePreview};

/// The local file of a clip's picture and its saved frame rate (`num`, `den`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatedPicture {
    pub file: PathBuf,
    pub fps: (u32, u32),
}

/// Finds the picture of a clip of the project the preview works in; cheap to clone and
/// safe on another thread (it holds no player, no open database).
#[derive(Clone)]
pub struct StillLocator {
    reader: SettingsReader,
    settings: WorkSettings,
    sources: Vec<SourceBinding>,
    content: Arc<dyn PlayerContentRead>,
}

impl StillLocator {
    /// The project this locator reads from.
    pub fn project_id(&self) -> &str {
        &self.settings.project_id
    }

    pub fn locate(&self, clip_id: &str) -> Result<LocatedPicture, String> {
        let input = InputReader::with_content_reader(self.reader.clone(), self.content.clone())
            .load(&self.settings.workspace_db_uri, clip_id)
            .map_err(|error| error.to_string())?;
        let media = input.media().map_err(|error| error.to_string())?;
        let rate = media
            .streams
            .iter()
            .find_map(|stream| match &stream.details {
                StreamDetails::Video(video) => video.frame_rate.as_ref().map(|rate| rate.value),
                _ => None,
            })
            .ok_or("Klip nema spremljeni frame rate.")?;
        let fps = (
            u32::try_from(rate.fps_num).map_err(|_| "Neispravan frame rate.")?,
            u32::try_from(rate.fps_den).map_err(|_| "Neispravan frame rate.")?,
        );
        Ok(LocatedPicture { file: self.local_file(&media.media_uri)?, fps })
    }

    /// The media URI on a local binding of this machine: a source or the project folder.
    fn local_file(&self, media_uri: &str) -> Result<PathBuf, String> {
        let reference = qnc_source_contract::SourceReference::from_uri(media_uri).map_err(|error| error.to_string())?;
        let project = self
            .reader
            .local_workspace_dir(&self.settings)
            .ok()
            .flatten()
            .zip(self.settings.project_media_source_uri().ok())
            .map(|(dir, uri)| (uri, dir));
        let root = self
            .sources
            .iter()
            .filter_map(|binding| Some((binding.uri.clone(), binding.file.clone()?)))
            .chain(project)
            .find(|(uri, _)| uri == reference.source_uri())
            .map(|(_, root)| root)
            .ok_or("Medij klipa nije lokalno dostupan za sliku.")?;
        let root = root.canonicalize().map_err(|error| error.to_string())?;
        let file = root.join(reference.relative_path()).canonicalize().map_err(|error| error.to_string())?;
        if !file.starts_with(&root) || !file.is_file() {
            return Err("Medij klipa je izvan svog izvora.".into());
        }
        Ok(file)
    }
}

impl SourcePreview {
    /// The locator of the project the preview works in; `None` before one is set.
    pub fn still_locator(&self) -> Option<StillLocator> {
        self.context.as_ref().map(PreviewContext::still_locator)
    }
}

impl PreviewContext {
    fn still_locator(&self) -> StillLocator {
        StillLocator {
            reader: self.reader.clone(),
            settings: self.settings.clone(),
            sources: self.sources.clone(),
            content: self.player_content.clone(),
        }
    }
}
