//! The picture J of a Ton segment (user 2026-10-21, news practice: the speaker can appear
//! before his sound starts): its picture starts `v_lead_frames` before its sound, over the end
//! of the segment before (an Off shows the speaker instead of black, a Ton cuts to him early).
//! The sound of both segments stays where it is; covers over the segment before stay on top.
//! It is as long as the segment's source has frames before its IN and the segment before is long.

use crate::{crossfade::split_at, BaseSegment, FlatProgramItem, ProgramMediaResolver, ProgramSegmentInput, ProgramVideoLayer};

pub(crate) fn lead_pictures(
    items: &mut Vec<FlatProgramItem>,
    segments: &[ProgramSegmentInput],
    resolver: &mut impl ProgramMediaResolver,
) -> Result<(), String> {
    for (index, segment) in segments.iter().enumerate().skip(1) {
        let is_off = segment.kind.trim().eq_ignore_ascii_case("offovi");
        let start = segment.record_range.in_frame;
        let room = start - segments[index - 1].record_range.in_frame;
        let length = segment.record_range.out_frame - start;
        let lead = segment.v_lead_frames.min(segment.source_range.source_in).min(room).min(length);
        if is_off || lead <= 0 {
            continue;
        }
        let from = start - lead;
        split_at(items, from)?;
        let media = resolver.resolve(segment.clip_id.trim())?;
        let base = BaseSegment { segment, media: &media, has_audio: false, is_off: false };
        for item in items.iter_mut().filter(|item| from <= item.record_range.in_frame && item.record_range.out_frame <= start) {
            let (record_in, record_out) = (item.record_range.in_frame, item.record_range.out_frame);
            let mut picture = base.source("lead_video", true, false, start, start + (record_out - record_in))?;
            let earlier = start - record_in;
            picture.source_range.source_in -= earlier;
            picture.source_range.source_out -= earlier;
            item.sources.retain(|source| source.video_layer != Some(ProgramVideoLayer::Base));
            item.sources.insert(0, picture);
        }
    }
    Ok(())
}
