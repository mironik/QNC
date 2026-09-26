//! Original source timecode of a clip (user rule 2026-09-25).
//!
//! Broadcast editing links proxy and original, and exports, by the timecode the
//! camera recorded. The start timecode comes only from the stored record of the
//! clip: when the card has a camera XML its LTC table is the one source (Sony
//! `LtcChangeTable`); the timecode of the one Ingest probe is read only for a clip
//! without camera XML. Nothing is probed here and nothing is invented: a clip
//! without a start timecode shows `--:--:--:--`.
//!
//! The source clip sets the rate: frames count at its nominal frame rate (0-49 on
//! 50p). A camera LTC at half that rate (`halfStep`, 25 on 50p) is scaled to it.
//! Drop-frame timecode is not supported yet and gives no start timecode.
//! The program may keep its own internal timecode; an export uses these originals.

pub const MODULE_ID: &str = "qnc.module.source-timecode";

#[cfg(feature = "read")]
mod read;
#[cfg(feature = "read")]
pub use read::{read_clip, SourceTimecodes};

/// No timecode known.
pub const UNKNOWN: &str = "--:--:--:--";

/// The timecode of a source clip: its start and nominal frame rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceTimecode {
    /// Frames from 00:00:00:00 to the first frame of the clip, at `fps`.
    pub start_frame: Option<u64>,
    /// Nominal frames per second of the source clip (50 for 50p, 30 for 29.97).
    pub fps: u32,
}

impl SourceTimecode {
    /// From the tags of the stored original record and the source frame rate.
    pub fn from_tags<'a>(
        tags: impl IntoIterator<Item = (&'a str, &'a str)>,
        (fps_num, fps_den): (u64, u64),
    ) -> Option<Self> {
        let fps = nominal_fps(fps_num, fps_den)?;
        let tags: Vec<(&str, &str)> = tags.into_iter().collect();
        Some(Self {
            start_frame: start_frame(&tags, fps),
            fps,
        })
    }

    /// Timecode of a frame of the clip (0 is its first frame).
    pub fn label(&self, frame: u64) -> String {
        match self.start_frame {
            Some(start) => format_frames(start + frame, self.fps),
            None => UNKNOWN.into(),
        }
    }

    /// A length as timecode, from 00:00:00:00.
    pub fn duration_label(&self, frames: u64) -> String {
        format_frames(frames, self.fps)
    }
}

/// HH:MM:SS:FF of a frame count at a nominal rate; hours wrap at 24 like a clock.
pub fn format_frames(frames: u64, fps: u32) -> String {
    let fps = u64::from(fps.max(1));
    let ff = frames % fps;
    let seconds = frames / fps;
    format!(
        "{:02}:{:02}:{:02}:{:02}",
        (seconds / 3600) % 24,
        (seconds / 60) % 60,
        seconds % 60,
        ff
    )
}

fn nominal_fps(num: u64, den: u64) -> Option<u32> {
    if num == 0 || den == 0 {
        return None;
    }
    u32::try_from((num + den / 2) / den).ok().filter(|fps| *fps > 0)
}

/// Whether the record holds a camera sidecar (any tag that is not from the probe).
fn has_camera_record(tags: &[(&str, &str)]) -> bool {
    tags.iter()
        .any(|(key, _)| key.contains(':') && !key.starts_with("ffprobe:"))
}

fn start_frame(tags: &[(&str, &str)], fps: u32) -> Option<u64> {
    if has_camera_record(tags) {
        camera_ltc_start(tags, fps)
    } else {
        probe_start(tags, fps)
    }
}

fn tag<'a>(tags: &[(&str, &'a str)], suffix: &str) -> Option<&'a str> {
    tags.iter()
        .find(|(key, _)| key.ends_with(suffix))
        .map(|(_, value)| *value)
}

/// Sony `LtcChangeTable`: the change at frame 0 holds the start as BCD bytes
/// FF SS MM HH at `tcFps`.
fn camera_ltc_start(tags: &[(&str, &str)], fps: u32) -> Option<u64> {
    let tc_fps: u32 = tag(tags, "/LtcChangeTable[1]/@tcFps")?.trim().parse().ok()?;
    let change = tags
        .iter()
        .filter(|(key, value)| key.ends_with("/@frameCount") && value.trim() == "0")
        .find_map(|(key, _)| {
            let prefix = key.strip_suffix("/@frameCount")?;
            prefix.contains("/LtcChangeTable[1]/LtcChange[").then_some(prefix)
        })?;
    let value = tag(tags, &format!("{change}/@value"))?;
    let bytes = hex_bytes(value.trim())?;
    let [ff, ss, mm, hh] = bytes;
    if ff & 0x40 != 0 {
        return None; // drop-frame flag
    }
    let (ff, ss, mm, hh) = (bcd(ff & 0x3F)?, bcd(ss & 0x7F)?, bcd(mm & 0x7F)?, bcd(hh & 0x3F)?);
    if tc_fps == 0 || fps % tc_fps != 0 || ff >= tc_fps {
        return None;
    }
    let seconds = (u64::from(hh) * 60 + u64::from(mm)) * 60 + u64::from(ss);
    Some(seconds * u64::from(fps) + u64::from(ff * (fps / tc_fps)))
}

/// Timecode of the one Ingest probe, only for a clip without camera XML.
fn probe_start(tags: &[(&str, &str)], fps: u32) -> Option<u64> {
    let value = tags
        .iter()
        .find(|(key, _)| *key == "timecode" || key.ends_with("/format/tags/timecode"))
        .map(|(_, value)| value.trim())?;
    if value.contains(';') || value.contains('.') {
        return None; // drop-frame
    }
    let parts: Vec<u64> = value
        .split(':')
        .map(|part| part.parse().ok())
        .collect::<Option<_>>()?;
    let [hh, mm, ss, ff] = parts[..] else {
        return None;
    };
    if ff >= u64::from(fps) || ss > 59 || mm > 59 {
        return None;
    }
    Some(((hh * 60 + mm) * 60 + ss) * u64::from(fps) + ff)
}

fn hex_bytes(value: &str) -> Option<[u8; 4]> {
    if value.len() != 8 || !value.is_ascii() {
        return None;
    }
    let mut bytes = [0u8; 4];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(bytes)
}

fn bcd(byte: u8) -> Option<u32> {
    let (tens, units) = (byte >> 4, byte & 0x0F);
    (tens <= 9 && units <= 9).then_some(u32::from(tens) * 10 + u32::from(units))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "sony.sidecar:/NonRealTimeMeta[1]/LtcChangeTable[1]";

    fn sony(value: &str) -> Vec<(String, String)> {
        vec![
            (format!("{TABLE}/@tcFps"), "25".into()),
            (format!("{TABLE}/@halfStep"), "true".into()),
            (format!("{TABLE}/LtcChange[1]/@frameCount"), "0".into()),
            (format!("{TABLE}/LtcChange[1]/@value"), value.into()),
            (format!("{TABLE}/LtcChange[2]/@frameCount"), "2000".into()),
            (format!("{TABLE}/LtcChange[2]/@value"), "01020300".into()),
            ("ffprobe:/format/tags/timecode".into(), "09:00:00:00".into()),
        ]
    }

    fn of(tags: &[(String, String)], fps: (u64, u64)) -> SourceTimecode {
        SourceTimecode::from_tags(tags.iter().map(|(k, v)| (k.as_str(), v.as_str())), fps)
            .unwrap()
    }

    #[test]
    fn camera_xml_ltc_is_the_start_at_the_source_rate() {
        // Mironik 2676: LTC 08371400 at 25 with halfStep; the clip is 50p.
        let tc = of(&sony("08371400"), (50, 1));
        assert_eq!(tc.label(0), "00:14:37:16", "the same as the camera timecode on 50p");
        assert_eq!(tc.label(1), "00:14:37:17");
        assert_eq!(tc.label(34), "00:14:38:00");
        let tc = of(&sony("13421700"), (50, 1));
        assert_eq!(tc.label(0), "00:17:42:26");
    }

    #[test]
    fn a_camera_record_never_falls_back_to_the_probe() {
        let mut tags = sony("zz");
        tags.retain(|(key, _)| !key.ends_with("@value") || key.contains("LtcChange[2]"));
        assert_eq!(of(&tags, (50, 1)).label(0), UNKNOWN);
        let tags = vec![("sony.sidecar:/NonRealTimeMeta[1]/Duration/@value".to_string(), "9".to_string()),
            ("ffprobe:/format/tags/timecode".to_string(), "09:00:00:00".to_string())];
        assert_eq!(of(&tags, (50, 1)).label(0), UNKNOWN, "XML without LTC: no probe");
    }

    #[test]
    fn the_probe_timecode_is_read_only_without_camera_xml() {
        let tags = vec![("ffprobe:/format/tags/timecode".to_string(), "10:00:00:24".to_string())];
        let tc = of(&tags, (25, 1));
        assert_eq!(tc.label(1), "10:00:01:00");
        let tags = vec![("timecode".to_string(), "01:00:00;00".to_string())];
        assert_eq!(of(&tags, (30000, 1001)).label(0), UNKNOWN, "drop-frame is not supported");
    }

    #[test]
    fn lengths_and_rates() {
        let tc = of(&[], (50, 1));
        assert_eq!(tc.label(0), UNKNOWN);
        assert_eq!(tc.duration_label(125), "00:00:02:25");
        assert!(SourceTimecode::from_tags([], (0, 1)).is_none());
        assert_eq!(nominal_fps(30000, 1001), Some(30));
        assert_eq!(format_frames(24 * 3600 * 25, 25), "00:00:00:00", "hours wrap at 24");
    }
}
