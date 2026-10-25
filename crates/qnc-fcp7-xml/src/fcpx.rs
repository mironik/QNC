//! The same story as Final Cut Pro X XML (`fcpxml` 1.10; user 2026-10-25: Edius read the FCP7
//! level as something else and A2 went silent, "try the FCP X rules"). Times are rational
//! seconds, a level is `adjust-volume` in dB, a channel is `audio-channel-source`. The primary
//! storyline is A1 (sound only) with gaps; V1, V2 and A2 are clips connected to it on lanes
//! 1, 2 and -1.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use qnc_program_playlist::FlatProgramPlaylist;

use super::{escape, timecode_frame, tracks, without_overlaps, ClipFile, Piece};

/// The text of the `fcpxml` of the program `name`, its clips' files by clip id, the sequence
/// starting at `start_timecode` (HH:MM:SS:FF), its sound at `sample_rate`.
pub fn fcpxml(
    name: &str,
    playlist: &FlatProgramPlaylist,
    files: &BTreeMap<String, ClipFile>,
    start_timecode: &str,
    sample_rate: u32,
) -> Result<String, String> {
    let rate = Rate { num: playlist.program_timebase.fps_num, den: playlist.program_timebase.fps_den.max(1) };
    let start = timecode_frame(start_timecode, rate.whole())?;
    let [v1, v2, a1, a2] = tracks(playlist);

    // Resources: one format per frame size, one asset per clip.
    let mut formats: BTreeMap<(u32, u32), String> = BTreeMap::new();
    let mut assets: BTreeMap<&str, String> = BTreeMap::new();
    // A clip used only for its sound (an Off) has no picture size: it takes the sequence's.
    let size = files.values().map(|file| (file.width, file.height)).find(|size| size.0 > 0 && size.1 > 0).unwrap_or((1920, 1080));
    let size_of = |file: &ClipFile| if file.width > 0 && file.height > 0 { (file.width, file.height) } else { size };
    formats.insert(size, "r1".into());
    for file in files.values() {
        let next = format!("r{}", formats.len() + 1);
        formats.entry(size_of(file)).or_insert(next);
    }
    for clip_id in files.keys() {
        assets.insert(clip_id, format!("r{}", formats.len() + assets.len() + 1));
    }
    let mut out = String::new();
    let w = &mut out;
    writeln!(w, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE fcpxml>\n<fcpxml version=\"1.10\">\n<resources>").ok();
    for ((fw, fh), id) in &formats {
        writeln!(w, "<format id=\"{id}\" frameDuration=\"{}\" width=\"{fw}\" height=\"{fh}\"/>", rate.time(1)).ok();
    }
    for (clip_id, id) in &assets {
        let file = &files[*clip_id];
        let tc = file.timecode_start.unwrap_or(0);
        writeln!(
            w,
            "<asset id=\"{id}\" name=\"{}\" start=\"{}\" duration=\"{}\" hasVideo=\"1\" format=\"{}\" hasAudio=\"1\" audioSources=\"{}\" audioChannels=\"1\" audioRate=\"{}\">",
            escape(&file.name),
            rate.time(tc),
            rate.time(file.duration_frames),
            formats[&size_of(file)],
            file.audio_channels.max(1),
            file.audio_sample_rate.max(1),
        )
        .ok();
        writeln!(w, "<media-rep kind=\"original-media\" src=\"{}\"/>\n</asset>", file_url(&file.path)).ok();
    }
    writeln!(w, "</resources>\n<library>\n<event name=\"QNC\">\n<project name=\"{}\">", escape(name)).ok();
    writeln!(
        w,
        "<sequence format=\"r1\" duration=\"{}\" tcStart=\"{}\" tcFormat=\"NDF\" audioLayout=\"stereo\" audioRate=\"{}k\">\n<spine>",
        rate.time(playlist.duration_frames),
        rate.time(start),
        sample_rate / 1000,
    )
    .ok();

    // The storyline: A1 and the gaps between, to the end of the program.
    let mut spine: Vec<(Option<Piece>, (i64, i64))> = Vec::new();
    let mut at = 0;
    for piece in without_overlaps(a1) {
        if piece.record.0 > at {
            spine.push((None, (at, piece.record.0)));
        }
        at = piece.record.1;
        let record = piece.record;
        spine.push((Some(piece), record));
    }
    if playlist.duration_frames > at {
        spine.push((None, (at, playlist.duration_frames)));
    }
    // Connected clips go to the storyline element they start in.
    let mut connected: Vec<Vec<(i32, Piece)>> = vec![Vec::new(); spine.len()];
    for (lane, track) in [(1, v1), (2, v2), (-1, without_overlaps(a2))] {
        for piece in track {
            let parent = spine.iter().position(|(_, record)| record.0 <= piece.record.0 && piece.record.0 < record.1).unwrap_or(spine.len().saturating_sub(1));
            if let Some(list) = connected.get_mut(parent) {
                list.push((lane, piece));
            }
        }
    }
    for ((element, record), anchored) in spine.iter().zip(&connected) {
        // The element's own time where the program is at `record.0`.
        let local_start = element.as_ref().map_or(0, |piece| files.get(&piece.clip_id).and_then(|file| file.timecode_start).unwrap_or(0) + piece.source.0);
        let offset = rate.time(start + record.0);
        let duration = rate.time(record.1 - record.0);
        match element {
            Some(piece) => {
                let file = files.get(&piece.clip_id).ok_or_else(|| format!("no file for clip {}", piece.clip_id))?;
                writeln!(w, "<asset-clip ref=\"{}\" name=\"{}\" offset=\"{offset}\" start=\"{}\" duration=\"{duration}\" srcEnable=\"audio\" tcFormat=\"NDF\">", assets[piece.clip_id.as_str()], escape(&file.name), rate.time(local_start)).ok();
                audio_body(w, piece, file);
            }
            None => {
                writeln!(w, "<gap name=\"Gap\" offset=\"{offset}\" start=\"0s\" duration=\"{duration}\">").ok();
            }
        }
        for (lane, piece) in anchored {
            let file = files.get(&piece.clip_id).ok_or_else(|| format!("no file for clip {}", piece.clip_id))?;
            let source_start = file.timecode_start.unwrap_or(0) + piece.source.0;
            let enable = if *lane < 0 { "audio" } else { "video" };
            writeln!(
                w,
                "<asset-clip ref=\"{}\" lane=\"{lane}\" name=\"{}\" offset=\"{}\" start=\"{}\" duration=\"{}\" srcEnable=\"{enable}\" tcFormat=\"NDF\">",
                assets[piece.clip_id.as_str()],
                escape(&file.name),
                rate.time(local_start + piece.record.0 - record.0),
                rate.time(source_start),
                rate.time(piece.record.1 - piece.record.0),
            )
            .ok();
            if *lane < 0 {
                audio_body(w, piece, file);
            }
            writeln!(w, "</asset-clip>").ok();
        }
        writeln!(w, "{}", if element.is_some() { "</asset-clip>" } else { "</gap>" }).ok();
    }
    writeln!(w, "</spine>\n</sequence>\n</project>\n</event>\n</library>\n</fcpxml>").ok();
    Ok(out)
}

/// The sound of a piece: its level in dB and its one camera channel (the others off).
fn audio_body(w: &mut String, piece: &Piece, file: &ClipFile) {
    if piece.level_db != 0 {
        writeln!(w, "<adjust-volume amount=\"{}dB\"/>", piece.level_db).ok();
    }
    for channel in 0..file.audio_channels.max(1) {
        let enabled = if channel == piece.channel { "" } else { " enabled=\"0\"" };
        writeln!(w, "<audio-channel-source srcCh=\"{}\" role=\"dialogue\"{enabled}/>", channel + 1).ok();
    }
}

/// `file:///D:/...` with the characters a URL may not hold escaped.
fn file_url(path: &std::path::Path) -> String {
    super::path_url(path).replacen("file://localhost/", "file:///", 1)
}

/// The program's frame rate.
#[derive(Clone, Copy)]
struct Rate {
    num: i64,
    den: i64,
}

impl Rate {
    /// `frames` as rational seconds.
    fn time(self, frames: i64) -> String {
        if frames == 0 {
            "0s".into()
        } else {
            format!("{}/{}s", frames * self.den, self.num)
        }
    }

    fn whole(self) -> i64 {
        (self.num as f64 / self.den as f64).round() as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rational_seconds_and_a_level_in_db() {
        let rate = Rate { num: 50, den: 1 };
        assert_eq!(rate.time(0), "0s");
        assert_eq!(rate.time(1_800_000), "1800000/50s");
        let piece = Piece { clip_id: "a".into(), record: (0, 10), source: (0, 10), channel: 1, level_db: -6 };
        let file = ClipFile { name: "a".into(), path: "D:/a b.MXF".into(), duration_frames: 10, timecode_start: None, width: 1920, height: 1080, audio_channels: 4, audio_sample_rate: 48000 };
        let mut text = String::new();
        audio_body(&mut text, &piece, &file);
        assert!(text.contains("<adjust-volume amount=\"-6dB\"/>"));
        assert!(text.contains("<audio-channel-source srcCh=\"2\" role=\"dialogue\"/>"));
        assert_eq!(text.matches("enabled=\"0\"").count(), 3);
        assert_eq!(file_url(&file.path), "file:///D:/a%20b.MXF");
    }
}
