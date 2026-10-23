use super::*;

/// v5 `now_str`.
pub(crate) fn story_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("epoch_{secs}")
}

/// v5 `validate_kind`: a story segment is `tonovi` or `offovi`.
pub(crate) fn story_kind(kind: &str) -> Result<&'static str> {
    match kind.trim() {
        "tonovi" => Ok("tonovi"),
        "offovi" => Ok("offovi"),
        other => Err(qnc_ui_text::fill("story.unknown_segment_kind", &[("kind", other)])),
    }
}

pub(crate) fn new_marker_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("marker_{nanos:x}")
}

/// v5 `require_current_story_program_source_fps`: the rate of the story; markers and
/// slots need it.
pub(crate) fn require_story_fps(conn: &Connection) -> Result<f64> {
    story_rate(conn)?
        .map(|(num, den)| f64::from(num) / f64::from(den))
        .ok_or_else(|| qnc_ui_text::text("story.timeline_fps_invalid_story_program_valid").into())
}

/// The rate of the story: set by its first segment and kept while the story has segments
/// (user 2026-10-09: sources of twice or half that rate may join, so a later first segment
/// must not change it). A story written before the rate was kept takes its first segment,
/// as v5. `None` for a story without segments.
pub(crate) fn story_rate(conn: &Connection) -> Result<Option<(u32, u32)>> {
    let any: bool = conn
        .query_row("SELECT EXISTS(SELECT 1 FROM story_parts WHERE active = 1)", [], |row| row.get(0))
        .map_err(err)?;
    if !any {
        return Ok(None);
    }
    let kept: (u32, u32) = conn
        .query_row("SELECT story_fps_num, story_fps_den FROM story_state WHERE id = 1", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .map_err(err)?;
    if kept.0 > 0 && kept.1 > 0 {
        return Ok(Some(kept));
    }
    conn.query_row(
        "SELECT source_fps_num, source_fps_den FROM story_parts
         WHERE active = 1 AND source_fps_num > 0 AND source_fps_den > 0
         ORDER BY sort_index LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
    .map_err(err)
}

/// Keeps `rate` as the rate of the story (its first segment).
pub(crate) fn keep_story_rate(conn: &Connection, (num, den): (u32, u32)) -> Result<()> {
    conn.execute(
        "UPDATE story_state SET story_fps_num = ?1, story_fps_den = ?2 WHERE id = 1",
        params![num, den],
    )
    .map_err(err)?;
    Ok(())
}

/// Story frames per source frame as (num, den): 1/1 the same rate; 2/1 a source of half
/// the story rate (1080i50 in a 50p story: each field is a story frame); 1/2 a source of
/// twice the story rate (a 50p source in a 25 story: two source frames are one story
/// frame). Any other rate is refused (user 2026-10-09: 2:1 only; v5 refuses any mix).
pub fn story_per_source(story: (u32, u32), source: (u32, u32)) -> Result<(i64, i64)> {
    let story_rate = u64::from(story.0) * u64::from(source.1);
    let source_rate = u64::from(source.0) * u64::from(story.1);
    if story_rate == source_rate {
        Ok((1, 1))
    } else if story_rate == 2 * source_rate {
        Ok((2, 1))
    } else if 2 * story_rate == source_rate {
        Ok((1, 2))
    } else {
        Err(qnc_ui_text::fill(
            "story.rate_not_allowed",
            &[("source", &format!("{}/{}", source.0, source.1)), ("story", &format!("{}/{}", story.0, story.1))],
        ))
    }
}

/// The source range trimmed to whole story frames (a 50p source in a 25 story ends on a
/// pair) and its length in story frames.
pub(crate) fn story_length(in_frame: i64, out_frame: i64, (num, den): (i64, i64)) -> Result<(i64, i64)> {
    let out_frame = out_frame - (out_frame - in_frame) % den;
    if out_frame <= in_frame {
        return Err(qnc_ui_text::text("story.range_shorter_than_one_picture_story").into());
    }
    Ok((out_frame, (out_frame - in_frame) * num / den))
}

/// v5 `round3(frame_to_seconds(frame, fps))`.
pub(crate) fn timeline_sec(frame: i64, fps: f64) -> f64 {
    if fps <= 0.0 {
        return 0.0;
    }
    ((frame.max(0) as f64 / fps) * 1000.0).round() / 1000.0
}

/// v5 `frame_to_timecode`: `hh:mm:ss:ff` with the rate rounded to whole frames.
pub(crate) fn frame_timecode(frame: i64, fps: f64) -> String {
    let fps = fps.round().max(1.0) as i64;
    let total = frame.max(0);
    let total_sec = total / fps;
    format!(
        "{:02}:{:02}:{:02}:{:02}",
        total_sec / 3600,
        (total_sec / 60) % 60,
        total_sec % 60,
        total % fps
    )
}

/// v5 `seconds_frames_label_from_frames`: `seconds:frames`.
pub(crate) fn frames_label(frames: i64, fps: f64) -> String {
    let fps = fps.round().max(1.0) as i64;
    let frames = frames.max(0);
    format!("{}:{:02}", frames / fps, frames % fps)
}

/// v5 `duration_color_key_from_frames` with the default 3, 5, 7 second marks.
pub(crate) fn duration_color_key(frames: i64, fps: f64) -> &'static str {
    if fps <= 0.0 {
        return "over_7";
    }
    match frames.max(0) as f64 / fps {
        seconds if seconds < 3.0 => "under_3",
        seconds if seconds < 5.0 => "under_5",
        seconds if seconds < 7.0 => "under_7",
        _ => "over_7",
    }
}
