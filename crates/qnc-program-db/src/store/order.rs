use super::*;

/// Program length in frames: the segments one after another.
pub(crate) fn program_length(conn: &Connection) -> Result<i64> {
    conn.query_row(
        "SELECT COALESCE(SUM(CASE WHEN duration_frames > 0 THEN duration_frames ELSE out_frame - in_frame END), 0)
         FROM story_parts WHERE active = 1",
        [],
        |row| row.get(0),
    )
    .map_err(err)
}

/// Program frames `[start, end)` of a segment in the stored order.
pub(crate) fn segment_window(conn: &Connection, segment_id: &str) -> Result<Option<(u64, u64)>> {
    let mut statement = conn
        .prepare(
            "SELECT part_id, CASE WHEN duration_frames > 0 THEN duration_frames ELSE out_frame - in_frame END
             FROM story_parts
             WHERE active = 1 ORDER BY sort_index",
        )
        .map_err(err)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    let mut start = 0u64;
    for (id, frames) in rows {
        let end = start + frames.max(0) as u64;
        if id == segment_id {
            return Ok(Some((start, end)));
        }
        start = end;
    }
    Ok(None)
}

/// The user markers at or after a program frame, with their frames.
pub(crate) fn user_markers_from(conn: &Connection, from: i64) -> Result<Vec<(String, i64)>> {
    let mut statement = conn
        .prepare(
            "SELECT marker_id, timeline_frame FROM story_markers
             WHERE timeline_frame >= ?1 AND system_role = ''",
        )
        .map_err(err)?;
    let rows = statement
        .query_map([from], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err);
    rows
}

/// Puts one marker on a program frame.
pub(crate) fn set_marker_frame(conn: &Connection, marker_id: &str, frame: i64, fps: f64) -> Result<()> {
    let frame = frame.max(0);
    conn.execute(
        "UPDATE story_markers SET timeline_frame = ?1, timeline_sec = ?2, tc = ?3, updated_at = ?4
         WHERE marker_id = ?5",
        params![frame, timeline_sec(frame, fps), frame_timecode(frame, fps), story_now(), marker_id],
    )
    .map_err(err)?;
    Ok(())
}

/// Moves the user markers from `from` up to and including `to` by `delta` frames.
pub(crate) fn shift_markers_between(conn: &Connection, from: i64, to: i64, delta: i64, fps: f64) -> Result<()> {
    let rows = {
        let mut statement = conn
            .prepare(
                "SELECT marker_id, timeline_frame FROM story_markers
                 WHERE timeline_frame >= ?1 AND timeline_frame <= ?2 AND system_role = ''",
            )
            .map_err(err)?;
        let rows = statement
            .query_map(params![from, to], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        rows
    };
    for (marker_id, frame) in rows {
        set_marker_frame(conn, &marker_id, frame + delta, fps)?;
    }
    Ok(())
}

/// Moves the user markers at or after a program frame by `delta` frames.
pub(crate) fn shift_markers_from(conn: &Connection, from: i64, delta: i64, fps: f64) -> Result<()> {
    let rows = {
        let mut statement = conn
            .prepare(
                "SELECT marker_id, timeline_frame FROM story_markers
                 WHERE timeline_frame >= ?1 AND system_role = ''",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([from], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        rows
    };
    let now = story_now();
    for (marker_id, frame) in rows {
        let frame = (frame + delta).max(0);
        conn.execute(
            "UPDATE story_markers SET timeline_frame = ?1, timeline_sec = ?2, tc = ?3,
                updated_at = ?4
             WHERE marker_id = ?5",
            params![
                frame,
                timeline_sec(frame, fps),
                frame_timecode(frame, fps),
                now,
                marker_id
            ],
        )
        .map_err(err)?;
    }
    Ok(())
}

/// Every segment, active or excluded, in its place: one order for both, so an
/// excluded segment comes back where it was.
pub(crate) fn all_segment_order(conn: &Connection) -> Result<Vec<(String, bool)>> {
    let mut statement = conn
        .prepare("SELECT part_id, active FROM story_parts ORDER BY sort_index, rowid")
        .map_err(err)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0))
        })
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    Ok(rows)
}

pub(crate) fn write_segment_order(conn: &Connection, ids: &[String]) -> Result<()> {
    let now = story_now();
    for (sort_index, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE story_parts SET sort_index = ?1, updated_at = ?3 WHERE part_id = ?2",
            params![sort_index as i64, id, now],
        )
        .map_err(err)?;
    }
    Ok(())
}

/// 0..n over all segments, keeping their order (active and excluded together).
pub(crate) fn renumber_segments(conn: &Connection) -> Result<()> {
    let ids: Vec<String> = all_segment_order(conn)?
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    write_segment_order(conn, &ids)
}
