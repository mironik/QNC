//! The video decoder inside the calling process (user 2026-10-11: no process start and no
//! pipe for each picture; measured on the camera original 4x faster to an arbitrary
//! picture). The FFmpeg libraries (LGPL shared build) decode the saved video stream and
//! give the same pictures and timestamps as the FFmpeg CLI adapter: the same seek (to the
//! key picture before the start, pictures before it dropped), the same filters
//! (`qnc_ffmpeg_decode::video_filter`), the same packed layout. Audio and every request it
//! does not take stay with the FFmpeg CLI adapter. No probe, no clock, no database.

use std::{path::Path, sync::Once};

use ffmpeg_next as ff;
use ff::ffi;
use qnc_ffmpeg_decode::FfmpegAdapter;
use qnc_media_decode::*;
use qnc_media_metadata::Rational;
use qnc_media_stream::CodecEndpoint;

mod reader_io;

/// Decoder threads of one video request (measured on the camera original: 4 threads
/// 170-198 pictures/s, an arbitrary picture in about 110 ms).
const VIDEO_THREADS: usize = 4;

#[derive(Debug, Clone)]
pub struct LibavAdapter {
    /// Audio and everything else go through the CLI adapter with the same executable.
    cli: FfmpegAdapter,
}

impl LibavAdapter {
    pub fn new(executable: impl Into<std::path::PathBuf>) -> Self {
        Self { cli: FfmpegAdapter::new(executable) }
    }
}

impl DecoderAdapter for LibavAdapter {
    fn validate(&self, request: &DecodeRequest, plan: &DecodePlan) -> Result<()> {
        self.cli.validate(request, plan)
    }
    fn launch(
        &self,
        request: &DecodeRequest,
        plan: &DecodePlan,
        endpoint: &CodecEndpoint,
        stamp: &str,
    ) -> Result<ProcessLaunch> {
        self.cli.launch(request, plan, endpoint, stamp)
    }
    fn open_in_process(
        &self,
        request: &DecodeRequest,
        plan: &DecodePlan,
        endpoint: &CodecEndpoint,
    ) -> Option<Result<SourceOpener>> {
        match plan.format {
            DecodedFormat::Video { .. } => {
                let (request, plan, endpoint) = (request.clone(), plan.clone(), endpoint.clone());
                // Opened on the decoder's thread (see `SourceOpener`).
                Some(Ok(Box::new(move || {
                    VideoSource::open(&request, &plan, &endpoint).map(|source| Box::new(source) as Box<dyn PictureSource>)
                })))
            }
            DecodedFormat::Audio { .. } => None,
        }
    }
}

struct VideoSource {
    input: ff::format::context::Input,
    stream: usize,
    stream_tb: ff::Rational,
    decoder: ff::decoder::Video,
    /// The filter spec of the request (deinterlace, field flag, conversion) and the graph,
    /// made from the first decoded picture as the CLI makes it.
    spec: String,
    frame_rate: ff::Rational,
    graph: Option<ff::filter::Graph>,
    /// Pictures before it (stream time base) are dropped: the accurate seek of the CLI.
    trim: Option<i64>,
    /// Timestamps out in the stream time base (no delivered rate), else in the graph's.
    stream_timestamps: bool,
    exact_packet: Option<usize>,
    ordinal: u64,
    sent_eof: bool,
    flushed_graph: bool,
    finished: bool,
    decoded: ff::frame::Video,
    filtered: ff::frame::Video,
    /// The reader the media is read through; after `input`, which uses it until dropped.
    _io: reader_io::ReaderIo,
}

// SAFETY: the source is moved once to the decoder thread of `Decoder::in_process` and only
// used there; the FFmpeg contexts it holds are never shared between threads.
unsafe impl Send for VideoSource {}

fn error(kind: ErrorKind, message: impl Into<String>) -> DecodeError {
    DecodeError::new(kind, message)
}

fn stream_error(message: impl std::fmt::Display) -> DecodeError {
    error(ErrorKind::Stream, format!("in-process decoder: {message}"))
}

impl VideoSource {
    fn open(request: &DecodeRequest, plan: &DecodePlan, endpoint: &CodecEndpoint) -> Result<Self> {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = ff::init();
            ff::log::set_level(ff::log::Level::Error);
        });
        if !endpoint.is_seekable_file() {
            return Err(error(ErrorKind::Unsupported, "in-process decoder requires a seekable codec endpoint"));
        }
        let DecodedFormat::Video { pixel_format, .. } = &plan.format else {
            return Err(error(ErrorKind::Contract, "in-process decoder takes video only"));
        };
        let demuxer = qnc_ffmpeg_decode::container_name(&plan.container)?;
        let path = Path::new(endpoint.input_arg());
        let (mut input, io) = reader_io::open_input(path, demuxer)?;
        if !input.format().name().split(',').any(|name| name == demuxer) {
            return Err(error(ErrorKind::Contract, "opened container differs from the saved record"));
        }
        let stream = request.stream_index as usize;
        let (parameters, stream_tb, frame_rate) = {
            let saved = input.stream(stream).ok_or_else(|| stream_error("saved stream is missing"))?;
            let rate = if saved.avg_frame_rate().numerator() > 0 { saved.avg_frame_rate() } else { saved.rate() };
            (saved.parameters(), saved.time_base(), rate)
        };
        let codec = ff::decoder::find_by_name(&plan.codec).ok_or_else(|| error(ErrorKind::Unsupported, "saved codec has no decoder"))?;
        let mut context = ff::codec::context::Context::from_parameters(parameters).map_err(stream_error)?;
        context.set_threading(ff::threading::Config { kind: ff::threading::Type::Frame, count: VIDEO_THREADS });
        // SAFETY: the context is not opened yet; the packet time base is a plain field.
        unsafe { (*context.as_mut_ptr()).pkt_timebase = stream_tb.into() };
        let decoder = context.decoder().open_as(codec).map_err(stream_error)?.video().map_err(stream_error)?;
        let trim = match request.start {
            Some(start) => Some(seek(&mut input, start, stream_tb)?),
            None => None,
        };
        let convert = request.output_pixel_format.is_some();
        Ok(Self {
            input,
            stream,
            stream_tb,
            decoder,
            spec: qnc_ffmpeg_decode::video_filter(request, pixel_format, convert),
            frame_rate,
            graph: None,
            trim,
            stream_timestamps: request.delivered_rate.is_none(),
            exact_packet: plan.exact_packet,
            ordinal: 0,
            sent_eof: false,
            flushed_graph: false,
            finished: false,
            decoded: ff::frame::Video::empty(),
            filtered: ff::frame::Video::empty(),
            _io: io,
        })
    }

    /// The filter graph of the first decoded picture, as the CLI configures its input.
    fn graph_for(&self, frame: &ff::frame::Video) -> Result<ff::filter::Graph> {
        // SAFETY: the frame stays alive while its plain fields are read below.
        let raw = unsafe { frame.as_ptr() };
        // SAFETY: a decoded frame; only plain fields are read.
        let (format, colorspace, range, aspect) = unsafe {
            ((*raw).format, (*raw).colorspace as i32, (*raw).color_range as i32, (*raw).sample_aspect_ratio)
        };
        let aspect = if aspect.num > 0 && aspect.den > 0 { (aspect.num, aspect.den) } else { (0, 1) };
        let mut args = format!(
            "video_size={}x{}:pix_fmt={}:time_base={}/{}:pixel_aspect={}/{}:colorspace={}:range={}",
            frame.width(),
            frame.height(),
            format,
            self.stream_tb.numerator(),
            self.stream_tb.denominator(),
            aspect.0,
            aspect.1,
            colorspace,
            range,
        );
        if self.frame_rate.numerator() > 0 && self.frame_rate.denominator() > 0 {
            args.push_str(&format!(":frame_rate={}/{}", self.frame_rate.numerator(), self.frame_rate.denominator()));
        }
        let mut graph = ff::filter::Graph::new();
        let buffer = ff::filter::find("buffer").ok_or_else(|| stream_error("no buffer filter"))?;
        let sink = ff::filter::find("buffersink").ok_or_else(|| stream_error("no buffersink filter"))?;
        graph.add(&buffer, "in", &args).map_err(stream_error)?;
        graph.add(&sink, "out", "").map_err(stream_error)?;
        graph
            .output("in", 0)
            .and_then(|parser| parser.input("out", 0))
            .and_then(|parser| parser.parse(&self.spec))
            .map_err(stream_error)?;
        graph.validate().map_err(stream_error)?;
        Ok(graph)
    }

    /// A filtered picture waiting in the graph, packed as the CLI rawvideo output.
    fn take_filtered(&mut self) -> Result<Option<(PacketHeader, Vec<u8>)>> {
        let Some(graph) = self.graph.as_mut() else {
            return Ok(None);
        };
        let mut sink = graph.get("out").ok_or_else(|| stream_error("no graph output"))?;
        // The sink moves its picture in without freeing what the frame holds (unlike the
        // decoder's receive), so the last picture is let go first, else every picture
        // stays in memory (2026-10-12: about 3 MB per picture).
        // SAFETY: a frame this source owns; unref only drops its buffer references.
        unsafe { ffi::av_frame_unref(self.filtered.as_mut_ptr()) };
        match sink.sink().frame(&mut self.filtered) {
            Ok(()) => {}
            Err(ff::Error::Eof) => return Ok(None),
            Err(ff::Error::Other { errno }) if errno == ff::util::error::EAGAIN => return Ok(None),
            Err(other) => return Err(stream_error(other)),
        }
        // SAFETY: the sink context of this graph.
        let sink_tb = unsafe { ffi::av_buffersink_get_time_base(sink.as_ptr()) };
        let pts = self.filtered.pts().ok_or_else(|| stream_error("picture without timestamp"))?;
        let (pts, tb) = if self.stream_timestamps {
            // SAFETY: plain arithmetic of the library.
            (unsafe { ffi::av_rescale_q(pts, sink_tb, self.stream_tb.into()) }, self.stream_tb.into())
        } else {
            (pts, sink_tb)
        };
        let bytes = pack(&self.filtered)?;
        if self.exact_packet.is_some_and(|size| size != bytes.len()) {
            return Err(stream_error("picture size differs from the saved record"));
        }
        let header = PacketHeader {
            ordinal: self.ordinal,
            pts,
            time_base: Rational { numerator: i64::from(tb.num), denominator: i64::from(tb.den) },
            size: bytes.len(),
        };
        self.ordinal += 1;
        Ok(Some((header, bytes)))
    }

    /// The next decoded picture after the accurate seek, or `None` when the decoder wants
    /// more packets or has ended.
    fn take_decoded(&mut self) -> Result<Option<()>> {
        loop {
            match self.decoder.receive_frame(&mut self.decoded) {
                Ok(()) => {}
                Err(ff::Error::Eof) => return Ok(None),
                Err(ff::Error::Other { errno }) if errno == ff::util::error::EAGAIN => return Ok(None),
                Err(other) => return Err(stream_error(other)),
            }
            let pts = self.decoded.timestamp().or(self.decoded.pts());
            self.decoded.set_pts(pts);
            if self.trim.is_some_and(|trim| pts.is_some_and(|pts| pts < trim)) {
                continue; // before the start: decoded only to reach it
            }
            return Ok(Some(()));
        }
    }
}

impl PictureSource for VideoSource {
    /// The same seek as opening at `start`, on the open media and decoder: the decoder is
    /// emptied, the filter graph made again from the next picture (deinterlace keeps no
    /// picture of before), the count starts again.
    fn seek(&mut self, start: Rational) -> Result<bool> {
        self.trim = Some(seek(&mut self.input, start, self.stream_tb)?);
        self.decoder.flush();
        self.graph = None;
        self.sent_eof = false;
        self.flushed_graph = false;
        self.finished = false;
        self.ordinal = 0;
        Ok(true)
    }

    fn next(&mut self) -> Result<Option<(PacketHeader, Vec<u8>)>> {
        loop {
            if let Some(picture) = self.take_filtered()? {
                return Ok(Some(picture));
            }
            if self.finished {
                return Ok(None);
            }
            if self.take_decoded()?.is_some() {
                if self.graph.is_none() {
                    self.graph = Some(self.graph_for(&self.decoded)?);
                }
                let graph = self.graph.as_mut().expect("graph");
                graph
                    .get("in")
                    .ok_or_else(|| stream_error("no graph input"))?
                    .source()
                    .add(&self.decoded)
                    .map_err(stream_error)?;
                continue;
            }
            if self.sent_eof {
                // The decoder has given everything: the graph gives what it holds.
                if !self.flushed_graph {
                    self.flushed_graph = true;
                    if let Some(graph) = self.graph.as_mut() {
                        graph.get("in").ok_or_else(|| stream_error("no graph input"))?.source().flush().map_err(stream_error)?;
                    }
                    continue;
                }
                self.finished = true;
                continue;
            }
            let mut packet = ff::Packet::empty();
            match packet.read(&mut self.input) {
                Ok(()) => {
                    if packet.stream() == self.stream {
                        self.decoder.send_packet(&packet).map_err(stream_error)?;
                    }
                }
                Err(ff::Error::Eof) => {
                    self.decoder.send_eof().map_err(stream_error)?;
                    self.sent_eof = true;
                }
                Err(other) => return Err(stream_error(other)),
            }
        }
    }
}

/// The CLI seek of `-ss start` with `-copyts`: to the key picture before the file's start
/// time plus `start` (microseconds, truncated as the CLI parses it); pictures before that
/// time are dropped. Returns that time in the stream time base.
fn seek(input: &mut ff::format::context::Input, start: Rational, stream_tb: ff::Rational) -> Result<i64> {
    if start.numerator < 0 || start.denominator <= 0 {
        return Err(error(ErrorKind::Contract, "invalid timestamp"));
    }
    let micros = i64::try_from(i128::from(start.numerator) * 1_000_000 / i128::from(start.denominator))
        .map_err(|_| error(ErrorKind::Contract, "invalid timestamp"))?;
    // SAFETY: the format context of this input; plain field and the library's seek.
    let target = unsafe {
        let context = input.as_mut_ptr();
        let file_start = (*context).start_time;
        let target = if file_start == ffi::AV_NOPTS_VALUE { micros } else { micros + file_start };
        if ffi::avformat_seek_file(context, -1, i64::MIN, target, target, 0) < 0 {
            return Err(stream_error("seek failed"));
        }
        target
    };
    let micro_tb = ffi::AVRational { num: 1, den: ffi::AV_TIME_BASE };
    // SAFETY: plain arithmetic of the library.
    Ok(unsafe { ffi::av_rescale_q(target, micro_tb, stream_tb.into()) })
}

/// The picture packed plane after plane without padding (the CLI rawvideo layout).
fn pack(frame: &ff::frame::Video) -> Result<Vec<u8>> {
    // SAFETY: the frame stays alive while it is copied below.
    let raw = unsafe { frame.as_ptr() };
    // SAFETY: a filtered frame of this graph; the library copies its planes into `bytes`.
    unsafe {
        let format: ffi::AVPixelFormat = std::mem::transmute((*raw).format);
        let size = ffi::av_image_get_buffer_size(format, (*raw).width, (*raw).height, 1);
        if size <= 0 {
            return Err(stream_error("unsupported picture layout"));
        }
        let mut bytes = vec![0u8; size as usize];
        let written = ffi::av_image_copy_to_buffer(
            bytes.as_mut_ptr(),
            size,
            (*raw).data.as_ptr() as *const *const u8,
            (*raw).linesize.as_ptr(),
            format,
            (*raw).width,
            (*raw).height,
            1,
        );
        if written != size {
            return Err(stream_error("picture copy failed"));
        }
        Ok(bytes)
    }
}
