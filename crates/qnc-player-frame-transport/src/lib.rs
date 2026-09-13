//! Local mailbox for passive QNC monitors.
//!
//! This module does not decode, probe, read databases, or own playback time.
//! The player posts the current picture and signals. The monitor receives that
//! mailbox. It does not drain history and does not own a display clock.

mod wake;

use memmap2::{Mmap, MmapMut, MmapOptions};
use qnc_player_contract::session::{MonitorHeader, SessionQuery};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    path::Path,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use wake::Wake;

pub const MAGIC: &[u8; 8] = b"QNCFRM03";
pub const VERSION: u8 = 1;
pub const SLOT_COUNT: usize = 8;
pub const HEADER_BYTES: usize = 64;
pub const SLOT_PREFIX_BYTES: usize = 32;
pub const METADATA_BYTES: usize = 8192;
pub const DEFAULT_FRAME_CAPACITY: usize = qnc_player_contract::session::MAX_FRAME_BYTES - 8192;
pub const MONITOR_PREVIEW_FRAME_CAPACITY: usize = 8 * 1024 * 1024;

const VERSION_OFFSET: usize = 8;
const SLOT_COUNT_OFFSET: usize = 9;
const CAPACITY_OFFSET: usize = 16;
const PUBLISHED_OFFSET: usize = 24;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LatestFrame {
    pub header: MonitorHeader,
    pub rgba: Arc<[u8]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LatestFrameUpdate {
    Clear,
    Picture(LatestFrame),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FrameMetadata {
    header: Option<MonitorHeader>,
}

pub struct LatestFrameWriter {
    mmap: MmapMut,
    capacity: usize,
    published: u64,
    wake: Wake,
}

impl LatestFrameWriter {
    pub fn create(path: impl AsRef<Path>, capacity: usize) -> Result<Self, String> {
        validate_capacity(capacity)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path.as_ref())
            .map_err(|e| e.to_string())?;
        file.set_len(file_len(capacity)? as u64)
            .map_err(|e| e.to_string())?;
        let mut mmap = map_mut(&file)?;
        mmap[..HEADER_BYTES].fill(0);
        mmap[..8].copy_from_slice(MAGIC);
        mmap[VERSION_OFFSET] = VERSION;
        mmap[SLOT_COUNT_OFFSET] = SLOT_COUNT as u8;
        write_u64(
            &mut mmap[CAPACITY_OFFSET..CAPACITY_OFFSET + 8],
            capacity as u64,
        );
        write_u64(&mut mmap[PUBLISHED_OFFSET..PUBLISHED_OFFSET + 8], 0);
        mmap.flush().map_err(|e| e.to_string())?;
        Ok(Self {
            mmap,
            capacity,
            published: 0,
            wake: Wake::create(path.as_ref())?,
        })
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path.as_ref())
            .map_err(|e| e.to_string())?;
        let mmap = map_mut(&file)?;
        let capacity = validate_header(&mmap)?;
        Ok(Self {
            published: read_u64(&mmap[PUBLISHED_OFFSET..PUBLISHED_OFFSET + 8]),
            mmap,
            capacity,
            wake: Wake::open(path.as_ref())?,
        })
    }

    pub fn clear(&mut self) -> Result<u64, String> {
        self.publish_record(None, &[])
    }

    pub fn publish(&mut self, header: &MonitorHeader, rgba: &[u8]) -> Result<u64, String> {
        let query = SessionQuery {
            contract_version: header.contract_version.clone(),
            session_id: header.session_id.clone(),
            source_generation: header.source_generation,
        };
        header.validate(&query, &header.source_id, rgba.len())?;
        self.publish_record(Some(header), rgba)
    }

    fn publish_record(
        &mut self,
        header: Option<&MonitorHeader>,
        rgba: &[u8],
    ) -> Result<u64, String> {
        if rgba.len() > self.capacity {
            return Err("latest-frame payload exceeds capacity".into());
        }
        let metadata = serde_json::to_vec(&FrameMetadata {
            header: header.cloned(),
        })
        .map_err(|e| e.to_string())?;
        if metadata.len() > METADATA_BYTES {
            return Err("latest-frame metadata exceeds capacity".into());
        }
        let generation = self
            .published
            .checked_add(1)
            .ok_or("latest-frame generation exhausted")?;
        let slot = (generation as usize) % SLOT_COUNT;
        let offset = slot_offset(self.capacity, slot);
        write_u64(&mut self.mmap[offset..offset + 8], 0);
        write_u32(
            &mut self.mmap[offset + 8..offset + 12],
            metadata.len() as u32,
        );
        write_u32(&mut self.mmap[offset + 12..offset + 16], rgba.len() as u32);
        write_u64(&mut self.mmap[offset + 16..offset + 24], generation);
        let metadata_offset = offset + SLOT_PREFIX_BYTES;
        let payload_offset = metadata_offset + METADATA_BYTES;
        self.mmap[metadata_offset..metadata_offset + METADATA_BYTES].fill(0);
        self.mmap[metadata_offset..metadata_offset + metadata.len()].copy_from_slice(&metadata);
        self.mmap[payload_offset..payload_offset + rgba.len()].copy_from_slice(rgba);
        write_u64(&mut self.mmap[offset..offset + 8], generation);
        write_u64(
            &mut self.mmap[PUBLISHED_OFFSET..PUBLISHED_OFFSET + 8],
            generation,
        );
        // Memory visibility between mappings does not require a disk flush.
        // Flushing every monitor frame turns IPC into storage work and causes
        // avoidable playback stalls on Windows.
        self.published = generation;
        self.wake.signal()?;
        Ok(generation)
    }
}

pub struct LatestFrameReader {
    mmap: Mmap,
    capacity: usize,
    observed: u64,
    wake: Wake,
}

impl LatestFrameReader {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let file = File::open(path.as_ref()).map_err(|e| e.to_string())?;
        let mmap = unsafe { MmapOptions::new().map(&file) }.map_err(|e| e.to_string())?;
        let capacity = validate_header(&mmap)?;
        Ok(Self {
            mmap,
            capacity,
            observed: 0,
            wake: Wake::open(path.as_ref())?,
        })
    }

    /// Block until the player posts a mailbox, then receive that picture.
    /// Does not walk unpublished history.
    pub fn recv(&mut self, timeout: Duration) -> Result<Option<LatestFrameUpdate>, String> {
        if let Some(update) = self.read_newest()? {
            return Ok(Some(update));
        }
        let deadline = Instant::now() + timeout;
        loop {
            let remain = deadline.saturating_duration_since(Instant::now());
            if remain.is_zero() {
                return self.read_newest();
            }
            // A wake can arrive while the writer is still filling the slot.
            // Retry the newest present immediately so the picture does not
            // sit behind the audio clock.
            let signaled = self.wake.wait(remain.min(Duration::from_millis(2)))?;
            for _ in 0..8 {
                if let Some(update) = self.read_newest()? {
                    return Ok(Some(update));
                }
                if signaled {
                    thread::yield_now();
                } else {
                    break;
                }
            }
        }
    }

    pub fn read_latest(&mut self) -> Result<Option<LatestFrameUpdate>, String> {
        let published = read_u64(&self.mmap[PUBLISHED_OFFSET..PUBLISHED_OFFSET + 8]);
        if published == 0 || published <= self.observed {
            return Ok(None);
        }
        let oldest_available = published
            .saturating_sub((SLOT_COUNT as u64).saturating_sub(1))
            .max(1);
        let generation = if self.observed == 0 {
            oldest_available
        } else {
            self.observed.saturating_add(1).max(oldest_available)
        };
        if generation > published {
            return Ok(None);
        }
        let slot = (generation as usize) % SLOT_COUNT;
        let offset = slot_offset(self.capacity, slot);
        let begin = read_u64(&self.mmap[offset..offset + 8]);
        let metadata_len = read_u32(&self.mmap[offset + 8..offset + 12]) as usize;
        let payload_len = read_u32(&self.mmap[offset + 12..offset + 16]) as usize;
        let end = read_u64(&self.mmap[offset + 16..offset + 24]);
        if begin != generation || end != generation {
            if generation < published {
                self.observed = generation;
            }
            return Ok(None);
        }
        if metadata_len == 0 || metadata_len > METADATA_BYTES || payload_len > self.capacity {
            return Err("latest-frame slot is invalid".into());
        }
        let metadata_offset = offset + SLOT_PREFIX_BYTES;
        let payload_offset = metadata_offset + METADATA_BYTES;
        let metadata: FrameMetadata =
            serde_json::from_slice(&self.mmap[metadata_offset..metadata_offset + metadata_len])
                .map_err(|e| e.to_string())?;
        self.observed = generation;
        let Some(header) = metadata.header else {
            return Ok(Some(LatestFrameUpdate::Clear));
        };
        let rgba: Arc<[u8]> = self.mmap[payload_offset..payload_offset + payload_len]
            .to_vec()
            .into();
        let query = SessionQuery {
            contract_version: header.contract_version.clone(),
            session_id: header.session_id.clone(),
            source_generation: header.source_generation,
        };
        header.validate(&query, &header.source_id, rgba.len())?;
        Ok(Some(LatestFrameUpdate::Picture(LatestFrame {
            header,
            rgba,
        })))
    }

    pub fn read_newest(&mut self) -> Result<Option<LatestFrameUpdate>, String> {
        let published = read_u64(&self.mmap[PUBLISHED_OFFSET..PUBLISHED_OFFSET + 8]);
        if published == 0 || published <= self.observed {
            return Ok(None);
        }
        self.read_generation(published)
    }

    fn read_generation(&mut self, generation: u64) -> Result<Option<LatestFrameUpdate>, String> {
        let slot = (generation as usize) % SLOT_COUNT;
        let offset = slot_offset(self.capacity, slot);
        let begin = read_u64(&self.mmap[offset..offset + 8]);
        let metadata_len = read_u32(&self.mmap[offset + 8..offset + 12]) as usize;
        let payload_len = read_u32(&self.mmap[offset + 12..offset + 16]) as usize;
        let end = read_u64(&self.mmap[offset + 16..offset + 24]);
        if begin != generation || end != generation {
            return Ok(None);
        }
        if metadata_len == 0 || metadata_len > METADATA_BYTES || payload_len > self.capacity {
            return Err("latest-frame slot is invalid".into());
        }
        let metadata_offset = offset + SLOT_PREFIX_BYTES;
        let payload_offset = metadata_offset + METADATA_BYTES;
        let metadata: FrameMetadata =
            serde_json::from_slice(&self.mmap[metadata_offset..metadata_offset + metadata_len])
                .map_err(|e| e.to_string())?;
        self.observed = generation;
        let Some(header) = metadata.header else {
            return Ok(Some(LatestFrameUpdate::Clear));
        };
        let rgba: Arc<[u8]> = self.mmap[payload_offset..payload_offset + payload_len]
            .to_vec()
            .into();
        let query = SessionQuery {
            contract_version: header.contract_version.clone(),
            session_id: header.session_id.clone(),
            source_generation: header.source_generation,
        };
        header.validate(&query, &header.source_id, rgba.len())?;
        Ok(Some(LatestFrameUpdate::Picture(LatestFrame {
            header,
            rgba,
        })))
    }
}

fn validate_capacity(capacity: usize) -> Result<(), String> {
    if capacity == 0 || capacity > qnc_player_contract::session::MAX_FRAME_BYTES {
        return Err("invalid latest-frame capacity".into());
    }
    Ok(())
}

fn validate_header(bytes: &[u8]) -> Result<usize, String> {
    if bytes.len() < HEADER_BYTES || &bytes[..8] != MAGIC || bytes[VERSION_OFFSET] != VERSION {
        return Err("latest-frame transport header mismatch".into());
    }
    if bytes[SLOT_COUNT_OFFSET] != SLOT_COUNT as u8 {
        return Err("latest-frame slot count mismatch".into());
    }
    let capacity = read_u64(&bytes[CAPACITY_OFFSET..CAPACITY_OFFSET + 8]) as usize;
    validate_capacity(capacity)?;
    if bytes.len() != file_len(capacity)? {
        return Err("latest-frame transport size mismatch".into());
    }
    Ok(capacity)
}

fn file_len(capacity: usize) -> Result<usize, String> {
    HEADER_BYTES
        .checked_add(
            SLOT_COUNT
                .checked_mul(
                    SLOT_PREFIX_BYTES
                        .checked_add(METADATA_BYTES)
                        .and_then(|v| v.checked_add(capacity))
                        .ok_or("latest-frame size overflow")?,
                )
                .ok_or("latest-frame size overflow")?,
        )
        .ok_or_else(|| "latest-frame size overflow".into())
}

fn slot_offset(capacity: usize, slot: usize) -> usize {
    HEADER_BYTES + slot * (SLOT_PREFIX_BYTES + METADATA_BYTES + capacity)
}

fn map_mut(file: &File) -> Result<MmapMut, String> {
    unsafe { MmapOptions::new().map_mut(file) }.map_err(|e| e.to_string())
}

fn read_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes.try_into().expect("u32 slice"))
}

fn read_u64(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().expect("u64 slice"))
}

fn write_u32(bytes: &mut [u8], value: u32) {
    bytes.copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], value: u64) {
    bytes.copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "qnc-frame-transport-{name}-{}-{}.map",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn header(sequence: u64, frame: u64) -> MonitorHeader {
        MonitorHeader {
            contract_version: qnc_player_contract::VERSION.into(),
            session_id: "session".into(),
            source_generation: 1,
            output_generation: 2,
            sequence,
            source_id: "clip".into(),
            frame,
            timebase: qnc_player_contract::Timebase::new(50, 1).unwrap(),
            width: 2,
            height: 1,
        }
    }

    #[test]
    fn reader_observes_only_new_complete_latest_frame() {
        let path = path("latest");
        let mut writer = LatestFrameWriter::create(&path, 128).unwrap();
        let mut reader = LatestFrameReader::open(&path).unwrap();
        assert!(reader.read_latest().unwrap().is_none());
        writer.publish(&header(1, 10), &[1; 8]).unwrap();
        let LatestFrameUpdate::Picture(first) = reader.read_latest().unwrap().unwrap() else {
            panic!("expected picture");
        };
        assert_eq!(first.header.frame, 10);
        assert_eq!(&*first.rgba, &[1; 8]);
        assert!(reader.read_latest().unwrap().is_none());
        writer.publish(&header(2, 11), &[2; 8]).unwrap();
        let LatestFrameUpdate::Picture(second) = reader.read_latest().unwrap().unwrap() else {
            panic!("expected picture");
        };
        assert_eq!(second.header.frame, 11);
        assert_eq!(&*second.rgba, &[2; 8]);
        drop(reader);
        drop(writer);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reader_observes_queued_frames_in_publish_order() {
        let path = path("ordered");
        let mut writer = LatestFrameWriter::create(&path, 128).unwrap();
        let mut reader = LatestFrameReader::open(&path).unwrap();
        writer.publish(&header(1, 10), &[1; 8]).unwrap();
        writer.publish(&header(2, 11), &[2; 8]).unwrap();
        writer.publish(&header(3, 12), &[3; 8]).unwrap();
        let LatestFrameUpdate::Picture(first) = reader.read_latest().unwrap().unwrap() else {
            panic!("expected picture");
        };
        let LatestFrameUpdate::Picture(second) = reader.read_latest().unwrap().unwrap() else {
            panic!("expected picture");
        };
        let LatestFrameUpdate::Picture(third) = reader.read_latest().unwrap().unwrap() else {
            panic!("expected picture");
        };
        assert_eq!(first.header.frame, 10);
        assert_eq!(second.header.frame, 11);
        assert_eq!(third.header.frame, 12);
        assert!(reader.read_latest().unwrap().is_none());
        drop(reader);
        drop(writer);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reader_skips_only_overwritten_history() {
        let path = path("overwritten");
        let mut writer = LatestFrameWriter::create(&path, 128).unwrap();
        let mut reader = LatestFrameReader::open(&path).unwrap();
        let total = SLOT_COUNT as u64 + 2;
        for sequence in 1..=total {
            writer
                .publish(&header(sequence, 100 + sequence), &[sequence as u8; 8])
                .unwrap();
        }
        let LatestFrameUpdate::Picture(first) = reader.read_latest().unwrap().unwrap() else {
            panic!("expected picture");
        };
        assert_eq!(first.header.sequence, 3);
        assert_eq!(first.header.frame, 103);
        let LatestFrameUpdate::Picture(second) = reader.read_latest().unwrap().unwrap() else {
            panic!("expected picture");
        };
        assert_eq!(second.header.sequence, 4);
        drop(reader);
        drop(writer);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn clear_is_a_visible_generation_without_picture_payload() {
        let path = path("clear");
        let mut writer = LatestFrameWriter::create(&path, 128).unwrap();
        let mut reader = LatestFrameReader::open(&path).unwrap();
        writer.publish(&header(1, 10), &[1; 8]).unwrap();
        assert!(reader.read_latest().unwrap().is_some());
        writer.clear().unwrap();
        assert_eq!(
            reader.read_latest().unwrap().unwrap(),
            LatestFrameUpdate::Clear
        );
        writer.publish(&header(2, 11), &[2; 8]).unwrap();
        let LatestFrameUpdate::Picture(picture) = reader.read_latest().unwrap().unwrap() else {
            panic!("expected picture");
        };
        assert_eq!(picture.header.frame, 11);
        drop(reader);
        drop(writer);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reader_receives_posted_mailbox_without_draining_history() {
        let path = path("mailbox");
        let mut writer = LatestFrameWriter::create(&path, 128).unwrap();
        let mut reader = LatestFrameReader::open(&path).unwrap();
        writer.publish(&header(1, 10), &[1; 8]).unwrap();
        writer.publish(&header(2, 11), &[2; 8]).unwrap();
        writer.publish(&header(3, 12), &[3; 8]).unwrap();
        let LatestFrameUpdate::Picture(posted) = reader
            .recv(Duration::from_millis(50))
            .unwrap()
            .unwrap()
        else {
            panic!("expected posted mailbox");
        };
        assert_eq!(posted.header.sequence, 3);
        assert_eq!(posted.header.frame, 12);
        assert!(reader.recv(Duration::from_millis(5)).unwrap().is_none());
        drop(reader);
        drop(writer);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reader_can_coalesce_preview_to_newest_complete_frame() {
        let path = path("newest");
        let mut writer = LatestFrameWriter::create(&path, 128).unwrap();
        let mut reader = LatestFrameReader::open(&path).unwrap();
        writer.publish(&header(1, 10), &[1; 8]).unwrap();
        writer.publish(&header(2, 11), &[2; 8]).unwrap();
        writer.publish(&header(3, 12), &[3; 8]).unwrap();

        let LatestFrameUpdate::Picture(newest) = reader.read_newest().unwrap().unwrap() else {
            panic!("expected picture");
        };
        assert_eq!(newest.header.sequence, 3);
        assert_eq!(newest.header.frame, 12);
        assert!(reader.read_latest().unwrap().is_none());

        drop(reader);
        drop(writer);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn writer_rejects_invalid_or_oversized_frames() {
        let path = path("bounds");
        let mut writer = LatestFrameWriter::create(&path, 8).unwrap();
        assert!(writer.publish(&header(1, 10), &[1; 7]).is_err());
        assert!(writer.publish(&header(1, 10), &[1; 12]).is_err());
        drop(writer);
        let _ = std::fs::remove_file(path);
    }
}
