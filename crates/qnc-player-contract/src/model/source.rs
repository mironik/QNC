use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{AudioFormat, FrameNumber, Timebase, VideoFormat};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRuntime {
    pub source_id: String,
    pub duration_frames: FrameNumber,
    pub timebase: Timebase,
    pub source_start_tc: Option<String>,
    pub video_format: Option<VideoFormat>,
    pub audio_format: Option<AudioFormat>,
}

impl SourceRuntime {
    pub fn new(
        source_id: impl Into<String>,
        duration_frames: FrameNumber,
        timebase: Timebase,
    ) -> Result<Self, String> {
        if duration_frames == 0 {
            return Err("source duration_frames must be greater than zero".to_string());
        }
        let source_id = source_id.into();
        if source_id.trim().is_empty() {
            return Err("source_id must not be blank".into());
        }
        Timebase::new(timebase.fps_num, timebase.fps_den)?;
        Ok(Self {
            source_id,
            duration_frames,
            timebase,
            source_start_tc: None,
            video_format: None,
            audio_format: None,
        })
    }

    pub fn with_video_format(mut self, video_format: VideoFormat) -> Self {
        self.video_format = Some(video_format);
        self
    }

    pub fn with_audio_format(mut self, audio_format: AudioFormat) -> Self {
        self.audio_format = Some(audio_format);
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        Self::new(&self.source_id, self.duration_frames, self.timebase)?;
        if let Some(video) = &self.video_format {
            VideoFormat::new(
                video.width,
                video.height,
                video.field_mode,
                video.color_space.clone(),
            )?;
            super::PixelAspect::new(video.pixel_aspect.num, video.pixel_aspect.den)?;
            if matches!(&video.color_space, super::ColorSpace::Custom(name) if name.trim().is_empty())
            {
                return Err("custom color space must not be blank".into());
            }
        }
        if let Some(audio) = &self.audio_format {
            AudioFormat::new(audio.sample_rate_hz, audio.channel_count)?;
        }
        Ok(())
    }
}

pub type SourceMap = BTreeMap<String, SourceRuntime>;
pub type SourceSet = BTreeSet<String>;
