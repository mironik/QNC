use super::*;

pub(crate) fn source_row(conn: &Connection, clip_id: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT shot_id FROM virtual_shots WHERE clip_id = ?1 AND class = 'source' LIMIT 1",
        [clip_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(err)
}

/// The next number of a shot of `class` of a clip (external audit 2026-10-17: a number
/// counted from the rows came back after a B-roll shot of a refused cover went, and the
/// next shot took an id already used). It only grows: a kept counter per clip and class,
/// never below the highest number an id of that class already has.
pub(crate) fn next_index(conn: &Connection, clip_id: &str, class: &str, marker: &str) -> Result<u32> {
    let kept: i64 = conn
        .query_row(
            "SELECT last FROM virtual_shot_numbers WHERE clip_id = ?1 AND class = ?2",
            [clip_id, class],
            |row| row.get(0),
        )
        .optional()
        .map_err(err)?
        .unwrap_or(0);
    let used = {
        let mut statement = conn
            .prepare("SELECT shot_id FROM virtual_shots WHERE clip_id = ?1 AND class = ?2")
            .map_err(err)?;
        let ids = statement
            .query_map([clip_id, class], |row| row.get::<_, String>(0))
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        ids.iter()
            .filter_map(|id| id.rsplit_once(marker).and_then(|(_, number)| number.parse::<i64>().ok()))
            .max()
            .unwrap_or(0)
    };
    let next = kept.max(used) + 1;
    conn.execute(
        "INSERT INTO virtual_shot_numbers (clip_id, class, last) VALUES (?1, ?2, ?3)
         ON CONFLICT(clip_id, class) DO UPDATE SET last = excluded.last",
        params![clip_id, class, next],
    )
    .map_err(err)?;
    u32::try_from(next).map_err(err)
}

pub(crate) fn short_clip_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ShortClip> {
    Ok(ShortClip {
        shot_id: row.get(0)?,
        clip_id: row.get(1)?,
        in_frame: row.get::<_, i64>(2)?.max(0) as u64,
        out_frame: row.get::<_, i64>(3)?.max(0) as u64,
        name: row.get(4)?,
        in_still_uri: row.get(5)?,
        out_still_uri: row.get(6)?,
        still_status: row.get(7)?,
        b_roll: false,
    })
}

pub(crate) fn object_exists(conn: &Connection, kind: &str, name: &str) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type=?1 AND name=?2)",
        params![kind, name],
        |r| r.get(0),
    )
    .map_err(err)
}

/// Only an imported clip gets a virtual shot, read through the one public reader of
/// the clip catalog.
pub(crate) fn require_imported_clip(conn: &Connection, clip_id: &str) -> Result<()> {
    match qnc_content_read::imported_on(conn, clip_id)? {
        Some(true) => Ok(()),
        Some(false) => Err(qnc_ui_text::fill("story.msg.clip_not_imported", &[("clip", &clip_id.to_string())])),
        None => Err(qnc_ui_text::text("ingest.msg.clip_not_found").into()),
    }
}

pub(crate) fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}
