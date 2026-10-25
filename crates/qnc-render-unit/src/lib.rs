//! One unit of the story's prerender (user 2026-10-12): a run of program frames between
//! two cuts, described as the player plays it. The picture is one source run (or none,
//! black), the sound one channel of a clip per program output channel (or silence). The
//! description comes from the player's own program plan, so a rendered unit holds the
//! pictures and samples the player shows; this block only describes and fingerprints.
//! No decoding, no files, no database.

use qnc_media_metadata::{FrameTimebase, MediaRepresentation, Rational};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderUnit {
    /// Program frames `[in, out)` of the unit (where it is, not what it is: not part of
    /// the fingerprint, so a unit moved in the story stays rendered).
    pub record_in: u64,
    pub record_out: u64,
    /// The program rate (frames of the unit).
    pub program_rate: FrameTimebase,
    pub sample_rate: u32,
    /// Program output channels.
    pub channels: u16,
    /// Output samples of the unit: the program sample boundaries of its frames, so units
    /// put together are the program's sound without a gap or an overlap.
    pub samples: u64,
    pub video: Option<VideoPart>,
    pub audio: Vec<AudioPart>,
}

/// The picture of a unit: delivered frames of one stream from `first_frame`, as the
/// player's decoder delivers them for this clip (fields, twice or half the rate).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoPart {
    pub media: MediaRepresentation,
    pub stream_index: u32,
    /// Where the decoder is opened (the player's seek start for `first_frame`).
    pub start: Option<Rational>,
    pub first_frame: u64,
    /// The origin and rate that turn a decoded timestamp into a delivered frame.
    pub origin: (i64, Rational),
    pub frame_rate: (i64, i64),
    pub progressive: bool,
    pub delivered_rate: Option<FrameTimebase>,
}

/// One program output channel: one channel of one stream from `first_sample`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioPart {
    pub output: u16,
    pub media: MediaRepresentation,
    pub stream_index: u32,
    pub channel_index: u16,
    pub stream_channels: u16,
    pub start: Option<Rational>,
    pub first_sample: u64,
    pub origin: (i64, Rational),
    /// The level of the channel in dB, 0 as recorded (user 2026-10-16). Left out of the
    /// description at 0, so a unit as recorded keeps the fingerprint it had.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub level_db: i8,
    /// The A2 crossfade over the unit (user 2026-10-21): the factor per mille at its first
    /// sample and after its last, linear between; left out without a fade.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade: Option<[u16; 2]>,
    /// Where the part starts in the unit's samples (user 2026-10-25: a crossfade is inside
    /// the unit of its picture, no unit of its own); left out at 0.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub at_sample: u64,
    /// How many samples of the unit the part sounds; left out when it sounds to the end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<u64>,
}

fn is_zero_u64(value: &u64) -> bool {
    *value == 0
}

fn is_zero(level_db: &i8) -> bool {
    *level_db == 0
}

impl AudioPart {
    /// The factor the decoded samples are multiplied by.
    pub fn gain(&self) -> f32 {
        qnc_gain::level_gain(self.level_db, false)
    }

    /// The unit samples `[from, to)` the part sounds in a unit of `samples`.
    pub fn span(&self, samples: u64) -> (u64, u64) {
        let from = self.at_sample.min(samples);
        (from, self.length.map_or(samples, |length| (from + length).min(samples)))
    }

    /// The fade factor at `sample` of `samples` of the part, 1 without a fade.
    pub fn fade_at(&self, sample: usize, samples: usize) -> f32 {
        let Some([from, to]) = self.fade else { return 1.0 };
        let (from, to) = (f32::from(from) / 1000.0, f32::from(to) / 1000.0);
        from + (to - from) * sample as f32 / samples.max(1) as f32
    }
}

impl RenderUnit {
    pub fn frames(&self) -> u64 {
        self.record_out.saturating_sub(self.record_in)
    }

    /// What the unit is, with the delivery format it is rendered in: equal fingerprints
    /// are the same rendered file wherever the unit stands in the story.
    pub fn fingerprint(&self, delivery: &serde_json::Value) -> String {
        let what = serde_json::json!({
            "frames": self.frames(),
            "program_rate": self.program_rate,
            "sample_rate": self.sample_rate,
            "channels": self.channels,
            "samples": self.samples,
            "video": self.video,
            "audio": self.audio,
            "delivery": delivery,
        });
        format!("{:016x}", fnv1a(what.to_string().as_bytes()))
    }
}

/// FNV-1a, 64 bits: stable across builds and platforms (the std hasher is not).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// A decoded timestamp as a position at `num/den` units per second from `origin`
/// (the same arithmetic as the player: frames of a picture, samples of a sound); `None`
/// for a timestamp off that grid or before the origin, which the player refuses too.
pub fn position(pts: i64, tb: Rational, origin: (i64, Rational), num: i64, den: i64) -> Option<u64> {
    if tb.numerator <= 0 || tb.denominator <= 0 || origin.1.numerator <= 0 || origin.1.denominator <= 0 || num <= 0 || den <= 0 {
        return None;
    }
    let a = i128::from(pts) * i128::from(tb.numerator) * i128::from(origin.1.denominator);
    let b = i128::from(origin.0) * i128::from(origin.1.numerator) * i128::from(tb.denominator);
    let n = (a - b) * i128::from(num);
    let d = i128::from(tb.denominator) * i128::from(origin.1.denominator) * i128::from(den);
    if n < 0 || n % d != 0 {
        return None;
    }
    u64::try_from(n / d).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(record_in: u64) -> RenderUnit {
        RenderUnit {
            record_in,
            record_out: record_in + 50,
            program_rate: FrameTimebase { fps_num: 50, fps_den: 1 },
            sample_rate: 48000,
            channels: 2,
            samples: 48000,
            video: None,
            audio: Vec::new(),
        }
    }

    #[test]
    fn the_fingerprint_is_what_the_unit_is_not_where_it_stands() {
        let delivery = serde_json::json!({"video_codec": "h264"});
        assert_eq!(unit(0).fingerprint(&delivery), unit(700).fingerprint(&delivery));
        assert_ne!(unit(0).fingerprint(&delivery), unit(0).fingerprint(&serde_json::json!({"video_codec": "mpeg2"})));
        let mut longer = unit(0);
        longer.record_out += 1;
        assert_ne!(unit(0).fingerprint(&delivery), longer.fingerprint(&delivery));
    }

    #[test]
    fn a_unit_as_recorded_keeps_its_fingerprint_and_a_level_makes_another() {
        let delivery = serde_json::json!({"video_codec": "h264"});
        let sound = |level_db| {
            let mut unit = unit(0);
            unit.audio.push(AudioPart {
                output: 0,
                media: MediaRepresentation {
                    media_uri: "qnc://local/source/a/file/a.MXF".into(),
                    container: None,
                    duration_seconds: None,
                    streams_complete: None,
                    streams: Vec::new(),
                    tags: Default::default(),
                },
                stream_index: 1,
                channel_index: 0,
                stream_channels: 1,
                start: None,
                first_sample: 0,
                origin: (0, Rational { numerator: 1, denominator: 48000 }),
                level_db,
                fade: None,
                at_sample: 0,
                length: None,
            });
            unit
        };
        let described = serde_json::to_value(&sound(0).audio[0]).unwrap();
        assert!(described.get("level_db").is_none(), "as recorded, described as before");
        assert_ne!(sound(0).fingerprint(&delivery), sound(3).fingerprint(&delivery));
        assert!((sound(-6).audio[0].gain() - 0.501).abs() < 0.001);
    }

    #[test]
    fn positions_count_from_the_origin() {
        let tb = Rational { numerator: 1, denominator: 50 };
        let origin = (0, Rational { numerator: 1, denominator: 50 });
        assert_eq!(position(120, tb, origin, 50, 1), Some(120));
        let sound = Rational { numerator: 1, denominator: 48000 };
        assert_eq!(position(96000, sound, (0, sound), 48000, 1), Some(96000));
        assert_eq!(position(10, tb, origin, 0, 1), None);
        assert_eq!(position(1, Rational { numerator: 1, denominator: 100 }, origin, 50, 1), None, "off the frame grid");
    }
}
