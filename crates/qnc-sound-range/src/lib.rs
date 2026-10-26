//! The sound of one camera channel of a clip over a range of its frames, read from the camera
//! original on this machine through the decoder catalog (user 2026-10-26: the fine wave of a
//! cut and the transcription read the same way, once written). No probe, no database,
//! nothing written.

use qnc_media_decode::{DecodeRequest, DecodedFormat, Decoder};
use qnc_media_metadata::Rational;
use qnc_still_locator::LocatedSound;

pub const MODULE_ID: &str = "qnc.module.sound-range";

/// Samples of one camera channel of a clip from `from_seconds` (media timestamps).
#[derive(Debug, Clone)]
pub struct SoundRange {
    pub from_seconds: f64,
    pub rate: u32,
    pub samples: Vec<f32>,
    /// The clip frames it was read for: whatever the media gave there is all there is (a clip
    /// starts or ends inside them).
    pub frames: (u64, u64),
}

impl SoundRange {
    pub fn to_seconds(&self) -> f64 {
        self.from_seconds + self.samples.len() as f64 / f64::from(self.rate.max(1))
    }

    pub fn covers(&self, from: f64, to: f64) -> bool {
        self.from_seconds <= from && to <= self.to_seconds()
    }

    /// Whether this range answers clip frames `[first, last)`: its samples cover them, or it
    /// was read for them (the media has no more sound there).
    pub fn answers(&self, first: u64, last: u64, seconds: impl Fn(f64) -> f64) -> bool {
        (self.frames.0 <= first && last <= self.frames.1) || self.covers(seconds(first as f64), seconds(last as f64))
    }
}

/// The samples of camera channel `channel` of the clip `place` locates, over clip frames
/// `[from, to)`. `on_progress` hears the seconds read so far and stops the read with `false`.
pub fn read(place: &LocatedSound, channel: u16, from: u64, to: u64, on_progress: &mut dyn FnMut(f64) -> bool) -> Result<SoundRange, String> {
    let (stream_index, inside, width, rate) = place.channel(channel).ok_or_else(|| format!("no camera channel {}", channel + 1))?;
    let config = qnc_decoder_catalog::installed_config().map_err(|error| error.to_string())?;
    // The decoder starts at the time of the first frame from the media's start (as the trim
    // pictures and the player seek).
    let start = (from > 0).then(|| Rational { numerator: (from * u64::from(place.fps.1)) as i64, denominator: i64::from(place.fps.0.max(1)) });
    let request = DecodeRequest {
        version: qnc_media_decode::VERSION.into(),
        media: place.media.clone(),
        stream_index,
        start,
        output_pixel_format: None,
        progressive: false,
        delivered_rate: None,
    };
    let endpoint = qnc_media_stream::CodecEndpoint::for_local_file(&place.file, &place.media.media_uri).map_err(|error| error.to_string())?;
    let mut decoder = Decoder::open_endpoint(request, endpoint, place.stamp.clone(), config).map_err(|error| error.to_string())?;
    let want_to = place.seconds_of(to as f64);
    let mut range: Option<SoundRange> = None;
    loop {
        let packet = match decoder.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            // Past the end of the clip the decoder refuses: the sound read so far is all there
            // is (an OUT near the clip's end).
            Err(_) if range.is_some() => break,
            Err(error) => return Err(error.to_string()),
        };
        let DecodedFormat::Audio { sample_rate_hz, channels, .. } = &packet.format else { continue };
        if *channels != u32::from(width) || *sample_rate_hz != rate {
            return Err("the decoded sound is not the stored format".into());
        }
        let at = packet.pts as f64 * packet.time_base.numerator as f64 / packet.time_base.denominator.max(1) as f64;
        if at > want_to {
            break;
        }
        let samples = packet
            .bytes
            .chunks_exact(usize::from(width) * 4)
            .map(|frame| f32::from_le_bytes(frame[usize::from(inside) * 4..usize::from(inside) * 4 + 4].try_into().expect("sample")));
        match &mut range {
            Some(range) => range.samples.extend(samples),
            None => range = Some(SoundRange { from_seconds: at, rate, samples: samples.collect(), frames: (from, to) }),
        }
        let Some(range) = &range else { continue };
        if range.to_seconds() >= want_to || !on_progress(range.to_seconds() - range.from_seconds) {
            break;
        }
    }
    range.ok_or_else(|| "no sound in the asked part".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_covers_the_seconds_of_its_samples() {
        let range = SoundRange { from_seconds: 10.0, rate: 48_000, samples: vec![0.0; 96_000], frames: (500, 600) };
        assert!(range.covers(10.5, 11.9));
        assert!(!range.covers(9.9, 11.0));
        assert!(!range.covers(10.0, 12.1));
    }

    #[test]
    fn a_range_read_for_frames_answers_them_even_where_the_media_has_no_sound() {
        // The end of a clip: the read stopped at the end of the media, short of the asked part.
        let range = SoundRange { from_seconds: 10.0, rate: 48_000, samples: vec![0.0; 48_000], frames: (500, 600) };
        let seconds = |frame: f64| frame / 50.0;
        assert!(range.answers(520, 590, seconds), "asked once, never again");
        assert!(!range.answers(590, 610, seconds), "frames past the read are asked");
    }
}
