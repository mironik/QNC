//! The level curve of a segment's A1 and a cover's A2 (docs/98 2.1): points on the frames of
//! the clip, between two the level goes in a line of dB, before the first and after the last
//! it stays theirs. Every item is split where a point lies, so over one item the level of a
//! route is one line (`LevelRamp`); the player, the prerender and the export use it as is.

use super::*;

/// The segment or cover a sound source belongs to, and the output its curve is on.
fn owner(source: &FlatProgramSource) -> Option<(&str, u16)> {
    let id = source.source_id.as_str();
    if let Some(rest) = id.strip_prefix("part:") {
        let segment = rest.find(":base_audio").map(|at| &rest[..at])?;
        return Some((segment, PROGRAM_AUDIO_OUTPUT_A1));
    }
    let rest = id.strip_prefix("cover:")?;
    let cover = [":audio", ":lead", ":tail"].iter().find_map(|suffix| rest.strip_suffix(suffix)).unwrap_or(rest);
    Some((cover, PROGRAM_AUDIO_OUTPUT_A2))
}

/// The program frame where source frame `frame` of `source` sounds in `item`.
fn program_frame(item: &FlatProgramItem, source: &FlatProgramSource, frame: i64) -> i64 {
    item.record_range.in_frame + (frame - source.source_range.source_in)
}

/// Every item split at the points of its curves, then every route of a curve given its line.
pub(crate) fn level_curves(items: &mut Vec<FlatProgramItem>, segments: &[ProgramSegmentInput]) -> Result<(), String> {
    let mut curves: std::collections::HashMap<(&str, u16), &[LevelPoint]> = std::collections::HashMap::new();
    for segment in segments {
        if !segment.a1_points.is_empty() {
            curves.insert((segment.segment_id.as_str(), PROGRAM_AUDIO_OUTPUT_A1), &segment.a1_points);
        }
        for cover in segment.covers.iter().filter(|cover| !cover.a2_points.is_empty()) {
            curves.insert((cover.cover_id.as_str(), PROGRAM_AUDIO_OUTPUT_A2), &cover.a2_points);
        }
    }
    if curves.is_empty() {
        return Ok(());
    }
    let mut cuts = Vec::new();
    for item in items.iter() {
        for source in item.sources.iter().filter(|source| source.has_audio()) {
            let Some(points) = owner(source).and_then(|key| curves.get(&key)) else { continue };
            for point in points.iter() {
                let at = program_frame(item, source, point.source_frame);
                if item.record_range.in_frame < at && at < item.record_range.out_frame {
                    cuts.push(at);
                }
            }
        }
    }
    cuts.sort_unstable();
    cuts.dedup();
    for at in cuts {
        crossfade::split_at(items, at)?;
    }
    for item in items.iter_mut() {
        let (from, to) = (item.record_range.in_frame, item.record_range.out_frame);
        for source in item.sources.iter_mut() {
            let Some((id, output)) = owner(source) else { continue };
            let Some(points) = curves.get(&(id, output)) else { continue };
            let source_in = source.source_range.source_in;
            let level = |frame: i64| LevelPoint::level_at(points, frame as f64).round() as i16;
            let ramp = LevelRamp { from, to, db10_from: level(source_in), db10_to: level(source_in + (to - from)) };
            for route in source.audio_routes.iter_mut().filter(|route| route.output_channel == output) {
                route.level_ramp = Some(ramp);
            }
        }
    }
    Ok(())
}
