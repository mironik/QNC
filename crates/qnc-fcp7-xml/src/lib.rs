//! The story as a Final Cut Pro 7 XML sequence (`xmeml` version 5; user 2026-10-24: Export to
//! XML, read by Premiere, Resolve and Avid tools). From the flat program playlist: the picture
//! of the segments on V1, the covers on V2, A1 and A2 on audio tracks 1 and 2, each piece with
//! its source channel and level; contiguous pieces of one clip are one clip item. Every clip is
//! one file, the camera original at its path on this computer, with its start timecode. It
//! reads and writes no file; it only makes the text.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use qnc_program_playlist::{FlatProgramPlaylist, ProgramVideoLayer};

pub const MODULE_ID: &str = "qnc.module.fcp7-xml";

/// A clip's file: its original on this computer.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipFile {
    pub name: String,
    pub path: std::path::PathBuf,
    /// Length in story frames.
    pub duration_frames: i64,
    /// The timecode of its first frame in story frames, when it has one.
    pub timecode_start: Option<i64>,
    pub width: u32,
    pub height: u32,
    pub audio_channels: u16,
}

/// One piece on a track: record and source frames, the source channel for sound, its level.
#[derive(Debug, Clone, PartialEq)]
struct Piece {
    clip_id: String,
    record: (i64, i64),
    source: (i64, i64),
    channel: u16,
    level_db: i8,
}

/// The tracks of the program: V1 (segments), V2 (covers), A1, A2.
fn tracks(playlist: &FlatProgramPlaylist) -> [Vec<Piece>; 4] {
    let mut tracks: [Vec<Piece>; 4] = Default::default();
    for item in &playlist.items {
        let record = (item.record_range.in_frame, item.record_range.out_frame);
        for source in &item.sources {
            let piece = |channel: u16, level_db: i8| Piece {
                clip_id: source.clip_id.clone(),
                record,
                source: (source.source_range.source_in, source.source_range.source_out),
                channel,
                level_db,
            };
            match source.video_layer {
                Some(ProgramVideoLayer::Base) => push(&mut tracks[0], piece(0, 0)),
                Some(ProgramVideoLayer::Cover) => push(&mut tracks[1], piece(0, 0)),
                _ => {}
            }
            for route in &source.audio_routes {
                if let Some(track) = tracks.get_mut(2 + usize::from(route.output_channel)).filter(|_| route.output_channel < 2) {
                    push(track, piece(route.source_channel, route.level_db));
                }
            }
        }
    }
    tracks
}

/// Adds a piece, joined to the last one when it goes on in the same clip.
fn push(track: &mut Vec<Piece>, piece: Piece) {
    if let Some(last) = track.last_mut() {
        let goes_on = last.clip_id == piece.clip_id
            && last.record.1 == piece.record.0
            && last.source.1 == piece.source.0
            && last.channel == piece.channel
            && last.level_db == piece.level_db;
        if goes_on {
            last.record.1 = piece.record.1;
            last.source.1 = piece.source.1;
            return;
        }
    }
    track.push(piece);
}

/// The text of the sequence `name` of the program, its clips' files by clip id.
pub fn sequence(name: &str, playlist: &FlatProgramPlaylist, files: &BTreeMap<String, ClipFile>) -> Result<String, String> {
    let rate = rate(playlist.program_timebase.fps_num, playlist.program_timebase.fps_den);
    let [v1, v2, a1, a2] = tracks(playlist);
    let (width, height) = files.values().next().map_or((1920, 1080), |file| (file.width, file.height));
    let mut ids = BTreeMap::new();
    for (index, clip_id) in files.keys().enumerate() {
        ids.insert(clip_id.clone(), (format!("file-{}", index + 1), std::cell::Cell::new(false)));
    }
    let mut out = String::new();
    let mut item = 0usize;
    let w = &mut out;
    writeln!(w, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE xmeml>\n<xmeml version=\"5\">").ok();
    writeln!(w, "<sequence id=\"sequence-1\">\n<name>{}</name>\n<duration>{}</duration>\n{rate}", escape(name), playlist.duration_frames).ok();
    writeln!(w, "<timecode>{rate}<string>00:00:00:00</string><frame>0</frame><displayformat>NDF</displayformat></timecode>").ok();
    writeln!(w, "<media>\n<video>\n<format><samplecharacteristics>{rate}<width>{width}</width><height>{height}</height></samplecharacteristics></format>").ok();
    for track in [&v1, &v2] {
        writeln!(w, "<track>").ok();
        for piece in track {
            item += 1;
            clip_item(w, item, piece, &rate, files, &ids, None)?;
        }
        writeln!(w, "</track>").ok();
    }
    writeln!(w, "</video>\n<audio>\n<numOutputChannels>2</numOutputChannels>").ok();
    for (output, track) in [(1, &a1), (2, &a2)] {
        writeln!(w, "<track>").ok();
        for piece in track {
            item += 1;
            clip_item(w, item, piece, &rate, files, &ids, Some(output))?;
        }
        writeln!(w, "<outputchannelindex>{output}</outputchannelindex>\n</track>").ok();
    }
    writeln!(w, "</audio>\n</media>\n</sequence>\n</xmeml>").ok();
    Ok(out)
}

fn clip_item(
    w: &mut String,
    item: usize,
    piece: &Piece,
    rate: &str,
    files: &BTreeMap<String, ClipFile>,
    ids: &BTreeMap<String, (String, std::cell::Cell<bool>)>,
    audio: Option<u16>,
) -> Result<(), String> {
    let file = files.get(&piece.clip_id).ok_or_else(|| format!("no file for clip {}", piece.clip_id))?;
    let (id, written) = ids.get(&piece.clip_id).ok_or("no file id")?;
    writeln!(w, "<clipitem id=\"clipitem-{item}\">\n<name>{}</name>\n<duration>{}</duration>\n{rate}", escape(&file.name), file.duration_frames).ok();
    writeln!(w, "<start>{}</start><end>{}</end><in>{}</in><out>{}</out>", piece.record.0, piece.record.1, piece.source.0, piece.source.1).ok();
    if written.replace(true) {
        writeln!(w, "<file id=\"{id}\"/>").ok();
    } else {
        file_element(w, id, file, rate);
    }
    if audio.is_some() {
        writeln!(w, "<sourcetrack><mediatype>audio</mediatype><trackindex>{}</trackindex></sourcetrack>", piece.channel + 1).ok();
        if piece.level_db != 0 {
            let gain = 10f64.powf(f64::from(piece.level_db) / 20.0);
            writeln!(
                w,
                "<filter><effect><name>Audio Levels</name><effectid>audiolevels</effectid><effectcategory>audiolevels</effectcategory><effecttype>audiolevels</effecttype><mediatype>audio</mediatype><parameter><parameterid>level</parameterid><name>Level</name><valuemin>0</valuemin><valuemax>3.98109</valuemax><value>{gain:.5}</value></parameter></effect></filter>"
            )
            .ok();
        }
    }
    writeln!(w, "</clipitem>").ok();
    Ok(())
}

fn file_element(w: &mut String, id: &str, file: &ClipFile, rate: &str) {
    writeln!(w, "<file id=\"{id}\">\n<name>{}</name>\n<pathurl>{}</pathurl>\n{rate}\n<duration>{}</duration>", escape(&file.name), path_url(&file.path), file.duration_frames).ok();
    if let Some(start) = file.timecode_start {
        writeln!(w, "<timecode>{rate}<string>{}</string><frame>{start}</frame><displayformat>NDF</displayformat></timecode>", timecode(start, rate_fps(rate))).ok();
    }
    writeln!(w, "<media><video><samplecharacteristics>{rate}<width>{}</width><height>{}</height></samplecharacteristics></video>", file.width, file.height).ok();
    writeln!(w, "<audio><channelcount>{}</channelcount></audio></media>\n</file>", file.audio_channels.max(1)).ok();
}

/// `<rate>`: the whole frames per second and whether it is an NTSC rate.
fn rate(num: i64, den: i64) -> String {
    let ntsc = den == 1001;
    let timebase = (num as f64 / den.max(1) as f64).round() as i64;
    format!("<rate><timebase>{timebase}</timebase><ntsc>{}</ntsc></rate>", if ntsc { "TRUE" } else { "FALSE" })
}

fn rate_fps(rate: &str) -> i64 {
    rate.split("<timebase>").nth(1).and_then(|rest| rest.split('<').next()).and_then(|n| n.parse().ok()).unwrap_or(25)
}

fn timecode(frame: i64, fps: i64) -> String {
    let fps = fps.max(1);
    let seconds = frame / fps;
    format!("{:02}:{:02}:{:02}:{:02}", seconds / 3600 % 24, seconds / 60 % 60, seconds % 60, frame % fps)
}

/// `file://localhost/` and the path with `/` and the characters a URL may not hold escaped.
fn path_url(path: &std::path::Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let text = text.strip_prefix("//?/").unwrap_or(&text);
    let mut url = String::from("file://localhost/");
    for byte in text.trim_start_matches('/').bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b':' | b'-' | b'_' | b'.' | b'~' => url.push(byte as char),
            other => url.push_str(&format!("%{other:02X}")),
        }
    }
    url
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_program_playlist::*;

    fn source(clip: &str, layer: Option<ProgramVideoLayer>, routes: Vec<ProgramAudioRoute>, range: (i64, i64)) -> FlatProgramSource {
        FlatProgramSource {
            source_id: format!("{clip}:{layer:?}"),
            clip_id: clip.into(),
            virtual_shot_id: String::new(),
            media: MediaRef { clip_id: clip.into(), media_uri: format!("qnc://local/source/card/file/{clip}.mxf") },
            source_range: FrameRange { source_in: range.0, source_out: range.1, timebase: FrameTimebase { fps_num: 50, fps_den: 1 } },
            source_duration_frames: 1000,
            video_layer: layer,
            source_video_format: None,
            audio_routes: routes,
            source_audio_channels: 4,
            source_audio_format: None,
            source_timecode_start: Some(0),
        }
    }

    fn route(source_channel: u16, output_channel: u16, level_db: i8) -> ProgramAudioRoute {
        ProgramAudioRoute { source_channel, output_channel, level_db, fade_in: None, fade_out: None }
    }

    #[test]
    fn segments_go_on_v1_and_a1_covers_on_v2_and_a2_and_pieces_of_one_clip_join() {
        let item = |record: (i64, i64), sources| FlatProgramItem { item_id: format!("{record:?}"), record_range: ProgramFrameRange::new(record.0, record.1).unwrap(), sources };
        let playlist = FlatProgramPlaylist {
            playlist_id: "p".into(),
            project_id: "p".into(),
            revision: 1,
            program_timebase: FrameTimebase { fps_num: 50, fps_den: 1 },
            audio_layout: ProgramAudioLayout::discrete(2).unwrap(),
            duration_frames: 200,
            items: vec![
                item((0, 100), vec![source("a", Some(ProgramVideoLayer::Base), vec![], (10, 110)), source("a", None, vec![route(1, 0, 6)], (10, 110))]),
                item((100, 150), vec![source("a", Some(ProgramVideoLayer::Base), vec![], (110, 160)), source("a", None, vec![route(1, 0, 6)], (110, 160)), source("b", Some(ProgramVideoLayer::Cover), vec![route(0, 1, 0)], (0, 50))]),
                item((150, 200), vec![]),
            ],
        };
        let file = |name: &str| ClipFile { name: name.into(), path: format!("D:/Clip/{name} 1.MXF").into(), duration_frames: 1000, timecode_start: Some(90_000), width: 1920, height: 1080, audio_channels: 4 };
        let files = BTreeMap::from([("a".to_string(), file("a")), ("b".to_string(), file("b"))]);
        let xml = sequence("Story & co", &playlist, &files).unwrap();
        assert_eq!(xml.matches("<clipitem").count(), 4, "V1 a, V2 b, A1 a, A2 b: {xml}");
        assert!(xml.contains("<start>0</start><end>150</end><in>10</in><out>160</out>"), "a goes on: one item");
        assert!(xml.contains("<trackindex>2</trackindex>"), "A1 is channel 2 of a");
        assert!(xml.contains("<value>1.99526</value>"), "+6 dB");
        assert!(xml.contains("file://localhost/D:/Clip/a%201.MXF"));
        assert!(xml.contains("<string>00:30:00:00</string>"));
        assert!(xml.contains("Story &amp; co"));
        assert_eq!(xml.matches("<file id=\"file-1\">").count(), 1, "a file is written once");
    }
}
