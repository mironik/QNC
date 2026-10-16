//! The sound elements of an MXF file, without its pictures (user 2026-10-14: the wave of a
//! camera original was read from the card as the whole file, 30 minutes of material in about
//! 10; the sound is a few percent of it). A frame-wrapped MXF keeps every content package
//! as KLV packets: the picture, then one element per sound channel, then data and system
//! items. This reader walks the packets with one read per content package and seeks over
//! every value that is not sound. The values are handed over as stored; the caller knows
//! the saved format. Read only: nothing is written, created, renamed or removed.

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

/// One read per content package: the sound elements, the data element and the system items
/// that follow a picture, and the key of the next picture (measured on a camera card: one
/// read of about 40 KB a picture covered the file at 56 MB/s, the whole file read 28 MB/s).
const CHUNK: usize = 40 * 1024;

/// The key of an MXF generic container essence element, before its item type.
const ESSENCE_KEY: [u8; 12] = [0x06, 0x0e, 0x2b, 0x34, 0x01, 0x02, 0x01, 0x01, 0x0d, 0x01, 0x03, 0x01];

/// The item type of a generic container sound element.
const SOUND_ITEM: u8 = 0x16;
/// The item type of a generic container picture element.
const PICTURE_ITEM: u8 = 0x15;
/// How much of a picture's start tells whether it is a key picture.
const PICTURE_HEAD: usize = 256;

struct Packet {
    key: [u8; 16],
    length: u64,
    value_at: u64,
}

/// Reads the file in chunks at the places asked for; a place inside the last chunk is not
/// read again.
struct Chunks {
    file: File,
    size: u64,
    at: u64,
    data: Vec<u8>,
}

impl Chunks {
    fn bytes(&mut self, from: u64, len: usize) -> Result<&[u8], String> {
        let end = from.saturating_add(len as u64).min(self.size);
        if from < self.at || end > self.at + self.data.len() as u64 {
            self.file.seek(SeekFrom::Start(from)).map_err(|error| format!("mxf sound seek: {error}"))?;
            let want = CHUNK.max(len).min((self.size - from) as usize);
            self.data.resize(want, 0);
            self.file.read_exact(&mut self.data).map_err(|error| format!("mxf sound read: {error}"))?;
            self.at = from;
        }
        let start = (from - self.at) as usize;
        Ok(&self.data[start..start + (end - from) as usize])
    }

    fn packet(&mut self, at: u64) -> Result<Option<Packet>, String> {
        if at + 17 > self.size {
            return Ok(None);
        }
        let head = self.bytes(at, 25)?.to_vec();
        if head.len() < 17 {
            return Ok(None);
        }
        let key: [u8; 16] = head[..16].try_into().map_err(|_| "mxf sound key")?;
        let first = head[16];
        let (length, length_bytes) = if first < 0x80 {
            (u64::from(first), 1)
        } else {
            let count = usize::from(first & 0x7f);
            if count == 0 || count > 8 || head.len() < 17 + count {
                return Err("mxf sound: invalid KLV length".into());
            }
            let length = head[17..17 + count].iter().fold(0u64, |value, byte| (value << 8) | u64::from(*byte));
            (length, 1 + count)
        };
        Ok(Some(Packet { key, length, value_at: at + 16 + length_bytes as u64 }))
    }
}

/// Every frame-wrapped sound element of the file in file order, as (element number, value).
/// The element number is the last byte of its key; one sound track's elements share it.
pub fn read_sound_elements(
    path: &Path,
    cancel: &AtomicBool,
    on_element: &mut dyn FnMut(u8, &[u8]) -> Result<(), String>,
) -> Result<(), String> {
    read_sound_and_pictures(path, cancel, &[], &|_| false, on_element, &mut |_, _, _| Ok(()))
}

/// The sound elements as `read_sound_elements`, and in the same pass the key pictures a
/// filmstrip needs (user 2026-10-15: one reading of the card for the wave and the
/// filmstrip). For each of `targets` (picture numbers counted from 0 in the file, in
/// order) the first key picture at or after it is read whole and given as (target index,
/// its picture number, value); a target with no key picture after it gets the last key
/// picture before it. `is_key` tells a key picture from the start of its value (the
/// picture's start is already in the read that found its key, so the test costs no read);
/// only a picture handed over is read whole.
pub fn read_sound_and_pictures(
    path: &Path,
    cancel: &AtomicBool,
    targets: &[u64],
    is_key: &dyn Fn(&[u8]) -> bool,
    on_element: &mut dyn FnMut(u8, &[u8]) -> Result<(), String>,
    on_picture: &mut dyn FnMut(usize, u64, &[u8]) -> Result<(), String>,
) -> Result<(), String> {
    let file = File::open(path).map_err(|error| format!("mxf sound open: {error}"))?;
    let size = file.metadata().map_err(|error| format!("mxf sound size: {error}"))?.len();
    let mut chunks = Chunks { file, size, at: 0, data: Vec::new() };
    let mut at = 0u64;
    let mut found = false;
    let (mut picture, mut want) = (0u64, 0usize);
    // The last key picture passed: (value place, length, picture number).
    let mut last_key: Option<(u64, u64, u64)> = None;
    while let Some(packet) = chunks.packet(at)? {
        if cancel.load(Ordering::Acquire) {
            return Err("mxf sound read cancelled".into());
        }
        let end = packet.value_at.checked_add(packet.length).ok_or("mxf sound: packet past the file")?;
        if end > size {
            return Err("mxf sound: packet past the end of the file".into());
        }
        if packet.key[..12] == ESSENCE_KEY && packet.key[12] == SOUND_ITEM {
            let value = chunks.bytes(packet.value_at, packet.length as usize)?;
            on_element(packet.key[15], value)?;
            found = true;
        } else if packet.key[..12] == ESSENCE_KEY && packet.key[12] == PICTURE_ITEM && !targets.is_empty() {
            let head = chunks.bytes(packet.value_at, PICTURE_HEAD.min(packet.length as usize))?;
            if is_key(head) {
                last_key = Some((packet.value_at, packet.length, picture));
                if want < targets.len() && picture >= targets[want] {
                    let value = chunks.bytes(packet.value_at, packet.length as usize)?;
                    // Targets closer than a group of pictures share its key picture.
                    while want < targets.len() && targets[want] <= picture {
                        on_picture(want, picture, value)?;
                        want += 1;
                    }
                }
            }
            picture += 1;
        }
        at = end;
    }
    if want < targets.len() {
        if let Some((value_at, length, number)) = last_key {
            let value = chunks.bytes(value_at, length as usize)?;
            for target in want..targets.len() {
                on_picture(target, number, value)?;
            }
        }
    }
    if !found {
        return Err("mxf sound: the file has no frame-wrapped sound elements".into());
    }
    Ok(())
}

/// Whether an H.264 picture (Annex B, as a camera's MXF stores it) begins a group of
/// pictures: its first NAL units carry a sequence parameter set or an IDR slice before any
/// other slice.
pub fn h264_gop_start(head: &[u8]) -> bool {
    let mut at = 0;
    while let Some(found) = head[at..].windows(3).position(|bytes| bytes == [0, 0, 1]) {
        let nal = at + found + 3;
        let Some(byte) = head.get(nal) else { return false };
        match byte & 0x1f {
            5 | 7 => return true,
            1 => return false,
            _ => at = nal,
        }
    }
    false
}

/// Whether an MPEG-2 picture (as XDCAM's MXF stores it) begins a group of pictures: its
/// first start code is a sequence header or a group of pictures header, not a picture.
pub fn mpeg2_gop_start(head: &[u8]) -> bool {
    head.windows(4)
        .find(|bytes| bytes[..3] == [0, 0, 1])
        .is_some_and(|bytes| matches!(bytes[3], 0xb3 | 0xb8))
}

/// The key picture test of a stored picture codec (`h264`, `mpeg2video`); `None` for a
/// codec whose key pictures this reader does not know.
pub fn key_picture_test(codec: &str) -> Option<fn(&[u8]) -> bool> {
    match codec {
        "h264" => Some(h264_gop_start),
        "mpeg2video" => Some(mpeg2_gop_start),
        _ => None,
    }
}

/// Stored 24-bit little-endian samples as f32, the same value as the FFmpeg decoder gives
/// (`pcm_s24le` to `pcm_f32le`).
pub fn s24le_to_f32(bytes: &[u8], out: &mut Vec<f32>) {
    out.extend(bytes.chunks_exact(3).map(|sample| {
        let value = i32::from_le_bytes([0, sample[0], sample[1], sample[2]]) >> 8;
        value as f32 / 8_388_608.0
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn klv(key: [u8; 16], value: &[u8]) -> Vec<u8> {
        let mut out = key.to_vec();
        let length = value.len() as u32;
        out.push(0x84);
        out.extend_from_slice(&length.to_be_bytes());
        out.extend_from_slice(value);
        out
    }

    fn essence(item: u8, number: u8) -> [u8; 16] {
        let mut key = [0u8; 16];
        key[..12].copy_from_slice(&ESSENCE_KEY);
        key[12] = item;
        key[13] = 2;
        key[14] = 3;
        key[15] = number;
        key
    }

    #[test]
    fn only_sound_elements_are_given_in_file_order_and_large_pictures_are_skipped() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let partition = klv([0x06, 0x0e, 0x2b, 0x34, 2, 5, 1, 1, 0x0d, 1, 2, 1, 1, 2, 4, 0], &[0; 100]);
        file.write_all(&partition).unwrap();
        for frame in 0..3u8 {
            file.write_all(&klv(essence(0x15, 0), &vec![9u8; 200_000])).unwrap();
            file.write_all(&klv(essence(SOUND_ITEM, 0), &[frame, 0, 0])).unwrap();
            file.write_all(&klv(essence(SOUND_ITEM, 1), &[frame, 1, 0])).unwrap();
            file.write_all(&klv(essence(0x17, 1), &[7u8; 5000])).unwrap();
        }
        file.flush().unwrap();
        let mut got = Vec::new();
        read_sound_elements(file.path(), &AtomicBool::new(false), &mut |number, value| {
            got.push((number, value.to_vec()));
            Ok(())
        })
        .unwrap();
        assert_eq!(got.len(), 6);
        assert_eq!(got[0], (0, vec![0, 0, 0]));
        assert_eq!(got[1], (1, vec![0, 1, 0]));
        assert_eq!(got[5], (1, vec![2, 1, 0]));
    }

    #[test]
    fn the_key_picture_at_or_after_each_target_comes_with_the_sound() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        for frame in 0..10u8 {
            // A key picture every 4 pictures: a sequence parameter set first.
            let nal = if frame % 4 == 0 { 0x67 } else { 0x41 };
            file.write_all(&klv(essence(PICTURE_ITEM, 0), &[0, 0, 0, 1, nal, frame, 9, 9])).unwrap();
            file.write_all(&klv(essence(SOUND_ITEM, 0), &[frame, 0, 0])).unwrap();
        }
        file.flush().unwrap();
        let (mut sound, mut pictures) = (0, Vec::new());
        read_sound_and_pictures(
            file.path(),
            &AtomicBool::new(false),
            &[0, 1, 5, 9],
            &h264_gop_start,
            &mut |_, _| {
                sound += 1;
                Ok(())
            },
            &mut |target, number, value| {
                pictures.push((target, number, value[5]));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(sound, 10);
        // 0 -> 0; 1 -> 4; 5 -> 8; 9 has none after it -> the last one, 8.
        assert_eq!(pictures, [(0, 0, 0), (1, 4, 4), (2, 8, 8), (3, 8, 8)]);
    }

    #[test]
    fn an_mpeg2_key_picture_begins_with_a_sequence_or_gop_header() {
        assert!(mpeg2_gop_start(&[0, 0, 1, 0xb3, 0x78, 4]));
        assert!(mpeg2_gop_start(&[0, 0, 1, 0xb8, 0, 8]));
        assert!(!mpeg2_gop_start(&[0, 0, 1, 0x00, 0, 0x1a]));
        assert!(key_picture_test("mpeg2video").is_some() && key_picture_test("prores").is_none());
    }

    #[test]
    fn a_file_without_sound_is_an_error_and_a_truncated_packet_too() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&klv(essence(0x15, 0), &[1u8; 50])).unwrap();
        file.flush().unwrap();
        let none = read_sound_elements(file.path(), &AtomicBool::new(false), &mut |_, _| Ok(()));
        assert!(none.is_err());
        let mut cut = klv(essence(SOUND_ITEM, 0), &[1u8; 30]);
        cut.truncate(40);
        file.write_all(&cut).unwrap();
        file.flush().unwrap();
        assert!(read_sound_elements(file.path(), &AtomicBool::new(false), &mut |_, _| Ok(())).is_err());
    }

    /// A camera original (QNC_MXF_SAMPLE) gives the same bytes on every sound track as
    /// FFmpeg (QNC_FFMPEG) gives for its audio streams. Read only.
    #[test]
    #[ignore = "needs a camera original and ffmpeg: QNC_MXF_SAMPLE, QNC_FFMPEG"]
    fn a_camera_original_gives_the_same_sound_as_ffmpeg() {
        let (Ok(sample), Ok(ffmpeg)) = (std::env::var("QNC_MXF_SAMPLE"), std::env::var("QNC_FFMPEG")) else {
            return;
        };
        let mut tracks: Vec<Vec<u8>> = Vec::new();
        read_sound_elements(Path::new(&sample), &AtomicBool::new(false), &mut |number, value| {
            let number = usize::from(number);
            if tracks.len() <= number {
                tracks.resize(number + 1, Vec::new());
            }
            tracks[number].extend_from_slice(value);
            Ok(())
        })
        .unwrap();
        assert!(!tracks.is_empty());
        for (number, track) in tracks.iter().enumerate() {
            let output = std::process::Command::new(&ffmpeg)
                .args(["-hide_banner", "-nostdin", "-loglevel", "error", "-f", "mxf", "-i", &sample])
                .args(["-map", &format!("0:a:{number}"), "-c:a", "pcm_s24le", "-f", "s24le", "-"])
                .output()
                .unwrap();
            assert!(output.status.success());
            assert_eq!(track.len(), output.stdout.len(), "track {number} length");
            assert!(track == &output.stdout, "track {number} bytes");
        }
    }

    #[test]
    fn s24_samples_become_the_same_f32_as_ffmpeg() {
        let mut out = Vec::new();
        s24le_to_f32(&[0xff, 0xff, 0x7f, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00], &mut out);
        assert_eq!(out, vec![8_388_607.0 / 8_388_608.0, -1.0, 0.0]);
    }
}
