//! A part of a frame-wrapped MXF original written as an MXF file of the same kind (user
//! 2026-10-23: Ingest SubClip, "it must be like the original"; a stream copy by ffmpeg
//! loses the display order of a Long GOP in MXF). The bytes of the original are copied:
//! the header partition with its metadata (durations and start timecode set for the part),
//! a new index, the content packages of the part and a footer with its random index pack.
//!
//! The part starts on an intra picture (user: "take the timecodes of the intra pictures"):
//! its pictures are the stored pictures from that intra picture on, without the leading
//! pictures of its group that refer to the group before; sound, system item (its timecode)
//! and data of each frame come from the package of that frame's place in time, so picture
//! and sound stay together frame by frame. The source is only read; nothing is decoded.

use std::{
    fs::File,
    io::{BufWriter, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use qnc_mxf_index::MxfIndex;

pub const MODULE_ID: &str = "qnc.module.mxf-cut";

const PARTITION_KEY: [u8; 13] = [0x06, 0x0e, 0x2b, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0d, 0x01, 0x02, 0x01, 0x01];
const INDEX_SEGMENT_KEY: [u8; 16] =
    [0x06, 0x0e, 0x2b, 0x34, 0x02, 0x53, 0x01, 0x01, 0x0d, 0x01, 0x02, 0x01, 0x01, 0x10, 0x01, 0x00];
const FILL_KEY: [u8; 16] =
    [0x06, 0x0e, 0x2b, 0x34, 0x01, 0x01, 0x01, 0x02, 0x03, 0x01, 0x02, 0x10, 0x01, 0x00, 0x00, 0x00];
const RIP_KEY: [u8; 16] =
    [0x06, 0x0e, 0x2b, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0d, 0x01, 0x02, 0x01, 0x01, 0x11, 0x01, 0x00];
const TIMECODE_COMPONENT: [u8; 2] = [0x01, 0x14];
/// Entries in one index segment at most (its entry array has a 2-byte length).
const ENTRIES_PER_SEGMENT: usize = 2048;

/// What a part of the original will hold: its first frame (a place in time of the
/// original), its length and which stored pictures it takes, in stored order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub first_frame: u64,
    pub frames: u64,
    stored: Vec<usize>,
}

/// The part covering frames `from..to` of the original (places in time), starting on the
/// intra picture at or before `from`.
pub fn plan(index: &MxfIndex, from: u64, to: u64) -> Result<Plan, String> {
    let count = index.entries.len();
    if from >= to || to as usize > count {
        return Err(qnc_ui_text::text("cut.range_outside_clip").into());
    }
    // The picture shown at place `d` is the stored picture d + temporal offset.
    let stored_at: Vec<usize> = (0..count)
        .map(|d| usize::try_from(d as i64 + i64::from(index.entries[d].temporal_offset)).ok().filter(|s| *s < count))
        .collect::<Option<_>>()
        .ok_or(qnc_ui_text::text("cut.index_order_not_readable"))?;
    let mut shown_at = vec![usize::MAX; count];
    for (d, s) in stored_at.iter().enumerate() {
        shown_at[*s] = d;
    }
    let start = (0..count)
        .filter(|s| index.entries[*s].intra() && (shown_at[*s] as u64) <= from)
        .max_by_key(|s| shown_at[*s])
        .ok_or(qnc_ui_text::text("cut.no_intra_before_in"))?;
    let first = shown_at[start];
    let intra_after: Vec<usize> = (start + 1..count).filter(|s| index.entries[*s].intra()).chain([count]).collect();
    for end in intra_after {
        // The stored pictures up to the next intra one, without those shown before the start.
        let stored: Vec<usize> = (start..end).filter(|s| shown_at[*s] >= first).collect();
        let mut shown: Vec<usize> = stored.iter().map(|s| shown_at[*s]).collect();
        shown.sort_unstable();
        if shown.iter().enumerate().any(|(i, d)| *d != first + i) {
            continue; // a group not closed yet: take the next one too
        }
        if (first + shown.len()) as u64 >= to || end == count {
            return Ok(Plan { first_frame: first as u64, frames: shown.len() as u64, stored });
        }
    }
    Err(qnc_ui_text::text("cut.index_order_not_readable").into())
}

/// Writes the part `plan` of `source` (its index `index`) to `dest`.
pub fn write(source: &Path, index: &MxfIndex, plan: &Plan, dest: &Path, cancel: &AtomicBool) -> Result<(), String> {
    let mut file = File::open(source).map_err(|e| format!("mxf cut open: {e}"))?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    let layout = Layout::read(&mut file)?;
    let mut header = read_at(&mut file, 0, layout.index_at as usize)?;
    patch_metadata(&mut header[layout.metadata_at..], index.entries.len() as u64, plan, index.rate)?;
    let template = index_template(&read_at(&mut file, layout.index_at, (layout.essence_at - layout.index_at) as usize)?)?;
    let package_end = |k: usize| index.package(k + 1).unwrap_or(layout.footer_at.min(size));
    let first = plan.first_frame as usize;
    let mut position: Vec<usize> = vec![usize::MAX; index.entries.len()];
    for (j, s) in plan.stored.iter().enumerate() {
        position[*s] = j;
    }
    // The packages go to a scratch file of the system first (a long part does not stay in
    // memory); the index that points at them is written before them in the part. The part is
    // written under its own name: a protected project folder allows no rename or delete.
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let essence = std::env::temp_dir().join(format!("qnc-mxf-cut-{}-{}.essence", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let written = (|| -> Result<(), String> {
        let mut scratch = BufWriter::new(File::create(&essence).map_err(|e| e.to_string())?);
        let mut entries = Vec::with_capacity(plan.stored.len());
        let mut offset = 0u64;
        for (j, stored) in plan.stored.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Err(qnc_ui_text::text("cut.interrupted").into());
            }
            let time = first + j;
            let picture_from = read_package(&mut file, index, *stored, package_end(*stored))?;
            let time_package = read_package(&mut file, index, time, package_end(time))?;
            let package = Package::join(&time_package, &picture_from)?;
            let old = &index.entries[*stored];
            // Place j of the part shows the stored picture its place in the original shows.
            let shows = (time as i64 + i64::from(index.entries[time].temporal_offset)) as usize;
            let to = position_of(&position, shows)? as i64 - j as i64;
            let key = position_of(&position, (*stored as i64 + i64::from(old.key_frame_offset)) as usize)? as i64 - j as i64;
            let to = i8::try_from(to).map_err(|_| qnc_ui_text::text("cut.index_order_not_readable").to_string())?;
            let key = i8::try_from(key).map_err(|_| qnc_ui_text::text("cut.index_order_not_readable").to_string())?;
            entries.push((to, key, old.flags, offset, package.slice_at));
            scratch.write_all(&package.bytes).map_err(|e| e.to_string())?;
            offset += package.bytes.len() as u64;
        }
        scratch.flush().map_err(|e| e.to_string())?;
        drop(scratch);
        let index_bytes = template.segments(index.rate, &entries, layout.kag)?;
        let footer_at = layout.index_at + index_bytes.len() as u64 + offset;
        patch_partition(&mut header[..layout.pack_len], footer_at, index_bytes.len() as u64)?;
        let mut out = BufWriter::new(File::create(dest).map_err(|e| e.to_string())?);
        out.write_all(&header).map_err(|e| e.to_string())?;
        out.write_all(&index_bytes).map_err(|e| e.to_string())?;
        std::io::copy(&mut File::open(&essence).map_err(|e| e.to_string())?, &mut out).map_err(|e| e.to_string())?;
        out.write_all(&footer(&header[..layout.pack_len], footer_at, layout.kag)).map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())
    })();
    let _ = std::fs::remove_file(&essence);
    written
}

/// Where stored picture `stored` of the original is in the part.
fn position_of(position: &[usize], stored: usize) -> Result<usize, String> {
    position
        .get(stored)
        .copied()
        .filter(|p| *p != usize::MAX)
        .ok_or_else(|| qnc_ui_text::text("cut.index_order_not_readable").to_string())
}

/// Where the parts of the header partition are in the original.
struct Layout {
    pack_len: usize,
    kag: u64,
    metadata_at: usize,
    index_at: u64,
    essence_at: u64,
    footer_at: u64,
}

impl Layout {
    fn read(file: &mut File) -> Result<Self, String> {
        let head = read_at(file, 0, 4096)?;
        let pack = klv(&head, 0).filter(|_| head[..13] == PARTITION_KEY).ok_or(qnc_ui_text::text("cut.not_mxf"))?;
        let value = &head[pack.0..pack.1];
        let mut metadata_at = pack.1;
        while head.len() >= metadata_at + 17 && head[metadata_at..metadata_at + 16] == FILL_KEY {
            metadata_at = klv(&head, metadata_at).ok_or(qnc_ui_text::text("cut.not_mxf"))?.1;
        }
        let index_at = metadata_at as u64 + be(&value[32..40]);
        Ok(Self {
            pack_len: pack.1,
            kag: be(&value[4..8]).max(1),
            metadata_at,
            index_at,
            essence_at: index_at + be(&value[40..48]),
            footer_at: be(&value[24..32]),
        })
    }
}

/// One built content package and where its second slice (after the picture) starts.
struct Package {
    bytes: Vec<u8>,
    slice_at: u32,
}

/// The picture element of a package and its fill: (start, end) in the package.
fn picture_chunk(package: &[u8]) -> Result<(usize, usize), String> {
    let mut at = 0;
    let mut start = None;
    while let Some((_, end)) = klv(package, at) {
        let key = &package[at..at + 16];
        if start.is_some() && key != FILL_KEY {
            return Ok((start.unwrap(), at));
        }
        if key[..12] == [0x06, 0x0e, 0x2b, 0x34, 0x01, 0x02, 0x01, 0x01, 0x0d, 0x01, 0x03, 0x01] && key[12] == 0x15 {
            start = Some(at);
        }
        at = end;
    }
    start.map(|start| (start, package.len())).ok_or_else(|| qnc_ui_text::text("cut.package_without_picture").to_string())
}

impl Package {
    /// The package of a place in time with the picture of another stored package.
    fn join(time: &[u8], picture: &[u8]) -> Result<Self, String> {
        let (time_start, time_end) = picture_chunk(time)?;
        let (picture_start, picture_end) = picture_chunk(picture)?;
        let mut bytes = Vec::with_capacity(time.len() + picture_end - picture_start);
        bytes.extend_from_slice(&time[..time_start]);
        bytes.extend_from_slice(&picture[picture_start..picture_end]);
        let slice_at = u32::try_from(bytes.len()).map_err(|e| e.to_string())?;
        bytes.extend_from_slice(&time[time_end..]);
        Ok(Self { bytes, slice_at })
    }
}

fn read_package(file: &mut File, index: &MxfIndex, k: usize, end: u64) -> Result<Vec<u8>, String> {
    let start = index.package(k).ok_or(qnc_ui_text::text("cut.index_order_not_readable"))?;
    read_at(file, start, (end - start) as usize)
}

/// Sets the durations of the header metadata to the part's and its start timecode to the
/// timecode of its first frame; every other byte stays the original's.
fn patch_metadata(metadata: &mut [u8], old_duration: u64, plan: &Plan, rate: (i32, i32)) -> Result<(), String> {
    let mut at = 0;
    while let Some((value_at, end)) = klv(metadata, at) {
        let key = metadata[at..at + 16].to_vec();
        // Local sets (two-byte tags and lengths); the primer and fill are not.
        if key[5] == 0x53 {
            let timecode = key[13..15] == TIMECODE_COMPONENT;
            let mut base = None;
            let mut tag_at = value_at;
            while tag_at + 4 <= end {
                let tag = u16::from_be_bytes([metadata[tag_at], metadata[tag_at + 1]]);
                let length = usize::from(u16::from_be_bytes([metadata[tag_at + 2], metadata[tag_at + 3]]));
                let item = tag_at + 4;
                if item + length > end {
                    break;
                }
                match tag {
                    0x0202 | 0x3002 if length == 8 && be(&metadata[item..item + 8]) == old_duration => {
                        metadata[item..item + 8].copy_from_slice(&plan.frames.to_be_bytes());
                    }
                    0x1502 if length == 2 => base = Some(be(&metadata[item..item + 2])),
                    _ => {}
                }
                tag_at = item + length;
            }
            if timecode {
                shift_start_timecode(&mut metadata[value_at..end], base, plan.first_frame, rate)?;
            }
        }
        at = end;
    }
    Ok(())
}

/// The start of a timecode component moved on by `frames` of the edit rate, counted in its
/// rounded timecode base.
fn shift_start_timecode(set: &mut [u8], base: Option<u64>, frames: u64, (num, den): (i32, i32)) -> Result<(), String> {
    let mut at = 0;
    while at + 4 <= set.len() {
        let tag = u16::from_be_bytes([set[at], set[at + 1]]);
        let length = usize::from(u16::from_be_bytes([set[at + 2], set[at + 3]]));
        if tag == 0x1501 && length == 8 {
            let base = base.ok_or(qnc_ui_text::text("cut.timecode_base_missing"))?;
            let per_second = (num as f64 / den.max(1) as f64).round() as u64;
            if per_second == 0 || (frames * base) % per_second != 0 {
                return Err(qnc_ui_text::text("cut.timecode_base_missing").into());
            }
            let start = be(&set[at + 4..at + 12]) + frames * base / per_second;
            set[at + 4..at + 12].copy_from_slice(&start.to_be_bytes());
        }
        at += 4 + length;
    }
    Ok(())
}

fn patch_partition(pack: &mut [u8], footer_at: u64, index_bytes: u64) -> Result<(), String> {
    let (value_at, _) = klv(pack, 0).ok_or(qnc_ui_text::text("cut.not_mxf"))?;
    pack[value_at + 24..value_at + 32].copy_from_slice(&footer_at.to_be_bytes());
    pack[value_at + 40..value_at + 48].copy_from_slice(&index_bytes.to_be_bytes());
    Ok(())
}

/// The footer partition (no metadata, no index) and the random index pack.
fn footer(header_pack: &[u8], footer_at: u64, kag: u64) -> Vec<u8> {
    let (value_at, end) = klv(header_pack, 0).expect("read before");
    let mut pack = header_pack[..end].to_vec();
    pack[13] = 0x04; // footer partition
    pack[14] = 0x04; // closed and complete
    let value = &mut pack[value_at..];
    value[8..16].copy_from_slice(&footer_at.to_be_bytes()); // this partition
    value[16..24].copy_from_slice(&0u64.to_be_bytes()); // the previous: the header
    value[24..32].copy_from_slice(&footer_at.to_be_bytes());
    for field in [32..40, 40..48, 52..60] {
        value[field].copy_from_slice(&0u64.to_be_bytes());
    }
    value[48..52].copy_from_slice(&0u32.to_be_bytes());
    value[60..64].copy_from_slice(&0u32.to_be_bytes());
    let mut out = pack;
    fill_to(&mut out, kag, footer_at);
    let mut rip = Vec::new();
    rip.extend_from_slice(&RIP_KEY);
    let body_sid = be(&header_pack[value_at + 60..value_at + 64]) as u32;
    let mut value = Vec::new();
    value.extend_from_slice(&body_sid.to_be_bytes());
    value.extend_from_slice(&0u64.to_be_bytes());
    value.extend_from_slice(&0u32.to_be_bytes());
    value.extend_from_slice(&footer_at.to_be_bytes());
    let total = (16 + 4 + value.len() + 4) as u32;
    value.extend_from_slice(&total.to_be_bytes());
    rip.extend_from_slice(&[0x83, 0, 0, value.len() as u8]);
    rip.extend_from_slice(&value);
    out.extend_from_slice(&rip);
    out
}

/// A fill item so that what follows `bytes` (written at `at`) starts on the KAG.
fn fill_to(bytes: &mut Vec<u8>, kag: u64, at: u64) {
    if kag <= 1 {
        return;
    }
    let end = at + bytes.len() as u64;
    let mut gap = (kag - end % kag) % kag;
    if gap == 0 {
        return;
    }
    while gap < 20 {
        gap += kag;
    }
    bytes.extend_from_slice(&FILL_KEY);
    let length = (gap - 20) as u32;
    bytes.extend_from_slice(&[0x83, (length >> 16) as u8, (length >> 8) as u8, length as u8]);
    bytes.resize(bytes.len() + length as usize, 0);
}

/// The items of the original's first index segment that every new segment repeats.
struct Template {
    items: Vec<(u16, Vec<u8>)>,
    entry_size: usize,
}

fn index_template(index: &[u8]) -> Result<Template, String> {
    let mut at = 0;
    while let Some((value_at, end)) = klv(index, at) {
        if index[at..at + 16] == INDEX_SEGMENT_KEY {
            let mut items = Vec::new();
            let mut entry_size = 0;
            let mut tag_at = value_at;
            while tag_at + 4 <= end {
                let tag = u16::from_be_bytes([index[tag_at], index[tag_at + 1]]);
                let length = usize::from(u16::from_be_bytes([index[tag_at + 2], index[tag_at + 3]]));
                let item = index[tag_at + 4..tag_at + 4 + length].to_vec();
                if tag == 0x3f0a {
                    entry_size = be(&item[4..8]) as usize;
                } else if !matches!(tag, 0x3f0c | 0x3f0d) {
                    items.push((tag, item));
                }
                tag_at += 4 + length;
            }
            let slices = items.iter().find(|(tag, _)| *tag == 0x3f08).map_or(0, |(_, v)| v[0] as usize);
            let positions = items.iter().find(|(tag, _)| *tag == 0x3f0e).map_or(0, |(_, v)| v[0] as usize);
            if slices > 1 || positions != 0 || entry_size != 11 + 4 * slices {
                return Err(qnc_ui_text::text("cut.index_kind_not_supported").into());
            }
            return Ok(Template { items, entry_size });
        }
        at = end;
    }
    Err(qnc_ui_text::text("cut.index_order_not_readable").into())
}

impl Template {
    /// The index of the part in segments of the original's kind, ending on the KAG.
    fn segments(&self, rate: (i32, i32), entries: &[(i8, i8, u8, u64, u32)], kag: u64) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        for (number, chunk) in entries.chunks(ENTRIES_PER_SEGMENT).enumerate() {
            let mut value = Vec::new();
            for (tag, item) in &self.items {
                let mut item = item.clone();
                if *tag == 0x3c0a {
                    item[15] = item[15].wrapping_add(number as u8 + 1); // a new instance per segment
                }
                if *tag == 0x3f0b {
                    item = [rate.0.to_be_bytes(), rate.1.to_be_bytes()].concat();
                }
                push_item(&mut value, *tag, &item)?;
            }
            push_item(&mut value, 0x3f0c, &((number * ENTRIES_PER_SEGMENT) as u64).to_be_bytes())?;
            push_item(&mut value, 0x3f0d, &(chunk.len() as u64).to_be_bytes())?;
            let mut array = Vec::with_capacity(8 + chunk.len() * self.entry_size);
            array.extend_from_slice(&(chunk.len() as u32).to_be_bytes());
            array.extend_from_slice(&(self.entry_size as u32).to_be_bytes());
            for (to, key, flags, offset, slice) in chunk {
                array.extend_from_slice(&[*to as u8, *key as u8, *flags]);
                array.extend_from_slice(&offset.to_be_bytes());
                if self.entry_size == 15 {
                    array.extend_from_slice(&slice.to_be_bytes());
                }
            }
            push_item(&mut value, 0x3f0a, &array)?;
            out.extend_from_slice(&INDEX_SEGMENT_KEY);
            let length = value.len() as u32;
            out.extend_from_slice(&[0x83, (length >> 16) as u8, (length >> 8) as u8, length as u8]);
            out.extend_from_slice(&value);
        }
        // The essence starts on the KAG (the index starts on it, as in the original).
        fill_to(&mut out, kag, 0);
        Ok(out)
    }
}

fn push_item(value: &mut Vec<u8>, tag: u16, item: &[u8]) -> Result<(), String> {
    let length = u16::try_from(item.len()).map_err(|_| qnc_ui_text::text("cut.index_kind_not_supported").to_string())?;
    value.extend_from_slice(&tag.to_be_bytes());
    value.extend_from_slice(&length.to_be_bytes());
    value.extend_from_slice(item);
    Ok(())
}

/// The value range of the KLV at `at`: (value start, value end).
fn klv(bytes: &[u8], at: usize) -> Option<(usize, usize)> {
    let first = *bytes.get(at + 16)?;
    let (length, size) = if first < 0x80 {
        (first as usize, 1)
    } else {
        let n = usize::from(first & 0x7f);
        (bytes.get(at + 17..at + 17 + n)?.iter().fold(0usize, |a, b| a << 8 | usize::from(*b)), 1 + n)
    };
    let value_at = at + 16 + size;
    let end = value_at.checked_add(length)?;
    (end <= bytes.len()).then_some((value_at, end))
}

fn be(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0, |a, b| a << 8 | u64::from(*b))
}

fn read_at(file: &mut File, at: u64, length: usize) -> Result<Vec<u8>, String> {
    file.seek(SeekFrom::Start(at)).map_err(|e| e.to_string())?;
    let mut bytes = vec![0; length];
    file.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

#[cfg(test)]
mod tests;
