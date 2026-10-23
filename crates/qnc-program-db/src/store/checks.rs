use super::*;

/// Only an imported clip goes into the story (docs/93 R10), read through the one
/// public reader of the clip catalog.
/// The source a client sends against the clip's stored record (external audit
/// 2026-10-17: the writer trusted the client): its rate is the stored one and its OUT
/// lies inside the clip. A clip without a stored length or rate is refused, never guessed.
pub(crate) fn require_source_range(conn: &Connection, clip_id: &str, (_, out_frame): (u64, u64), (fps_num, fps_den): (u32, u32)) -> Result<()> {
    let Some((frames, (num, den))) = qnc_content_read::clip_timing_on(conn, clip_id)? else {
        return Err(qnc_ui_text::text("story.clip_timing_unknown").into());
    };
    if u64::from(fps_num) * u64::from(den) != u64::from(num) * u64::from(fps_den) {
        let (sent, stored) = (format!("{fps_num}/{fps_den}"), format!("{num}/{den}"));
        return Err(qnc_ui_text::fill("story.source_rate_not_stored", &[("sent", &sent), ("stored", &stored)]));
    }
    if out_frame > frames {
        let (out, frames) = (out_frame.to_string(), frames.to_string());
        return Err(qnc_ui_text::fill("story.out_past_clip", &[("out", &out), ("frames", &frames)]));
    }
    Ok(())
}

pub(crate) fn require_imported_clip(conn: &Connection, clip_id: &str) -> Result<()> {
    match qnc_content_read::imported_on(conn, clip_id)? {
        Some(true) => Ok(()),
        Some(false) => Err(qnc_ui_text::fill("story.msg.clip_not_imported", &[("clip", &clip_id.to_string())])),
        None => Err(qnc_ui_text::text("ingest.msg.clip_not_found").into()),
    }
}

pub(crate) fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    conn.prepare(&format!("PRAGMA table_info({table})"))
        .and_then(|mut statement| {
            statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(err)
}

/// A stored source channel index; the column never holds a negative one.
/// The lowest and highest level of A1/A2 in dB (the source timeline's range).
pub(crate) const LANE_LEVEL_DB: (i8, i8) = (-40, 12);

pub(crate) fn lane_level(level_db: i8) -> Result<i8> {
    if (LANE_LEVEL_DB.0..=LANE_LEVEL_DB.1).contains(&level_db) {
        Ok(level_db)
    } else {
        Err(format!("lane level {level_db} dB outside {}..{} dB", LANE_LEVEL_DB.0, LANE_LEVEL_DB.1))
    }
}

pub(crate) fn level(value: i64) -> i8 {
    value.clamp(i64::from(LANE_LEVEL_DB.0), i64::from(LANE_LEVEL_DB.1)) as i8
}

pub(crate) fn channel(value: i64) -> u16 {
    u16::try_from(value.max(0)).unwrap_or(u16::MAX)
}


pub(crate) fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}
pub(crate) fn object_exists(conn: &Connection, kind: &str, name: &str) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type=?1 AND name=?2)",
        params![kind, name],
        |r| r.get(0),
    )
    .map_err(err)
}
