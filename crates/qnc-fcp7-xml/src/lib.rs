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
    /// The sample rate of its sound, 0 when the record does not say.
    pub audio_sample_rate: u32,
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

/// Adds a piece, joined to an earlier one that goes on in the same clip (under a crossfade two
/// pieces of a track run side by side, so it is not always the last one).
fn push(track: &mut Vec<Piece>, piece: Piece) {
    let goes_on = |last: &Piece| {
        last.clip_id == piece.clip_id
            && last.record.1 == piece.record.0
            && last.source.1 == piece.source.0
            && last.channel == piece.channel
            && last.level_db == piece.level_db
    };
    let from = track.len().saturating_sub(2);
    if let Some(last) = track[from..].iter_mut().rev().find(|last| goes_on(last)) {
        last.record.1 = piece.record.1;
        last.source.1 = piece.source.1;
        return;
    }
    track.push(piece);
}

/// A track holds one clip item at a time (FCP7 XML; Resolve and Edius drop or refuse overlapping
/// items). Where the next piece starts under the last one (an A2 crossfade, a J or an L) the
/// last is cut where the next starts: a clean cut, no transition (Edius played no sound in an
/// item after a cross fade, 2026-10-25).
fn without_overlaps(mut pieces: Vec<Piece>) -> Vec<Piece> {
    pieces.sort_by_key(|piece| piece.record.0);
    for index in 1..pieces.len() {
        let next_start = pieces[index].record.0;
        let last = &mut pieces[index - 1];
        let overlap = last.record.1 - next_start;
        if overlap <= 0 {
            continue;
        }
        let kept = (next_start - last.record.0).max(1);
        let cut = last.record.0 + kept;
        last.source.1 -= last.record.1 - cut;
        last.record.1 = cut;
        if cut != next_start {
            pieces[index].source.0 += cut - next_start;
            pieces[index].record.0 = cut;
        }
    }
    pieces
}

/// The text of the sequence `name` of the program, its clips' files by clip id.
/// `start_timecode` (HH:MM:SS:FF, the project's setting) is the timecode of the sequence's first
/// frame.
/// `sample_rate` is the project's sound.
pub fn sequence(
    name: &str,
    playlist: &FlatProgramPlaylist,
    files: &BTreeMap<String, ClipFile>,
    start_timecode: &str,
    sample_rate: u32,
) -> Result<String, String> {
    let rate = rate(playlist.program_timebase.fps_num, playlist.program_timebase.fps_den);
    let start_frame = timecode_frame(start_timecode, rate_fps(&rate))?;
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
    writeln!(w, "<timecode>{rate}<string>{start_timecode}</string><frame>{start_frame}</frame><displayformat>NDF</displayformat></timecode>").ok();
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
    writeln!(w, "<format><samplecharacteristics><depth>24</depth><samplerate>{sample_rate}</samplerate></samplecharacteristics></format>").ok();
    writeln!(w, "<outputs><group><index>1</index><numchannels>1</numchannels><downmix>0</downmix><channel><index>1</index></channel></group><group><index>2</index><numchannels>1</numchannels><downmix>0</downmix><channel><index>2</index></channel></group></outputs>").ok();
    for (output, track) in [(1, a1), (2, a2)] {
        writeln!(w, "<track>").ok();
        for piece in &without_overlaps(track) {
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
        // No level filter: Edius 2026-10-25 read the FCP7 level as something else (covers
        // silent) and applied none in dB; the level stays in the story and the MXF exports.
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
    // One mono track per channel: a clip item's `trackindex` is the channel it plays.
    let characteristics = match file.audio_sample_rate {
        0 => String::new(),
        rate => format!("<samplecharacteristics><samplerate>{rate}</samplerate></samplecharacteristics>"),
    };
    for channel in 1..=file.audio_channels {
        writeln!(w, "<audio>{characteristics}<channelcount>1</channelcount><layout>mono</layout><audiochannel><sourcechannel>{channel}</sourcechannel><channellabel>discrete</channellabel></audiochannel></audio>").ok();
    }
    writeln!(w, "</media>\n</file>").ok();
}

/// `<rate>`: the whole frames per second and whether it is an NTSC rate.
fn rate(num: i64, den: i64) -> String {
    let ntsc = den == 1001;
    let timebase = (num as f64 / den.max(1) as f64).round() as i64;
    format!("<rate><timebase>{timebase}</timebase><ntsc>{}</ntsc></rate>", if ntsc { "TRUE" } else { "FALSE" })
}

/// The whole frames per second of a `<rate>` this file wrote (never a guessed rate).
fn rate_fps(rate: &str) -> i64 {
    rate.split("<timebase>")
        .nth(1)
        .and_then(|rest| rest.split('<').next())
        .and_then(|n| n.parse().ok())
        .expect("a <rate> written by rate()")
}

/// HH:MM:SS:FF of a frame (`qnc-source-timecode` holds the one rule).
fn timecode(frame: i64, fps: i64) -> String {
    qnc_source_timecode::format_frames(frame.max(0) as u64, fps.clamp(1, i64::from(u32::MAX)) as u32)
}

/// The frame of a timecode HH:MM:SS:FF at `fps` whole frames per second.
fn timecode_frame(text: &str, fps: i64) -> Result<i64, String> {
    qnc_source_timecode::parse_timecode(text, fps.clamp(1, i64::from(u32::MAX)) as u32)
        .map(|frame| frame as i64)
        .ok_or_else(|| format!("Not a timecode at {fps} frames per second: {text}"))
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
        let file = |name: &str| ClipFile { name: name.into(), path: format!("D:/Clip/{name} 1.MXF").into(), duration_frames: 1000, timecode_start: Some(90_000), width: 1920, height: 1080, audio_channels: 4, audio_sample_rate: 48000 };
        let files = BTreeMap::from([("a".to_string(), file("a")), ("b".to_string(), file("b"))]);
        let xml = sequence("Story & co", &playlist, &files, "10:00:00:00", 48000).unwrap();
        assert!(xml.contains("<string>10:00:00:00</string><frame>1800000</frame>"), "the start timecode of the sequence");
        assert_eq!(xml.matches("<clipitem").count(), 4, "V1 a, V2 b, A1 a, A2 b: {xml}");
        assert!(xml.contains("<start>0</start><end>150</end><in>10</in><out>160</out>"), "a goes on: one item");
        assert!(xml.contains("<trackindex>2</trackindex>"), "A1 is channel 2 of a");
        assert!(!xml.contains("audiolevels"), "no level filter");
        assert!(xml.contains("file://localhost/D:/Clip/a%201.MXF"));
        assert!(xml.contains("<string>00:30:00:00</string>"));
        assert!(xml.contains("Story &amp; co"));
        assert_eq!(xml.matches("<file id=\"file-1\">").count(), 1, "a file is written once");
    }

    /// Edius and Resolve 2026-10-25: two A2 items side by side under a crossfade; one channel
    /// track of a file that said it had one track of four. Then Edius: no transitions, clean cuts.
    #[test]
    fn an_a2_crossfade_is_a_clean_cut_and_every_channel_is_its_own_track() {
        let piece = |clip: &str, record: (i64, i64), source: (i64, i64)| Piece { clip_id: clip.into(), record, source, channel: 1, level_db: 0 };
        let pieces = without_overlaps(vec![piece("b", (182, 359), (31, 208)), piece("a", (0, 186), (54, 240))]);
        assert_eq!(pieces[0].record, (0, 182), "the last is cut where the next starts");
        assert_eq!(pieces[0].source, (54, 236));
        assert_eq!(pieces[1].record, (182, 359));

        let file = ClipFile { name: "a".into(), path: "D:/a.MXF".into(), duration_frames: 1000, timecode_start: None, width: 1920, height: 1080, audio_channels: 4, audio_sample_rate: 48000 };
        let mut text = String::new();
        file_element(&mut text, "file-1", &file, &rate(50, 1));
        assert_eq!(text.matches("<channelcount>1</channelcount>").count(), 4);
        assert!(text.contains("<samplerate>48000</samplerate></samplecharacteristics>"));
        assert!(text.contains("<sourcechannel>4</sourcechannel>"));
    }
}
