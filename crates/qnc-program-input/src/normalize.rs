//! Every cover's A2 brought into the project's loudness rule on its own (user 2026-10-24: "A2
//! is not normalised and some covers stand out of the rule"). Each cover is measured from the
//! loudness segments stored with its clip's wave (`qnc-program-loudness`), at the level the
//! user gave it; one outside the rule gets the whole dB that bring it in, raised at most
//! 12 dB. The gain goes on the cover's level as the program is built, so the player, the
//! export and the render hear the same. Nothing is written: the story keeps what the user set.

use std::collections::HashMap;

use qnc_loudness::ChannelLoudness;
use qnc_program_loudness::{ClipMeasure, CoverRule};

use super::*;

/// The wave lanes in their order: lane k is channel k of the original.
const LANES: [qnc_wave::WaveLane; 4] = [qnc_wave::WaveLane::A1, qnc_wave::WaveLane::A2, qnc_wave::WaveLane::A3, qnc_wave::WaveLane::A4];

/// The loudness measured with the waves of the story's clips, read once per clip (the
/// normalisation and the sound assistant read them alike).
pub(crate) struct Measures {
    reader: Option<qnc_artifact_db::ArtifactReader>,
    measured: HashMap<String, (Vec<Option<ChannelLoudness>>, u64)>,
}

impl Measures {
    /// A project whose waves cannot be read has no measures (its parts stay as edited).
    pub(crate) fn open(target: &ProjectDbTarget) -> Self {
        Self { reader: qnc_artifact_db::ArtifactReader::open(target).ok(), measured: HashMap::new() }
    }

    /// The measures of `clip_id` read, if not yet.
    pub(crate) fn read<R: ProgramMediaResolver>(&mut self, clip_id: &str, resolver: &mut R) -> Result<(), String> {
        let Some(reader) = self.reader.as_mut() else { return Ok(()) };
        if self.measured.contains_key(clip_id) {
            return Ok(());
        }
        let channels = reader
            .read_wave(clip_id)
            .ok()
            .flatten()
            .map(|record| LANES.iter().map(|lane| record.loudness.iter().find(|at| at.lane == *lane).map(|at| at.measured.clone())).collect())
            .unwrap_or_default();
        let duration = u64::try_from(resolver.resolve(clip_id)?.duration_frames).unwrap_or(0);
        self.measured.insert(clip_id.to_string(), (channels, duration));
        Ok(())
    }

    pub(crate) fn get(&self, clip_id: &str) -> Option<ClipMeasure<'_>> {
        self.measured.get(clip_id).map(|(channels, duration_frames)| ClipMeasure { channels, duration_frames: *duration_frames })
    }

    pub(crate) fn readable(&self) -> bool {
        self.reader.is_some()
    }
}

/// A cover where the program plays it, its source at the story rate.
pub(crate) fn cover_placement(cover: &ProgramCover, (num, den): (i64, i64)) -> qnc_program_waveform::Placement {
    let source_in = cover.source_in_frame * num as u64 / den as u64;
    qnc_program_waveform::Placement {
        clip_id: cover.clip_id.clone(),
        channel: cover.a2_source_channel,
        program_start: cover.program_start_frame,
        program_end: cover.program_end_frame,
        source_in,
        source_out: source_in + (cover.program_end_frame - cover.program_start_frame),
        level_db: cover.a2_level_db,
        off: cover.a2_off,
        curve: cover.level_keys.iter().map(|key| (key.frame * num as u64 / den as u64, key.db10)).collect(),
    }
}

pub(crate) fn normalize_covers<R: ProgramMediaResolver>(
    measures: &mut Measures,
    covers: &mut [ProgramCover],
    resolver: &mut R,
    story_rate: (u32, u32),
    rule: CoverRule,
) -> Result<(), String> {
    if !measures.readable() {
        return Ok(());
    }
    for cover in covers.iter_mut() {
        if cover.a2_off || cover.fps_num == 0 || cover.fps_den == 0 || cover.program_end_frame <= cover.program_start_frame {
            continue;
        }
        measures.read(&cover.clip_id, resolver)?;
        let ratio = qnc_program_db::story_per_source(story_rate, (cover.fps_num, cover.fps_den))?;
        let placement = cover_placement(cover, ratio);
        let gain = qnc_program_loudness::cover_gain(story_rate, &placement, rule, |id| measures.get(id));
        if let Some(gain) = gain {
            cover.a2_level_db = (i16::from(cover.a2_level_db) + i16::from(gain)).clamp(-60, 24) as i8;
        }
    }
    Ok(())
}
