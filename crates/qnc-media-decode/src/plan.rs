use crate::*;
use qnc_media_metadata::{Signal, StreamDetails};
use qnc_media_stream::SourceReference;

pub struct DecodePlan {
    pub format: DecodedFormat,
    pub max_packet: usize,
    pub exact_packet: Option<usize>,
    pub container: String,
    pub codec: String,
}
fn invalid(message: &str) -> DecodeError {
    DecodeError::new(ErrorKind::Contract, message)
}
fn unsupported(message: &str) -> DecodeError {
    DecodeError::new(ErrorKind::Unsupported, message)
}

impl DecodeRequest {
    pub fn validate(&self, config: &DecoderConfig) -> Result<()> {
        let plan = DecodePlan::new(self, config)?;
        config.adapter.validate(self, &plan)
    }
}
impl DecodePlan {
    pub fn new(request: &DecodeRequest, config: &DecoderConfig) -> Result<Self> {
        if request.version != VERSION
            || !(1..=8).contains(&config.queued_packets)
            || !(1..=60).contains(&config.read_timeout.as_secs())
            || config.memory_budget_bytes > 512 * 1024 * 1024
        {
            return Err(invalid("invalid decode version or limits"));
        }
        SourceReference::from_uri(&request.media.media_uri)
            .map_err(|_| invalid("invalid media URI"))?;
        if request.media.streams_complete.as_ref().map(|f| f.value) != Some(true) {
            return Err(invalid("missing complete saved stream map"));
        }
        if let Some(start) = request.start {
            if start.numerator < 0 || start.denominator <= 0 {
                return Err(invalid("invalid timestamp"));
            }
            let duration = request
                .media
                .duration_seconds
                .as_ref()
                .ok_or_else(|| invalid("missing saved duration for timestamp seek"))?
                .value;
            if duration.numerator <= 0
                || duration.denominator <= 0
                || i128::from(start.numerator) * i128::from(duration.denominator)
                    >= i128::from(duration.numerator) * i128::from(start.denominator)
            {
                return Err(invalid("timestamp outside saved duration"));
            }
        }
        let mut indices = std::collections::BTreeSet::new();
        for s in &request.media.streams {
            if !indices.insert(
                s.index
                    .as_ref()
                    .ok_or_else(|| invalid("missing saved stream index"))?
                    .value,
            ) {
                return Err(invalid("duplicate saved stream index"));
            }
        }
        let stream = request
            .media
            .streams
            .iter()
            .find(|s| {
                s.index
                    .as_ref()
                    .is_some_and(|i| i.value == request.stream_index)
            })
            .ok_or_else(|| invalid("stream not in saved map"))?;
        let codec = match stream.codec.as_ref().map(|f| &f.value) {
            Some(Signal::Known(s))
                if !s.is_empty()
                    && s.len() <= 80
                    && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') =>
            {
                s.clone()
            }
            _ => return Err(invalid("missing saved codec")),
        };
        let tb = stream
            .time_base
            .as_ref()
            .ok_or_else(|| invalid("missing saved timebase"))?
            .value;
        if tb.numerator <= 0 || tb.denominator <= 0 {
            return Err(invalid("invalid saved timebase"));
        }
        let container = request
            .media
            .container
            .as_ref()
            .filter(|f| !f.value.is_empty() && f.value.len() <= 128)
            .ok_or_else(|| invalid("missing saved container"))?
            .value
            .clone();
        let (format, max_packet, exact_packet) = match &stream.details {
            StreamDetails::Video(v) => {
                let w = v
                    .width
                    .as_ref()
                    .ok_or_else(|| invalid("missing width"))?
                    .value;
                let h = v
                    .height
                    .as_ref()
                    .ok_or_else(|| invalid("missing height"))?
                    .value;
                let pix = v
                    .pixel_format
                    .as_ref()
                    .ok_or_else(|| invalid("missing pixel format"))?
                    .value
                    .clone();
                if v.exact_frame_count().is_none()
                    || v.frame_rate
                        .as_ref()
                        .is_none_or(|f| f.value.fps_num <= 0 || f.value.fps_den <= 0)
                {
                    return Err(invalid("missing saved video timing"));
                }
                let size = video_bytes(w, h, &pix)?;
                (
                    DecodedFormat::Video {
                        width: w,
                        height: h,
                        pixel_format: pix,
                    },
                    size,
                    Some(size),
                )
            }
            StreamDetails::Audio(a) => {
                let rate = a
                    .sample_rate_hz
                    .as_ref()
                    .ok_or_else(|| invalid("missing sample rate"))?
                    .value;
                let channels = a
                    .channels
                    .as_ref()
                    .ok_or_else(|| invalid("missing channels"))?
                    .value;
                if !(1..=64).contains(&channels) || !(8000..=768000).contains(&rate) {
                    return Err(unsupported("audio format outside decoder limits"));
                }
                (
                    DecodedFormat::Audio {
                        sample_rate_hz: rate,
                        channels,
                        sample_format: "f32le".into(),
                    },
                    channels as usize * 4 * 65536,
                    None,
                )
            }
            _ => return Err(unsupported("data/subtitle streams are not audio or video")),
        };
        if max_packet > 64 * 1024 * 1024
            || max_packet
                .checked_mul(config.queued_packets + 2)
                .is_none_or(|n| n > config.memory_budget_bytes)
        {
            return Err(invalid("decode packet queue exceeds memory budget"));
        }
        Ok(Self {
            format,
            max_packet,
            exact_packet,
            container,
            codec,
        })
    }
}
pub fn video_bytes(w: u32, h: u32, pix: &str) -> Result<usize> {
    if w == 0 || h == 0 || w > 16384 || h > 16384 {
        return Err(unsupported("video dimensions outside decoder limits"));
    }
    let (w, h) = (w as usize, h as usize);
    let y = w * h;
    let p420 = y + 2 * w.div_ceil(2) * h.div_ceil(2);
    let p422 = y + 2 * w.div_ceil(2) * h;
    Ok(match pix {
        "yuv420p" | "nv12" => p420,
        "yuv422p" => p422,
        "yuv444p" | "rgb24" | "bgr24" => 3 * y,
        "yuv420p10le" | "p010le" => 2 * p420,
        "yuv422p10le" => 2 * p422,
        "yuv444p10le" => 6 * y,
        "rgba" | "bgra" => 4 * y,
        "gray" => y,
        _ => {
            return Err(unsupported(
                "native pixel format not supported; no conversion fallback",
            ));
        }
    })
}
