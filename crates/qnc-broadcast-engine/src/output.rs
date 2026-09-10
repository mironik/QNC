use crate::*;
use qnc_audio_output::{AudioOutput, ChannelMap, Config, Format};
use qnc_media_metadata::Rational;
use std::{collections::VecDeque, sync::Arc};

const MAX_MONITOR_PENDING: usize = 8;

pub(crate) struct VideoSink {
    pub output: Option<VideoOutput>,
    pub config: OutputConfig,
    pub images: BTreeMap<u64, Picture>,
    pub sequence: u64,
    pub inflight: bool,
    pub submit_us: u128,
    pub conversion_us: u128,
    pub upload_us: u128,
    pub converted: u64,
    pub monitor: Option<(FrameHeader, Arc<[u8]>)>,
    pub(crate) monitor_pending: VecDeque<(FrameHeader, Arc<[u8]>)>,
}
pub(crate) type SharedVideo = Rc<RefCell<VideoSink>>;
impl VideoSink {
    pub fn push_monitor(&mut self, header: FrameHeader, rgba: Arc<[u8]>) {
        let frame = (header, rgba);
        self.monitor = Some(frame.clone());
        self.monitor_pending.push_back(frame);
        while self.monitor_pending.len() > MAX_MONITOR_PENDING {
            self.monitor_pending.pop_front();
        }
    }

    pub fn clear_monitor(&mut self) {
        self.monitor = None;
        self.monitor_pending.clear();
    }

    pub fn take_monitor_pending(&mut self) -> Vec<(FrameHeader, Arc<[u8]>)> {
        self.monitor_pending.drain(..).collect()
    }

    pub fn poll_and_collect(&mut self) -> Result<()> {
        if let Some(output) = &mut self.output
            && output.poll().map_err(error)?.is_some()
        {
            self.inflight = false;
        }
        if self.inflight {
            return Ok(());
        }
        let expired: Vec<_> = self
            .images
            .iter()
            .filter(|(_, image)| Rc::strong_count(image) == 1)
            .map(|(f, _)| *f)
            .collect();
        for frame in expired {
            let image = self.images.get(&frame).expect("retained image");
            if let Some(output) = &mut self.output {
                match output.release(
                    image
                        .token
                        .as_ref()
                        .ok_or_else(|| error("missing native frame"))?,
                ) {
                    Ok(()) => (),
                    Err(qnc_video_output::OutputError::Busy) => continue,
                    Err(e) => return Err(error(e)),
                }
            }
            self.images.remove(&frame);
        }
        Ok(())
    }
}
pub(crate) struct Presenter(pub SharedVideo);
impl FramePresenter for Presenter {
    type VideoFrame = Picture;
    fn prepare_presentation(&mut self, _: &EngineSourceHandle) -> Result<Vec<BroadcastEvent>> {
        Ok(Vec::new())
    }
    fn prepare_start_frame(&mut self, frame: &DecodedVideoFrame<Picture>) -> Result<bool> {
        let sink = self.0.borrow();
        match (&sink.output, &frame.payload.token) {
            (Some(output), Some(token)) => output.is_ready(token).map_err(error),
            (None, None) => Ok(true),
            _ => Err(error("frame output mismatch")),
        }
    }
    fn present_frame(&mut self, frame: DecodedVideoFrame<Picture>) -> Result<Vec<BroadcastEvent>> {
        let mut gpu = self.0.borrow_mut();
        let start = Instant::now();
        match (&mut gpu.output, &frame.payload.token) {
            (Some(output), Some(token)) => {
                let submitted = output.submit(token).map_err(error)?;
                if submitted.frame.frame_number != frame.frame {
                    return Err(error("expected matching picture submission"));
                }
                gpu.inflight = true;
            }
            (None, None) => (),
            _ => return Err(error("frame output mismatch")),
        }
        gpu.submit_us = start.elapsed().as_micros();
        gpu.push_monitor(frame.payload.header.clone(), frame.payload.rgba.clone());
        Ok(vec![BroadcastEvent::VideoFrameSubmitted {
            frame: frame.frame,
        }])
    }
}

pub(crate) struct Audio {
    pub device: Option<Rc<RefCell<AudioOutput>>>,
    decoders: Vec<Decoder>,
    input: Rc<DecodeInput>,
    pending_seek: Option<u64>,
    tracks: Vec<PcmTrack>,
    source: SourceRuntime,
    origin: (i64, Rational),
    media_uri: String,
    consumed_through: u64,
    generation: Option<u64>,
    channel_map: Option<ChannelMap>,
    routed: Vec<f32>,
}
impl Audio {
    pub fn open(
        plan: &InputPlan,
        input: Rc<DecodeInput>,
        device_id: Option<String>,
        channel_map: Option<ChannelMap>,
        prebuffer_frames: usize,
    ) -> Result<Self> {
        validate_channel_map(plan.source.audio_format.as_ref(), channel_map.as_ref())?;
        let device = plan
            .source
            .audio_format
            .as_ref()
            .map(|format| {
                let ready = sample_boundary(
                    plan.source.duration_frames.min(prebuffer_frames as u64),
                    plan.source.timebase,
                    format.sample_rate_hz,
                )?;
                AudioOutput::open(Config {
                    version: qnc_audio_output::VERSION.into(),
                    format: Format {
                        sample_rate_hz: format.sample_rate_hz,
                        channels: channel_map
                            .as_ref()
                            .expect("validated map")
                            .output_channels()
                            .len() as u16,
                    },
                    device_id,
                    capacity_frames: format.sample_rate_hz,
                    ready_frames: u32::try_from(ready).map_err(error)?,
                })
                .map(|output| Rc::new(RefCell::new(output)))
                .map_err(error)
            })
            .transpose()?;
        let decoders = plan
            .audio_streams
            .iter()
            .map(|(index, _)| input.open(*index, None))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            device,
            decoders,
            input,
            pending_seek: None,
            tracks: plan
                .audio_streams
                .iter()
                .map(|(index, channels)| PcmTrack::new(*index, *channels))
                .collect(),
            source: plan.source.clone(),
            origin: plan.audio_origin,
            media_uri: plan.audio_media.media_uri.clone(),
            consumed_through: 0,
            generation: None,
            channel_map,
            routed: Vec::new(),
        })
    }
}
impl AudioOutputAdapter for Audio {
    type AudioPacket = Arc<[f32]>;
    fn cue_audio(&mut self, request: EngineFrameRequest) -> Result<()> {
        if request.source_id != self.source.source_id || request.timebase != self.source.timebase {
            return Err(error("audio seek source mismatch"));
        }
        seek_start(&self.source, request.frame)?;
        self.pending_seek = Some(request.frame);
        Ok(())
    }
    fn prepare_audio(&mut self, _: &EngineSourceHandle) -> Result<Vec<BroadcastEvent>> {
        Ok(Vec::new())
    }
    fn render_audio_for_frame(
        &mut self,
        request: EngineFrameRequest,
    ) -> Result<AudioFramePacket<Arc<[f32]>>> {
        let format = self
            .source
            .audio_format
            .as_ref()
            .ok_or_else(|| error("audio not declared"))?;
        let start = sample_boundary(request.frame, request.timebase, format.sample_rate_hz)?;
        let end = sample_boundary(
            request
                .frame
                .checked_add(1)
                .ok_or_else(|| error("frame overflow"))?,
            request.timebase,
            format.sample_rate_hz,
        )?;
        if let Some(frame) = self.pending_seek {
            if request.frame != frame {
                return Err(error("audio seek target mismatch"));
            }
            let seek = seek_start(&self.source, frame)?;
            self.decoders.clear();
            for track in &mut self.tracks {
                track.samples.clear();
                track.decoded_through = None;
                track.discard_before = start;
                self.decoders
                    .push(self.input.open(track.stream_index, seek)?);
            }
            self.consumed_through = start;
            self.pending_seek = None;
        }
        if request.source_id != self.source.source_id
            || start != self.consumed_through
            || request.timebase != self.source.timebase
        {
            return Err(error(
                "audio request requires explicit decoder repositioning",
            ));
        }
        let sample_frames = usize::try_from(end - start).map_err(error)?;
        // Poll each existing decoder fairly; drain nothing until all channels are ready.
        for (track, decoder) in self.tracks.iter_mut().zip(&mut self.decoders) {
            let needed = sample_frames * usize::from(track.channels);
            for _ in 0..2 {
                if track.samples.len() >= needed {
                    break;
                }
                match decoder.try_next_packet().map_err(error)? {
                    Poll::Pending => break,
                    Poll::Ready(None) => {
                        return Err(error("audio ended before saved video boundary"));
                    }
                    Poll::Ready(Some(packet)) => {
                        track.push(packet, &self.media_uri, self.origin, format.sample_rate_hz)?
                    }
                }
            }
        }
        let samples = interleave_tracks(&mut self.tracks, sample_frames)?;
        self.consumed_through = end;
        Ok(AudioFramePacket {
            source_id: request.source_id,
            start_frame: request.frame,
            frame_count: 1,
            audio_format: Some(format.clone()),
            payload: samples.into(),
        })
    }
    fn submit_audio_packet(
        &mut self,
        packet: AudioFramePacket<Arc<[f32]>>,
    ) -> Result<Vec<BroadcastEvent>> {
        let format = self
            .source
            .audio_format
            .as_ref()
            .ok_or_else(|| error("missing audio format"))?;
        let start = sample_boundary(
            packet.start_frame,
            self.source.timebase,
            format.sample_rate_hz,
        )?;
        let mut device = self
            .device
            .as_ref()
            .ok_or_else(|| error("missing audio device"))?
            .borrow_mut();
        let generation = match self.generation {
            Some(generation) => generation,
            None => {
                let generation = device.begin(start).map_err(error)?;
                self.generation = Some(generation);
                generation
            }
        };
        self.channel_map
            .as_ref()
            .ok_or_else(|| error("missing device channel map"))?
            .route(&packet.payload, &mut self.routed)
            .map_err(error)?;
        device
            .queue(generation, start, &self.routed)
            .map_err(error)?;
        if packet.start_frame + 1 == self.source.duration_frames {
            device.finish(generation).map_err(error)?;
        }
        Ok(Vec::new())
    }
    fn begin_audio_preroll(&mut self) -> Result<Vec<BroadcastEvent>> {
        self.generation = None;
        Ok(Vec::new())
    }
    fn commit_audio_preroll(&mut self) -> Result<Vec<BroadcastEvent>> {
        self.device
            .as_ref()
            .ok_or_else(|| error("missing audio device"))?
            .borrow_mut()
            .commit(self.generation.ok_or_else(|| error("audio not queued"))?)
            .map_err(error)?;
        Ok(Vec::new())
    }
    fn start_audio(&mut self) -> Result<Vec<BroadcastEvent>> {
        self.device
            .as_ref()
            .ok_or_else(|| error("missing audio device"))?
            .borrow_mut()
            .start(self.generation.ok_or_else(|| error("audio not ready"))?)
            .map_err(error)?;
        Ok(Vec::new())
    }
    fn pause_audio(&mut self) -> Result<Vec<BroadcastEvent>> {
        if let Some(device) = &self.device {
            device.borrow_mut().pause().map_err(error)?;
        }
        self.generation = None;
        Ok(Vec::new())
    }
    fn stop_audio(&mut self) -> Result<Vec<BroadcastEvent>> {
        self.pause_audio()
    }
}

pub(crate) fn validate_channel_map(
    format: Option<&AudioFormat>,
    map: Option<&ChannelMap>,
) -> Result<()> {
    match (format, map) {
        (None, None) => Ok(()),
        (Some(format), Some(map)) => {
            map.validate().map_err(error)?;
            if map.source_channels() != format.channel_count {
                return Err(error(
                    "device channel map differs from decoded channel inventory",
                ));
            }
            Ok(())
        }
        _ => Err(error("audio requires an explicit device channel map")),
    }
}

struct PcmTrack {
    stream_index: u32,
    channels: u16,
    samples: VecDeque<f32>,
    decoded_through: Option<u64>,
    discard_before: u64,
}
impl PcmTrack {
    fn new(stream_index: u32, channels: u16) -> Self {
        Self {
            stream_index,
            channels,
            samples: VecDeque::new(),
            decoded_through: Some(0),
            discard_before: 0,
        }
    }
    fn push(
        &mut self,
        packet: qnc_media_decode::DecodedPacket,
        uri: &str,
        origin: (i64, Rational),
        rate: u32,
    ) -> Result<()> {
        let channels = usize::from(self.channels);
        let position = relative_position(packet.pts, packet.time_base, origin, rate.into(), 1)?;
        if self
            .decoded_through
            .map_or(position > self.discard_before, |end| position != end)
            || packet.stream_index != self.stream_index
            || packet.media_uri != uri
            || packet.format
                != (DecodedFormat::Audio {
                    sample_rate_hz: rate,
                    channels: self.channels.into(),
                    sample_format: "f32le".into(),
                })
            || packet.bytes.is_empty()
            || !packet.bytes.len().is_multiple_of(channels * 4)
        {
            return Err(error("PCM timestamp or layout differs from saved input"));
        }
        let count = (packet.bytes.len() / (channels * 4)) as u64;
        let skip = self.discard_before.saturating_sub(position).min(count) as usize * channels * 4;
        if self.samples.len() + (packet.bytes.len() - skip) / 4 > rate as usize * channels * 2 {
            return Err(error("PCM slice buffer limit exceeded"));
        }
        self.decoded_through = Some(
            position
                .checked_add(count)
                .ok_or_else(|| error("PCM position overflow"))?,
        );
        self.samples.extend(
            packet.bytes[skip..]
                .chunks_exact(4)
                .map(|v| f32::from_le_bytes(v.try_into().expect("sample"))),
        );
        Ok(())
    }
}

fn interleave_tracks(tracks: &mut [PcmTrack], frames: usize) -> Result<Vec<f32>> {
    if tracks
        .iter()
        .any(|track| track.samples.len() < frames * usize::from(track.channels))
    {
        return Err(pending());
    }
    let channels: usize = tracks.iter().map(|t| usize::from(t.channels)).sum();
    let mut samples = Vec::with_capacity(frames * channels);
    for _ in 0..frames {
        for track in tracks.iter_mut() {
            samples.extend(track.samples.drain(..usize::from(track.channels)));
        }
    }
    Ok(samples)
}

#[cfg(test)]
#[path = "audio_live_test.rs"]
mod audio_live_test;

#[cfg(test)]
mod tests {
    use super::*;
    fn pcm(pts: i64, values: &[f32]) -> qnc_media_decode::DecodedPacket {
        qnc_media_decode::DecodedPacket {
            version: qnc_media_decode::VERSION.into(),
            media_uri: "saved".into(),
            stream_index: 1,
            ordinal: 0,
            pts,
            time_base: Rational {
                numerator: 1,
                denominator: 48000,
            },
            format: DecodedFormat::Audio {
                sample_rate_hz: 48000,
                channels: 1,
                sample_format: "f32le".into(),
            },
            bytes: values.iter().flat_map(|v| v.to_le_bytes()).collect(),
        }
    }
    #[test]
    fn seek_trims_pcm_by_sample_position_without_rounding_or_relabelling() {
        let mut track = PcmTrack::new(1, 1);
        track.decoded_through = None;
        track.discard_before = 5;
        let origin = (
            0,
            Rational {
                numerator: 1,
                denominator: 48000,
            },
        );
        track
            .push(pcm(1, &[1., 2., 3.]), "saved", origin, 48000)
            .unwrap();
        assert!(track.samples.is_empty());
        track
            .push(pcm(4, &[4., 5., 6.]), "saved", origin, 48000)
            .unwrap();
        assert_eq!(track.samples, [5., 6.]);
        assert_eq!(track.decoded_through, Some(7));
        assert!(track.push(pcm(8, &[8.]), "saved", origin, 48000).is_err());
        let mut missing = PcmTrack::new(1, 1);
        missing.decoded_through = None;
        missing.discard_before = 5;
        assert!(missing.push(pcm(6, &[6.]), "saved", origin, 48000).is_err());
    }
    #[test]
    fn pending_track_does_not_consume_other_channels() {
        let mut tracks: Vec<_> = (1..=4).map(|i| PcmTrack::new(i, 1)).collect();
        for (i, track) in tracks.iter_mut().enumerate().take(3) {
            track.samples.extend([i as f32, i as f32 + 10.0]);
        }
        assert_eq!(
            interleave_tracks(&mut tracks, 2).unwrap_err().kind,
            BroadcastEngineErrorKind::NotReady
        );
        assert_eq!(tracks[0].samples.len(), 2);
        tracks[3].samples.extend([3.0, 13.0, 23.0]);
        assert_eq!(
            interleave_tracks(&mut tracks, 2).unwrap(),
            [0., 1., 2., 3., 10., 11., 12., 13.]
        );
        assert_eq!(tracks[3].samples, [23.0]);
    }
    #[test]
    fn device_routing_is_explicit_and_cannot_change_source_format() {
        let format = AudioFormat::new(48000, 4).unwrap();
        assert!(validate_channel_map(Some(&format), None).is_err());
        assert!(
            validate_channel_map(
                Some(&format),
                Some(&ChannelMap::new(2, vec![0, 1]).unwrap())
            )
            .is_err()
        );
        assert!(
            validate_channel_map(
                Some(&format),
                Some(&ChannelMap::new(4, vec![2, 3]).unwrap())
            )
            .is_ok()
        );
        assert_eq!(format.channel_count, 4);
    }
}
