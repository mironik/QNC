//! The sound of an MP4 or QuickTime file and its key pictures, read through the file's own
//! index (user 2026-10-16: the wave and the filmstrip from one reading of the card, also
//! for MP4). An MP4 keeps its media in one data block and says in its index (`moov`) where
//! every sample is; the sound comes in chunks of many samples, so a few hundred reads cover
//! the sound of minutes of picture, and the key pictures are listed (`stss`). This reader
//! reads the index, then only the sound samples of one track and the key pictures asked
//! for. The values are handed over as stored (AAC frames, PCM); the caller decodes them.
//! Read only: nothing is written, created, renamed or removed.

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

pub const MODULE_ID: &str = "qnc.module.mp4-sound";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The largest single read: a longer run of sound samples is read in parts.
const MAX_READ: u64 = 8 << 20;
/// The largest single sample and the most samples a track is taken to have: lengths from the
/// file past them are a broken file, not an allocation (audit 2026-10-25).
const MAX_SAMPLE: u32 = 64 << 20;
const MAX_SAMPLES: usize = 32 << 20;

/// One track of the file, in the order of the file's tracks (the stream index FFmpeg gives).
#[derive(Debug, Clone, Default)]
pub struct Track {
    /// `soun`, `vide`, ...
    pub handler: [u8; 4],
    /// The sample entry: `mp4a`, `avc1`, `sowt`, `twos`, ...
    pub codec: [u8; 4],
    /// Every sample: (place in the file, length), in decoding order.
    pub samples: Vec<(u64, u32)>,
    /// The key samples (numbered from 0); `None` when every sample is a key sample.
    pub sync: Option<Vec<u32>>,
    /// H.264: the length of the NAL size fields and the parameter sets (SPS, PPS).
    pub avc: Option<(u8, Vec<Vec<u8>>)>,
    /// AAC: the AudioSpecificConfig of the track.
    pub aac_config: Option<Vec<u8>>,
}

impl Track {
    pub fn is_sound(&self) -> bool {
        &self.handler == b"soun"
    }

    pub fn is_picture(&self) -> bool {
        &self.handler == b"vide"
    }

    /// The key sample at or after `sample`, else the last one.
    fn key_at_or_after(&self, sample: u64) -> Option<u64> {
        let count = self.samples.len() as u64;
        if count == 0 {
            return None;
        }
        match &self.sync {
            None => Some(sample.min(count - 1)),
            Some(keys) => keys.iter().map(|key| u64::from(*key)).find(|key| *key >= sample).or_else(|| keys.last().map(|key| u64::from(*key))),
        }
    }
}

/// The tracks of the file from its index.
pub fn read_index(path: &Path) -> Result<Vec<Track>, String> {
    let mut file = File::open(path).map_err(|error| format!("mp4 open: {error}"))?;
    let size = file.metadata().map_err(|error| format!("mp4 size: {error}"))?.len();
    let mut at = 0u64;
    while at + 8 <= size {
        let mut head = [0u8; 16];
        read_at(&mut file, at, &mut head[..8.min((size - at) as usize)])?;
        let mut length = u64::from(u32::from_be_bytes(head[..4].try_into().unwrap()));
        let kind: [u8; 4] = head[4..8].try_into().unwrap();
        if length == 1 {
            read_at(&mut file, at, &mut head)?;
            length = u64::from_be_bytes(head[8..16].try_into().unwrap());
        } else if length == 0 {
            length = size - at;
        }
        if length < 8 {
            return Err("mp4: invalid box".into());
        }
        if &kind == b"moov" {
            if length > 256 << 20 {
                return Err("mp4: index too large".into());
            }
            let mut moov = vec![0u8; length as usize];
            read_at(&mut file, at, &mut moov)?;
            let (start, end) = box_body(&moov, 0).ok_or("mp4: index")?;
            return Ok(children(&moov, start, end).filter(|(kind, _, _)| kind == b"trak").filter_map(|(_, s, e)| track(&moov, s, e)).collect());
        }
        at += length;
    }
    Err("mp4: the file has no index".into())
}

/// The sound samples of track `sound` in order, and in the same reading the key picture
/// at or after each of `targets` (picture numbers counted from 0) of track `picture`,
/// given as (target index, its picture number, the stored sample).
pub fn read_sound_and_pictures(
    path: &Path,
    cancel: &AtomicBool,
    tracks: &[Track],
    sound: usize,
    picture: Option<(usize, &[u64])>,
    on_sound: &mut dyn FnMut(&[u8]) -> Result<(), String>,
    on_picture: &mut dyn FnMut(usize, u64, &[u8]) -> Result<(), String>,
) -> Result<(), String> {
    let mut file = File::open(path).map_err(|error| format!("mp4 open: {error}"))?;
    let track = tracks.get(sound).filter(|track| track.is_sound()).ok_or("mp4: no such sound track")?;
    if let Some((index, targets)) = picture {
        let video = tracks.get(index).filter(|track| track.is_picture()).ok_or("mp4: no such picture track")?;
        let mut value = Vec::new();
        for (target, wanted) in targets.iter().enumerate() {
            let number = video.key_at_or_after(*wanted).ok_or("mp4: the picture track is empty")?;
            let (place, length) = *video.samples.get(number as usize).ok_or("mp4: no such picture")?;
            if length > MAX_SAMPLE {
                return Err("mp4: a picture sample is past what a file holds".into());
            }
            value.resize(length as usize, 0);
            read_at(&mut file, place, &mut value)?;
            on_picture(target, number, &value)?;
        }
    }
    let mut buffer = Vec::new();
    let mut index = 0;
    while index < track.samples.len() {
        if cancel.load(Ordering::Acquire) {
            return Err("mp4 sound read cancelled".into());
        }
        // One read for samples that follow each other in the file.
        let start = track.samples[index].0;
        let mut end = index;
        let mut length = 0u64;
        while end < track.samples.len() && track.samples[end].0 == start + length && length < MAX_READ {
            length += u64::from(track.samples[end].1);
            end += 1;
        }
        if length > u64::from(MAX_SAMPLE) + MAX_READ {
            return Err("mp4: a sound sample is past what a file holds".into());
        }
        buffer.resize(length as usize, 0);
        read_at(&mut file, start, &mut buffer)?;
        let mut at = 0usize;
        for sample in &track.samples[index..end] {
            on_sound(&buffer[at..at + sample.1 as usize])?;
            at += sample.1 as usize;
        }
        index = end;
    }
    Ok(())
}

/// An AAC frame with its ADTS header, as a decoder of an AAC stream takes it.
pub fn adts_frame(config: &[u8], frame: &[u8], out: &mut Vec<u8>) -> Result<(), String> {
    if config.len() < 2 {
        return Err("aac: short AudioSpecificConfig".into());
    }
    let object = config[0] >> 3;
    let rate = ((config[0] & 0x07) << 1) | (config[1] >> 7);
    let channels = (config[1] >> 3) & 0x0f;
    if !(1..=4).contains(&object) || rate > 12 {
        return Err("aac: no ADTS form for this AudioSpecificConfig".into());
    }
    let length = frame.len() + 7;
    if length > 0x1fff {
        return Err("aac: frame too long for ADTS".into());
    }
    out.extend_from_slice(&[
        0xff,
        0xf1,
        ((object - 1) << 6) | (rate << 2) | (channels >> 2),
        ((channels & 3) << 6) | (length >> 11) as u8,
        (length >> 3) as u8,
        (((length & 7) << 5) as u8) | 0x1f,
        0xfc,
    ]);
    out.extend_from_slice(frame);
    Ok(())
}

/// An H.264 sample of an MP4 (NAL units with length fields) as a stream with start codes,
/// the track's parameter sets first, so it decodes on its own.
pub fn avc_annexb(sample: &[u8], nal_length: u8, parameter_sets: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(sample.len() + 64);
    for set in parameter_sets {
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(set);
    }
    let size = usize::from(nal_length);
    if !(1..=4).contains(&size) {
        return Err("h264: invalid NAL length size".into());
    }
    let mut at = 0;
    while at + size <= sample.len() {
        let length = sample[at..at + size].iter().fold(0usize, |value, byte| (value << 8) | usize::from(*byte));
        at += size;
        let nal = sample.get(at..at + length).ok_or("h264: NAL past the sample")?;
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(nal);
        at += length;
    }
    Ok(out)
}

fn read_at(file: &mut File, at: u64, buffer: &mut [u8]) -> Result<(), String> {
    file.seek(SeekFrom::Start(at)).map_err(|error| format!("mp4 seek: {error}"))?;
    file.read_exact(buffer).map_err(|error| format!("mp4 read: {error}"))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(data: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(data.get(at..at + 8)?.try_into().ok()?))
}

/// The body of the box at `at`: (start, end).
fn box_body(data: &[u8], at: usize) -> Option<(usize, usize)> {
    let length = u32_at(data, at)? as usize;
    let (header, length) = match length {
        1 => (16, u64_at(data, at + 8)? as usize),
        0 => (8, data.len() - at),
        length => (8, length),
    };
    (length >= header && at + length <= data.len()).then_some((at + header, at + length))
}

/// The boxes inside `[start, end)`: (kind, body start, body end).
fn children(data: &[u8], start: usize, end: usize) -> impl Iterator<Item = ([u8; 4], usize, usize)> + '_ {
    let mut at = start;
    std::iter::from_fn(move || {
        if at + 8 > end {
            return None;
        }
        let kind: [u8; 4] = data[at + 4..at + 8].try_into().ok()?;
        let (body, next) = box_body(data, at)?;
        if next > end {
            return None;
        }
        at = next;
        Some((kind, body, next))
    })
}

fn child(data: &[u8], start: usize, end: usize, path: &[&[u8; 4]]) -> Option<(usize, usize)> {
    let (first, rest) = path.split_first()?;
    let (_, s, e) = children(data, start, end).find(|(kind, _, _)| kind == *first)?;
    if rest.is_empty() {
        Some((s, e))
    } else {
        child(data, s, e, rest)
    }
}

fn track(data: &[u8], start: usize, end: usize) -> Option<Track> {
    let (hdlr, _) = child(data, start, end, &[b"mdia", b"hdlr"])?;
    let handler: [u8; 4] = data.get(hdlr + 8..hdlr + 12)?.try_into().ok()?;
    let (stbl, stbl_end) = child(data, start, end, &[b"mdia", b"minf", b"stbl"])?;
    let mut track = Track { handler, ..Track::default() };
    let table = |kind: &[u8; 4]| child(data, stbl, stbl_end, &[kind]);
    if let Some((stsd, stsd_end)) = table(b"stsd") {
        sample_entry(data, stsd + 8, stsd_end, &mut track);
    }
    let (stsz, _) = table(b"stsz")?;
    let (fixed, count) = (u32_at(data, stsz + 4)?, u32_at(data, stsz + 8)? as usize);
    if count > MAX_SAMPLES || fixed > MAX_SAMPLE {
        return None;
    }
    let sizes: Vec<u32> = if fixed != 0 {
        vec![fixed; count]
    } else {
        (0..count).map(|index| u32_at(data, stsz + 12 + 4 * index)).collect::<Option<_>>()?
    };
    let offsets: Vec<u64> = if let Some((stco, _)) = table(b"stco") {
        let count = u32_at(data, stco + 4)? as usize;
        (0..count).map(|index| u32_at(data, stco + 8 + 4 * index).map(u64::from)).collect::<Option<_>>()?
    } else {
        let (co64, _) = table(b"co64")?;
        let count = u32_at(data, co64 + 4)? as usize;
        (0..count).map(|index| u64_at(data, co64 + 8 + 8 * index)).collect::<Option<_>>()?
    };
    let (stsc, _) = table(b"stsc")?;
    let entries = (0..u32_at(data, stsc + 4)? as usize)
        .map(|index| Some((u32_at(data, stsc + 8 + 12 * index)?, u32_at(data, stsc + 12 + 12 * index)?)))
        .collect::<Option<Vec<_>>>()?;
    let mut sample = 0usize;
    let mut entry = 0usize;
    for (chunk, offset) in offsets.iter().enumerate() {
        let number = chunk as u32 + 1;
        while entry + 1 < entries.len() && entries[entry + 1].0 <= number {
            entry += 1;
        }
        let per_chunk = entries.get(entry)?.1;
        let mut place = *offset;
        for _ in 0..per_chunk {
            let Some(size) = sizes.get(sample) else { break };
            track.samples.push((place, *size));
            place += u64::from(*size);
            sample += 1;
        }
    }
    if let Some((stss, _)) = table(b"stss") {
        let count = u32_at(data, stss + 4)? as usize;
        track.sync = Some((0..count).map(|index| u32_at(data, stss + 8 + 4 * index).map(|key| key.saturating_sub(1))).collect::<Option<_>>()?);
    }
    Some(track)
}

/// The first sample entry: its codec and, for H.264 and AAC, what a decoder needs.
fn sample_entry(data: &[u8], at: usize, end: usize, track: &mut Track) {
    let Some((body, entry_end)) = box_body(data, at).filter(|(_, e)| *e <= end) else { return };
    let Some(codec) = data.get(at + 4..at + 8).and_then(|kind| kind.try_into().ok()) else { return };
    track.codec = codec;
    if track.is_picture() {
        // A visual sample entry has 78 bytes before its boxes.
        if let Some((avcc, avcc_end)) = child(data, body + 78, entry_end, &[b"avcC"]) {
            track.avc = avc_config(&data[avcc..avcc_end]);
        }
    } else if track.is_sound() {
        // A sound sample entry has 28 bytes before its boxes; QuickTime versions 1 and 2
        // add 16 and 36.
        let version = data.get(body + 8..body + 10).map_or(0, |bytes| u16::from_be_bytes([bytes[0], bytes[1]]));
        let skip = 28 + match version {
            1 => 16,
            2 => 36,
            _ => 0,
        };
        let esds = child(data, body + skip, entry_end, &[b"esds"])
            .or_else(|| child(data, body + skip, entry_end, &[b"wave", b"esds"]));
        if let Some((esds, esds_end)) = esds {
            track.aac_config = data.get(esds + 4..esds_end).and_then(decoder_specific_info);
        }
    }
}

fn avc_config(avcc: &[u8]) -> Option<(u8, Vec<Vec<u8>>)> {
    let nal_length = (avcc.get(4)? & 3) + 1;
    let mut sets = Vec::new();
    let mut at = 5;
    for mask in [0x1f, 0xff] {
        let count = avcc.get(at)? & mask;
        at += 1;
        for _ in 0..count {
            let length = usize::from(u16::from_be_bytes([*avcc.get(at)?, *avcc.get(at + 1)?]));
            sets.push(avcc.get(at + 2..at + 2 + length)?.to_vec());
            at += 2 + length;
        }
    }
    Some((nal_length, sets))
}

/// The DecoderSpecificInfo (tag 5) inside an ES descriptor: the AudioSpecificConfig.
fn decoder_specific_info(descriptors: &[u8]) -> Option<Vec<u8>> {
    let mut at = 0;
    while at < descriptors.len() {
        let tag = descriptors[at];
        at += 1;
        let mut length = 0usize;
        for _ in 0..4 {
            let byte = *descriptors.get(at)?;
            at += 1;
            length = (length << 7) | usize::from(byte & 0x7f);
            if byte & 0x80 == 0 {
                break;
            }
        }
        match tag {
            // ES descriptor: id (2), flags (1), then the optional parts the flags name.
            3 => {
                let flags = *descriptors.get(at + 2)?;
                at += 3;
                if flags & 0x80 != 0 {
                    at += 2;
                }
                if flags & 0x40 != 0 {
                    at += 1 + usize::from(*descriptors.get(at)?);
                }
                if flags & 0x20 != 0 {
                    at += 2;
                }
            }
            // Decoder config: 13 bytes, then its descriptors.
            4 => at += 13,
            5 => return descriptors.get(at..at + length).map(<[u8]>::to_vec),
            _ => at += length,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        out
    }

    /// Audit 2026-10-25: a sample count from a broken file allocated up to 16 GB.
    #[test]
    fn a_broken_sample_table_is_no_track_not_an_allocation() {
        let mut hdlr = vec![0u8; 8];
        hdlr.extend_from_slice(b"soun");
        let mut stsz = vec![0u8; 4];
        stsz.extend_from_slice(&1u32.to_be_bytes());
        stsz.extend_from_slice(&u32::MAX.to_be_bytes());
        let stbl = boxed(b"stbl", &boxed(b"stsz", &stsz));
        let minf = boxed(b"minf", &stbl);
        let mut mdia_body = boxed(b"hdlr", &hdlr);
        mdia_body.extend_from_slice(&minf);
        let trak = boxed(b"trak", &boxed(b"mdia", &mdia_body));
        let (start, end) = box_body(&trak, 0).unwrap();
        assert!(track(&trak, start, end).is_none());
    }

    #[test]
    fn an_aac_frame_gets_the_adts_header_of_its_config() {
        // AAC LC, 48 kHz, 2 channels.
        let mut out = Vec::new();
        adts_frame(&[0x11, 0x90], &[1, 2, 3], &mut out).unwrap();
        assert_eq!(out, [0xff, 0xf1, 0x4c, 0x80, 0x01, 0x5f, 0xfc, 1, 2, 3]);
    }

    #[test]
    fn an_mp4_h264_sample_becomes_a_stream_with_its_parameter_sets() {
        let sample = [0, 0, 0, 2, 0x65, 9, 0, 0, 0, 1, 0x06];
        let out = avc_annexb(&sample, 4, &[vec![0x67, 1], vec![0x68, 2]]).unwrap();
        assert_eq!(out, [0, 0, 0, 1, 0x67, 1, 0, 0, 0, 1, 0x68, 2, 0, 0, 0, 1, 0x65, 9, 0, 0, 0, 1, 0x06]);
    }

    #[test]
    fn the_config_is_found_inside_the_es_descriptor() {
        let esds = [3, 0x19, 0, 1, 0, 4, 0x11, 0x40, 0x15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 2, 0x11, 0x90, 6, 1, 2];
        assert_eq!(decoder_specific_info(&esds), Some(vec![0x11, 0x90]));
    }

    /// A camera MP4 (QNC_MP4_SAMPLE): its index has sound and picture tracks and the sound
    /// is read through it. Read only.
    #[test]
    #[ignore]
    fn a_camera_mp4_is_read_through_its_index() {
        let Ok(path) = std::env::var("QNC_MP4_SAMPLE") else { return };
        let tracks = read_index(Path::new(&path)).unwrap();
        let sound = tracks.iter().position(Track::is_sound).unwrap();
        let picture = tracks.iter().position(Track::is_picture).unwrap();
        assert!(tracks[sound].aac_config.is_some() && tracks[picture].avc.is_some());
        let (mut frames, mut pictures) = (0, 0);
        read_sound_and_pictures(Path::new(&path), &AtomicBool::new(false), &tracks, sound, Some((picture, &[0, 100])), &mut |_| {
            frames += 1;
            Ok(())
        }, &mut |_, _, _| {
            pictures += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!((frames, pictures), (tracks[sound].samples.len(), 2));
    }
}
