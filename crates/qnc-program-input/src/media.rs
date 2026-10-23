use super::*;

/// The saved facts of a clip for the program: picture from the representation
/// the project plays, sound from the original, channels in inventory order
/// (channel 1 is the first channel of the first audio stream).
pub fn resolved_media(
    clip_id: &str,
    picture: &MediaRepresentation,
    sound: &MediaRepresentation,
    layout: &StreamLayout,
) -> Result<ResolvedProgramMedia, String> {
    let video = layout
        .video
        .as_ref()
        .ok_or_else(|| qnc_ui_text::fill("story.msg.clip_no_picture", &[("clip", clip_id)]))?;
    let details = picture
        .streams
        .iter()
        .find(|stream| stream.index.as_ref().map(|i| i.value) == Some(video.stream_index))
        .and_then(|stream| match &stream.details {
            StreamDetails::Video(details) => Some(details),
            _ => None,
        })
        .ok_or_else(|| qnc_ui_text::fill("story.msg.clip_no_video_stream", &[("clip", clip_id)]))?;
    let known = |what: &'static str| qnc_ui_text::fill("story.msg.clip_missing_fact", &[("clip", clip_id), ("what", qnc_ui_text::text(what))]);
    let video_format = ProbedVideoFormat {
        width: details.width.as_ref().ok_or_else(|| known("sirina"))?.value,
        height: details
            .height
            .as_ref()
            .ok_or_else(|| known("visina"))?
            .value,
        scan_mode: match details
            .scan_mode
            .as_ref()
            .ok_or_else(|| known("scan"))?
            .value
        {
            qnc_media_metadata::ScanMode::Progressive => ScanMode::Progressive,
            qnc_media_metadata::ScanMode::InterlacedTopFieldFirst => {
                ScanMode::InterlacedTopFieldFirst
            }
            qnc_media_metadata::ScanMode::InterlacedBottomFieldFirst => {
                ScanMode::InterlacedBottomFieldFirst
            }
        },
    };
    let audio_channels =
        u16::try_from(layout.audio_channels.len()).map_err(|_| known("story.fact.audio_channels"))?;
    let audio_format = if audio_channels == 0 {
        None
    } else {
        let mut rates = layout.audio_channels.iter().map(|channel| {
            sound
                .streams
                .iter()
                .find(|stream| stream.index.as_ref().map(|i| i.value) == Some(channel.stream_index))
                .and_then(|stream| match &stream.details {
                    StreamDetails::Audio(audio) => audio.sample_rate_hz.as_ref(),
                    _ => None,
                })
                .map(|rate| rate.value)
                .ok_or_else(|| known("story.fact.sample_rate"))
        });
        let rate = rates
            .next()
            .unwrap_or_else(|| Err(known("story.fact.sample_rate")))?;
        for other in rates {
            if other? != rate {
                return Err(qnc_ui_text::fill("story.msg.clip_mixed_sample_rates", &[("clip", clip_id)]));
            }
        }
        Some(ProbedAudioFormat {
            sample_rate_hz: rate,
            channel_count: audio_channels,
        })
    };
    Ok(ResolvedProgramMedia {
        media: MediaRef {
            clip_id: clip_id.to_string(),
            media_uri: picture.media_uri.clone(),
        },
        audio_media: MediaRef {
            clip_id: clip_id.to_string(),
            media_uri: sound.media_uri.clone(),
        },
        duration_frames: i64::try_from(video.duration_frames).map_err(|_| known("trajanje"))?,
        timebase: video.timebase,
        video_format: Some(video_format),
        has_audio: audio_channels > 0,
        audio_channels,
        audio_format,
        timecode_start: None,
    })
}

/// The original timecode of frame 0 of a clip from its stored record (user rule
/// 2026-10-01: proxy and original are linked by timecode). When the picture is the
/// proxy, its stored timecode must be the one of the original, or frame N of the proxy
/// would not be frame N of the original: that is a controlled error, never a guess.
pub fn timecode_start(clip_id: &str, input: &PreparedInput) -> Result<Option<i64>, String> {
    let Some(video) = input.layout.video.as_ref() else {
        return Ok(None);
    };
    let (Ok(num), Ok(den)) = (u64::try_from(video.timebase.fps_num), u64::try_from(video.timebase.fps_den)) else {
        return Ok(None);
    };
    let tags = |media: &MediaRepresentation| -> Vec<(String, String)> {
        media.tags.iter().map(|(key, fact)| (key.clone(), fact.value.clone())).collect()
    };
    let original = tags(input.audio_media());
    let Some(timecode) = qnc_source_timecode::SourceTimecode::from_tags(
        original.iter().map(|(key, value)| (key.as_str(), value.as_str())),
        (num, den),
    ) else {
        return Ok(None);
    };
    if input.representation == qnc_player_input::Representation::Proxy {
        let proxy = tags(input.media().map_err(|error| error.to_string())?);
        if timecode.proxy_matches(proxy.iter().map(|(key, value)| (key.as_str(), value.as_str()))) == Some(false) {
            return Err(qnc_ui_text::fill("story.msg.proxy_timecode_differs", &[("clip", clip_id)]));
        }
    }
    timecode
        .start_frame
        .map(|frames| i64::try_from(frames).map_err(|_| qnc_ui_text::text("story.msg.timecode_out_of_range").to_string()))
        .transpose()
}

pub(crate) struct PreparedResolver<'a, C> {
    pub(crate) clips: &'a C,
    pub(crate) prepared: BTreeMap<String, PreparedInput>,
    pub(crate) story_rate: (u32, u32),
}

impl<C: ClipInputs> PreparedResolver<'_, C> {
    pub(crate) fn prepared(&mut self, clip_id: &str) -> Result<&PreparedInput, String> {
        if !self.prepared.contains_key(clip_id) {
            let input = self.clips.prepared(clip_id)?;
            self.prepared.insert(clip_id.to_string(), input);
        }
        self.prepared
            .get(clip_id)
            .ok_or_else(|| qnc_ui_text::fill("story.msg.clip_not_prepared", &[("clip", clip_id)]))
    }
}

impl<C: ClipInputs> ProgramMediaResolver for PreparedResolver<'_, C> {
    fn resolve(&mut self, clip_id: &str) -> Result<ResolvedProgramMedia, String> {
        let input = self.prepared(clip_id)?;
        let picture = input.media().map_err(|error| error.to_string())?;
        let timecode = timecode_start(clip_id, input)?;
        let mut media = resolved_media(clip_id, picture, input.audio_media(), &input.layout)?;
        media.timecode_start = timecode;
        at_story_rate(&mut media, self.story_rate)?;
        Ok(media)
    }
}

/// A clip of twice or half the story rate described at the story rate, as the player
/// plays it (user 2026-10-09: 2:1 only): its frames and the timecode of its frame 0 are
/// counted in story frames.
pub fn at_story_rate(media: &mut ResolvedProgramMedia, story_rate: (u32, u32)) -> Result<(), String> {
    let own = (
        u32::try_from(media.timebase.fps_num).map_err(|_| qnc_ui_text::text("story.msg.bad_clip_timebase"))?,
        u32::try_from(media.timebase.fps_den).map_err(|_| qnc_ui_text::text("story.msg.bad_clip_timebase"))?,
    );
    let (num, den) = qnc_program_db::story_per_source(story_rate, own)?;
    if (num, den) != (1, 1) {
        media.timebase = timebase(story_rate.0, story_rate.1, &media.media.clip_id)?;
        media.duration_frames = media.duration_frames * num / den;
        media.timecode_start = media.timecode_start.map(|start| start * num / den);
    }
    Ok(())
}

pub(crate) fn timebase(num: u32, den: u32, label: &str) -> Result<FrameTimebase, String> {
    FrameTimebase::new(i64::from(num), i64::from(den))
        .map_err(|_| qnc_ui_text::fill("story.msg.no_original_timebase", &[("source", label)]))
}

pub(crate) fn frame(value: u64) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| qnc_ui_text::text("story.msg.frame_out_of_range").to_string())
}
