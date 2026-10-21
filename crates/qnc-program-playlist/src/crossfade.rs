//! The A2 crossfade of a flat program (user 2026-10-21, news practice: picture cuts hard, the
//! ambience of a cover never jumps). At every A2 cut the sound crosses over a window around
//! the picture cut: a J (the next cover's sound starts before its picture) and an L (the
//! cover's sound goes on after its picture). The cover's own J and L are its frames from the
//! story; a cut that has neither gets the project's `audio.crossfade_frames` as a J. Each side
//! is as long as the source has frames (before IN for a J, after OUT for an L) and as the room
//! next to it allows. Over the window the sound before falls and the sound after rises. Without
//! frames to cross, a cover's sound rises over its own first frames and falls over its last.
//! A1 (the story) is never faded.

use crate::{trim_program_item, FlatProgramItem, FlatProgramSource, ProgramAudioRoute, PROGRAM_AUDIO_OUTPUT_A2};

/// One A2 sound of covers: continuous source, program frames `[start, end)`.
struct Run {
    start: i64,
    end: i64,
    /// The cover's source, its sound, as it is at `start`.
    sound: FlatProgramSource,
    cover_id: String,
}

impl Run {
    /// Source frames the sound has before its IN and after its OUT.
    fn handles(&self) -> (i64, i64) {
        let range = &self.sound.source_range;
        let after = self.sound.source_duration_frames - (range.source_in + (self.end - self.start));
        (range.source_in.max(0), after.max(0))
    }
}

/// The J and L a cover asks for, story frames.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Overlap {
    pub lead: i64,
    pub tail: i64,
}

fn is_cover_sound(source: &FlatProgramSource) -> bool {
    source.source_id.starts_with("cover:") && source.audio_routes.iter().any(|route| route.output_channel == PROGRAM_AUDIO_OUTPUT_A2)
}

fn is_extension(source: &FlatProgramSource) -> bool {
    source.source_id.ends_with(":lead") || source.source_id.ends_with(":tail")
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

/// Sets the fades of the A2 routes of a run (not of a J or L added next to another run).
fn fade_routes(items: &mut [FlatProgramItem], run: &Run, fade: impl Fn(&mut ProgramAudioRoute)) {
    for item in items.iter_mut().filter(|item| item.record_range.in_frame < run.end && run.start < item.record_range.out_frame) {
        for source in item
            .sources
            .iter_mut()
            .filter(|source| is_cover_sound(source) && !is_extension(source) && cover_id_of(source) == run.cover_id)
        {
            source.audio_routes.iter_mut().filter(|route| route.output_channel == PROGRAM_AUDIO_OUTPUT_A2).for_each(&fade);
        }
    }
}

/// The run's sound over program frames `[from, to)` outside its own picture (`suffix` "lead"
/// before it, "tail" after it), every route with `fade`.
fn extend(items: &mut Vec<FlatProgramItem>, run: &Run, (from, to): (i64, i64), suffix: &str, fade: impl Fn(&mut ProgramAudioRoute)) -> Result<(), String> {
    if from >= to {
        return Ok(());
    }
    split_at(items, from)?;
    split_at(items, to)?;
    for item in items.iter_mut().filter(|item| from <= item.record_range.in_frame && item.record_range.out_frame <= to) {
        let mut sound = run.sound.clone();
        sound.source_id = format!("cover:{}:{suffix}", run.cover_id);
        sound.source_range.source_in = run.sound.source_range.source_in + (item.record_range.in_frame - run.start);
        sound.source_range.source_out = sound.source_range.source_in + (item.record_range.out_frame - item.record_range.in_frame);
        sound.audio_routes.iter_mut().for_each(&fade);
        item.sources.push(sound);
    }
    Ok(())
}

/// The window of every cut: (J before the start of run `i`, L after the end of run `i`).
fn windows(runs: &[Run], crossfade: i64, asked: &impl Fn(&str) -> Overlap, program_end: i64) -> Vec<(i64, i64)> {
    let asks: Vec<Overlap> = runs.iter().map(|run| asked(&run.cover_id)).collect();
    runs.iter()
        .enumerate()
        .map(|(index, run)| {
            let (before, after) = run.handles();
            let previous = index.checked_sub(1).map(|i| &runs[i]);
            let next = runs.get(index + 1);
            let lead_room = match previous {
                Some(previous) if previous.end == run.start => previous.end - previous.start,
                Some(previous) => run.start - previous.end,
                None => run.start,
            };
            let tail_room = match next {
                Some(next) if next.start == run.end => next.end - next.start,
                Some(next) => next.start - run.end,
                None => program_end - run.end,
            };
            // A cut with neither J nor L takes the project's crossfade as a J.
            let previous_tail = previous.filter(|previous| previous.end == run.start).map_or(0, |_| asks[index - 1].tail);
            let lead = match asks[index].lead {
                0 if previous_tail > 0 => 0,
                0 => crossfade,
                lead => lead,
            };
            (lead.min(before).min(lead_room).max(0), asks[index].tail.min(after).min(tail_room).max(0))
        })
        .collect()
}

/// Applies the crossfade of every A2 cut: `crossfade` frames from the project, `asked` the
/// cover's own J and L (frames) by cover id; `program_end` the program's last frame.
pub(crate) fn crossfade_a2(items: &mut Vec<FlatProgramItem>, crossfade: i64, asked: impl Fn(&str) -> Overlap, program_end: i64) -> Result<(), String> {
    let runs = runs(items);
    let windows = windows(&runs, crossfade, &asked, program_end);
    for (index, run) in runs.iter().enumerate() {
        let adjacent = index.checked_sub(1).filter(|&i| runs[i].end == run.start);
        // The window at the start: the J of this run and the L of the run before, if it is next to it.
        let lead = windows[index].0;
        let previous_tail = adjacent.map_or(0, |i| windows[i].1);
        let window = (run.start - lead, run.start + previous_tail);
        if window.0 < window.1 {
            extend(items, run, (window.0, run.start), "lead", |route| route.fade_in = Some(window))?;
            fade_routes(items, run, |route| route.fade_in = Some(window));
            if let Some(i) = adjacent {
                let previous = &runs[i];
                extend(items, previous, (previous.end, window.1), "tail", |route| route.fade_out = Some(window))?;
                fade_routes(items, previous, |route| route.fade_out = Some(window));
            }
        } else {
            let length = crossfade.min(run.end - run.start).max(0);
            if length > 0 {
                fade_routes(items, run, |route| route.fade_in = Some((run.start, run.start + length)));
            }
        }
        // The end of this run when no run starts right after it (an adjacent one handles the cut).
        let next_adjacent = runs.get(index + 1).is_some_and(|next| next.start == run.end);
        let next_crosses = next_adjacent && windows[index + 1].0 + windows[index].1 > 0;
        if next_crosses {
            continue;
        }
        let tail = windows[index].1;
        if tail > 0 && !next_adjacent {
            let window = (run.end, run.end + tail);
            extend(items, run, window, "tail", |route| route.fade_out = Some(window))?;
            fade_routes(items, run, |route| route.fade_out = Some(window));
        } else {
            let length = crossfade.min(run.end - run.start).max(0);
            if length > 0 {
                fade_routes(items, run, |route| route.fade_out = Some((run.end - length, run.end)));
            }
        }
    }
    Ok(())
}
