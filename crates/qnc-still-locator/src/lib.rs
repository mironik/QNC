//! A piece of its own (user 2026-10-20: the keyframe jumps and the trim zoom depended on
//! the whole player module for it). Where this machine reads the picture a clip plays from, for a still of one frame (the
//! edit pictures of a trim, user 2026-10-10). The same choice the player makes: the
//! saved record through the same reader and the project's `playback.input` (original,
//! proxy or optimized copy), the file through the same source bindings, the project
//! folder included. No probe; a source that is not local to this machine gives an error.

use std::{path::PathBuf, sync::Arc};

use qnc_media_metadata::StreamDetails;
use qnc_player_input::{InputReader, PlayerContentRead};
use qnc_source_bindings::SourceBinding;
use qnc_work_settings::{SettingsReader, WorkSettings};

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

/// Where this machine reads a clip's sound (the camera original, as the player hears it,
/// user 2026-10-25: the fine wave of the trim zoom): the local file, its saved record, each
/// audio stream in order with its channel count (camera channel k is the k-th channel of
/// them), and the saved start and rate of the picture, which place a clip frame in time.
#[derive(Debug, Clone, PartialEq)]
pub struct LocatedSound {
    pub file: PathBuf,
    pub media: qnc_media_metadata::MediaRepresentation,
    pub stamp: String,
    /// `(stream index, channels, sample rate)` of every audio stream, in order.
    pub streams: Vec<(u32, u16, u32)>,
    pub fps: (u32, u32),
    /// The time of clip frame 0 in seconds of the media's timestamps.
    pub frame_zero_seconds: f64,
}

impl LocatedSound {
    /// The stream, the channel inside it and the stream's width of camera channel `channel`.
    pub fn channel(&self, channel: u16) -> Option<(u32, u16, u16, u32)> {
        let mut first = 0u16;
        for (index, channels, rate) in &self.streams {
            if channel < first + channels {
                return Some((*index, channel - first, *channels, *rate));
            }
            first += channels;
        }
        None
    }

    /// The time of clip frame `frame` (seconds of the media's timestamps).
    pub fn seconds_of(&self, frame: f64) -> f64 {
        self.frame_zero_seconds + frame * f64::from(self.fps.1) / f64::from(self.fps.0.max(1))
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
    /// A locator of the project these work settings belong to.
    pub fn new(reader: SettingsReader, settings: WorkSettings, sources: Vec<SourceBinding>, content: Arc<dyn PlayerContentRead>) -> Self {
        Self { reader, settings, sources, content }
    }

    /// The project this locator reads from.
    pub fn project_id(&self) -> &str {
        &self.settings.project_id
    }

    /// The reader of that project's settings (its database binding on this machine).
    pub fn reader(&self) -> &SettingsReader {
        &self.reader
    }

    /// The work settings of that project, as read from its database.
    pub fn settings(&self) -> &WorkSettings {
        &self.settings
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

    /// The sound of a clip: its camera original on this machine (no probe).
    pub fn locate_sound(&self, clip_id: &str) -> Result<LocatedSound, String> {
        let picture = self.locate(clip_id)?;
        let input = InputReader::with_content_reader(self.reader.clone(), self.content.clone())
            .load(&self.settings.workspace_db_uri, clip_id)
            .map_err(|error| error.to_string())?;
        let media = input.audio_media().clone();
        let streams = media
            .streams
            .iter()
            .filter_map(|stream| match &stream.details {
                StreamDetails::Audio(audio) => Some((
                    stream.index.as_ref()?.value,
                    u16::try_from(audio.channels.as_ref()?.value).ok()?,
                    audio.sample_rate_hz.as_ref()?.value,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        if streams.is_empty() {
            return Err(qnc_ui_text::text("player.clip_stored_audio_stream").into());
        }
        let (file, stamp) = self.local_file(&media.media_uri)?;
        let (pts, (num, den)) = picture.start;
        Ok(LocatedSound { file, media, stamp, streams, fps: picture.fps, frame_zero_seconds: pts as f64 * num as f64 / den.max(1) as f64 })
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
        let local = |sources: &[qnc_source_bindings::SourceBinding]| {
            sources
                .iter()
                .filter_map(|binding| Some((binding.uri.clone(), binding.file.clone()?)))
                .chain(project.clone())
                .find(|(uri, _)| uri == reference.source_uri())
        };
        // A disk attached after the sources were read is looked for only when it is missing.
        let (source_uri, root) = local(&self.sources)
            .or_else(|| local(&qnc_source_bindings::with_mounted(&self.sources)))
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
