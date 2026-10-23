use super::*;

/// The slot a cover of `cover_frames` story frames takes: `slot_id` itself when the cover
/// fills it, else its first part, cut off by a new M marker at the cover's end.
pub(crate) fn split_slot_for(conn: &Connection, slot_id: &str, cover_frames: i64) -> Result<String> {
    let found: Option<(String, i64, i64)> = conn
        .query_row(
            "SELECT start_marker_id, start_frame, end_frame FROM story_marker_slots WHERE slot_id = ?1",
            [slot_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(err)?;
    let Some((start_marker, start, end)) = found else {
        return Err(format!("slot not found: {slot_id}"));
    };
    if cover_frames <= 0 || start + cover_frames >= end {
        return Ok(slot_id.to_string());
    }
    let fps = require_story_fps(conn)?;
    let frame = start + cover_frames;
    // The segment the new marker lies in, and its frame there.
    let mut part_start = 0;
    let mut origin = None;
    for part in active_part_spans(conn)? {
        if frame < part_start + part.frames {
            origin = Some((part.part_id, frame - part_start));
            break;
        }
        part_start += part.frames;
    }
    let Some((part_id, local)) = origin else {
        return Ok(slot_id.to_string());
    };
    let marker_id = new_marker_id();
    let (now, tc) = (story_now(), frame_timecode(frame, fps));
    conn.execute(
        "INSERT INTO story_markers
            (marker_id, timeline_frame, timeline_sec, tc, label, sort_index,
             origin_part_id, origin_local_frame, origin_local_sec, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4, 0, ?5, ?6, ?7, ?8, ?8)",
        params![marker_id, frame, timeline_sec(frame, fps), tc, part_id, local, local as f64 / fps, now],
    )
    .map_err(err)?;
    finalize_story(conn)?;
    Ok(self::slot_id(&start_marker, &marker_id))
}

/// The longest J or L, story frames: two hours at 50p, as long as wanted (user 2026-10-21);
/// the source and the room next to it bound it in the playlist.
pub(crate) const MAX_OVERLAP_FRAMES: u32 = 360_000;
