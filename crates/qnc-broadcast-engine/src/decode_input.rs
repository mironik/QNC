use crate::*;
use qnc_media_metadata::{MediaRepresentation, Rational};
use qnc_media_stream::{CodecEndpoint, MediaStream};

type Opener = dyn FnMut(&str) -> std::io::Result<DecodeMediaAccess>;

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
    opener: Rc<RefCell<Box<Opener>>>,
}
impl DecodeInput {
    pub fn new_access(
        media: MediaRepresentation,
        config: DecoderConfig,
        opener: impl FnMut(&str) -> std::io::Result<DecodeMediaAccess> + 'static,
    ) -> Self {
        Self {
            media,
            config,
            opener: Rc::new(RefCell::new(Box::new(opener))),
        }
    }
    /// A second saved representation through the same session transport binding.
    pub fn for_media(&self, media: MediaRepresentation) -> Self {
        Self {
            media,
            config: self.config.clone(),
            opener: self.opener.clone(),
        }
    }
    pub fn open(&self, stream_index: u32, start: Option<Rational>) -> Result<Decoder> {
        let request = DecodeRequest {
            version: qnc_media_decode::VERSION.into(),
            media: self.media.clone(),
            stream_index,
            start,
        };
        request.validate(&self.config).map_err(error)?;
        let context = decode_open_context(&request);
        match (self.opener.borrow_mut())(&self.media.media_uri)
            .map_err(|err| error(format!("{context}: {err}")))?
        {
            DecodeMediaAccess::Stream(media) => Decoder::open(request, media, self.config.clone())
                .map_err(|err| error(format!("{context}: {err}"))),
            DecodeMediaAccess::Endpoint {
                endpoint,
                storage_stamp,
            } => Decoder::open_endpoint(request, endpoint, storage_stamp, self.config.clone())
                .map_err(|err| error(format!("{context}: {err}"))),
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
    // v4's short decode preroll. Source PTS, never the output ordinal, confirms the target.
    let start = frame.saturating_sub(25);
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
                numerator: 2975 * 1001,
                denominator: 30000
            })
        );
        assert_eq!(seek_start(&source, 0).unwrap(), None);
        assert!(seek_start(&source, 3001).is_err());
    }
}
