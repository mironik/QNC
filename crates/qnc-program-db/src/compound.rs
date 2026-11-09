//! A compound statement in the story is one segment (user 2026-11-09): the row keeps its parts,
//! pauses, joins and smooth cuts; its IN and OUT are a window on the statement's own axis, in story
//! frames. Whoever plays or draws it asks here what lies where; nothing is written.

use serde::{Deserialize, Serialize};

use crate::{story_per_source, ProgramSegment};

/// One part of a compound segment: frames `[from, to)` of a clip and how it joins the part before.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompoundPart {
    pub clip_id: String,
    pub from_frame: u64,
    pub to_frame: u64,
    /// The clip's rate (the story fills it in from the catalog when the part is written).
    #[serde(default)]
    pub fps_num: u32,
    #[serde(default)]
    pub fps_den: u32,
    /// The pause before it, story frames.
    pub gap_before: u64,
    /// The sounds cross at the join; else a cut.
    pub crossfade: bool,
    pub a1_channel: u16,
    pub a1_level_db: i8,
    pub a1_off: bool,
    /// J and L of the sound of the join before it, in frames of its clip and of the clip before;
    /// negative: a hole of silence.
    pub sound_lead: i64,
    pub sound_tail: i64,
    /// J and L of the picture of that join.
    pub picture_lead: u64,
    pub picture_tail: u64,
    pub level_keys: Vec<crate::LevelKey>,
    /// The clip of the in-between pictures of a smooth cut and their count.
    pub smooth: Option<(String, u64)>,
}

/// A join of two parts in the program: the frame the part before ends and the one the next starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompoundJoin {
    pub end: i64,
    pub start: i64,
    pub crossfade: bool,
}

/// In-between pictures of a smooth cut: the program frames they overwrite, their clip's frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmoothPicture {
    pub id: String,
    pub clip_id: String,
    pub program: (i64, i64),
    pub clip: (i64, i64),
}

/// The active segments with every compound laid out as plain segments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Expanded {
    pub segments: Vec<ProgramSegment>,
    pub joins: Vec<CompoundJoin>,
    pub smooths: Vec<SmoothPicture>,
}

/// How long the parts are one after another, story frames.
pub fn compound_frames(parts: &[CompoundPart], rate: (u32, u32)) -> Result<u64, String> {
    parts.iter().try_fold(0u64, |total, part| {
        let (num, den) = story_per_source(rate, (part.fps_num, part.fps_den))?;
        Ok(total + part.gap_before + part.to_frame.saturating_sub(part.from_frame) * num as u64 / den as u64)
    })
}

/// The active segments as the program plays them.
pub fn expand_compounds(segments: &[ProgramSegment], rate: (u32, u32)) -> Result<Expanded, String> {
    let mut expanded = Expanded::default();
    let mut start = 0i64;
    for segment in segments.iter().filter(|segment| segment.active) {
        if segment.parts.is_empty() {
            expanded.segments.push(segment.clone());
        } else {
            lay_out(segment, start, rate, &mut expanded)?;
        }
        start += segment.duration_frames as i64;
    }
    Ok(expanded)
}

fn lay_out(segment: &ProgramSegment, start: i64, rate: (u32, u32), out: &mut Expanded) -> Result<(), String> {
    let (from, to) = (segment.in_frame as i64, segment.out_frame as i64);
    let at = |axis: i64| start + axis - from;
    let first = out.segments.len();
    let (mut axis, mut before_scale) = (0i64, (1i64, 1i64));
    for (index, part) in segment.parts.iter().enumerate() {
        let (num, den) = story_per_source(rate, (part.fps_num, part.fps_den))?;
        let gap = part.gap_before as i64;
        let frames = (part.to_frame as i64 - part.from_frame as i64) * num / den;
        if frames <= 0 {
            return Err(qnc_ui_text::text("script.sot_marks").into());
        }
        let (gap_end, part_end) = (axis + gap, axis + gap + frames);
        let inside = index > 0 && axis > from && gap_end < to;
        let (pause_from, pause_to) = (axis.max(from), gap_end.min(to));
        if pause_to > pause_from {
            let holder = &segment.parts[index.saturating_sub(1)];
            let length = (pause_to - pause_from) as u64;
            out.segments.push(ProgramSegment {
                segment_id: format!("{}-pause-{index}", segment.segment_id),
                kind: "offovi".into(),
                clip_id: holder.clip_id.clone(),
                out_frame: length,
                fps_num: holder.fps_num,
                fps_den: holder.fps_den,
                active: true,
                a1_off: true,
                duration_frames: length,
                ..ProgramSegment::default()
            });
        }
        let (shown_from, shown_to) = (gap_end.max(from), part_end.min(to));
        if shown_to > shown_from {
            let in_frame = part.from_frame as i64 + (shown_from - gap_end) * den / num;
            let joined = inside && gap == 0;
            let (lead, tail) = if joined { (part.sound_lead * num / den, part.sound_tail * before_scale.0 / before_scale.1) } else { (0, 0) };
            let shown_before = out.segments.len() > first;
            if let Some(before) = out.segments.last_mut().filter(|_| joined && shown_before) {
                before.a1_tail_frames = tail as i32;
                before.v_tail_frames = (part.picture_tail as i64 * before_scale.0 / before_scale.1) as u32;
            }
            out.segments.push(ProgramSegment {
                segment_id: format!("{}-part-{index}", segment.segment_id),
                kind: segment.kind.clone(),
                clip_id: part.clip_id.clone(),
                in_frame: in_frame as u64,
                out_frame: (in_frame + (shown_to - shown_from) * den / num) as u64,
                fps_num: part.fps_num,
                fps_den: part.fps_den,
                active: true,
                a1_source_channel: part.a1_channel,
                a1_level_db: part.a1_level_db.saturating_add(segment.a1_level_db),
                a1_off: part.a1_off || segment.a1_off,
                duration_frames: (shown_to - shown_from) as u64,
                v_lead_frames: if joined { (part.picture_lead as i64 * num / den) as u32 } else { 0 },
                a1_lead_frames: lead as i32,
                a2_source_channel: segment.a2_source_channel,
                a2_level_db: segment.a2_level_db,
                a2_off: segment.a2_off,
                level_keys: part.level_keys.clone(),
                ..ProgramSegment::default()
            });
        }
        if inside {
            let shaped = part.sound_lead != 0 || part.sound_tail != 0;
            out.joins.push(CompoundJoin { end: at(axis), start: at(gap_end), crossfade: part.crossfade && !shaped });
            if let Some((clip, n)) = part.smooth.as_ref().filter(|_| (num, den) == (1, 1) && before_scale == (1, 1)) {
                let [ahead, pause, after] = smooth_parts(at(axis), gap, *n as i64);
                if ahead.0 .0 >= start && after.0 .1 <= start + (to - from) {
                    for (side, (program, pictures)) in [("before", ahead), ("pause", pause), ("after", after)] {
                        if program.1 > program.0 {
                            out.smooths.push(SmoothPicture { id: format!("smooth-{}-{index}-{side}", segment.segment_id), clip_id: clip.clone(), program, clip: pictures });
                        }
                    }
                }
            }
        }
        before_scale = (num, den);
        axis = part_end;
    }
    // What the story itself did at the ends of the segment goes on its first and last piece.
    if let Some(piece) = out.segments.get_mut(first) {
        piece.v_lead_frames = segment.v_lead_frames;
        piece.a1_lead_frames = segment.a1_lead_frames;
        piece.a2_lead_frames = segment.a2_lead_frames;
    }
    if out.segments.len() > first {
        if let Some(piece) = out.segments.last_mut() {
            piece.v_tail_frames = segment.v_tail_frames;
            piece.a1_tail_frames = segment.a1_tail_frames;
            piece.a2_tail_frames = segment.a2_tail_frames;
        }
    }
    Ok(())
}

/// Where the `n` in-between pictures of a smooth cut go, as ((program from, to), (clip from, to)):
/// those that replace the end of the part before the join at `start`, those that fill its pause
/// of `gap` frames, and those that replace the start of the part after it.
fn smooth_parts(start: i64, gap: i64, n: i64) -> [((i64, i64), (i64, i64)); 3] {
    let half = (n - gap) / 2;
    [((start - half, start), (0, half)), ((start, start + gap), (half, half + gap)), ((start + gap, start + n - half), (half + gap, n))]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(clip: &str, from: u64, to: u64, gap: u64) -> CompoundPart {
        CompoundPart { clip_id: clip.into(), from_frame: from, to_frame: to, fps_num: 50, fps_den: 1, gap_before: gap, ..CompoundPart::default() }
    }

    fn compound(parts: Vec<CompoundPart>, window: (u64, u64)) -> ProgramSegment {
        ProgramSegment { segment_id: "s".into(), kind: "tonovi".into(), in_frame: window.0, out_frame: window.1, fps_num: 50, fps_den: 1, active: true, duration_frames: window.1 - window.0, parts, ..ProgramSegment::default() }
    }

    #[test]
    fn the_in_between_pictures_run_through_the_pause_without_a_hole() {
        let [ahead, pause, after] = smooth_parts(100, 5, 13);
        assert_eq!(ahead, ((96, 100), (0, 4)));
        assert_eq!(pause, ((100, 105), (4, 9)));
        assert_eq!(after, ((105, 109), (9, 13)));
        assert_eq!(smooth_parts(100, 0, 8)[2], ((100, 104), (4, 8)), "without a pause as before");
    }

    #[test]
    fn a_compound_is_its_parts_and_pause_with_a_smooth_cut_at_their_join() {
        let mut second = part("b", 200, 300, 0);
        second.smooth = Some(("sm".into(), 8));
        let plain = ProgramSegment { segment_id: "p".into(), kind: "offovi".into(), active: true, fps_num: 50, fps_den: 1, out_frame: 50, duration_frames: 50, ..ProgramSegment::default() };
        let segments = [plain, compound(vec![part("a", 10, 110, 0), second, part("c", 0, 50, 25)], (0, 275))];
        let expanded = expand_compounds(&segments, (50, 1)).unwrap();
        let shown: Vec<_> = expanded.segments.iter().map(|s| (s.clip_id.as_str(), s.in_frame, s.duration_frames, s.a1_off)).collect();
        assert_eq!(shown, vec![("", 0, 50, false), ("a", 10, 100, false), ("b", 200, 100, false), ("b", 0, 25, true), ("c", 0, 50, false)]);
        assert_eq!(expanded.joins[0], CompoundJoin { end: 150, start: 150, crossfade: false });
        assert_eq!(expanded.smooths.iter().map(|s| s.program).collect::<Vec<_>>(), vec![(146, 150), (150, 154)]);
    }

    #[test]
    fn a_trimmed_compound_shows_only_its_window_and_no_smooth_cut_past_it() {
        let mut second = part("b", 200, 300, 0);
        second.smooth = Some(("sm".into(), 8));
        let expanded = expand_compounds(&[compound(vec![part("a", 10, 110, 0), second], (98, 150))], (50, 1)).unwrap();
        let shown: Vec<_> = expanded.segments.iter().map(|s| (s.clip_id.as_str(), s.in_frame, s.out_frame)).collect();
        assert_eq!(shown, vec![("a", 108, 110), ("b", 200, 250)]);
        assert!(expanded.smooths.is_empty(), "the smooth cut needs pictures the window took");
    }
}
