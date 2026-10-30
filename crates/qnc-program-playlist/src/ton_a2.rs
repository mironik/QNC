//! The A2 of a Ton into the Off next to it (user 2026-10-30: "the picture of a Ton may go into the
//! Off at its start and end, the A1 of the Ton may not, only its A2 as a J or an L"). A Ton's A2
//! is silent under the Ton itself; it is heard only where it goes into an Off: `a2_lead_frames`
//! before its start (a J over the end of the Off before it) and `a2_tail_frames` after its end
//! (an L over the start of the Off after it), from its clip's frames before IN and after OUT, on
//! its A2 channel and level. A cover overwrites A2 where it lies, so the Ton's A2 is heard only
//! where no other sound holds A2. It fades in over the far end of a J and out over the far end
//! of an L (`fade` frames), and is as long as its source has frames and the Off is long.

use crate::{
    build::audio_routes, crossfade::split_at, BaseSegment, FlatProgramItem, ProgramMediaResolver, ProgramSegmentInput,
    PROGRAM_AUDIO_OUTPUT_A2,
};

pub(crate) fn ton_a2_into_offs(
    items: &mut Vec<FlatProgramItem>,
    segments: &[ProgramSegmentInput],
    fade: i64,
    resolver: &mut impl ProgramMediaResolver,
) -> Result<(), String> {
    for pair in segments.windows(2) {
        let (before, after) = (&pair[0], &pair[1]);
        let join = after.record_range.in_frame;
        // The L of a Ton before an Off: its A2 goes on over the start of the Off.
        if !is_off(before) && is_off(after) && before.a2_tail_frames > 0 && !before.a2_off {
            let media = resolver.resolve(before.clip_id.trim())?;
            let room = (media.duration_frames - before.source_range.source_out).max(0);
            let tail = before.a2_tail_frames.min(after.record_range.frame_len()).min(room);
            if tail > 0 {
                let base = BaseSegment { segment: before, media: &media, has_audio: true, is_off: false };
                let fade_out = (fade > 0).then(|| (join + tail - fade.min(tail), join + tail));
                lay(items, &base, (join, join + tail), (before.source_range.source_out, join), fade_out, None)?;
            }
        }
        // The J of a Ton after an Off: its A2 starts over the end of the Off.
        if is_off(before) && !is_off(after) && after.a2_lead_frames > 0 && !after.a2_off {
            let media = resolver.resolve(after.clip_id.trim())?;
            let lead = after.a2_lead_frames.min(before.record_range.frame_len()).min(after.source_range.source_in.max(0));
            if lead > 0 {
                let base = BaseSegment { segment: after, media: &media, has_audio: true, is_off: false };
                let from = join - lead;
                let fade_in = (fade > 0).then(|| (from, from + fade.min(lead)));
                lay(items, &base, (from, join), (after.source_range.source_in - lead, from), None, fade_in)?;
            }
        }
    }
    Ok(())
}

fn is_off(segment: &ProgramSegmentInput) -> bool {
    segment.kind.trim().eq_ignore_ascii_case("offovi")
}

/// The A2 of `base` over program frames `[from, to)`, its source frame `source` at program frame
/// `origin`; only into items whose A2 is free.
fn lay(
    items: &mut Vec<FlatProgramItem>,
    base: &BaseSegment<'_>,
    (from, to): (i64, i64),
    (source, origin): (i64, i64),
    fade_out: Option<(i64, i64)>,
    fade_in: Option<(i64, i64)>,
) -> Result<(), String> {
    if to <= from {
        return Ok(());
    }
    split_at(items, from)?;
    split_at(items, to)?;
    let segment = base.segment;
    for item in items.iter_mut().filter(|item| from <= item.record_range.in_frame && item.record_range.out_frame <= to) {
        let held = item.sources.iter().flat_map(|s| &s.audio_routes).any(|route| route.output_channel == PROGRAM_AUDIO_OUTPUT_A2);
        if held {
            continue;
        }
        let (record_in, record_out) = (item.record_range.in_frame, item.record_range.out_frame);
        let mut sound = base.audio_source(segment.record_range.in_frame, segment.record_range.in_frame + (record_out - record_in))?;
        sound.source_id = format!("part:{}:a2", segment.segment_id);
        sound.source_range.source_in = source + (record_in - origin);
        sound.source_range.source_out = sound.source_range.source_in + (record_out - record_in);
        sound.audio_routes = audio_routes(true, segment.a2_source_channel, PROGRAM_AUDIO_OUTPUT_A2, segment.a2_level_db);
        for route in &mut sound.audio_routes {
            route.fade_in = fade_in;
            route.fade_out = fade_out;
        }
        item.sources.push(sound);
    }
    Ok(())
}
