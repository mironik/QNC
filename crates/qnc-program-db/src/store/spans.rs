use super::*;

/// v5 lock of the start and the end marker on delete and move.
pub(crate) fn locked_check(conn: &Connection, marker_id: &str, duration: i64) -> Result<()> {
    let marker: Option<(i64, String)> = conn
        .query_row(
            "SELECT timeline_frame, system_role FROM story_markers WHERE marker_id = ?1",
            [marker_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(err)?;
    let (frame, role) = marker.ok_or_else(|| format!("marker not found: {marker_id}"))?;
    if frame == 0 || role == "program_start" {
        return Err(qnc_ui_text::text("story.msg.start_marker_locked").into());
    }
    if role == "program_end" || frame == duration {
        return Err(qnc_ui_text::text("story.end_m_marker_locked").into());
    }
    Ok(())
}

pub(crate) struct StoryPartSpan {
    pub(crate) part_id: String,
    pub(crate) frames: i64,
}

/// Active segments in order with v5 `part_span_frames`.
pub(crate) fn active_part_spans(conn: &Connection) -> Result<Vec<StoryPartSpan>> {
    let mut statement = conn
        .prepare(
            "SELECT part_id, duration_frames, in_frame, out_frame FROM story_parts
             WHERE active = 1 ORDER BY sort_index",
        )
        .map_err(err)?;
    let rows = statement
        .query_map([], |row| {
            let duration: i64 = row.get(1)?;
            let (in_frame, out_frame): (i64, i64) = (row.get(2)?, row.get(3)?);
            let frames = if duration > 0 {
                duration
            } else {
                (out_frame - in_frame).max(0)
            };
            Ok(StoryPartSpan {
                part_id: row.get(0)?,
                frames,
            })
        })
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    Ok(rows)
}

pub(crate) fn first_marker(conn: &Connection, sql: &str, value: i64) -> Result<Option<String>> {
    conn.query_row(sql, [value], |row| row.get(0))
        .optional()
        .map_err(err)
}

/// v5 `ensure_start_marker` + `ensure_end_marker`: exactly one locked start
/// marker at frame 0 on the first segment and one locked end marker at the
/// program length on the last segment; no segments, no markers.
pub(crate) fn ensure_boundary_markers(conn: &Connection) -> Result<()> {
    let parts = active_part_spans(conn)?;
    let duration: i64 = parts.iter().map(|part| part.frames).sum();
    if duration <= 0 {
        conn.execute_batch(
            "DELETE FROM story_covers;
             DELETE FROM story_marker_slots;
             DELETE FROM story_markers;
             UPDATE story_state SET selected_slot_id = '' WHERE id = 1;",
        )
        .map_err(err)?;
        return Ok(());
    }
    let fps = require_story_fps(conn)?;
    let now = story_now();
    let first = parts
        .first()
        .map(|part| part.part_id.as_str())
        .unwrap_or("");
    let start_tc = frame_timecode(0, fps);
    let start = match first_marker(
        conn,
        "SELECT marker_id FROM story_markers WHERE system_role = 'program_start'
         AND ?1 = ?1 ORDER BY timeline_frame, marker_id LIMIT 1",
        0,
    )? {
        Some(id) => Some(id),
        None => first_marker(
            conn,
            "SELECT marker_id FROM story_markers WHERE timeline_frame = ?1
             ORDER BY marker_id LIMIT 1",
            0,
        )?,
    };
    match start {
        Some(id) => {
            conn.execute(
                "UPDATE story_markers
                 SET timeline_frame = 0, origin_part_id = ?1, origin_local_frame = 0,
                     origin_local_sec = 0, tc = ?2, label = ?2, system_role = 'program_start'
                 WHERE marker_id = ?3",
                params![first, start_tc, id],
            )
            .map_err(err)?;
            conn.execute(
                "UPDATE story_markers SET system_role = ''
                 WHERE system_role = 'program_start' AND marker_id != ?1",
                [&id],
            )
            .map_err(err)?;
        }
        None => {
            conn.execute(
                "INSERT INTO story_markers
                    (marker_id, timeline_frame, timeline_sec, tc, label, sort_index, system_role,
                     origin_part_id, origin_local_frame, origin_local_sec, created_at, updated_at)
                 VALUES (?1, 0, 0, ?2, ?2, 0, 'program_start', ?3, 0, 0, ?4, ?4)",
                params![new_marker_id(), start_tc, first, now],
            )
            .map_err(err)?;
        }
    }
    let at_duration = first_marker(
        conn,
        "SELECT marker_id FROM story_markers
         WHERE timeline_frame = ?1 AND system_role != 'program_start'
         ORDER BY marker_id LIMIT 1",
        duration,
    )?;
    let system_end = first_marker(
        conn,
        "SELECT marker_id FROM story_markers WHERE system_role = 'program_end'
         AND ?1 = ?1 ORDER BY timeline_frame, marker_id LIMIT 1",
        0,
    )?;
    if let (Some(system_end), Some(at_duration)) = (&system_end, &at_duration) {
        if system_end != at_duration {
            conn.execute(
                "DELETE FROM story_markers WHERE marker_id = ?1",
                [system_end],
            )
            .map_err(err)?;
        }
    }
    let last = parts.last().expect("duration > 0 has a segment");
    let end_sec = timeline_sec(duration, fps);
    let end_tc = frame_timecode(duration, fps);
    let local_sec = last.frames as f64 / fps;
    match at_duration.or(system_end) {
        Some(id) => {
            conn.execute(
                "UPDATE story_markers
                 SET timeline_frame = ?1, timeline_sec = ?2, tc = ?3, label = ?3,
                     sort_index = 0, system_role = 'program_end', origin_part_id = ?4,
                     origin_local_frame = ?5, origin_local_sec = ?6, updated_at = ?7
                 WHERE marker_id = ?8",
                params![
                    duration,
                    end_sec,
                    end_tc,
                    last.part_id,
                    last.frames,
                    local_sec,
                    now,
                    id
                ],
            )
            .map_err(err)?;
            conn.execute(
                "UPDATE story_markers SET system_role = ''
                 WHERE system_role = 'program_end' AND marker_id != ?1",
                [&id],
            )
            .map_err(err)?;
        }
        None => {
            conn.execute(
                "INSERT INTO story_markers
                    (marker_id, timeline_frame, timeline_sec, tc, label, sort_index, system_role,
                     origin_part_id, origin_local_frame, origin_local_sec, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4, 0, 'program_end', ?5, ?6, ?7, ?8, ?8)",
                params![
                    new_marker_id(),
                    duration,
                    end_sec,
                    end_tc,
                    last.part_id,
                    last.frames,
                    local_sec,
                    now
                ],
            )
            .map_err(err)?;
        }
    }
    Ok(())
}
