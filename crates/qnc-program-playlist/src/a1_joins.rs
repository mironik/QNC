//! The A1 joins of a SubSegment (user 2026-10-29): one statement put together from several
//! ranges, played one after another on V1 and A1. At a join the sound is cut or crossed over
//! `frames` frames (a choice per join, "we will test which is better"): over the window the
//! sound of the part before falls on into its frames after its OUT while the part after rises
//! from its frames before its IN. A join after a pause (frames of silence between the parts)
//! with a crossfade fades the part before out over its last frames and the part after in over
//! its first ones. Each side is as long as the source has frames and as the part allows.

use crate::{crossfade::split_at, FlatProgramItem, FlatProgramSource, ProgramAudioRoute, PROGRAM_AUDIO_OUTPUT_A1};

/// One join of two parts: the part before ends at `end`, the part after starts at `start`
/// (`start > end` after a pause); `frames` of crossfade, 0 for a cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct A1Join {
    pub end: i64,
    pub start: i64,
    pub frames: i64,
}

fn a1_sound(item: &FlatProgramItem) -> Option<&FlatProgramSource> {
    item.sources.iter().find(|source| source.audio_routes.iter().any(|route| route.output_channel == PROGRAM_AUDIO_OUTPUT_A1))
}

/// Source frame of `sound` (as it is in `item`) at program frame `at`.
fn source_at(item: &FlatProgramItem, sound: &FlatProgramSource, at: i64) -> i64 {
    sound.source_range.source_in + (at - item.record_range.in_frame)
}

fn fade_a1(items: &mut [FlatProgramItem], (from, to): (i64, i64), fade: impl Fn(&mut ProgramAudioRoute)) {
    for item in items.iter_mut().filter(|item| from <= item.record_range.in_frame && item.record_range.out_frame <= to) {
        for source in &mut item.sources {
            source.audio_routes.iter_mut().filter(|route| route.output_channel == PROGRAM_AUDIO_OUTPUT_A1).for_each(&fade);
        }
    }
}

/// `sound` (its source frame `source` at program frame `from`) over `[from, to)`, `fade` on it.
fn extend(items: &mut [FlatProgramItem], sound: &FlatProgramSource, (from, to): (i64, i64), source: i64, suffix: &str, fade: impl Fn(&mut ProgramAudioRoute)) {
    for item in items.iter_mut().filter(|item| from <= item.record_range.in_frame && item.record_range.out_frame <= to) {
        let mut extra = sound.clone();
        extra.source_id = format!("{}:{suffix}", sound.source_id);
        extra.video_layer = None;
        extra.source_video_format = None;
        extra.source_range.source_in = source + (item.record_range.in_frame - from);
        extra.source_range.source_out = extra.source_range.source_in + (item.record_range.out_frame - item.record_range.in_frame);
        extra.audio_routes.retain(|route| route.output_channel == PROGRAM_AUDIO_OUTPUT_A1);
        extra.audio_routes.iter_mut().for_each(&fade);
        item.sources.push(extra);
    }
}

/// Applies every join to the items of a program of parts.
pub fn join_a1(items: &mut Vec<FlatProgramItem>, joins: &[A1Join]) -> Result<(), String> {
    for join in joins.iter().filter(|join| join.frames > 0) {
        let half = (join.frames + 1) / 2;
        let before = items.iter().find(|item| item.record_range.out_frame == join.end).cloned();
        let after = items.iter().find(|item| item.record_range.in_frame == join.start).cloned();
        let (Some(before), Some(after)) = (before, after) else { continue };
        let (Some(left), Some(right)) = (a1_sound(&before).cloned(), a1_sound(&after).cloned()) else {
            if join.start > join.end {
                // A pause next to a part without sound: only the side with sound fades.
                fade_edges(items, join, half, a1_sound(&before).is_some(), a1_sound(&after).is_some())?;
            }
            continue;
        };
        if join.start > join.end {
            fade_edges(items, join, half, true, true)?;
            continue;
        }
        let at = join.end;
        let left_out = source_at(&before, &left, at);
        let right_in = source_at(&after, &right, at);
        let room = half
            .min(before.record_range.frame_len())
            .min(after.record_range.frame_len())
            .min((left.source_duration_frames - left_out).max(0))
            .min(right_in.max(0));
        if room <= 0 {
            continue;
        }
        let window = (at - room, at + room);
        split_at(items, window.0)?;
        split_at(items, window.1)?;
        fade_a1(items, (window.0, at), |route| route.fade_out = Some(window));
        fade_a1(items, (at, window.1), |route| route.fade_in = Some(window));
        extend(items, &right, (window.0, at), right_in - room, "lead", |route| route.fade_in = Some(window));
        extend(items, &left, (at, window.1), left_out, "tail", |route| route.fade_out = Some(window));
    }
    Ok(())
}

/// The part before a pause fades out over its last `half` frames, the part after in over its first.
fn fade_edges(items: &mut Vec<FlatProgramItem>, join: &A1Join, half: i64, before: bool, after: bool) -> Result<(), String> {
    if before {
        let window = (join.end - half, join.end);
        split_at(items, window.0)?;
        fade_a1(items, window, |route| route.fade_out = Some(window));
    }
    if after {
        let window = (join.start, join.start + half);
        split_at(items, window.1)?;
        fade_a1(items, window, |route| route.fade_in = Some(window));
    }
    Ok(())
}
