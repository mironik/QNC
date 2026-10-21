//! A story program played as one virtual source (v5
//! `qnc-player-runtime/program_playlist.rs`).
//!
//! The transport, clock, cue, Play and Pause stay the ones of a single clip: the
//! program is one source on the program frame axis. The picture adapter decodes
//! program frames in order across cuts (the prefetch reaches the next clip before
//! the cut, so its decoder starts early); an item without picture (Off) is black.
//! The sound adapter builds every program frame from the channel of each bus
//! (A1, A2) and keeps the other outputs silent. No probe, no database, no new
//! decoder choice: the clips are the saved inputs of `ProgramInput`.

use crate::decode_input::{DecodeInput, seek_start};
use crate::input::{AudioStreamPlan, InputPlan, native_audio_layout, preview_raster_bounds};
use crate::output::{DeviceSink, PcmTrack, SharedVideo, device_sink_adapter};
use crate::*;
use crate::decode_input::Opening;
use qnc_player_input::ProgramInput;
use std::{collections::BTreeMap, sync::Arc};

/// A program item's picture: which clip, from which source frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VideoSpan {
    record_in: u64,
    record_out: u64,
    video: Option<(usize, u64)>,
}

/// One program output channel fed from one channel of a clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bus {
    output: u16,
    clip: usize,
    source_in: u64,
    stream_index: u32,
    channel_index: u16,
    stream_channels: u16,
    /// The level of the lane in dB, 0 as recorded (user 2026-10-16); the lane's samples
    /// are taken as decoded, the level is applied where they go to the output.
    level_db: i8,
}

impl Bus {
    fn gain(&self) -> f32 {
        10f32.powf(f32::from(self.level_db) / 20.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AudioSpan {
    record_in: u64,
    record_out: u64,
    buses: Vec<Bus>,
}

/// The program checked against the saved clips: its virtual source, the clip
/// plans and, per item, the picture and the sound buses.
pub struct ProgramPlan {
    pub(crate) source: SourceRuntime,
    pub(crate) clips: Vec<InputPlan>,
    video: Vec<VideoSpan>,
    audio: Vec<AudioSpan>,
    channels: u16,
}

impl ProgramPlan {
    pub fn new(input: &ProgramInput) -> Result<Self> {
        input.validate_for(&input.workspace_db_uri).map_err(error)?;
        let ids: Vec<&str> = input.clips.keys().map(String::as_str).collect();
        let clips = input
            .clips
            .iter()
            .map(|(id, clip)| {
                InputPlan::for_program(clip, &input.workspace_db_uri, id, input.playlist.program_timebase)
            })
            .collect::<Result<Vec<_>>>()?;
        let playlist = &input.playlist;
        let timebase = Timebase::new(
            playlist.program_timebase.fps_num,
            playlist.program_timebase.fps_den,
        )
        .map_err(error)?;
        let frame = |value: i64| u64::try_from(value).map_err(error);
        let channels = input.project_audio.channels;
        let rate = input.project_audio.sample_rate_hz;
        let (video, audio) = program_spans(
            playlist,
            &ids,
            |clip_id| {
                let prepared = &input.clips[clip_id];
                let (streams, format) = native_audio_layout(prepared.audio_media())?;
                Ok(ClipSound {
                    channels: prepared.layout.audio_channels.clone(),
                    streams,
                    rate: format.map(|format| format.sample_rate_hz),
                })
            },
            rate,
        )?;
        let first_picture = video
            .iter()
            .find_map(|span| span.video.map(|(clip, _)| clip))
            .unwrap_or(0);
        let picture_format = clips
            .get(first_picture)
            .and_then(|plan| plan.source.video_format.clone())
            .ok_or_else(|| error("program has no saved picture format"))?;
        let source = SourceRuntime::new(
            playlist.playlist_id.as_str(),
            frame(playlist.duration_frames)?,
            timebase,
        )
        .map_err(error)?
        .with_video_format(picture_format)
        .with_audio_format(AudioFormat::new(rate, channels).map_err(error)?);
        source.validate().map_err(error)?;
        let (clips, video, audio) = with_prerendered(input, clips, video, audio, channels)?;
        Ok(Self {
            source,
            clips,
            video,
            audio,
            channels,
        })
    }

    pub fn source(&self) -> &SourceRuntime {
        &self.source
    }

    /// Output pool of the picture the program opens with.
    pub fn output_config(&self, session_id: &str, generation: u64) -> Result<OutputConfig> {
        let first = self
            .video
            .iter()
            .find_map(|span| span.video.map(|(clip, _)| clip))
            .unwrap_or(0);
        self.clips
            .get(first)
            .ok_or_else(|| error("program has no clip"))?
            .output_config(session_id, generation)
    }
}

/// The story's parts whose rendered unit is here play from that one file (user
/// 2026-10-12: one stream from the project disk instead of the camera originals of a cut
/// on the card): the plan's spans are cut at the units of the story, as its prerender
/// fingerprints them, and a part with its unit takes the unit's picture and sound; the
/// others keep their sources.
fn with_prerendered(
    input: &ProgramInput,
    mut clips: Vec<InputPlan>,
    video: Vec<VideoSpan>,
    audio: Vec<AudioSpan>,
    channels: u16,
) -> Result<(Vec<InputPlan>, Vec<VideoSpan>, Vec<AudioSpan>)> {
    if input.prerendered.is_empty() {
        return Ok((clips, video, audio));
    }
    let units = ProgramPlan::new(&original_pictures(input)?)?.render_units()?;
    let mut keys = Vec::new();
    for unit in &input.prerendered {
        if !keys.contains(&&unit.key) {
            keys.push(&unit.key);
        }
    }
    let rate = input.playlist.program_timebase;
    let (mut cut_video, mut cut_audio) = (Vec::new(), Vec::new());
    for unit in &units {
        let (from, to) = (unit.record_in, unit.record_out);
        let rendered = keys.iter().find_map(|key| {
            let fingerprint = unit.fingerprint(key);
            input
                .prerendered
                .iter()
                .find(|done| done.fingerprint == fingerprint && done.frames == to - from)
        });
        if let Some(done) = rendered {
            let stream = done
                .media
                .streams
                .iter()
                .find(|s| matches!(s.details, qnc_media_metadata::StreamDetails::Audio(_)))
                .ok_or_else(|| error("rendered unit has no sound"))?;
            let qnc_media_metadata::StreamDetails::Audio(sound) = &stream.details else {
                return Err(error("rendered unit has no sound"));
            };
            let stream_index = stream.index.as_ref().ok_or_else(|| error("rendered unit has no stream index"))?.value;
            let stream_channels = sound
                .channels
                .as_ref()
                .and_then(|c| u16::try_from(c.value).ok())
                .ok_or_else(|| error("rendered unit has no channel count"))?;
            if stream_channels < channels {
                return Err(error("rendered unit has fewer channels than the program"));
            }
            let index = clips.len();
            clips.push(InputPlan::for_unit(&done.media, &format!("prerender-{}", done.fingerprint), done.frames, rate)?);
            cut_video.push(VideoSpan { record_in: from, record_out: to, video: Some((index, 0)) });
            cut_audio.push(AudioSpan {
                record_in: from,
                record_out: to,
                buses: (0..channels)
                    .map(|output| Bus { output, clip: index, source_in: 0, stream_index, channel_index: output, stream_channels, level_db: 0 }) // the level is in the rendered unit
                    .collect(),
            });
            continue;
        }
        let piece = |span_in: u64| from - span_in;
        let picture = video
            .iter()
            .find(|span| span.record_in <= from && from < span.record_out)
            .ok_or_else(|| error("program part without a picture span"))?;
        cut_video.push(VideoSpan {
            record_in: from,
            record_out: to,
            video: picture.video.map(|(clip, source_in)| (clip, source_in + piece(picture.record_in))),
        });
        let sound = audio
            .iter()
            .find(|span| span.record_in <= from && from < span.record_out)
            .ok_or_else(|| error("program part without a sound span"))?;
        cut_audio.push(AudioSpan {
            record_in: from,
            record_out: to,
            buses: sound
                .buses
                .iter()
                .map(|bus| Bus { source_in: bus.source_in + piece(sound.record_in), ..*bus })
                .collect(),
        });
    }
    Ok((clips, cut_video, cut_audio))
}

/// The story with every picture from the clip's original (the prerender's view of it):
/// the clips' representation and the playlist's picture media both name the original.
fn original_pictures(input: &ProgramInput) -> Result<ProgramInput> {
    let mut input = input.clone();
    input.prerendered.clear();
    for clip in input.clips.values_mut() {
        // A project playing the proxy: its clips were prepared for the proxy (log
        // 2026-10-12: the prerender of such a project stopped on InvalidDescriptor).
        *clip = clip.clone().with_original_picture().map_err(error)?;
    }
    for source in input.playlist.items.iter_mut().flat_map(|item| item.sources.iter_mut()) {
        if source.has_video() {
            if let Some(clip) = input.clips.get(&source.clip_id) {
                source.media.media_uri = clip.audio_media().media_uri.clone();
            }
        }
    }
    Ok(input)
}

impl ProgramPlan {
    /// The plan of the story's prerender (user 2026-10-12): the same cuts and sound as the
    /// plan that plays, every picture from the clip's camera original (the delivery is made
    /// from the original whatever the project plays).
    pub fn for_render(input: &ProgramInput) -> Result<Self> {
        Self::new(&original_pictures(input)?)
    }

    /// The story as prerender units: a unit between every two cuts of picture or sound,
    /// each described as this plan plays it (the decoder's seek start, the origin and the
    /// rate that place each picture and sample).
    pub fn render_units(&self) -> Result<Vec<qnc_render_unit::RenderUnit>> {
        let rate = self
            .source
            .audio_format
            .as_ref()
            .ok_or_else(|| error("program has no audio format"))?
            .sample_rate_hz;
        let program = self.source.timebase;
        let mut cuts: Vec<u64> = self
            .video
            .iter()
            .flat_map(|span| [span.record_in, span.record_out])
            .chain(self.audio.iter().flat_map(|span| [span.record_in, span.record_out]))
            .collect();
        cuts.sort_unstable();
        cuts.dedup();
        let mut units = Vec::new();
        for pair in cuts.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            if from >= to {
                continue;
            }
            let video = match self.video.iter().find(|span| span.record_in <= from && from < span.record_out) {
                Some(VideoSpan { record_in, video: Some((clip, source_in)), .. }) => {
                    let plan = &self.clips[*clip];
                    let first = source_in + (from - record_in);
                    Some(qnc_render_unit::VideoPart {
                        media: plan.media.clone(),
                        stream_index: plan.video_index,
                        start: seek_start(&plan.source, first)?,
                        first_frame: first,
                        origin: plan.origin,
                        frame_rate: (plan.source.timebase.fps_num, plan.source.timebase.fps_den),
                        progressive: plan.progressive,
                        delivered_rate: plan.delivered_rate,
                    })
                }
                _ => None,
            };
            let audio = match self.audio.iter().find(|span| span.record_in <= from && from < span.record_out) {
                Some(span) => span
                    .buses
                    .iter()
                    .map(|bus| {
                        let plan = &self.clips[bus.clip];
                        let first = bus.source_in + (from - span.record_in);
                        Ok(qnc_render_unit::AudioPart {
                            output: bus.output,
                            media: plan.audio_media.clone(),
                            stream_index: bus.stream_index,
                            channel_index: bus.channel_index,
                            stream_channels: bus.stream_channels,
                            start: seek_start(&plan.source, first)?,
                            first_sample: sample_boundary(first, plan.source.timebase, rate)?,
                            origin: plan.audio_origin,
                            level_db: bus.level_db,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
                None => Vec::new(),
            };
            units.push(qnc_render_unit::RenderUnit {
                record_in: from,
                record_out: to,
                program_rate: qnc_media_metadata::FrameTimebase { fps_num: program.fps_num, fps_den: program.fps_den },
                sample_rate: rate,
                channels: self.channels,
                samples: sample_boundary(to, program, rate)? - sample_boundary(from, program, rate)?,
                video,
                audio,
            });
        }
        Ok(units)
    }
}

struct Worker {
    spec: qnc_pixel_convert::ConversionSpec,
    conversion: conversion::ConversionWorker,
    raster: [u32; 2],
    rgba: Vec<Option<Arc<[u8]>>>,
}

struct ActiveDecoder {
    clip: usize,
    /// Opened on a thread of its own (`DecodeInput::open_in_background`).
    decoder: Opening,
    next_source: u64,
    discard_before: Option<u64>,
}

/// Program picture: one decode sequence in program frames.
pub(crate) struct ProgramVideo {
    source: SourceRuntime,
    spans: Vec<VideoSpan>,
    clips: Vec<(InputPlan, Rc<DecodeInput>, usize)>,
    workers: Vec<Worker>,
    active: Option<ActiveDecoder>,
    /// The decoder of the next cut to another picture, opened ahead of it.
    upcoming: Option<ActiveDecoder>,
    next_decode_frame: u64,
    ready: BTreeMap<u64, DecodedVideoFrame<Picture>>,
    pending_seek: Option<u64>,
    /// The frame of the last cue: pictures up to `STEP_BACK_FRAMES` behind it stay, so a
    /// frame step needs no new decoder (user 2026-10-10: a step in Wrap took 0.5-1.5 s).
    cue_anchor: u64,
    /// Every frame from here up to `next_decode_frame` is in `ready` or in conversion.
    kept_from: u64,
    prefetch_frames: usize,
    gpu: SharedVideo,
    /// The pictures before where the program stands, made in the background.
    back: Option<Rc<RefCell<crate::backfill::BackFill>>>,
}

impl ProgramVideo {
    pub fn open(
        plan: &ProgramPlan,
        inputs: &[Rc<DecodeInput>],
        gpu: SharedVideo,
        prefetch_frames: usize,
    ) -> Result<Self> {
        let mut workers: Vec<Worker> = Vec::new();
        let mut clips = Vec::new();
        for (clip, input) in plan.clips.iter().zip(inputs) {
            let worker = match workers.iter().position(|w| w.spec == clip.spec) {
                Some(index) => index,
                None => {
                    let converter = conversion::Raster::Gpu(
                        qnc_gpu_raster::GpuRasterConverter::prepare(
                            clip.spec.clone(),
                            preview_raster_bounds(clip.spec.width, clip.spec.height),
                        )
                        .map_err(error)?,
                    );
                    let raster = converter.size();
                    // Room for the prefetch, the pictures in conversion and the ones the
                    // presenter and monitor still hold: with only prefetch + 4 the decoder
                    // waited for every presented picture and ran in real time, so a cut
                    // left a gap of half a second (player acceptance 2026-10-02).
                    let rgba = (0..program_picture_buffers(prefetch_frames))
                        .map(|_| Some(Arc::from(vec![0; converter.output_bytes()])))
                        .collect();
                    workers.push(Worker {
                        spec: clip.spec.clone(),
                        conversion: conversion::ConversionWorker::with_capacity(
                            converter,
                            input::CONVERT_IN_FLIGHT.min(prefetch_frames),
                        )
                        .map_err(error)?,
                        raster,
                        rgba,
                    });
                    workers.len() - 1
                }
            };
            clips.push((clip.clone(), input.clone(), worker));
        }
        // The pieces of the program for the background: each picture item from its clip,
        // an Off item without a picture.
        let pieces = plan
            .video
            .iter()
            .map(|span| crate::backfill::Piece {
                record: (span.record_in, span.record_out),
                picture: span.video.map(|(clip, source_in)| (plan.clips[clip].clone(), inputs[clip].clone(), source_in)),
            })
            .collect();
        let per_second = plan.source.timebase.fps_num as f64 / plan.source.timebase.fps_den.max(1) as f64;
        let back = Rc::new(RefCell::new(crate::backfill::BackFill::new(pieces, per_second)));
        gpu.borrow_mut().back = Some(back.clone());
        Ok(Self {
            source: plan.source.clone(),
            spans: plan.video.clone(),
            clips,
            workers,
            active: None,
            upcoming: None,
            next_decode_frame: 0,
            ready: BTreeMap::new(),
            pending_seek: None,
            cue_anchor: 0,
            kept_from: 0,
            prefetch_frames,
            gpu,
            back: Some(back),
        })
    }

    fn span_at(&self, frame: u64) -> Result<VideoSpan> {
        self.spans
            .iter()
            .find(|span| (span.record_in..span.record_out).contains(&frame))
            .copied()
            .ok_or_else(|| error("program frame outside its items"))
    }

    fn header(&mut self, frame: u64, raster: [u32; 2], generation: u64) -> Result<FrameHeader> {
        let gpu = self.gpu.borrow();
        let header = FrameHeader {
            version: qnc_video_output::VERSION.into(),
            session_id: gpu.config.session_id.clone(),
            generation,
            // The sequence is given when the picture is handed out (`handed`), in the order
            // it is shown.
            sequence: 0,
            source_id: self.source.source_id.clone(),
            frame_number: frame,
            width: raster[0],
            height: raster[1],
            pixel_format: PixelFormat::Rgba8Srgb,
        };
        Ok(header)
    }

    fn picture(
        &self,
        frame: u64,
        header: FrameHeader,
        rgba: Arc<[u8]>,
    ) -> DecodedVideoFrame<Picture> {
        DecodedVideoFrame {
            source_id: self.source.source_id.clone(),
            frame,
            video_format: self.source.video_format.clone(),
            payload: Rc::new(PictureData {
                token: None,
                header,
                rgba,
            }),
        }
    }

    /// Off: no picture, black in the raster of the program's first picture.
    fn black(&mut self, frame: u64) -> Result<()> {
        let raster = self
            .workers
            .first()
            .map(|worker| worker.raster)
            .ok_or_else(|| error("program has no picture raster"))?;
        let mut rgba = vec![0u8; raster[0] as usize * raster[1] as usize * 4];
        rgba.chunks_exact_mut(4).for_each(|pixel| pixel[3] = 255);
        let generation = self.gpu.borrow().config.generation;
        let header = self.header(frame, raster, generation)?;
        let picture = self.picture(frame, header, Arc::from(rgba));
        self.ready.insert(frame, picture);
        Ok(())
    }

    fn drain(&mut self) -> Result<()> {
        for index in 0..self.workers.len() {
            while self.workers[index].conversion.busy() {
                let completed = match self.workers[index].conversion.poll().map_err(error)? {
                    Poll::Pending => break,
                    Poll::Ready(completed) => completed,
                };
                self.workers[index].rgba[completed.slot] = Some(completed.rgba.clone());
                if completed.generation != self.gpu.borrow().config.generation {
                    continue;
                }
                completed.result.as_ref().map_err(error)?;
                {
                    let mut gpu = self.gpu.borrow_mut();
                    gpu.conversion_us += completed.elapsed_us;
                    gpu.converted += 1;
                }
                let raster = self.workers[index].raster;
                let header = self.header(completed.frame, raster, completed.generation)?;
                let picture = self.picture(completed.frame, header, completed.rgba);
                self.ready.insert(completed.frame, picture);
            }
        }
        Ok(())
    }

    fn open_decoder(&self, clip: usize, source_frame: u64) -> Result<ActiveDecoder> {
        let (plan, input, _) = &self.clips[clip];
        Ok(ActiveDecoder {
            clip,
            decoder: input.open_in_background(plan.video_index, seek_start(&plan.source, source_frame)?)?,
            next_source: source_frame,
            discard_before: Some(source_frame),
        })
    }

    /// Opens the decoder of the next cut to another picture source within the lookahead
    /// (user report 2026-10-02: the picture stalled just before a cover, its decoder
    /// started only half a second ahead). A cut that continues the same source needs none.
    fn open_upcoming(&mut self, frame: u64) -> Result<()> {
        let timebase = self.source.timebase;
        let seconds = i64::try_from(PICTURE_LOOKAHEAD_SECONDS).map_err(error)?;
        let lookahead = u64::try_from(seconds * timebase.fps_num / timebase.fps_den.max(1)).unwrap_or(0);
        let Some((clip, source_in)) = next_picture_cut(&self.spans, frame, lookahead) else {
            return Ok(());
        };
        if self.upcoming.as_ref().is_some_and(|up| up.clip == clip && up.next_source == source_in) {
            return Ok(());
        }
        diag(format!("program-video upcoming clip={clip} source={source_in} at_frame={frame}"));
        retire(self.upcoming.replace(self.open_decoder(clip, source_in)?));
        Ok(())
    }

    fn fill(&mut self, frame: u64) -> Result<()> {
        let target_end = frame
            .saturating_add(self.prefetch_frames as u64)
            .min(self.source.duration_frames);
        // A frame far ahead of the decoding (late while playing) is jumped to; one within
        // the prefetch is decoded to, so the pictures before it stay for steps back.
        if frame > self.next_decode_frame.saturating_add(self.prefetch_frames as u64) {
            self.next_decode_frame = frame;
            self.kept_from = frame;
        }
        // From where the decoding is, not the playhead: decoding runs ahead of the playhead,
        // so a cut it has passed still lies ahead of the playhead and searching from there
        // opened the decoder of that cut a second time at every cut (live 2026-10-08).
        self.open_upcoming(self.next_decode_frame)?;
        while self.next_decode_frame < target_end {
            let next = self.next_decode_frame;
            if self.ready.contains_key(&next) {
                self.next_decode_frame += 1;
                continue;
            }
            let span = self.span_at(next)?;
            let Some((clip, source_in)) = span.video else {
                self.black(next)?;
                self.next_decode_frame += 1;
                continue;
            };
            let source_frame = source_in + (next - span.record_in);
            let worker = self.clips[clip].2;
            if !self.workers[worker].conversion.can_accept() {
                stopped(next, "converter full");
                break;
            }
            let Some(slot) = self.workers[worker]
                .rgba
                .iter()
                .position(|b| b.as_ref().is_some_and(|b| Arc::strong_count(b) == 1))
            else {
                stopped(next, "no free picture buffer");
                break;
            };
            let continues = self
                .active
                .as_ref()
                .is_some_and(|a| a.clip == clip && a.next_source == source_frame);
            if !continues {
                // The decoder opened ahead for this cut takes over; else one opens now.
                let ahead = if self.upcoming.as_ref().is_some_and(|up| up.clip == clip && up.next_source == source_frame) {
                    self.upcoming.take()
                } else {
                    None
                };
                diag(format!("program-video cut frame={next} clip={clip} opened_ahead={}", ahead.is_some()));
                let decoder = match ahead {
                    Some(up) => up,
                    None => self.open_decoder(clip, source_frame)?,
                };
                retire(self.active.replace(decoder));
            }
            let active = self.active.as_mut().expect("decoder just opened");
            let Some(decoder) = active.decoder.ready()? else {
                stopped(next, "decoder still opening");
                break;
            };
            let packet = match decoder.try_next_packet().map_err(error)? {
                Poll::Pending => {
                    stopped(next, "decoder has no picture yet");
                    break;
                }
                Poll::Ready(None) => return Err(error("video ended before saved frame boundary")),
                Poll::Ready(Some(packet)) => packet,
            };
            let plan = &self.clips[clip].0;
            let decoded = input::relative_position(
                packet.pts,
                packet.time_base,
                plan.origin,
                plan.source.timebase.fps_num,
                plan.source.timebase.fps_den,
            )?;
            let spec = &plan.spec;
            if packet.stream_index != plan.video_index
                || packet.media_uri != plan.media.media_uri
                || packet.format
                    != (DecodedFormat::Video {
                        width: spec.width,
                        height: spec.height,
                        pixel_format: spec.layout.name().into(),
                    })
            {
                return Err(error("decoded frame does not match saved input/request"));
            }
            if active.discard_before.is_some_and(|before| decoded < before) {
                continue;
            }
            active.discard_before = None;
            if decoded < active.next_source {
                continue;
            }
            if decoded != active.next_source {
                return Err(error("decoder skipped requested source frame"));
            }
            let generation = self.gpu.borrow().config.generation;
            let rgba = self.workers[worker].rgba[slot]
                .take()
                .expect("exclusive frame buffer");
            self.workers[worker]
                .conversion
                .submit(conversion::Job {
                    generation,
                    frame: next,
                    slot,
                    input: packet.bytes,
                    rgba,
                })
                .map_err(error)?;
            active.next_source += 1;
            self.next_decode_frame += 1;
        }
        Ok(())
    }
}

impl VideoDecodeAdapter for ProgramVideo {
    type VideoFrame = Picture;
    fn cue_video(&mut self, request: EngineFrameRequest) -> Result<()> {
        if request.source_id != self.source.source_id || request.timebase != self.source.timebase {
            return Err(error("seek source mismatch"));
        }
        seek_start(&self.source, request.frame)?;
        self.pending_seek = Some(request.frame);
        Ok(())
    }
    fn prepare_video(&mut self, _: &EngineSourceHandle) -> Result<Vec<BroadcastEvent>> {
        Ok(Vec::new())
    }
    fn decode_video_frame(
        &mut self,
        request: EngineFrameRequest,
    ) -> Result<DecodedVideoFrame<Picture>> {
        if request.source_id != self.source.source_id || request.timebase != self.source.timebase {
            return Err(error("video request differs from the program"));
        }
        if self.gpu.borrow().playing {
            // Playing: the background gives way to the playing decoder.
            if let Some(back) = &self.back {
                back.borrow_mut().pause();
            }
        }
        let cued = self.pending_seek.is_some();
        if let Some(frame) = self.pending_seek {
            if request.frame != frame {
                return Err(error("seek target mismatch"));
            }
            self.pending_seek = None;
            let stepping_back = frame < self.cue_anchor;
            self.cue_anchor = frame;
            self.drain()?;
            // A frame kept, made in the background, or within reach of the open decoding:
            // the decoder stays open and paused (a frame step, like a clip's). Else a new
            // decoding; stepping back it starts 25 frames earlier.
            let decoding = self.ready.contains_key(&frame)
                || (frame >= self.kept_from
                    && frame <= self.next_decode_frame.saturating_add(self.prefetch_frames as u64));
            let in_back = |f: u64| self.back.as_ref().is_some_and(|back| back.borrow().picture(f).is_some());
            if !decoding && in_back(frame) {
                // Served from the background: the decoding goes on right after its run
                // (live 2026-10-13: it stayed where it was before the jump, the picture
                // after the run was never made and the story stood at that frame).
                let mut after = frame;
                while after < self.source.duration_frames && in_back(after) {
                    after += 1;
                }
                if after != self.next_decode_frame {
                    self.restart(after)?;
                }
            } else if !decoding {
                let from = if stepping_back { frame.saturating_sub(STEP_BACK_FRAMES) } else { frame };
                self.restart(from)?;
            }
        }
        self.drain()?;
        self.forget_far_from(request.frame);
        if cued && let Some(back) = self.back.clone() {
            let (ready, kept_from, decoding) = (&self.ready, self.kept_from, self.next_decode_frame);
            back.borrow_mut().stand(request.frame, |f| ready.contains_key(&f) || (f >= kept_from && f < decoding));
        }
        if !self.ready.contains_key(&request.frame)
            && let Some((raster, rgba)) = self.back.as_ref().and_then(|back| back.borrow().picture(request.frame))
        {
            let generation = self.gpu.borrow().config.generation;
            let header = self.header(request.frame, raster, generation)?;
            let picture = self.picture(request.frame, header, rgba);
            return self.handed(picture);
        }
        if let Some(frame) = self.ready.get(&request.frame).cloned() {
            // The read-ahead is topped up on every frame, not only when one is missing:
            // else it worked in batches of four, just in time, with no reserve.
            self.fill(request.frame)?;
            return self.handed(frame);
        }
        self.fill(request.frame)?;
        self.drain()?;
        let picture = self.ready.get(&request.frame).cloned();
        if picture.is_none() && first_wait(&VIDEO_WAIT, request.frame) {
            diag(format!("program-video pending frame={} next_decode={}", request.frame, self.next_decode_frame));
        }
        self.handed(picture.ok_or_else(pending)?)
    }
}

impl ProgramVideo {
    /// A picture takes its monitor sequence when it is handed out, in the order it is
    /// shown. Given when made, a black Off picture (made at once) got a higher one than
    /// the converted pictures shown before it, and the monitor dropped those as stale
    /// for about half a second (player acceptance 2026-10-02).
    /// A picture shown again after a step back takes the next sequence too, so the monitor
    /// never takes it for stale.
    fn handed(&mut self, picture: DecodedVideoFrame<Picture>) -> Result<DecodedVideoFrame<Picture>> {
        let mut gpu = self.gpu.borrow_mut();
        gpu.sequence = gpu.sequence.checked_add(1).ok_or_else(|| error("frame sequence exhausted"))?;
        let mut header = picture.payload.header.clone();
        header.generation = gpu.config.generation;
        header.sequence = gpu.sequence;
        let rgba = picture.payload.rgba.clone();
        Ok(DecodedVideoFrame { payload: Rc::new(PictureData { token: None, header, rgba }), ..picture })
    }
}

impl ProgramVideo {
    /// A new decoding from `from`: a new generation drops the conversions still running.
    fn restart(&mut self, from: u64) -> Result<()> {
        let mut gpu = self.gpu.borrow_mut();
        gpu.config.generation = gpu
            .config
            .generation
            .checked_add(1)
            .ok_or_else(|| error("GPU generation exhausted"))?;
        gpu.sequence = 0;
        gpu.images.clear();
        drop(gpu);
        diag(format!("program-video restart from={from}"));
        retire((self.active.take(), self.upcoming.take()));
        self.ready.clear();
        self.next_decode_frame = from;
        self.kept_from = from;
        Ok(())
    }

    /// Keeps the pictures from 25 behind the frame on screen (the served one is the
    /// prefetch ahead of it); nothing far behind holds a buffer while playing.
    fn forget_far_from(&mut self, served: u64) {
        let prefetch = self.prefetch_frames as u64;
        let screen = served.saturating_sub(prefetch).max(self.cue_anchor.min(served));
        let back = screen.saturating_sub(STEP_BACK_FRAMES);
        self.ready.retain(|kept, _| *kept >= back);
        self.kept_from = self.kept_from.max(back);
    }
}

/// Pictures kept behind the last cue, so frame steps back need no new decoder.
const STEP_BACK_FRAMES: u64 = 25;

/// A lane is one channel of a clip played with one offset between program and source
/// frames: the same channel in a later item with another offset is another lane, so it
/// can be opened before its cut while the current one still plays.
type LaneKey = (usize, u32, u16, i128);

/// How far ahead (seconds of program) the sound of the next items is opened, so the
/// decoder of a cover has started before its cut instead of on it (a cut used to
/// empty the audio queue: `audio_underrun` on every cover).
const LANE_LOOKAHEAD_SECONDS: u64 = 2;
/// Packets a lane takes from its decoder in one call at most (non-blocking; keeps one
/// call short while the lane fills its seconds ahead).
const LANE_PACKETS_PER_CALL: usize = 32;

/// How far ahead (seconds of program) the picture decoder of the next cut is opened.
const PICTURE_LOOKAHEAD_SECONDS: u64 = 2;

fn lane_key(span: &AudioSpan, bus: &Bus) -> LaneKey {
    (
        bus.clip,
        bus.stream_index,
        bus.channel_index,
        i128::from(bus.source_in) - i128::from(span.record_in),
    )
}

/// The lanes of the items starting in `(frame, horizon]` that are not playing now, each
/// once, at the first item that uses it. An item cut in two (a cover over the end of a
/// segment) gives the same lane twice; opening it at both starts made each undo the
/// other on every frame, so the sound of the cover was never ready at its cut.
fn next_lanes(spans: &[AudioSpan], current: &[LaneKey], frame: u64, horizon: u64) -> Vec<(LaneKey, Bus)> {
    let mut next: Vec<(LaneKey, Bus)> = Vec::new();
    for span in spans.iter().filter(|span| span.record_in > frame && span.record_in <= horizon) {
        for bus in &span.buses {
            let key = lane_key(span, bus);
            if !current.contains(&key) && !next.iter().any(|(seen, _)| *seen == key) {
                next.push((key, *bus));
            }
        }
    }
    next
}

/// The frame last reported as waiting, picture and sound: a wait is written once, not on
/// every poll of the engine.
static VIDEO_WAIT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(u64::MAX);
static AUDIO_WAIT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(u64::MAX);

fn first_wait(last: &std::sync::atomic::AtomicU64, frame: u64) -> bool {
    last.swap(frame, std::sync::atomic::Ordering::Relaxed) != frame
}

/// A diagnostics line of program playback (only with player diagnostics on).
fn diag(text: String) {
    if qnc_dev_diagnostics::player_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, text);
    }
}

/// The first cut within `(frame, frame + lookahead]` to a picture that does not continue
/// the one before it: (clip, its source frame at the cut).
fn next_picture_cut(spans: &[VideoSpan], frame: u64, lookahead: u64) -> Option<(usize, u64)> {
    spans.windows(2).find_map(|pair| {
        let (before, after) = (pair[0], pair[1]);
        if after.record_in <= frame || after.record_in > frame.saturating_add(lookahead) {
            return None;
        }
        let (clip, source_in) = after.video?;
        let continues = before.video.is_some_and(|(prev, prev_in)| {
            prev == clip && prev_in + (after.record_in - before.record_in) == source_in
        });
        (!continues).then_some((clip, source_in))
    })
}

struct Lane {
    clip: usize,
    /// Opened on a thread of its own (`DecodeInput::open_in_background`).
    decoder: Option<Opening>,
    track: PcmTrack,
    consumed_through: Option<u64>,
}

/// Program sound: A1/A2 buses per frame into the project output channels.
pub(crate) struct ProgramAudio {
    pub sink: DeviceSink,
    sound: ProgramSound,
}

/// The samples of a program's frames, A1/A2 buses into the project output channels; it
/// owns no device (the program's queue, or the Sync bed under a clip, user 2026-10-20).
pub(crate) struct ProgramSound {
    source: SourceRuntime,
    spans: Vec<AudioSpan>,
    clips: Vec<(InputPlan, Rc<DecodeInput>)>,
    lanes: BTreeMap<LaneKey, Lane>,
    channels: u16,
    rate: u32,
}

impl ProgramAudio {
    pub fn open(
        plan: &ProgramPlan,
        inputs: &[Rc<DecodeInput>],
        device_id: Option<String>,
        prebuffer_frames: usize,
    ) -> Result<Self> {
        let map = qnc_audio_output::ChannelMap::identity(plan.channels).map_err(error)?;
        Ok(Self {
            sink: DeviceSink::open(&plan.source, device_id, Some(map), prebuffer_frames)?,
            sound: ProgramSound::open(plan, inputs)?,
        })
    }
}

impl ProgramSound {
    pub fn open(plan: &ProgramPlan, inputs: &[Rc<DecodeInput>]) -> Result<Self> {
        let rate = plan
            .source
            .audio_format
            .as_ref()
            .map(|format| format.sample_rate_hz)
            .ok_or_else(|| error("program audio format missing"))?;
        Ok(Self {
            source: plan.source.clone(),
            spans: plan.audio.clone(),
            clips: plan
                .clips
                .iter()
                .zip(inputs)
                .map(|(plan, input)| {
                    (
                        plan.clone(),
                        Rc::new(input.for_media(plan.audio_media.clone())),
                    )
                })
                .collect(),
            lanes: BTreeMap::new(),
            channels: plan.channels,
            rate,
        })
    }

    /// Makes the lane of a bus hold the samples of its source frame; false while
    /// the decoder still prepares them. A frame outside the sound in memory opens the
    /// decoder `back` frames earlier, so the steps back after it are in memory too.
    fn fill_lane(&mut self, key: LaneKey, bus: &Bus, source_frame: u64, back: u64) -> Result<bool> {
        let (plan, input) = &self.clips[bus.clip];
        let start = sample_boundary(source_frame, plan.source.timebase, self.rate)?;
        let end = sample_boundary(source_frame + 1, plan.source.timebase, self.rate)?;
        let lane = self.lanes.entry(key).or_insert_with(|| Lane {
            clip: bus.clip,
            decoder: None,
            track: PcmTrack::new(&AudioStreamPlan {
                stream_index: bus.stream_index,
                source_channels: bus.stream_channels,
                selected_channels: vec![bus.channel_index],
            }),
            consumed_through: None,
        });
        if lane.consumed_through != Some(start) {
            // A cue within the sound in memory (back or a little ahead) keeps the decoder
            // (user 2026-10-21: the sound is heard whenever the playhead moves).
            let ahead = sample_boundary(
                crate::output::frames_of_seconds(plan.source.timebase, crate::output::SOUND_AHEAD_SECONDS),
                plan.source.timebase,
                self.rate,
            )?;
            if lane.consumed_through.is_some() && lane.decoder.is_some() && lane.track.can_aim(start, ahead) {
                lane.track.aim(start);
            } else {
                let from_frame = source_frame.saturating_sub(back);
                diag(format!("program-audio open clip={} stream={} source={source_frame}", bus.clip, bus.stream_index));
                let decoder = input.open_in_background(bus.stream_index, seek_start(&plan.source, from_frame)?)?;
                retire(lane.decoder.replace(decoder));
                lane.track.restart(sample_boundary(from_frame, plan.source.timebase, self.rate)?, start);
            }
            lane.consumed_through = Some(start);
        }
        let needed = usize::try_from(end - start).map_err(error)?;
        // Everything the decoder has ready is taken, up to a few seconds ahead: the decoder
        // keeps reading the media ahead, and a short stall of the card (or of the processor)
        // is ridden out from memory instead of emptying the sound (live 2026-10-08: the
        // sound of an original on a card ran dry with only this frame taken each time).
        let frames_ahead = u64::try_from(LANE_LOOKAHEAD_SECONDS * plan.source.timebase.fps_num.max(0) as u64)
            .unwrap_or(0)
            / u64::try_from(plan.source.timebase.fps_den.max(1)).unwrap_or(1);
        let ahead = needed.saturating_mul(usize::try_from(frames_ahead).unwrap_or(0).max(1));
        let Some(decoder) = lane
            .decoder
            .as_mut()
            .ok_or_else(|| error("audio lane without decoder"))?
            .ready()?
        else {
            // Still opening: the sound waits like a decoder with no samples yet.
            return Ok(lane.track.ready(needed));
        };
        // On the way to a cue the decoder catches up faster.
        let packets = if lane.track.aiming() { LANE_PACKETS_PER_CALL * 4 } else { LANE_PACKETS_PER_CALL };
        for _ in 0..packets {
            lane.track.settle();
            if !lane.track.aiming() && lane.track.samples.len() >= needed.saturating_add(ahead) {
                break;
            }
            match decoder.try_next_packet().map_err(error)? {
                Poll::Pending => break,
                // Reading ahead reaches the end of the source: an error only when the frame
                // played now is not there.
                Poll::Ready(None) if lane.track.ready(needed) => break,
                Poll::Ready(None) => return Err(error("audio ended before saved video boundary")),
                Poll::Ready(Some(packet)) => lane.track.push(
                    packet,
                    &plan.audio_media.media_uri,
                    plan.audio_origin,
                    self.rate,
                )?,
            }
        }
        lane.track.settle();
        Ok(lane.track.ready(needed))
    }

    /// Opens the lanes of the items starting within the lookahead (each one at its
    /// first frame) and drops the lanes no item near the playhead uses.
    fn open_next_lanes(&mut self, current: &AudioSpan, frame: u64) -> Result<()> {
        let timebase = self.source.timebase;
        let seconds = i64::try_from(LANE_LOOKAHEAD_SECONDS).map_err(error)?;
        let lookahead = u64::try_from(seconds * timebase.fps_num / timebase.fps_den.max(1)).unwrap_or(0);
        let horizon = frame.saturating_add(lookahead);
        let current_keys: Vec<LaneKey> = current
            .buses
            .iter()
            .map(|bus| lane_key(current, bus))
            .collect();
        let next = next_lanes(&self.spans, &current_keys, frame, horizon);
        let unused: Vec<LaneKey> = self
            .lanes
            .keys()
            .filter(|key| !current_keys.contains(key) && !next.iter().any(|(next_key, _)| next_key == *key))
            .copied()
            .collect();
        let gone: Vec<Lane> = unused.iter().filter_map(|key| self.lanes.remove(key)).collect();
        if !gone.is_empty() {
            retire(gone);
        }
        // A lane opened ahead starts at its item, nothing before it is played.
        for (key, bus) in next {
            self.fill_lane(key, &bus, bus.source_in, 0)?;
        }
        Ok(())
    }
}

impl AudioOutputAdapter for ProgramAudio {
    type AudioPacket = Arc<[f32]>;
    fn cue_audio(&mut self, request: EngineFrameRequest) -> Result<()> {
        if request.source_id != self.sound.source.source_id || request.timebase != self.sound.source.timebase {
            return Err(error("audio seek source mismatch"));
        }
        seek_start(&self.sound.source, request.frame)?;
        // The frame a cue went to is heard once, as in a clip (user 2026-10-21).
        self.sink.audition_at(request.frame);
        Ok(())
    }
    fn prepare_audio(&mut self, _: &EngineSourceHandle) -> Result<Vec<BroadcastEvent>> {
        Ok(Vec::new())
    }
    fn render_audio_for_frame(
        &mut self,
        request: EngineFrameRequest,
    ) -> Result<AudioFramePacket<Arc<[f32]>>> {
        if request.source_id != self.sound.source.source_id || request.timebase != self.sound.source.timebase {
            return Err(error("audio request differs from the program"));
        }
        let samples = self.sound.render(request.frame)?;
        Ok(AudioFramePacket {
            source_id: request.source_id,
            start_frame: request.frame,
            frame_count: 1,
            audio_format: self.sound.source.audio_format.clone(),
            payload: samples.into(),
        })
    }
    device_sink_adapter!();
}

impl ProgramSound {
    /// The output channels of the program, as many as the project has.
    pub fn channels(&self) -> usize {
        usize::from(self.channels)
    }

    /// The program's sample rate.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// The program's frames.
    pub fn duration_frames(&self) -> u64 {
        self.source.duration_frames
    }

    /// The interleaved samples of program frame `frame`; a `pending` error while a decoder
    /// still prepares them.
    pub fn render(&mut self, frame: u64) -> Result<Vec<f32>> {
        let timebase = self.source.timebase;
        let start = sample_boundary(frame, timebase, self.rate)?;
        let end = sample_boundary(frame + 1, timebase, self.rate)?;
        let frames = usize::try_from(end - start).map_err(error)?;
        let channels = usize::from(self.channels);
        let span = self
            .spans
            .iter()
            .find(|span| (span.record_in..span.record_out).contains(&frame))
            .cloned()
            .ok_or_else(|| error("program frame outside its items"))?;
        let mut ready = true;
        for bus in &span.buses {
            let source_frame = bus.source_in + (frame - span.record_in);
            let back = crate::output::frames_of_seconds(
                self.clips[bus.clip].0.source.timebase,
                crate::output::SOUND_BACK_SECONDS,
            )
            .min(source_frame - bus.source_in);
            ready &= self.fill_lane(lane_key(&span, bus), bus, source_frame, back)?;
        }
        self.open_next_lanes(&span, frame)?;
        if !ready {
            if first_wait(&AUDIO_WAIT, frame) {
                diag(format!("program-audio pending frame={frame}"));
            }
            return Err(pending());
        }
        let mut samples = vec![0.0f32; frames * channels];
        let mut taken: BTreeMap<LaneKey, Vec<f32>> = BTreeMap::new();
        for bus in &span.buses {
            let key = lane_key(&span, bus);
            if !taken.contains_key(&key) {
                let source_frame = bus.source_in + (frame - span.record_in);
                let timebase = self.clips[bus.clip].0.source.timebase;
                let lane_start = sample_boundary(source_frame, timebase, self.rate)?;
                let lane_end = sample_boundary(source_frame + 1, timebase, self.rate)?;
                let count = usize::try_from(lane_end - lane_start).map_err(error)?;
                let lane = self.lanes.get_mut(&key).expect("filled lane");
                debug_assert_eq!(lane.clip, bus.clip);
                taken.insert(key, lane.track.take(count));
                lane.consumed_through = Some(lane_end);
            }
            let source = &taken[&key];
            let gain = bus.gain();
            for (index, value) in source.iter().take(frames).enumerate() {
                samples[index * channels + usize::from(bus.output)] = *value * gain;
            }
        }
        Ok(samples)
    }
}

/// The saved sound of a clip: its channel inventory (program channel k is entry
/// k), its native streams and their sample rate.
pub(crate) struct ClipSound {
    pub channels: Vec<qnc_player_input::AudioChannel>,
    pub streams: Vec<(u32, u16)>,
    pub rate: Option<u32>,
}

/// Per program item: the top picture layer (v5: a cover over the segment
/// picture) and one bus per audio route, from the saved channel inventory.
fn program_spans(
    playlist: &qnc_program_playlist::FlatProgramPlaylist,
    ids: &[&str],
    sound: impl Fn(&str) -> Result<ClipSound>,
    rate: u32,
) -> Result<(Vec<VideoSpan>, Vec<AudioSpan>)> {
    let index = |clip_id: &str| {
        ids.iter()
            .position(|id| *id == clip_id)
            .ok_or_else(|| error("program source has no prepared clip"))
    };
    let frame = |value: i64| u64::try_from(value).map_err(error);
    let mut video = Vec::new();
    let mut audio = Vec::new();
    for item in &playlist.items {
        let (record_in, record_out) = (
            frame(item.record_range.in_frame)?,
            frame(item.record_range.out_frame)?,
        );
        let picture = item
            .sources
            .iter()
            .rev()
            .find(|source| source.has_video())
            .map(|source| {
                Ok::<_, BroadcastEngineError>((
                    index(&source.clip_id)?,
                    frame(source.source_range.source_in)?,
                ))
            })
            .transpose()?;
        video.push(VideoSpan {
            record_in,
            record_out,
            video: picture,
        });
        let mut buses = Vec::new();
        for source in &item.sources {
            for route in &source.audio_routes {
                let clip_sound = sound(&source.clip_id)?;
                let channel = clip_sound
                    .channels
                    .get(usize::from(route.source_channel))
                    .ok_or_else(|| error("program routes a channel the clip does not have"))?;
                if clip_sound.rate != Some(rate) {
                    return Err(error(
                        "Project audio sample rate requires conversion not supported by this playback adapter.",
                    ));
                }
                let stream_channels = clip_sound
                    .streams
                    .iter()
                    .find(|(stream, _)| *stream == channel.stream_index)
                    .map(|(_, count)| *count)
                    .ok_or_else(|| error("program channel references a missing stream"))?;
                buses.push(Bus {
                    output: route.output_channel,
                    clip: index(&source.clip_id)?,
                    source_in: frame(source.source_range.source_in)?,
                    stream_index: channel.stream_index,
                    channel_index: u16::try_from(channel.channel_index).map_err(error)?,
                    stream_channels,
                    level_db: route.level_db,
                });
            }
        }
        audio.push(AudioSpan {
            record_in,
            record_out,
            buses,
        });
    }
    Ok((video, audio))
}

#[cfg(test)]
#[path = "program_tests.rs"]
mod tests;

/// Why the program decode loop stopped ahead of the screen (once per frame).
fn stopped(frame: u64, why: &str) {
    static LAST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(u64::MAX);
    if first_wait(&LAST, frame) {
        diag(format!("program-video stopped next={frame} why={why}"));
    }
}

/// Picture buffers of a program converter: twice the prefetch, the conversions in flight
/// and the pictures kept for steps back.
fn program_picture_buffers(prefetch_frames: usize) -> usize {
    prefetch_frames * 2 + input::CONVERT_IN_FLIGHT + 4 + STEP_BACK_FRAMES as usize
}
