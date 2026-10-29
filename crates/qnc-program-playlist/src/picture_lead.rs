//! The picture of a join apart from its sound (user 2026-10-21 the picture J, 2026-10-29 "a cut
//! is the OUT of the clip before and the IN of the clip after, picture and sound each"): the
//! picture of a segment starts `v_lead_frames` before its sound, over the end of the segment
//! before (the speaker before his sound), or goes on `v_tail_frames` after it, over the start of
//! the segment after. An Off's picture is black. The sound of both segments stays where it is;
//! covers stay on top. A picture is as long as its source has frames beyond its IN or OUT and
//! the neighbour is long.

use crate::{crossfade::split_at, BaseSegment, FlatProgramItem, ProgramMediaResolver, ProgramSegmentInput, ProgramVideoLayer};

pub(crate) fn lead_pictures(
    items: &mut Vec<FlatProgramItem>,
    segments: &[ProgramSegmentInput],
    resolver: &mut impl ProgramMediaResolver,
) -> Result<(), String> {
    for pair in segments.windows(2) {
        let (before, after) = (&pair[0], &pair[1]);
        let join = after.record_range.in_frame;
        let before_length = join - before.record_range.in_frame;
        let after_length = after.record_range.out_frame - join;
        // The picture J of the segment after, over the end of the one before.
        let lead = after.v_lead_frames.min(before_length).min(after_length);
        if lead > 0 {
            let lead = if is_off(after) { lead } else { lead.min(after.source_range.source_in) };
            over(items, after, (join - lead, join), join, resolver)?;
        }
        // The picture L of the segment before, over the start of the one after.
        let tail = before.v_tail_frames.min(after_length).min(before_length);
        if tail > 0 {
            let tail = if is_off(before) {
                tail
            } else {
                let media = resolver.resolve(before.clip_id.trim())?;
                tail.min(media.duration_frames - before.source_range.source_out)
            };
            over(items, before, (join, join + tail), join, resolver)?;
        }
    }
    Ok(())
}

fn is_off(segment: &ProgramSegmentInput) -> bool {
    segment.kind.trim().eq_ignore_ascii_case("offovi")
}

/// The base picture of `segment` over the program frames `[from, to)` next to it (`join` is
/// where its own picture ends or starts); black for an Off.
fn over(
    items: &mut Vec<FlatProgramItem>,
    segment: &ProgramSegmentInput,
    (from, to): (i64, i64),
    join: i64,
    resolver: &mut impl ProgramMediaResolver,
) -> Result<(), String> {
    if to <= from {
        return Ok(());
    }
    split_at(items, from)?;
    split_at(items, to)?;
    let inside = |item: &FlatProgramItem| from <= item.record_range.in_frame && item.record_range.out_frame <= to;
    if is_off(segment) {
        // Black.
        for item in items.iter_mut().filter(|item| inside(item)) {
            item.sources.retain(|source| source.video_layer != Some(ProgramVideoLayer::Base));
        }
        return Ok(());
    }
    let media = resolver.resolve(segment.clip_id.trim())?;
    let base = BaseSegment { segment, media: &media, has_audio: false, is_off: false };
    let after = join <= from;
    for item in items.iter_mut().filter(|item| inside(item)) {
        let (record_in, record_out) = (item.record_range.in_frame, item.record_range.out_frame);
        item.sources.retain(|source| source.video_layer != Some(ProgramVideoLayer::Base));
        let length = record_out - record_in;
        let picture = if after {
            // The last frames of the segment, then as many frames after its OUT.
            let mut picture = base.source("tail_video", true, false, join - length, join)?;
            let later = record_out - join;
            picture.source_range.source_in += later;
            picture.source_range.source_out += later;
            picture
        } else {
            // Its first frames, then as many frames before its IN.
            let mut picture = base.source("lead_video", true, false, join, join + length)?;
            let earlier = join - record_in;
            picture.source_range.source_in -= earlier;
            picture.source_range.source_out -= earlier;
            picture
        };
        item.sources.insert(0, picture);
    }
    Ok(())
}
