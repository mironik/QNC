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
#[derive(Debug, Clone, PartialEq)]
pub struct LocatedPicture {
    pub file: PathBuf,
    pub fps: (u32, u32),
    /// The saved start of the video stream, `(start_pts, time_base)`: a picture's frame is
    /// counted from it, as the player counts them.
    pub start: (i64, (i64, i64)),
    /// The saved record of the media the picture plays from and its video stream: a
    /// decoder is opened on them as the player opens one (user 2026-10-13: a decoder that
    /// stays open for the trim zoom).
    pub media: qnc_media_metadata::MediaRepresentation,
    pub stream_index: u32,
    /// The storage stamp of the local file (the decoder checks the media is the same).
    pub stamp: String,
}

impl LocatedPicture {
    /// The frame of the clip at a presentation time (seconds of the media timestamps).
    pub fn frame_at(&self, seconds: f64) -> Option<u64> {
        let (pts, (num, den)) = self.start;
        let start = pts as f64 * num as f64 / den.max(1) as f64;
        let frame = ((seconds - start) * f64::from(self.fps.0) / f64::from(self.fps.1.max(1))).round();
        (frame >= 0.0).then_some(frame as u64)
    }

    /// The seek position (seconds from the media start) of a frame.
    pub fn seek_of(&self, frame: u64) -> f64 {
        frame as f64 * f64::from(self.fps.1) / f64::from(self.fps.0.max(1))
    }
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
        let video = media.streams.iter().find_map(|stream| match &stream.details {
            StreamDetails::Video(video) => Some((stream, video)),
            _ => None,
        });
        let (stream, video) = video.ok_or(qnc_ui_text::text("player.clip_stored_video_stream"))?;
        let rate = video.frame_rate.as_ref().map(|rate| rate.value).ok_or(qnc_ui_text::text("player.clip_stored_frame_rate"))?;
        let fps = (
            u32::try_from(rate.fps_num).map_err(|_| "Neispravan frame rate.")?,
            u32::try_from(rate.fps_den).map_err(|_| "Neispravan frame rate.")?,
        );
        let start_pts = stream.start_pts.as_ref().map(|pts| pts.value).ok_or(qnc_ui_text::text("player.clip_stored_start_pts"))?;
        let base = stream.time_base.as_ref().map(|tb| tb.value).ok_or(qnc_ui_text::text("player.clip_stored_time_base"))?;
        let start = (start_pts, (base.numerator, base.denominator));
        let stream_index = stream.index.as_ref().map(|index| index.value).ok_or(qnc_ui_text::text("player.clip_stored_video_stream_index"))?;
        let (file, stamp) = self.local_file(&media.media_uri)?;
        Ok(LocatedPicture { file, fps, start, media: media.clone(), stream_index, stamp })
    }

    /// The media URI on a local binding of this machine: a source or the project folder,
    /// and the storage stamp of the file.
    fn local_file(&self, media_uri: &str) -> Result<(PathBuf, String), String> {
        let reference = qnc_source_contract::SourceReference::from_uri(media_uri).map_err(|error| error.to_string())?;
        let project = self
            .reader
            .local_workspace_dir(&self.settings)
            .ok()
            .flatten()
            .zip(self.settings.project_media_source_uri().ok())
            .map(|(dir, uri)| (uri, dir));
        let (source_uri, root) = self
            .sources
            .iter()
            .filter_map(|binding| Some((binding.uri.clone(), binding.file.clone()?)))
            .chain(project)
            .find(|(uri, _)| uri == reference.source_uri())
            .ok_or(qnc_ui_text::text("player.clip_media_available_locally_picture"))?;
        let root = root.canonicalize().map_err(|error| error.to_string())?;
        let file = root.join(reference.relative_path()).canonicalize().map_err(|error| error.to_string())?;
        if !file.starts_with(&root) || !file.is_file() {
            return Err(qnc_ui_text::text("player.clip_media_outside_its_source").into());
        }
        let source = qnc_media_stream::LocalSource::new(&source_uri, &root).map_err(|error| error.to_string())?;
        let stamp = qnc_media_stream::MediaStream::local(&source, media_uri)
            .map_err(|error| error.to_string())?
            .info()
            .storage_stamp
            .clone();
        Ok((file, stamp))
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
