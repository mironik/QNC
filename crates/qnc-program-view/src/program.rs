use super::*;

/// Builds the program: the stored order, one window after another, in story frames at
/// the story rate (a source of twice or half that rate is counted in story frames).
pub fn program(
    segments: &[ProgramSegment],
    selected: Option<&str>,
    story_rate: Option<(u32, u32)>,
) -> SegmentsView {
    let timebase = story_rate.or_else(|| {
        segments
            .iter()
            .find(|segment| segment.active && SegmentKind::from_db(&segment.kind).is_some())
            .map(|first| (first.fps_num, first.fps_den))
    });
    let (rate_num, rate_den) = timebase.unwrap_or((0, 1));
    let mut start = 0u64;
    let mut rows = Vec::with_capacity(segments.len());
    let mut parts = Vec::with_capacity(segments.len());
    for segment in segments {
        let Some(kind) = SegmentKind::from_db(&segment.kind) else {
            continue;
        };
        let frames = segment.duration_frames;
        // The lead-in and lead-out (user 2026-10-24) are rows of the program, not segments of
        // the Segment tab: the user does not edit them.
        if segment.lead.is_empty() {
            parts.push(SegmentPart {
            segment_id: segment.segment_id.clone(),
            kind,
            duration_label: duration_label(frames, rate_num, rate_den),
            active: segment.active,
            selected: segment.active && selected == Some(segment.segment_id.as_str()),
            });
        }
        if !segment.active {
            continue;
        }
        rows.push(SegmentRow {
            segment_id: segment.segment_id.clone(),
            kind,
            clip_id: segment.clip_id.clone(),
            source_in_frame: segment.in_frame,
            source_out_frame: segment.out_frame,
            a1_source_channel: segment.a1_source_channel,
            a1_level_db: segment.a1_level_db,
            a1_off: segment.a1_off,
            start_frame: start,
            end_frame: start + frames,
            duration_label: duration_label(frames, rate_num, rate_den),
            duration_color_key: duration_color_key(frames, rate_num, rate_den),
            selected: selected == Some(segment.segment_id.as_str()),
            lead: !segment.lead.is_empty(),
        });
        start += frames;
    }
    SegmentsView {
        parts,
        timebase,
        total_frames: start,
        rows,
        ..SegmentsView::default()
    }
}

/// The source frame after the last one a cover plays: its slot length counted in its
/// source rate (the story rate, twice it or half it); the stored OUT when the rates
/// do not fit.
pub(crate) fn played_source_out(cover: &ProgramCover, story_rate: Option<(u32, u32)>) -> u64 {
    let slot = cover.program_end_frame.saturating_sub(cover.program_start_frame);
    story_rate
        .and_then(|rate| qnc_program_db::story_per_source(rate, (cover.fps_num, cover.fps_den)).ok())
        .map_or(cover.source_out_frame, |(num, den)| {
            cover.source_in_frame + (slot * den as u64).div_ceil(num as u64)
        })
}

/// `seconds:frames` with the frame rate rounded to whole frames per second (v5).
pub fn duration_label(frames: u64, fps_num: u32, fps_den: u32) -> String {
    let fps = whole_fps(fps_num, fps_den);
    if fps == 0 {
        return "0:00".into();
    }
    format!("{}:{:02}", frames / fps, frames % fps)
}

/// v5 duration marks: 3, 5 and 7 seconds.
pub fn duration_color_key(frames: u64, fps_num: u32, fps_den: u32) -> &'static str {
    if fps_num == 0 || fps_den == 0 {
        return "over_7";
    }
    let seconds = frames as f64 * f64::from(fps_den) / f64::from(fps_num);
    match seconds {
        s if s < 3.0 => "under_3",
        s if s < 5.0 => "under_5",
        s if s < 7.0 => "under_7",
        _ => "over_7",
    }
}

pub(crate) fn whole_fps(fps_num: u32, fps_den: u32) -> u64 {
    if fps_den == 0 {
        return 0;
    }
    (f64::from(fps_num) / f64::from(fps_den)).round().max(0.0) as u64
}


/// The covers on the program axis, each drawn over its slot (v5 `segment_timeline_covers`),
/// with the source frames it plays.
pub fn cover_spans(covers: &[ProgramCover], selected: Option<&str>, story_rate: Option<(u32, u32)>) -> Vec<CoverSpan> {
    covers
        .iter()
        .map(|cover| CoverSpan {
            cover_id: cover.cover_id.clone(),
            slot_id: cover.slot_id.clone(),
            start_frame: cover.program_start_frame,
            end_frame: cover.program_end_frame,
            selected: selected == Some(cover.cover_id.as_str()),
            clip_id: cover.clip_id.clone(),
            source_in_frame: cover.source_in_frame,
            source_out_frame: played_source_out(cover, story_rate),
            a2_source_channel: cover.a2_source_channel,
            a2_level_db: cover.a2_level_db,
            a2_off: cover.a2_off,
        })
        .collect()
}
