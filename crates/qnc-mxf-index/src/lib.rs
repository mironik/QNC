//! The index of a frame-wrapped MXF original: where each frame's content package lies in
//! the file and which pictures are intra, so a filmstrip (and a coarse wave) reads only the
//! frames it needs instead of walking the file (user 2026-10-18: "the intra picture nearest
//! each of 13 places"; a camera's XAVC original keeps a full index after its header). Only
//! an index the file itself carries is used: a file whose index is missing, does not cover
//! the clip or whose essence is spread over more than one partition gives `None`, and the
//! caller reads the media as before. Read only: nothing is written, created, renamed or
//! removed.

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

pub const MODULE_ID: &str = "qnc.module.mxf-index";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const MXF_KEY: [u8; 4] = [0x06, 0x0e, 0x2b, 0x34];
/// A partition pack: the key without its kind and status bytes.
const PARTITION_KEY: [u8; 13] = [0x06, 0x0e, 0x2b, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0d, 0x01, 0x02, 0x01, 0x01];
const INDEX_SEGMENT_KEY: [u8; 16] =
    [0x06, 0x0e, 0x2b, 0x34, 0x02, 0x53, 0x01, 0x01, 0x0d, 0x01, 0x02, 0x01, 0x01, 0x10, 0x01, 0x00];
const RANDOM_INDEX_KEY: [u8; 16] =
    [0x06, 0x0e, 0x2b, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0d, 0x01, 0x02, 0x01, 0x01, 0x11, 0x01, 0x00];
/// The key of a generic container essence element, before its item type.
const ESSENCE_KEY: [u8; 12] = [0x06, 0x0e, 0x2b, 0x34, 0x01, 0x02, 0x01, 0x01, 0x0d, 0x01, 0x03, 0x01];
const PICTURE_ITEM: u8 = 0x15;
const SOUND_ITEM: u8 = 0x16;
/// The header metadata and the index are read in one go up to this size.
const MOST_HEADER: u64 = 64 * 1024 * 1024;
/// How far after the index the first content package is looked for (fill).
const ESSENCE_SEARCH: usize = 64 * 1024;
/// The start of a content package read to find its elements: the system item and the key
/// of the picture follow its start.
const PACKAGE_HEAD: usize = 64 * 1024;

/// One frame of the index, in stored order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub temporal_offset: i8,
    pub key_frame_offset: i8,
    pub flags: u8,
    /// Where the frame's content package starts, from the start of the essence.
    pub stream_offset: u64,
}

impl Entry {
    /// The picture of this package decodes on its own (no prediction, SMPTE 377-1 flags).
    pub fn intra(&self) -> bool {
        self.flags & 0x30 == 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MxfIndex {
    /// Edit rate of the index (frames per second as a fraction).
    pub rate: (i32, i32),
    /// Where the essence starts in the file.
    pub essence_start: u64,
    pub entries: Vec<Entry>,
}

impl MxfIndex {
    /// The index of `path`, or `None` when the file carries none that this reader can use.
    pub fn read(path: &Path) -> Result<Option<Self>, String> {
        let mut file = File::open(path).map_err(|error| format!("mxf index open: {error}"))?;
        let size = file.metadata().map_err(|error| format!("mxf index size: {error}"))?.len();
        if size < 64 {
            return Ok(None);
        }
        // Every partition must hold the one essence container of the header partition: one
        // stream, one place, so stream offsets are file offsets past its start.
        if !one_essence_partition(&mut file, size)? {
            return Ok(None);
        }
        let head = read_at(&mut file, 0, 512.min(size as usize))?;
        let Some(pack) = klv(&head, 0) else { return Ok(None) };
        if head[..13] != PARTITION_KEY || pack.value_end > head.len() {
            return Ok(None);
        }
        let value = &head[pack.value_at..pack.value_end];
        if value.len() < 64 {
            return Ok(None);
        }
        let header_bytes = be(&value[32..40]);
        let index_bytes = be(&value[40..48]);
        let body_sid = be(&value[60..64]);
        if index_bytes == 0 || body_sid == 0 {
            return Ok(None);
        }
        // The byte counts start after the fill that follows the partition pack.
        let mut metadata_at = pack.value_end;
        while head.len() >= metadata_at + 17 && is_fill(&head[metadata_at..metadata_at + 16]) {
            match klv(&head, metadata_at) {
                Some(fill) => metadata_at = fill.value_end,
                None => return Ok(None),
            }
        }
        let index_at = metadata_at as u64 + header_bytes;
        let index_end = index_at + index_bytes;
        if index_end > size || index_end > MOST_HEADER {
            return Ok(None);
        }
        let index = read_at(&mut file, index_at, index_bytes as usize)?;
        let mut rate = None;
        let mut entries: Vec<Entry> = Vec::new();
        let mut at = 0usize;
        while let Some(packet) = klv(&index, at) {
            if packet.value_end > index.len() {
                return Ok(None);
            }
            if index[at..at + 16] == INDEX_SEGMENT_KEY {
                match segment(&index[packet.value_at..packet.value_end], entries.len() as u64)? {
                    Some((segment_rate, segment_entries)) => {
                        if rate.is_some_and(|rate| rate != segment_rate) {
                            return Ok(None);
                        }
                        rate = Some(segment_rate);
                        entries.extend(segment_entries);
                    }
                    None => return Ok(None),
                }
            }
            at = packet.value_end;
        }
        let Some(rate) = rate.filter(|_| !entries.is_empty()) else { return Ok(None) };
        let Some(essence_start) = essence_start(&mut file, index_end, size)? else { return Ok(None) };
        let index = Self { rate, essence_start, entries };
        // A package found where the index says confirms how offsets are read.
        let last = index.package(index.entries.len() - 1).filter(|at| *at + 17 <= size);
        for place in [index.package(0), last] {
            let Some(place) = place else { return Ok(None) };
            if read_at(&mut file, place, 4)? != MXF_KEY {
                return Ok(None);
            }
        }
        Ok(Some(index))
    }

    /// Where frame `entry`'s content package starts in the file.
    pub fn package(&self, entry: usize) -> Option<u64> {
        self.entries.get(entry).map(|found| self.essence_start + found.stream_offset)
    }

    /// The intra frame nearest `frame` (the earlier of two as near); `None` without one.
    pub fn intra_nearest(&self, frame: u64) -> Option<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.intra())
            .min_by_key(|(index, _)| ((*index as i64 - frame as i64).abs(), *index))
            .map(|(index, _)| index)
    }

    /// Every intra frame, in order.
    pub fn intra_frames(&self) -> Vec<usize> {
        self.entries.iter().enumerate().filter(|(_, entry)| entry.intra()).map(|(index, _)| index).collect()
    }
}

/// The elements of one content package read from the file.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Package {
    /// The picture element's value, as stored.
    pub picture: Option<Vec<u8>>,
    /// The sound elements as (element number, value), as stored.
    pub sound: Vec<(u8, Vec<u8>)>,
}

/// Reads frame `entry`'s package: its picture when `picture`, its sound when `sound`.
pub fn read_package(file: &mut File, index: &MxfIndex, entry: usize, picture: bool, sound: bool) -> Result<Package, String> {
    let Some(start) = index.package(entry) else {
        return Err("mxf index: no such frame".into());
    };
    let end = index.package(entry + 1).unwrap_or(u64::MAX);
    let size = file.metadata().map_err(|error| format!("mxf index size: {error}"))?.len();
    let end = end.min(size);
    let head_length = PACKAGE_HEAD.min((end - start) as usize);
    let head = read_at(file, start, head_length)?;
    let mut package = Package::default();
    let mut at = 0usize;
    while at + 17 <= head.len() {
        let Some(packet) = klv(&head, at) else { break };
        let key = &head[at..at + 16];
        if key[..12] == ESSENCE_KEY && key[12] == PICTURE_ITEM && picture {
            let length = packet.value_end - packet.value_at;
            package.picture = Some(if packet.value_end <= head.len() {
                head[packet.value_at..packet.value_end].to_vec()
            } else {
                read_at(file, start + packet.value_at as u64, length)?
            });
        } else if key[..12] == ESSENCE_KEY && key[12] == SOUND_ITEM && sound {
            let value = if packet.value_end <= head.len() {
                head[packet.value_at..packet.value_end].to_vec()
            } else {
                read_at(file, start + packet.value_at as u64, packet.value_end - packet.value_at)?
            };
            package.sound.push((key[15], value));
        }
        if packet.value_end > head.len() {
            // The rest of the package (after a long picture) is read from where it is.
            if !sound {
                break;
            }
            let rest_at = start + packet.value_end as u64;
            if rest_at >= end {
                break;
            }
            let rest = read_at(file, rest_at, PACKAGE_HEAD.min((end - rest_at) as usize))?;
            let mut inner = 0usize;
            while inner + 17 <= rest.len() {
                let Some(next) = klv(&rest, inner) else { break };
                let key = &rest[inner..inner + 16];
                if key[..12] == ESSENCE_KEY && key[12] == SOUND_ITEM {
                    let value = if next.value_end <= rest.len() {
                        rest[next.value_at..next.value_end].to_vec()
                    } else {
                        read_at(file, rest_at + next.value_at as u64, next.value_end - next.value_at)?
                    };
                    package.sound.push((key[15], value));
                }
                if next.value_end > rest.len() {
                    break;
                }
                inner = next.value_end;
            }
            break;
        }
        at = packet.value_end;
    }
    Ok(package)
}

struct Klv {
    value_at: usize,
    value_end: usize,
}

fn klv(bytes: &[u8], at: usize) -> Option<Klv> {
    if at + 17 > bytes.len() || bytes[at..at + 4] != MXF_KEY {
        return None;
    }
    let first = bytes[at + 16];
    let (length, length_bytes) = if first < 0x80 {
        (u64::from(first), 1)
    } else {
        let count = usize::from(first & 0x7f);
        if count == 0 || count > 8 || at + 17 + count > bytes.len() {
            return None;
        }
        (be(&bytes[at + 17..at + 17 + count]), 1 + count)
    };
    let value_at = at + 16 + length_bytes;
    let value_end = value_at.checked_add(usize::try_from(length).ok()?)?;
    Some(Klv { value_at, value_end })
}

/// A KLV fill item (SMPTE 336, any version byte).
fn is_fill(key: &[u8]) -> bool {
    key[..4] == MXF_KEY && key[4..7] == [0x01, 0x01, 0x01] && key[8..16] == [0x03, 0x01, 0x02, 0x10, 0x01, 0x00, 0x00, 0x00]
}

fn be(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0u64, |value, byte| (value << 8) | u64::from(*byte))
}

/// The most one read of this block takes: a length from the file past it is a broken file,
/// not an allocation (audit 2026-10-25: a KLV length of 2^64 aborted the worker).
const MAX_ITEM: usize = 256 << 20;

fn read_at(file: &mut File, at: u64, length: usize) -> Result<Vec<u8>, String> {
    if length > MAX_ITEM {
        return Err(format!("mxf index: an item of {length} bytes is past what a file holds"));
    }
    file.seek(SeekFrom::Start(at)).map_err(|error| format!("mxf index seek: {error}"))?;
    let mut bytes = vec![0u8; length];
    file.read_exact(&mut bytes).map_err(|error| format!("mxf index read: {error}"))?;
    Ok(bytes)
}

/// One index table segment: its edit rate and entries, when it is a variable-size index
/// that continues the entries read before it.
fn segment(value: &[u8], next: u64) -> Result<Option<((i32, i32), Vec<Entry>)>, String> {
    let mut rate = None;
    let mut start = None;
    let mut edit_unit_bytes = 0u64;
    let mut entries = None;
    let mut at = 0usize;
    while at + 4 <= value.len() {
        let tag = u16::from_be_bytes([value[at], value[at + 1]]);
        let length = usize::from(u16::from_be_bytes([value[at + 2], value[at + 3]]));
        let item = value.get(at + 4..at + 4 + length).ok_or("mxf index: segment item past its end")?;
        match tag {
            0x3f0b if length == 8 => {
                rate = Some((
                    i32::from_be_bytes(item[0..4].try_into().unwrap()),
                    i32::from_be_bytes(item[4..8].try_into().unwrap()),
                ))
            }
            0x3f0c if length == 8 => start = Some(be(item)),
            0x3f05 if length == 4 => edit_unit_bytes = be(item),
            0x3f0a if length >= 8 => {
                let count = be(&item[0..4]) as usize;
                let size = be(&item[4..8]) as usize;
                let table = count.checked_mul(size).and_then(|bytes| bytes.checked_add(8));
                if size < 11 || table.is_none_or(|table| item.len() < table) {
                    return Ok(None);
                }
                entries = Some(
                    (0..count)
                        .map(|index| {
                            let row = &item[8 + index * size..8 + (index + 1) * size];
                            Entry {
                                temporal_offset: row[0] as i8,
                                key_frame_offset: row[1] as i8,
                                flags: row[2],
                                stream_offset: be(&row[3..11]),
                            }
                        })
                        .collect::<Vec<_>>(),
                );
            }
            _ => {}
        }
        at += 4 + length;
    }
    match (rate, start, entries) {
        (Some(rate), Some(start), Some(entries)) if edit_unit_bytes == 0 && start == next => Ok(Some((rate, entries))),
        _ => Ok(None),
    }
}

/// Whether the file's partitions (from its random index pack) hold one essence container,
/// all in the header partition.
fn one_essence_partition(file: &mut File, size: u64) -> Result<bool, String> {
    let tail = read_at(file, size - 4, 4)?;
    let length = be(&tail);
    if length < 20 || length > size || length > 1024 * 1024 {
        return Ok(false);
    }
    let pack = read_at(file, size - length, length as usize)?;
    if pack[..16] != RANDOM_INDEX_KEY {
        return Ok(false);
    }
    let Some(found) = klv(&pack, 0) else { return Ok(false) };
    // Rows of (body stream, partition place), then the length of the whole pack.
    let end = found.value_end.min(pack.len());
    if end < found.value_at + 4 {
        return Ok(false);
    }
    let body = &pack[found.value_at..end - 4];
    let mut essence_partitions = 0;
    for row in body.chunks_exact(12) {
        let (sid, offset) = (be(&row[0..4]), be(&row[4..12]));
        if sid != 0 {
            essence_partitions += 1;
            if offset != 0 {
                return Ok(false);
            }
        }
    }
    Ok(essence_partitions == 1)
}

/// The first content package after the index (fill skipped).
fn essence_start(file: &mut File, from: u64, size: u64) -> Result<Option<u64>, String> {
    let bytes = read_at(file, from, ESSENCE_SEARCH.min((size - from) as usize))?;
    let mut at = 0usize;
    while at + 16 <= bytes.len() {
        if bytes[at..at + 4] == MXF_KEY {
            // An essence element or a system item of the generic container.
            if bytes[at + 8..at + 12] == ESSENCE_KEY[8..12] {
                return Ok(Some(from + at as u64));
            }
            // Fill or another set: skip it whole.
            match klv(&bytes, at) {
                Some(packet) => {
                    at = packet.value_end;
                    continue;
                }
                None => return Ok(None),
            }
        }
        at += 1;
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Audit 2026-10-25: a KLV length from a broken file aborted the worker on allocation.
    #[test]
    fn a_length_past_any_file_is_an_error_not_an_allocation() {
        let path = std::env::temp_dir().join(format!("qnc-mxf-index-test-{}", std::process::id()));
        std::fs::write(&path, [0u8; 16]).unwrap();
        let mut file = File::open(&path).unwrap();
        assert!(read_at(&mut file, 0, usize::MAX).is_err());
        assert_eq!(read_at(&mut file, 0, 16).unwrap().len(), 16);
        drop(file);
        std::fs::remove_file(&path).unwrap();
    }

    fn entry(flags: u8, stream_offset: u64) -> Entry {
        Entry { temporal_offset: 0, key_frame_offset: 0, flags, stream_offset }
    }

    #[test]
    fn the_nearest_intra_frame_is_taken_and_the_earlier_of_two_as_near() {
        // A group of 24 pictures: intra at 0, 24 and 48 (SMPTE flags: 0xc4 and 0x40 intra,
        // 0x22 and 0x33 predicted).
        let entries = (0..60).map(|index| entry(if index % 24 == 0 { if index == 0 { 0xc4 } else { 0x40 } } else if index % 3 == 0 { 0x22 } else { 0x33 }, index * 1000)).collect();
        let index = MxfIndex { rate: (50, 1), essence_start: 1_048_576, entries };
        assert_eq!(index.intra_frames(), vec![0, 24, 48]);
        assert_eq!(index.intra_nearest(10), Some(0));
        assert_eq!(index.intra_nearest(12), Some(0), "as near: the earlier");
        assert_eq!(index.intra_nearest(13), Some(24));
        assert_eq!(index.intra_nearest(1000), Some(48));
        assert_eq!(index.package(2), Some(1_048_576 + 2000));
        assert_eq!(index.package(60), None);
    }

    #[test]
    fn a_file_without_an_index_gives_none_and_is_not_changed() {
        let dir = std::env::temp_dir().join(format!("qnc-mxf-index-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("not.mxf");
        std::fs::write(&file, vec![7u8; 4096]).unwrap();
        assert_eq!(MxfIndex::read(&file).unwrap(), None);
        assert_eq!(std::fs::read(&file).unwrap(), vec![7u8; 4096]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A camera original (read only): `QNC_MXF_SAMPLE=<file> cargo test -p qnc-mxf-index -- --ignored`.
    #[test]
    #[ignore]
    fn a_camera_original_gives_its_index_and_intra_pictures() {
        let path = std::env::var("QNC_MXF_SAMPLE").expect("QNC_MXF_SAMPLE");
        let index = MxfIndex::read(Path::new(&path)).unwrap().expect("an index");
        let intra = index.intra_frames();
        assert!(!intra.is_empty());
        let mut file = File::open(&path).unwrap();
        let package = read_package(&mut file, &index, intra[intra.len() / 2], true, true).unwrap();
        assert!(package.picture.is_some_and(|picture| !picture.is_empty()));
        assert!(!package.sound.is_empty());
    }
}
