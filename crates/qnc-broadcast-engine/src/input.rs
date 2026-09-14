use crate::{Result, error};
use qnc_broadcast_player::{
    AudioFormat, ColorSpace, FieldMode, SourceRuntime, Timebase, VideoFormat,
};
use qnc_media_metadata::{
    FrameRateMode, MediaRepresentation, Rational, ScanMode, Signal, StreamDetails,
};
use qnc_pixel_convert::{ConversionSpec, Transfer};
use qnc_player_input::{AudioChannel, PreparedInput};
use qnc_video_output::{OutputConfig, PixelFormat};

pub(crate) const PREBUFFER_FRAMES: usize = 8;
/// Keep several conversions ahead of the audio clock so the due picture is
/// already ready at present. A short queue makes Play wait on convert and
/// the monitor lags the sound.
pub(crate) const CONVERT_IN_FLIGHT: usize = 4;

/// UI preview raster only. Source remains the saved width/height; integer 2:1
/// keeps even lines together so 1080p50 does not weave. Full-HD RGBA readback
/// cannot hold 50 fps through the mmap preview path on current iGPU hosts.
pub(crate) fn preview_raster_bounds(width: u32, height: u32) -> [u32; 2] {
    if width >= 2 && height >= 2 && width % 2 == 0 && height % 2 == 0 {
        [width / 2, height / 2]
    } else {
        [width, height]
    }
}

// Prepared before Play. A bounded half-second read-ahead absorbs delivery jitter.
pub(crate) fn monitor_prebuffer_frames(timebase: Timebase) -> Result<usize> {
    let num = u64::try_from(timebase.fps_num).map_err(error)?;
    let den = u64::try_from(timebase.fps_den).map_err(error)?;
    if num == 0 || den == 0 {
        return Err(error("invalid saved timebase"));
    }
    Ok((num.div_ceil(den * 2).clamp(PREBUFFER_FRAMES as u64, 120)) as usize)
}

#[cfg(test)]
mod monitor_buffer_tests {
    use super::*;

    #[test]
    fn preview_raster_is_integer_half_of_saved_even_size() {
        assert_eq!(preview_raster_bounds(1920, 1080), [960, 540]);
        assert_eq!(preview_raster_bounds(1280, 720), [640, 360]);
        assert_eq!(preview_raster_bounds(720, 576), [360, 288]);
        assert_eq!(preview_raster_bounds(1, 1), [1, 1]);
    }

    #[test]
    fn half_second_is_bounded_and_uses_saved_rational_fps() {
        assert_eq!(
            monitor_prebuffer_frames(Timebase {
                fps_num: 50,
                fps_den: 1
            })
            .unwrap(),
            25
        );
        assert_eq!(
            monitor_prebuffer_frames(Timebase {
                fps_num: 30000,
                fps_den: 1001
            })
            .unwrap(),
            15
        );
        assert_eq!(
            monitor_prebuffer_frames(Timebase {
                fps_num: 1000,
                fps_den: 1
            })
            .unwrap(),
            120
        );
        assert!(
            monitor_prebuffer_frames(Timebase {
                fps_num: 50,
                fps_den: 0
            })
            .is_err()
        );
    }
}

/// Immutable, validated saved input. No database lookup or representation choice here.
pub struct InputPlan {
    pub(crate) source: SourceRuntime,
    pub(crate) media: MediaRepresentation,
    pub(crate) video_index: u32,
    pub(crate) origin: (i64, Rational),
    pub(crate) audio_media: MediaRepresentation,
    pub(crate) audio_origin: (i64, Rational),
    pub(crate) spec: ConversionSpec,
    pub(crate) audio_streams: Vec<AudioStreamPlan>,
    pub(crate) audio_channels: Option<qnc_audio_output::ChannelMap>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AudioStreamPlan {
    pub stream_index: u32,
    pub source_channels: u16,
    pub selected_channels: Vec<u16>,
}
impl InputPlan {
    pub fn new(input: &PreparedInput, workspace_uri: &str, clip_id: &str) -> Result<Self> {
        input.validate_for(workspace_uri, clip_id).map_err(error)?;
        let media = input.media().map_err(error)?;
        let layout = input
            .layout
            .video
            .as_ref()
            .ok_or_else(|| error("video stream required"))?;
        if layout.frame_rate_mode == FrameRateMode::Variable {
            return Err(error(
                "variable frame timing requires a saved frame-time index",
            ));
        }
        let stream = media
            .streams
            .iter()
            .find(|s| s.index.as_ref().map(|i| i.value) == Some(layout.stream_index))
            .ok_or_else(|| error("missing saved video stream"))?;
        let StreamDetails::Video(video) = &stream.details else {
            return Err(error("invalid video map"));
        };
        let spec = ConversionSpec::from_saved(video).map_err(error)?;
        let sar = video
            .sample_aspect_ratio
            .as_ref()
            .ok_or_else(|| error("missing pixel aspect"))?
            .value;
        require_native_geometry(sar, video.rotation_degrees.as_ref().map(|r| &r.value))?;
        let origin = (
            stream
                .start_pts
                .as_ref()
                .ok_or_else(|| error("missing video start PTS"))?
                .value,
            stream
                .time_base
                .as_ref()
                .ok_or_else(|| error("missing video time base"))?
                .value,
        );
        let mut source = SourceRuntime::new(
            clip_id,
            layout.duration_frames,
            Timebase::new(layout.timebase.fps_num, layout.timebase.fps_den).map_err(error)?,
        )
        .map_err(error)?
        .with_video_format(
            VideoFormat::new(
                spec.width,
                spec.height,
                field_mode_from_saved(spec.scan_mode),
                color_space_from_saved(&spec)?,
            )
            .map_err(error)?,
        );
        let audio_media = input.audio_media();
        let (native_audio_streams, native_audio_format) = native_audio_layout(audio_media)?;
        let (audio_streams, audio_format, audio_channels) = project_audio_layout(
            &native_audio_streams,
            native_audio_format.as_ref(),
            &input.layout.audio_channels,
            &input.project_audio,
        )?;
        let audio_video = audio_media
            .streams
            .iter()
            .find(|s| matches!(s.details, StreamDetails::Video(_)))
            .ok_or_else(|| error("missing saved original video timing for audio"))?;
        let audio_origin = (
            audio_video
                .start_pts
                .as_ref()
                .ok_or_else(|| error("missing original video start PTS"))?
                .value,
            audio_video
                .time_base
                .as_ref()
                .ok_or_else(|| error("missing original video time base"))?
                .value,
        );
        source.audio_format = audio_format;
        source.validate().map_err(error)?;
        Ok(Self {
            source,
            media: media.clone(),
            video_index: layout.stream_index,
            origin,
            audio_media: audio_media.clone(),
            audio_origin,
            spec,
            audio_streams,
            audio_channels,
        })
    }
    pub fn source(&self) -> &SourceRuntime {
        &self.source
    }
    pub fn audio_channels(&self) -> Option<&qnc_audio_output::ChannelMap> {
        self.audio_channels.as_ref()
    }
    pub fn output_config(&self, session_id: &str, generation: u64) -> Result<OutputConfig> {
        let slots = monitor_prebuffer_frames(self.source.timebase)?.saturating_add(4);
        let config = OutputConfig {
            version: qnc_video_output::VERSION.into(),
            session_id: session_id.into(),
            generation,
            width: self.spec.width,
            height: self.spec.height,
            pixel_format: PixelFormat::Rgba8Srgb,
            slots,
            pool_budget_bytes: self.spec.output_bytes().map_err(error)? as u64 * slots as u64,
        };
        config.validate().map_err(error)?;
        Ok(config)
    }
}

type ProjectAudioLayout = (
    Vec<AudioStreamPlan>,
    Option<AudioFormat>,
    Option<qnc_audio_output::ChannelMap>,
);

fn field_mode_from_saved(scan_mode: ScanMode) -> FieldMode {
    match scan_mode {
        ScanMode::Progressive => FieldMode::Progressive,
        ScanMode::InterlacedTopFieldFirst => FieldMode::InterlacedUpperFirst,
        ScanMode::InterlacedBottomFieldFirst => FieldMode::InterlacedLowerFirst,
    }
}

fn color_space_from_saved(spec: &ConversionSpec) -> Result<ColorSpace> {
    match (spec.primaries.as_str(), spec.matrix.as_str(), spec.transfer) {
        ("bt709", "bt709", Transfer::Bt709) => Ok(ColorSpace::Rec709),
        ("bt709", "bt709", Transfer::Srgb) => Ok(ColorSpace::Srgb),
        _ => Err(error("unsupported saved color space")),
    }
}

fn project_audio_layout(
    native_streams: &[(u32, u16)],
    native: Option<&AudioFormat>,
    saved_channels: &[AudioChannel],
    project: &qnc_player_input::ProjectAudio,
) -> Result<ProjectAudioLayout> {
    project.validate().map_err(error)?;
    let Some(native) = native else {
        return Ok((Vec::new(), None, None));
    };
    if project.sample_rate_hz != native.sample_rate_hz {
        return Err(error(
            "Project audio sample rate requires conversion not supported by this playback adapter.",
        ));
    }
    if project.channels > native.channel_count
        || usize::from(project.channels) > saved_channels.len()
    {
        return Err(error(
            "Project audio channel count exceeds saved native channel inventory.",
        ));
    }
    // A1/A2 are mono source lanes. Preserve saved channel identity; never collapse them as stereo.
    let mut selected_streams = Vec::new();
    for channel in saved_channels.iter().take(usize::from(project.channels)) {
        let Some((_, source_channels)) = native_streams
            .iter()
            .find(|(stream_index, _)| *stream_index == channel.stream_index)
        else {
            return Err(error("saved audio channel references a missing stream"));
        };
        if channel.channel_index >= u32::from(*source_channels) {
            return Err(error("saved audio channel is outside its native stream"));
        }
        let channel_index = u16::try_from(channel.channel_index).map_err(error)?;
        if let Some(plan) = selected_streams
            .iter_mut()
            .find(|plan: &&mut AudioStreamPlan| plan.stream_index == channel.stream_index)
        {
            plan.selected_channels.push(channel_index);
        } else {
            selected_streams.push(AudioStreamPlan {
                stream_index: channel.stream_index,
                source_channels: *source_channels,
                selected_channels: vec![channel_index],
            });
        }
    }
    let selected_channels: u16 = selected_streams
        .iter()
        .try_fold(0u16, |sum, plan| {
            let count = u16::try_from(plan.selected_channels.len()).ok()?;
            sum.checked_add(count)
        })
        .ok_or_else(|| error("selected audio channel inventory overflow"))?;
    if selected_channels != project.channels {
        return Err(error(
            "Saved native audio stream inventory cannot satisfy project audio channels.",
        ));
    }
    let format = AudioFormat::new(native.sample_rate_hz, project.channels).map_err(error)?;
    let map = qnc_audio_output::ChannelMap::identity(project.channels).map_err(error)?;
    Ok((selected_streams, Some(format), Some(map)))
}

// Canonical stream order matches the saved input inventory. Never infer stereo pairs.
type NativeAudioLayout = (Vec<(u32, u16)>, Option<AudioFormat>);
pub(crate) fn native_audio_layout(media: &MediaRepresentation) -> Result<NativeAudioLayout> {
    let mut streams = Vec::new();
    let mut rate = None;
    let mut total = 0u16;
    for stream in &media.streams {
        if let StreamDetails::Audio(audio) = &stream.details {
            let index = stream
                .index
                .as_ref()
                .ok_or_else(|| error("missing audio index"))?
                .value;
            let sample_rate = audio
                .sample_rate_hz
                .as_ref()
                .ok_or_else(|| error("missing audio rate"))?
                .value;
            let count = u16::try_from(
                audio
                    .channels
                    .as_ref()
                    .ok_or_else(|| error("missing audio channels"))?
                    .value,
            )
            .map_err(error)?;
            if count == 0
                || rate.is_some_and(|r| r != sample_rate)
                || streams.iter().any(|(i, _)| *i == index)
            {
                return Err(error(
                    "invalid audio inventory or differing native sample rates",
                ));
            }
            rate = Some(sample_rate);
            total = total
                .checked_add(count)
                .filter(|n| *n <= 64)
                .ok_or_else(|| error("too many native audio channels"))?;
            streams.push((index, count));
        }
    }
    streams.sort_by_key(|(index, _)| *index);
    let format = rate
        .map(|r| AudioFormat::new(r, total).map_err(error))
        .transpose()?;
    Ok((streams, format))
}

fn require_native_geometry(sar: Rational, rotation: Option<&Signal<i32>>) -> Result<()> {
    // Unspecified is saved evidence of no declared transform, not missing metadata.
    if sar.numerator <= 0
        || sar.numerator != sar.denominator
        || !matches!(rotation, Some(Signal::Known(0) | Signal::Unspecified))
    {
        return Err(error(
            "non-square/rotated or missing geometry is not supported",
        ));
    }
    Ok(())
}

pub(crate) fn sample_boundary(frame: u64, timebase: Timebase, rate: u32) -> Result<u64> {
    Timebase::new(timebase.fps_num, timebase.fps_den).map_err(error)?;
    let n = u128::from(frame)
        .checked_mul(u128::try_from(timebase.fps_den).map_err(error)?)
        .and_then(|v| v.checked_mul(u128::from(rate)))
        .ok_or_else(|| error("sample boundary overflow"))?;
    if timebase.fps_num == 0 {
        return Err(error("invalid frame rate"));
    }
    u64::try_from(n / u128::try_from(timebase.fps_num).map_err(error)?).map_err(error)
}

/// Audio clock → source frame using the same saved clip timebase as Play.
#[cfg(test)]
pub(crate) fn frame_from_samples(sample: u64, timebase: Timebase, rate: u32) -> Result<u64> {
    Timebase::new(timebase.fps_num, timebase.fps_den).map_err(error)?;
    if rate == 0 || timebase.fps_num == 0 {
        return Err(error("invalid audio clock or saved timebase"));
    }
    let n = u128::from(sample)
        .checked_mul(u128::try_from(timebase.fps_num).map_err(error)?)
        .ok_or_else(|| error("audio frame overflow"))?;
    let den = u128::from(rate)
        .checked_mul(u128::try_from(timebase.fps_den).map_err(error)?)
        .ok_or_else(|| error("audio frame overflow"))?;
    u64::try_from(n / den).map_err(error)
}

/// Picture minus audio, in source frames. Negative means the picture lags sound.
#[cfg(test)]
pub(crate) fn picture_audio_offset_frames(picture: u64, audio_frame: u64) -> i64 {
    i64::try_from(picture).unwrap_or(i64::MAX) - i64::try_from(audio_frame).unwrap_or(i64::MAX)
}

/// Exact PTS-to-unit conversion. Never use decode ordinal as a source frame or round VFR.
pub(crate) fn relative_position(
    pts: i64,
    tb: Rational,
    origin: (i64, Rational),
    num: i64,
    den: i64,
) -> Result<u64> {
    let mul = |a: i128, b: i128| a.checked_mul(b).ok_or_else(|| error("timestamp overflow"));
    if tb.numerator <= 0
        || tb.denominator <= 0
        || origin.1.numerator <= 0
        || origin.1.denominator <= 0
        || num <= 0
        || den <= 0
    {
        return Err(error("invalid timestamp scale"));
    }
    let a = mul(
        mul(pts.into(), tb.numerator.into())?,
        origin.1.denominator.into(),
    )?;
    let b = mul(
        mul(origin.0.into(), origin.1.numerator.into())?,
        tb.denominator.into(),
    )?;
    let n = mul(
        a.checked_sub(b)
            .ok_or_else(|| error("timestamp overflow"))?,
        num.into(),
    )?;
    let d = mul(
        mul(tb.denominator.into(), origin.1.denominator.into())?,
        den.into(),
    )?;
    if n < 0 || n % d != 0 {
        return Err(error("decoded timestamp does not match saved source grid"));
    }
    u64::try_from(n / d).map_err(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn saved_channels(streams: &[(u32, u16)]) -> Vec<AudioChannel> {
        streams
            .iter()
            .flat_map(|(stream_index, channel_count)| {
                (0..u32::from(*channel_count)).map(|channel_index| AudioChannel {
                    stream_index: *stream_index,
                    channel_index,
                })
            })
            .collect()
    }

    fn audio_plan(stream_index: u32, source_channels: u16, selected: &[u16]) -> AudioStreamPlan {
        AudioStreamPlan {
            stream_index,
            source_channels,
            selected_channels: selected.to_vec(),
        }
    }

    #[test]
    fn source_video_format_is_derived_from_saved_media_facts() {
        assert_eq!(
            field_mode_from_saved(qnc_media_metadata::ScanMode::Progressive),
            FieldMode::Progressive
        );
        assert_eq!(
            field_mode_from_saved(qnc_media_metadata::ScanMode::InterlacedTopFieldFirst),
            FieldMode::InterlacedUpperFirst
        );
        assert_eq!(
            field_mode_from_saved(qnc_media_metadata::ScanMode::InterlacedBottomFieldFirst),
            FieldMode::InterlacedLowerFirst
        );
        let mut spec = ConversionSpec {
            version: qnc_pixel_convert::VERSION.into(),
            width: 1920,
            height: 1080,
            layout: qnc_pixel_convert::PixelLayout::Yuv422p10le,
            primaries: "bt709".into(),
            matrix: "bt709".into(),
            scan_mode: qnc_media_metadata::ScanMode::Progressive,
            range: qnc_pixel_convert::Range::Limited,
            transfer: qnc_pixel_convert::Transfer::Bt709,
        };
        assert_eq!(color_space_from_saved(&spec).unwrap(), ColorSpace::Rec709);
        spec.transfer = qnc_pixel_convert::Transfer::Srgb;
        assert_eq!(color_space_from_saved(&spec).unwrap(), ColorSpace::Srgb);
        spec.primaries = "hardcoded-default".into();
        assert!(color_space_from_saved(&spec).is_err());
    }

    #[test]
    fn source_decode_count_comes_from_project_not_full_native_inventory() {
        let native = AudioFormat::new(48000, 4).unwrap();
        let streams = [(1, 1), (2, 1), (3, 1), (4, 1)];
        for count in [2, 4] {
            let project = qnc_player_input::ProjectAudio {
                channels: count,
                sample_rate_hz: 48000,
            };
            let (selected, format, map) =
                project_audio_layout(&streams, Some(&native), &saved_channels(&streams), &project)
                    .unwrap();
            let format = format.unwrap();
            let map = map.unwrap();
            let expected: Vec<_> = streams
                .iter()
                .take(usize::from(count))
                .map(|(stream_index, channels)| audio_plan(*stream_index, *channels, &[0]))
                .collect();
            assert_eq!(selected, expected);
            assert_eq!(format.channel_count, count);
            assert_eq!(map.source_channels(), count);
            let source = [0.1, 0.2, 0.3, 0.4][..usize::from(count)].to_vec();
            let mut output = Vec::new();
            map.route(&source, &mut output).unwrap();
            assert_eq!(output, source);
            assert_eq!(native.channel_count, 4);
        }
    }
    #[test]
    fn project_audio_never_falls_back_to_native_count_or_rate() {
        let mut project = qnc_player_input::ProjectAudio {
            channels: 4,
            sample_rate_hz: 48000,
        };
        assert!(
            project_audio_layout(
                &[(1, 1), (2, 1)],
                Some(&AudioFormat::new(48000, 2).unwrap()),
                &saved_channels(&[(1, 1), (2, 1)]),
                &project,
            )
            .is_err()
        );
        project.channels = 2;
        assert!(
            project_audio_layout(
                &[(1, 1), (2, 1)],
                Some(&AudioFormat::new(44100, 2).unwrap()),
                &saved_channels(&[(1, 1), (2, 1)]),
                &project,
            )
            .is_err()
        );
        let (streams, format, map) = project_audio_layout(&[], None, &[], &project).unwrap();
        assert!(streams.is_empty());
        assert_eq!(format, None);
        assert_eq!(map, None);
        project.channels = 0;
        assert!(project_audio_layout(&[], None, &[], &project).is_err());
    }
    fn fact<T>(value: T) -> Option<qnc_media_metadata::Fact<T>> {
        Some(qnc_media_metadata::Fact {
            value,
            evidence_id: "saved".into(),
            locator: "/stream".into(),
        })
    }
    fn audio_media() -> MediaRepresentation {
        MediaRepresentation {
            media_uri: "qnc://local/source/card/file/original".into(),
            container: None,
            duration_seconds: None,
            streams_complete: fact(true),
            tags: Default::default(),
            streams: (1..=4)
                .rev()
                .map(|index| qnc_media_metadata::MediaStream {
                    index: fact(index),
                    codec: fact(Signal::Known("pcm_s24le".into())),
                    profile: None,
                    start_pts: fact(0),
                    time_base: fact(Rational {
                        numerator: 1,
                        denominator: 48000,
                    }),
                    duration_ts: None,
                    details: StreamDetails::Audio(Box::new(qnc_media_metadata::AudioMetadata {
                        sample_rate_hz: fact(48000),
                        channels: fact(1),
                        sample_format: fact(Signal::Known("s32".into())),
                        channel_layout: fact(Signal::Unspecified),
                        bits_per_sample: fact(24),
                    })),
                })
                .collect(),
        }
    }
    #[test]
    fn four_mono_streams_keep_saved_stream_order_and_all_channels() {
        let media = audio_media();
        let before = media.clone();
        let (streams, format) = native_audio_layout(&media).unwrap();
        assert_eq!(streams, [(1, 1), (2, 1), (3, 1), (4, 1)]);
        assert_eq!(format.unwrap(), AudioFormat::new(48000, 4).unwrap());
        assert_eq!(media, before);
    }
    #[test]
    fn four_mono_project_two_decodes_only_first_two_streams() {
        let media = audio_media();
        let (native_streams, native_format) = native_audio_layout(&media).unwrap();
        let project = qnc_player_input::ProjectAudio {
            channels: 2,
            sample_rate_hz: 48000,
        };
        let (streams, format, map) = project_audio_layout(
            &native_streams,
            native_format.as_ref(),
            &saved_channels(&native_streams),
            &project,
        )
        .unwrap();
        let format = format.unwrap();
        let map = map.unwrap();
        assert_eq!(streams, [audio_plan(1, 1, &[0]), audio_plan(2, 1, &[0])]);
        assert_eq!(format, AudioFormat::new(48000, 2).unwrap());
        assert_eq!(map.output_channels(), &[0, 1]);
    }
    #[test]
    fn one_multichannel_stream_still_exports_a1_a2_as_two_mono_lanes() {
        let native = AudioFormat::new(48000, 4).unwrap();
        let streams = [(1, 4)];
        let project = qnc_player_input::ProjectAudio {
            channels: 2,
            sample_rate_hz: 48000,
        };
        let (plans, format, map) =
            project_audio_layout(&streams, Some(&native), &saved_channels(&streams), &project)
                .unwrap();
        assert_eq!(plans, [audio_plan(1, 4, &[0, 1])]);
        assert_eq!(format.unwrap(), AudioFormat::new(48000, 2).unwrap());
        assert_eq!(map.unwrap().output_channels(), &[0, 1]);
    }
    #[test]
    fn mismatched_rates_and_duplicate_streams_are_not_silently_mixed() {
        let mut media = audio_media();
        if let StreamDetails::Audio(audio) = &mut media.streams[0].details {
            audio.sample_rate_hz = fact(44100);
        }
        assert!(native_audio_layout(&media).is_err());
        let mut media = audio_media();
        media.streams[0].index = fact(1);
        assert!(native_audio_layout(&media).is_err());
    }
    #[test]
    fn saved_absence_of_rotation_is_not_missing_metadata() {
        let sar = Rational {
            numerator: 64,
            denominator: 64,
        };
        assert!(require_native_geometry(sar, Some(&Signal::Unspecified)).is_ok());
        assert!(require_native_geometry(sar, Some(&Signal::Known(0))).is_ok());
        assert!(require_native_geometry(sar, None).is_err());
        assert!(require_native_geometry(sar, Some(&Signal::Known(90))).is_err());
        assert!(
            require_native_geometry(
                Rational {
                    numerator: 4,
                    denominator: 3
                },
                Some(&Signal::Known(0))
            )
            .is_err()
        );
    }
    #[test]
    fn fractional_fps_audio_slices_do_not_accumulate_rounding() {
        let tb = Timebase::new(30000, 1001).unwrap();
        let mut count = 0;
        for f in 0..30000 {
            count +=
                sample_boundary(f + 1, tb, 48000).unwrap() - sample_boundary(f, tb, 48000).unwrap();
        }
        assert_eq!(count, 48000 * 1001);
    }
    #[test]
    fn source_pts_not_decode_ordinal_identifies_frame_after_seek() {
        let tb = Rational {
            numerator: 1,
            denominator: 50000,
        };
        assert_eq!(relative_position(241000, tb, (0, tb), 50, 1).unwrap(), 241);
        assert!(relative_position(241001, tb, (0, tb), 50, 1).is_err());
        assert!(relative_position(-1, tb, (0, tb), 50, 1).is_err());
        assert_eq!(
            relative_position(243000, tb, (2000, tb), 50, 1).unwrap(),
            241
        );
    }
}
