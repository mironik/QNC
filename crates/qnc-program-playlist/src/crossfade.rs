//! The A2 crossfade of a flat program (user 2026-10-21, news practice: picture cuts hard, the
//! ambience of a cover never jumps). Every cover's A2 comes in with a short crossfade: its
//! sound starts `lead` frames before its picture (a J), rising, while the A2 before it, if it
//! runs up to the cut, falls over the same frames. The lead is the project's
//! `audio.crossfade_frames`, or the cover's own J when longer, as far as the cover's source
//! has frames before its IN. Without them the cover's sound rises over its own first frames;
//! an A2 with no cover after it falls over its last frames. A1 (the story) is never faded.

use crate::{trim_program_item, FlatProgramItem, FlatProgramSource, ProgramAudioRoute, PROGRAM_AUDIO_OUTPUT_A2};

/// One A2 sound of covers: continuous source, program frames `[start, end)`.
struct Run {
    start: i64,
    end: i64,
    /// The cover's source, its sound, as it is at `start`.
    sound: FlatProgramSource,
    cover_id: String,
}

fn is_cover_sound(source: &FlatProgramSource) -> bool {
    source.source_id.starts_with("cover:") && source.audio_routes.iter().any(|route| route.output_channel == PROGRAM_AUDIO_OUTPUT_A2)
}

fn cover_id_of(source: &FlatProgramSource) -> String {
    source.source_id.trim_start_matches("cover:").trim_end_matches(":audio").to_string()
}

/// The A2 runs of the items, in program order.
fn runs(items: &[FlatProgramItem]) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for item in items {
        for source in item.sources.iter().filter(|source| is_cover_sound(source)) {
            let continues = runs.last().is_some_and(|run| {
                run.end == item.record_range.in_frame
                    && run.sound.clip_id == source.clip_id
                    && run.sound.media == source.media
                    && run.sound.source_range.source_in + (run.end - run.start) == source.source_range.source_in
            });
            if continues {
                if let Some(run) = runs.last_mut() {
                    run.end = item.record_range.out_frame;
                }
                continue;
            }
            let mut sound = source.clone();
            sound.video_layer = None;
            sound.source_video_format = None;
            runs.push(Run { start: item.record_range.in_frame, end: item.record_range.out_frame, sound, cover_id: cover_id_of(source) });
        }
    }
    runs
}

/// Splits the item that holds `frame` strictly inside into two.
fn split_at(items: &mut Vec<FlatProgramItem>, frame: i64) -> Result<(), String> {
    let Some(index) = items.iter().position(|item| item.record_range.in_frame < frame && frame < item.record_range.out_frame) else {
        return Ok(());
    };
    let item = items.remove(index);
    let left = trim_program_item(&item, item.record_range.in_frame, frame)?;
    let right = trim_program_item(&item, frame, item.record_range.out_frame)?;
    items.insert(index, right);
    items.insert(index, left);
    Ok(())
}

/// Sets the fades of the A2 routes of a run (not of a lead added before another run).
fn fade_routes(items: &mut [FlatProgramItem], run: &Run, fade: impl Fn(&mut ProgramAudioRoute)) {
    for item in items.iter_mut().filter(|item| item.record_range.in_frame < run.end && run.start < item.record_range.out_frame) {
        for source in item
            .sources
            .iter_mut()
            .filter(|source| is_cover_sound(source) && !source.source_id.ends_with(":lead") && cover_id_of(source) == run.cover_id)
        {
            source.audio_routes.iter_mut().filter(|route| route.output_channel == PROGRAM_AUDIO_OUTPUT_A2).for_each(&fade);
        }
    }
}

/// The frames each run comes in before its picture: the project crossfade or the cover's J,
/// within the frames its source has before its IN and the room before it.
fn leads(runs: &[Run], crossfade: i64, lead: &impl Fn(&str) -> i64) -> Vec<i64> {
    runs.iter()
        .enumerate()
        .map(|(index, run)| {
            let before = index.checked_sub(1).map(|i| &runs[i]);
            // Within the A2 before it, or the stretch without A2 after an earlier one.
            let room = match before {
                Some(previous) if previous.end == run.start => previous.end - previous.start,
                Some(previous) => run.start - previous.end,
                None => run.start,
            };
            crossfade.max(lead(&run.cover_id)).min(run.sound.source_range.source_in).min(room).max(0)
        })
        .collect()
}

/// Applies the crossfade of every A2 cut: `crossfade` frames from the project, `lead` the
/// cover's own J (frames), by cover id.
pub(crate) fn crossfade_a2(items: &mut Vec<FlatProgramItem>, crossfade: i64, lead: impl Fn(&str) -> i64) -> Result<(), String> {
    let runs = runs(items);
    let leads = leads(&runs, crossfade, &lead);
    for (index, run) in runs.iter().enumerate() {
        let adjacent = index.checked_sub(1).map(|i| &runs[i]).filter(|previous| previous.end == run.start);
        let in_frames = leads[index];
        if in_frames > 0 {
            let from = run.start - in_frames;
            split_at(items, from)?;
            for item in items.iter_mut().filter(|item| from <= item.record_range.in_frame && item.record_range.out_frame <= run.start) {
                let mut sound = run.sound.clone();
                sound.source_id = format!("cover:{}:lead", run.cover_id);
                sound.source_range.source_in = run.sound.source_range.source_in - (run.start - item.record_range.in_frame);
                sound.source_range.source_out = sound.source_range.source_in + (item.record_range.out_frame - item.record_range.in_frame);
                for route in &mut sound.audio_routes {
                    route.fade_in = Some((from, run.start));
                }
                item.sources.push(sound);
            }
            fade_routes(items, run, |route| route.fade_in = Some((from, run.start)));
            if let Some(previous) = adjacent {
                fade_routes(items, previous, |route| route.fade_out = Some((from, run.start)));
            }
        } else {
            let length = crossfade.min(run.end - run.start).max(0);
            if length > 0 {
                fade_routes(items, run, |route| route.fade_in = Some((run.start, run.start + length)));
            }
        }
        // An A2 that no cover follows, or whose next cover cannot come in early, falls over
        // its own last frames.
        let next_leads = runs.get(index + 1).is_some_and(|next| next.start == run.end && leads[index + 1] > 0);
        let length = crossfade.min(run.end - run.start).max(0);
        if !next_leads && length > 0 {
            fade_routes(items, run, |route| route.fade_out = Some((run.end - length, run.end)));
        }
    }
    Ok(())
}
