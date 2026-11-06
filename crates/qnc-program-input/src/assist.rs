//! The sound assistant on the program (user 2026-11-06): while the story has it on, every
//! segment and cover without a curve of the user's plays the curve the assistant proposes
//! (`qnc-sound-assist`: covers ducked under speech, speech evened out). The player, the prerender
//! and the export build the program here, so all three hear the same. Nothing is written.

use super::normalize::{cover_placement, Measures};
use super::*;

pub(crate) fn assist_sound<R: ProgramMediaResolver>(
    measures: &mut Measures,
    segments: &mut [ProgramSegment],
    covers: &mut [ProgramCover],
    resolver: &mut R,
    story_rate: (u32, u32),
) -> Result<(), String> {
    if !measures.readable() {
        return Ok(());
    }
    // The items where the program plays them, their sources at the story rate (as `build_input`).
    let mut items = Vec::new();
    let mut ratios = Vec::new();
    let mut start = 0u64;
    for (index, segment) in segments.iter().enumerate().filter(|(_, segment)| segment.active) {
        let ratio = qnc_program_db::story_per_source(story_rate, (segment.fps_num, segment.fps_den))?;
        let frames = segment.duration_frames;
        measures.read(&segment.clip_id, resolver)?;
        let source_in = segment.in_frame * ratio.0 as u64 / ratio.1 as u64;
        let at = qnc_program_waveform::Placement {
            clip_id: segment.clip_id.clone(),
            channel: segment.a1_source_channel,
            program_start: start,
            program_end: start + frames,
            source_in,
            source_out: source_in + frames,
            level_db: segment.a1_level_db,
            off: segment.a1_off,
            curve: segment.level_keys.iter().map(|key| (key.frame * ratio.0 as u64 / ratio.1 as u64, key.db10)).collect(),
        };
        items.push(qnc_sound_assist::Item { lane: 0, at, own_curve: !segment.level_keys.is_empty() });
        ratios.push((0u8, index, ratio));
        start += frames;
    }
    for (index, cover) in covers.iter().enumerate() {
        if cover.fps_num == 0 || cover.fps_den == 0 || cover.program_end_frame <= cover.program_start_frame {
            continue;
        }
        let ratio = qnc_program_db::story_per_source(story_rate, (cover.fps_num, cover.fps_den))?;
        measures.read(&cover.clip_id, resolver)?;
        items.push(qnc_sound_assist::Item { lane: 1, at: cover_placement(cover, ratio), own_curve: !cover.level_keys.is_empty() });
        ratios.push((1, index, ratio));
    }
    let proposals = qnc_sound_assist::propose(start, story_rate, &items, |id| measures.get(id));
    for proposal in proposals {
        let (lane, index, (num, den)) = ratios[proposal.index];
        // Back to the frames of the clip the story keeps its curves on.
        let keys: Vec<qnc_program_db::LevelKey> =
            proposal.points.iter().map(|(frame, db10)| qnc_program_db::LevelKey { frame: frame * den as u64 / num.max(1) as u64, db10: *db10 }).collect();
        let Ok(keys) = qnc_program_db::LevelKey::checked(keys) else { continue };
        if lane == 0 {
            segments[index].level_keys = keys;
        } else {
            covers[index].level_keys = keys;
        }
    }
    Ok(())
}
