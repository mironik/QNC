//! The key (intra) pictures of an original through its own index, read only (user 2026-11-10:
//! "I ingested more clips but Video AI does not analyse them": FX3A originals are MP4, the
//! analysis read only MXF). One place for the shot analysis and the shot content: an MXF through
//! its index table, an MP4/QuickTime through its sample tables; another file has no usable index.

use std::fs::File;
use std::path::Path;

enum Index {
    Mxf(qnc_mxf_index::MxfIndex),
    Mp4(qnc_mp4_sound::Track),
}

/// The key pictures of one original.
pub struct KeyPictures {
    file: File,
    index: Index,
    /// The original's pictures and their rate.
    pub frames: u64,
    pub rate: f64,
    /// Every key picture as (its id in the index, the frame it is shown at), in order.
    pub keys: Vec<(usize, u64)>,
}

impl KeyPictures {
    /// The key pictures of the original at `path`; `Ok(None)` when it has no index this reads.
    pub fn open(path: &Path) -> Result<Option<Self>, String> {
        if let Some(index) = qnc_mxf_index::MxfIndex::read(path)? {
            let frames = index.entries.len() as u64;
            let (num, den) = index.rate;
            if frames == 0 || num <= 0 || den <= 0 {
                return Ok(None);
            }
            let keys = index
                .intra_frames()
                .into_iter()
                .map(|entry| (entry, (entry as i64 + index.entries[entry].temporal_offset as i64).clamp(0, frames as i64 - 1) as u64))
                .collect();
            let file = File::open(path).map_err(|e| e.to_string())?;
            return Ok(Some(Self { file, rate: num as f64 / den as f64, frames, keys, index: Index::Mxf(index) }));
        }
        let mut file = File::open(path).map_err(|e| e.to_string())?;
        let mut head = [0u8; 8];
        let is_mp4 = std::io::Read::read_exact(&mut file, &mut head).is_ok() && &head[4..8] == b"ftyp";
        if !is_mp4 {
            return Ok(None);
        }
        let tracks = qnc_mp4_sound::read_index(path)?;
        let Some(track) = tracks.into_iter().find(|track| track.is_picture() && track.picture_codec().is_some()) else {
            return Ok(None);
        };
        let (frames, Some(rate)) = (track.samples.len() as u64, track.rate()) else { return Ok(None) };
        if frames == 0 {
            return Ok(None);
        }
        // A sample is shown at its number (a camera's MP4 key pictures are not reordered).
        let keys = track.key_samples().into_iter().filter(|key| *key < frames).map(|key| (key as usize, key)).collect();
        Ok(Some(Self { file, rate, frames, keys, index: Index::Mp4(track) }))
    }

    /// The picture codec of an MP4 as its index says; `None` for an MXF (the clip's record says).
    pub fn codec(&self) -> Option<&'static str> {
        match &self.index {
            Index::Mxf(_) => None,
            Index::Mp4(track) => track.picture_codec(),
        }
    }

    /// The encoded key picture `id` (an MXF essence picture, an MP4 sample as Annex B).
    pub fn picture(&mut self, id: usize) -> Result<Vec<u8>, String> {
        match &self.index {
            Index::Mxf(index) => qnc_mxf_index::read_package(&mut self.file, index, id, true, false)?
                .picture
                .ok_or_else(|| qnc_ui_text::text("shots.no_index").to_string()),
            Index::Mp4(track) => qnc_mp4_sound::picture_at(&mut self.file, track, id as u64),
        }
    }

    /// The key picture shown nearest `frame`, as (id, shown frame).
    pub fn nearest(&self, frame: u64) -> Option<(usize, u64)> {
        self.keys.iter().copied().min_by_key(|(_, shown)| shown.abs_diff(frame))
    }
}
