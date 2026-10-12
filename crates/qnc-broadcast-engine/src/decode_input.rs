use crate::*;
use qnc_media_metadata::{MediaRepresentation, Rational};
use qnc_media_stream::{CodecEndpoint, MediaStream};

type Opener = dyn FnMut(&str) -> std::io::Result<DecodeMediaAccess> + Send;

#[derive(Debug)]
pub enum DecodeMediaAccess {
    Stream(MediaStream),
    Endpoint {
        endpoint: CodecEndpoint,
        storage_stamp: String,
    },
}

/// One session's existing saved media binding, reused on explicit seek. No DB lookup.
pub(crate) struct DecodeInput {
    media: MediaRepresentation,
    config: DecoderConfig,
    /// The picture layout asked of the decoder instead of the saved one (video only).
    output_pixel_format: Option<String>,
    /// Progressive pictures of an interlaced saved source (see `InputPlan`).
    progressive: bool,
    /// Pictures at the program rate of a clip of twice or half it (see `InputPlan`).
    delivered_rate: Option<qnc_media_metadata::FrameTimebase>,
    /// Shared with the threads that open decoders in the background (see `open_in_background`).
    opener: std::sync::Arc<std::sync::Mutex<Box<Opener>>>,
}
impl DecodeInput {
    pub fn new_access(
        media: MediaRepresentation,
        config: DecoderConfig,
        opener: impl FnMut(&str) -> std::io::Result<DecodeMediaAccess> + Send + 'static,
    ) -> Self {
        Self {
            media,
            config,
            output_pixel_format: None,
            progressive: false,
            delivered_rate: None,
            opener: std::sync::Arc::new(std::sync::Mutex::new(Box::new(opener))),
        }
    }
    /// The same input, its pictures delivered in `format` (see `InputPlan`).
    pub fn with_delivery(mut self, format: Option<String>, progressive: bool) -> Self {
        self.output_pixel_format = format;
        self.progressive = progressive;
        self
    }
    /// The same input, its pictures delivered at `rate` (a program clip of twice or half
    /// the program rate, see `InputPlan`).
    pub fn at_rate(mut self, rate: Option<qnc_media_metadata::FrameTimebase>) -> Self {
        self.delivered_rate = rate;
        self
    }
    /// A second saved representation through the same session transport binding.
    pub fn for_media(&self, media: MediaRepresentation) -> Self {
        Self {
            media,
            config: self.config.clone(),
            output_pixel_format: self.output_pixel_format.clone(),
            progressive: self.progressive,
            delivered_rate: self.delivered_rate,
            opener: self.opener.clone(),
        }
    }
    /// Whether the configured decoder supports this saved stream, without opening it.
    pub fn validate(&self, stream_index: u32) -> Result<()> {
        DecodeRequest {
            version: qnc_media_decode::VERSION.into(),
            media: self.media.clone(),
            stream_index,
            start: None,
            output_pixel_format: self.output_pixel_format.clone(),
            progressive: self.progressive,
            delivered_rate: self.delivered_rate,
        }
        .validate(&self.config)
        .map_err(error)
    }
    pub fn open(&self, stream_index: u32, start: Option<Rational>) -> Result<Decoder> {
        let (request, access, context) = self.prepare(stream_index, start)?;
        open_access(request, access, self.config.clone(), &context)
    }

    /// The same decoder opened on a thread of its own: the media binding is resolved here,
    /// the decoder process or the media itself opened there. The playback thread stood 70
    /// to 350 ms each time a cover's decoder was opened ahead of its cut, a picture gap at
    /// every cover (live 2026-10-12).
    pub fn open_in_background(&self, stream_index: u32, start: Option<Rational>) -> Result<Opening> {
        let request = self.request(stream_index, start)?;
        let (config, opener, uri) = (self.config.clone(), self.opener.clone(), self.media.media_uri.clone());
        let (send, receive) = std::sync::mpsc::channel();
        // The media binding too: opening the file and reading its metadata on a busy card
        // waited behind its reads (a playback tick of 0.5 s stopped the story, 2026-10-12).
        std::thread::Builder::new()
            .name("qnc-decoder-open".into())
            .spawn(move || {
                let context = decode_open_context(&request);
                let access = opener
                    .lock()
                    .map_err(|_| error("decoder media binding poisoned"))
                    .and_then(|mut open| open(&uri).map_err(|err| error(format!("{context}: {err}"))));
                let _ = send.send(access.and_then(|access| open_access(request, access, config, &context)));
            })
            .map_err(|_| error("cannot start decoder open thread"))?;
        Ok(Opening::Waiting(receive, std::time::Instant::now()))
    }

    fn prepare(&self, stream_index: u32, start: Option<Rational>) -> Result<(DecodeRequest, DecodeMediaAccess, String)> {
        let request = self.request(stream_index, start)?;
        let context = decode_open_context(&request);
        let access = (self.opener.lock().map_err(|_| error("decoder media binding poisoned"))?)(&self.media.media_uri)
            .map_err(|err| error(format!("{context}: {err}")))?;
        Ok((request, access, context))
    }

    /// The validated request of a stream from `start`.
    fn request(&self, stream_index: u32, start: Option<Rational>) -> Result<DecodeRequest> {
        let request = DecodeRequest {
            version: qnc_media_decode::VERSION.into(),
            media: self.media.clone(),
            stream_index,
            start,
            output_pixel_format: self.output_pixel_format.clone(),
            progressive: self.progressive,
            delivered_rate: self.delivered_rate,
        };
        request.validate(&self.config).map_err(error)?;
        Ok(request)
    }
}

fn open_access(request: DecodeRequest, access: DecodeMediaAccess, config: DecoderConfig, context: &str) -> Result<Decoder> {
    match access {
        DecodeMediaAccess::Stream(media) => Decoder::open(request, media, config),
        DecodeMediaAccess::Endpoint {
            endpoint,
            storage_stamp,
        } => Decoder::open_endpoint(request, endpoint, storage_stamp, config),
    }
    .map_err(|err| error(format!("{context}: {err}")))
}

/// A decoder that is being opened on its own thread, or is open.
pub(crate) enum Opening {
    Waiting(std::sync::mpsc::Receiver<Result<Decoder>>, std::time::Instant),
    Ready(Decoder),
}

impl Opening {
    /// The decoder once it is open; `None` while it still opens (never waits).
    pub fn ready(&mut self) -> Result<Option<&mut Decoder>> {
        if let Opening::Waiting(receive, started) = self {
            match receive.try_recv() {
                Ok(opened) => {
                    let decoder = opened?;
                    if qnc_dev_diagnostics::player_diagnostics_enabled() {
                        qnc_dev_diagnostics::log_line(
                            qnc_dev_diagnostics::DiagnosticsStream::Player,
                            format!("decoder-open background_ms={}", started.elapsed().as_millis()),
                        );
                    }
                    *self = Opening::Ready(decoder);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return Ok(None),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return Err(error("decoder open thread ended")),
            }
        }
        match self {
            Opening::Ready(decoder) => Ok(Some(decoder)),
            Opening::Waiting(..) => Ok(None),
        }
    }
}

fn decode_open_context(request: &DecodeRequest) -> String {
    let start = request
        .start
        .map(|r| format!("{}/{}", r.numerator, r.denominator))
        .unwrap_or_else(|| "0".into());
    format!(
        "decode open stream={} start={} media={}",
        request.stream_index, start, request.media.media_uri
    )
}

pub(crate) fn seek_start(source: &SourceRuntime, frame: u64) -> Result<Option<Rational>> {
    if frame >= source.duration_frames {
        return Err(error("seek outside saved duration"));
    }
    // A short decode preroll (v4 had 25 pictures): the decoders seek accurately and drop
    // the pictures before the start themselves, so 2 only cover timestamp rounding; every
    // preroll picture is still decoded, converted and handed over, then dropped (25 cost a
    // jump about 200 ms, 2026-10-12). Source PTS, never the output ordinal, confirms the target.
    let start = frame.saturating_sub(2);
    if start == 0 {
        return Ok(None);
    }
    let numerator = i128::from(start)
        .checked_mul(i128::from(source.timebase.fps_den))
        .and_then(|n| i64::try_from(n).ok())
        .ok_or_else(|| error("seek time overflow"))?;
    Ok(Some(Rational {
        numerator,
        denominator: source.timebase.fps_num,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seek_preroll_uses_saved_rational_timebase_and_rejects_out() {
        let source = SourceRuntime::new("clip", 3001, Timebase::new(30000, 1001).unwrap()).unwrap();
        assert_eq!(
            seek_start(&source, 3000).unwrap(),
            Some(Rational {
                numerator: 2998 * 1001,
                denominator: 30000
            })
        );
        assert_eq!(seek_start(&source, 0).unwrap(), None);
        assert!(seek_start(&source, 3001).is_err());
    }
}
