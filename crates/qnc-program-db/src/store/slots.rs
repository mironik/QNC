use super::*;

/// v5 `finalize_story_mutation` under the v5 rule: boundary markers, slots from
/// adjacent markers, covers bound to their slot again, selection kept valid.
pub(crate) fn finalize_story(conn: &Connection) -> Result<()> {
    sync_leads(conn)?;
    ensure_boundary_markers(conn)?;
    // A shorter lead-in or lead-out (audit 2026-10-25) can leave a user's marker on a
    // boundary or past the end: past the story it goes, on one frame with another they are one.
    drop_markers_past_the_story(conn)?;
    merge_coincident_markers(conn)?;
    recompute_marker_slots(conn)?;
    rebind_covers(conn)?;
    normalize_story_selection(conn)
}

/// The user's markers before the story's first frame or at or after its end, with their
/// slots and covers.
fn drop_markers_past_the_story(conn: &Connection) -> Result<()> {
    let duration: i64 = active_part_spans(conn)?.iter().map(|part| part.frames).sum();
    let outside = {
        let mut statement = conn
            .prepare("SELECT marker_id FROM story_markers WHERE system_role = '' AND (timeline_frame < 0 OR timeline_frame >= ?1)")
            .map_err(err)?;
        let rows = statement
            .query_map([duration], |row| row.get::<_, String>(0))
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        rows
    };
    delete_markers_with_slots(conn, &outside)
}

/// v5 `recompute_marker_slots`: one slot between every two adjacent markers,
/// empty spans skipped. Identity is the marker pair (v5 rule), not seconds.
pub(crate) fn recompute_marker_slots(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM story_marker_slots", [])
        .map_err(err)?;
    let fps = match require_story_fps(conn) {
        Ok(fps) => fps,
        Err(_) => return Ok(()),
    };
    let markers = {
        let mut statement = conn
            .prepare("SELECT marker_id, timeline_frame FROM story_markers ORDER BY timeline_frame, marker_id")
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        rows
    };
    let now = story_now();
    let mut slot_index = 0i64;
    for pair in markers.windows(2) {
        let (start_id, start) = (&pair[0].0, pair[0].1.max(0));
        let (end_id, end) = (&pair[1].0, pair[1].1.max(start));
        if end <= start {
            continue;
        }
        let (start_sec, end_sec) = (timeline_sec(start, fps), timeline_sec(end, fps));
        conn.execute(
            "INSERT INTO story_marker_slots
                (slot_id, slot_index, start_frame, end_frame, duration_frames,
                 start_sec, end_sec, duration_sec, start_marker_id, end_marker_id,
                 slot_signature, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                slot_id(start_id, end_id),
                slot_index,
                start,
                end,
                end - start,
                start_sec,
                end_sec,
                timeline_sec(end - start, fps),
                start_id,
                end_id,
                format!("start:{start_sec:.3}|end:{end_sec:.3}"),
                now
            ],
        )
        .map_err(err)?;
        slot_index += 1;
    }
    Ok(())
}

/// Deletes markers with their slots: the covers of every slot that starts or
/// ends on one of them go too (user decision 2026-09-24).
pub(crate) fn delete_markers_with_slots(conn: &Connection, marker_ids: &[String]) -> Result<()> {
    if marker_ids.is_empty() {
        return Ok(());
    }
    let covers = {
        let mut statement = conn
            .prepare("SELECT cover_id, slot_id FROM story_covers")
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        rows
    };
    for (cover_id, slot) in covers {
        let (start, end) = slot.split_once('|').unwrap_or(("", ""));
        if marker_ids.iter().any(|id| id == start || id == end) {
            conn.execute("DELETE FROM story_covers WHERE cover_id = ?1", [cover_id])
                .map_err(err)?;
        }
    }
    for marker_id in marker_ids {
        conn.execute(
            "DELETE FROM story_markers WHERE marker_id = ?1",
            [marker_id],
        )
        .map_err(err)?;
    }
    Ok(())
}

/// Roll edit (user 2026-10-11, not in v5): the start marker of a slot moved by `shift`
/// story frames while its end marker stays, so the slot's cover starts that much later
/// (or earlier) in its source: the picture after the marker changes, as the picture
/// before it does, as far as its source allows (it never starts before its first
/// picture; a cover running past its clip plays to its end, `qnc-program-input`); a 1080i source in a 50p
/// story moves by whole source frames (an odd shift is rounded toward zero).
pub(crate) fn roll_cover(conn: &Connection, start_marker_id: &str, shift: i64) -> Result<()> {
    if shift == 0 {
        return Ok(());
    }
    let Some(rate) = story_rate(conn)? else {
        return Ok(());
    };
    type CoverRow = (String, i64, i64, u32, u32);
    let covers = {
        let mut statement = conn
            .prepare(
                "SELECT cover_id, source_in_frame, source_out_frame, source_fps_num, source_fps_den
                 FROM story_covers WHERE substr(slot_id, 1, length(?1) + 1) = ?1 || '|'",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([start_marker_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)))
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<CoverRow>>>()
            .map_err(err)?;
        rows
    };
    for (cover_id, source_in, source_out, fps_num, fps_den) in covers {
        if fps_num == 0 || fps_den == 0 {
            continue;
        }
        let (num, den) = story_per_source(rate, (fps_num, fps_den))?;
        // As far as the source allows: a cover already at its clip's first picture keeps
        // it and only grows (user 2026-10-11: a move is never refused for a cover).
        let new_in = (source_in + shift * den / num).max(0);
        let new_out = new_in + (source_out - source_in);
        let fps = f64::from(fps_num) / f64::from(fps_den.max(1));
        conn.execute(
            "UPDATE story_covers
             SET source_in_frame = ?1, source_out_frame = ?2, in_seconds = ?3, out_seconds = ?4,
                 in_tc = ?5, out_tc = ?6, updated_at = ?7
             WHERE cover_id = ?8",
            params![
                new_in,
                new_out,
                timeline_sec(new_in, fps),
                timeline_sec(new_out, fps),
                frame_timecode(new_in, fps),
                frame_timecode(new_out, fps),
                story_now(),
                cover_id
            ],
        )
        .map_err(err)?;
    }
    Ok(())
}

/// Markers that came onto one program frame (an excluded segment with a marker on its
/// first and on its last frame, audit 2026-10-13) become one: the locked one, else the
/// one created first, stays. The covers of the slots of the other one go to the marker
/// that stays; a cover of the slot between them (the excluded picture) goes.
pub(crate) fn merge_coincident_markers(conn: &Connection) -> Result<()> {
    let markers = {
        let mut statement = conn
            .prepare(
                "SELECT marker_id, timeline_frame FROM story_markers
                 ORDER BY timeline_frame, system_role = '', created_at, marker_id",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        rows
    };
    let mut kept: Option<(String, i64)> = None;
    for (marker_id, frame) in markers {
        match &kept {
            Some((keep, at)) if *at == frame => {
                let covers = {
                    let mut statement = conn.prepare("SELECT cover_id, slot_id FROM story_covers").map_err(err)?;
                    let rows = statement
                        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
                        .map_err(err)?
                        .collect::<rusqlite::Result<Vec<_>>>()
                        .map_err(err)?;
                    rows
                };
                for (cover_id, slot) in covers {
                    let (a, b) = slot.split_once('|').unwrap_or(("", ""));
                    let swap = |id: &str| if id == marker_id { keep.as_str() } else { id }.to_string();
                    let (a, b) = (swap(a), swap(b));
                    if a == b {
                        conn.execute("DELETE FROM story_covers WHERE cover_id = ?1", [&cover_id]).map_err(err)?;
                    } else if format!("{a}|{b}") != slot {
                        conn.execute(
                            "UPDATE story_covers SET slot_id = ?1 WHERE cover_id = ?2",
                            params![slot_id(&a, &b), cover_id],
                        )
                        .map_err(err)?;
                    }
                }
                conn.execute("DELETE FROM story_markers WHERE marker_id = ?1", [&marker_id]).map_err(err)?;
            }
            _ => kept = Some((marker_id, frame)),
        }
    }
    Ok(())
}

/// A new marker inside a covered slot splits its cover in two (user 2026-10-13, not v5,
/// which kept the cover only before the marker and left the rest empty): the cover keeps
/// the part before the marker, a second cover of the same shot plays on from the marker
/// in the slot after it, so the picture does not change and each part can be changed.
pub(crate) fn split_cover_at(conn: &Connection, marker_id: &str, frame: i64) -> Result<()> {
    let slot: Option<(String, i64, String)> = conn
        .query_row(
            "SELECT slot_id, start_frame, end_marker_id FROM story_marker_slots
             WHERE start_frame < ?1 AND ?1 < end_frame",
            [frame],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(err)?;
    let Some((covered, start, end_marker)) = slot else {
        return Ok(());
    };
    let cover: Option<(String, i64, i64, u32, u32)> = conn
        .query_row(
            "SELECT cover_id, source_in_frame, source_out_frame, source_fps_num, source_fps_den
             FROM story_covers WHERE slot_id = ?1",
            [&covered],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()
        .map_err(err)?;
    let (Some((cover_id, source_in, source_out, fps_num, fps_den)), Some(rate)) = (cover, story_rate(conn)?) else {
        return Ok(());
    };
    if fps_num == 0 || fps_den == 0 {
        return Ok(());
    }
    let (num, den) = story_per_source(rate, (fps_num, fps_den))?;
    let split = source_in + (frame - start) * den / num;
    let fps = f64::from(fps_num) / f64::from(fps_den);
    let created = unique_stamp();
    let now = story_now();
    conn.execute(
        "INSERT INTO story_covers (
            cover_id, slot_id, timeline_start_frame, timeline_end_frame, timeline_start_sec,
            timeline_end_sec, slot_signature, slot_index, clip_id, virtual_shot_id, title, note,
            in_tc, out_tc, in_seconds, out_seconds, source_in_frame, source_out_frame, source_fps,
            source_fps_num, source_fps_den, sort_index, a2_source_channel, a2_level_db, a2_off,
            a2_lead_frames, a2_tail_frames, created_at, updated_at)
         SELECT ?1, ?2, ?3, timeline_end_frame, ?4, timeline_end_sec, slot_signature, slot_index,
            clip_id, virtual_shot_id, title, note, ?5, out_tc, ?6, out_seconds, ?7, source_out_frame,
            source_fps, source_fps_num, source_fps_den, sort_index, a2_source_channel, a2_level_db,
            a2_off, 0, a2_tail_frames, ?8, ?8
         FROM story_covers WHERE cover_id = ?9",
        params![
            format!("cover_{created:x}"),
            slot_id(marker_id, &end_marker),
            frame,
            timeline_sec(frame, fps),
            frame_timecode(split, fps),
            timeline_sec(split, fps),
            split,
            now,
            cover_id
        ],
    )
    .map_err(err)?;
    conn.execute(
        "UPDATE story_covers SET source_out_frame = ?1, a2_tail_frames = 0, out_tc = ?2, out_seconds = ?3, updated_at = ?4
         WHERE cover_id = ?5",
        params![split.min(source_out), frame_timecode(split, fps), timeline_sec(split, fps), now, cover_id],
    )
    .map_err(err)?;
    Ok(())
}

/// Every cover whose clip ends before its slot does: cover id -> (pictures missing, the
/// picture its slot would need, the clip's length). A cover plays its clip from IN for its
/// slot's length, at the story's ratio to its source.
pub(crate) fn cover_shortfalls(conn: &Connection) -> Result<std::collections::HashMap<String, (i64, i64, u64)>> {
    let mut short = std::collections::HashMap::new();
    let Some(rate) = story_rate(conn)? else {
        return Ok(short);
    };
    type Row = (String, String, i64, i64, u32, u32);
    let rows = {
        let mut statement = conn
            .prepare(
                "SELECT c.cover_id, c.clip_id, c.source_in_frame, s.end_frame - s.start_frame,
                        c.source_fps_num, c.source_fps_den
                 FROM story_covers c JOIN story_marker_slots s ON s.slot_id = c.slot_id",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)))
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<Row>>>()
            .map_err(err)?;
        rows
    };
    for (cover_id, clip_id, source_in, slot_frames, fps_num, fps_den) in rows {
        // A cover without its source rate (not made by CreateCover) cannot be measured.
        if fps_num == 0 || fps_den == 0 {
            continue;
        }
        let (num, den) = story_per_source(rate, (fps_num, fps_den))?;
        let source_end = source_in + (slot_frames * den + num - 1) / num;
        if let Some(frames) = qnc_content_read::duration_frames_on(conn, &clip_id)? {
            if source_end > frames as i64 {
                short.insert(cover_id, (source_end - frames as i64, source_end, frames));
            }
        }
    }
    Ok(short)
}

/// A cover the edit left shorter than its slot (or shorter than it was): the refusal.
pub(crate) fn cover_hole(before: &std::collections::HashMap<String, (i64, i64, u64)>, after: &std::collections::HashMap<String, (i64, i64, u64)>) -> Option<String> {
    after.iter().find(|(cover_id, (missing, _, _))| before.get(*cover_id).is_none_or(|(was, _, _)| missing > was)).map(
        |(_, (_, need, frames))| {
            qnc_ui_text::fill("story.cover_hole", &[("need", &need.to_string()), ("frames", &frames.to_string())])
        },
    )
}

/// Slot identity: the ids of its start and end marker.
pub(crate) fn slot_id(start_marker_id: &str, end_marker_id: &str) -> String {
    format!("{start_marker_id}|{end_marker_id}")
}

pub(crate) struct SlotRow {
    pub(crate) slot_id: String,
    pub(crate) slot_index: i64,
    pub(crate) start_frame: i64,
    pub(crate) end_frame: i64,
    pub(crate) start_marker_id: String,
    pub(crate) signature: String,
}

/// v5 rule: a cover is removed only by the user. Deleting a marker deletes its
/// slots and their covers (`delete_markers_with_slots`). Any other change keeps
/// the cover on its logical slot: the same marker pair (moved markers give it the
/// new frames), else the slot that keeps its start marker (a new marker split the
/// slot: the cover is trimmed). A cover without a slot keeps no frames.
pub(crate) fn rebind_covers(conn: &Connection) -> Result<()> {
    let slots = {
        let mut statement = conn
            .prepare(
                "SELECT slot_id, slot_index, start_frame, end_frame, start_marker_id,
                        slot_signature
                 FROM story_marker_slots ORDER BY slot_index",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                Ok(SlotRow {
                    slot_id: row.get(0)?,
                    slot_index: row.get(1)?,
                    start_frame: row.get(2)?,
                    end_frame: row.get(3)?,
                    start_marker_id: row.get(4)?,
                    signature: row.get(5)?,
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        rows
    };
    let covers = {
        let mut statement = conn
            .prepare("SELECT cover_id, slot_id FROM story_covers ORDER BY created_at, cover_id")
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        rows
    };
    let fps = require_story_fps(conn).unwrap_or(0.0);
    let mut taken: Vec<&str> = Vec::new();
    // Exact pairs first, so a cover that still has its slot keeps it.
    let mut order: Vec<usize> = (0..covers.len()).collect();
    order.sort_by_key(|&index| !slots.iter().any(|slot| slot.slot_id == covers[index].1));
    for index in order {
        let (cover_id, old_slot) = &covers[index];
        let start_marker = old_slot.split_once('|').map_or("", |(start, _)| start);
        let free = |slot: &&SlotRow| !taken.contains(&slot.slot_id.as_str());
        let target = slots
            .iter()
            .filter(free)
            .find(|slot| slot.slot_id == *old_slot)
            .or_else(|| {
                slots
                    .iter()
                    .filter(free)
                    .find(|slot| !start_marker.is_empty() && slot.start_marker_id == start_marker)
            });
        match target {
            Some(slot) => {
                taken.push(slot.slot_id.as_str());
                conn.execute(
                    "UPDATE story_covers
                     SET slot_id = ?1, slot_signature = ?2, slot_index = ?3,
                         timeline_start_frame = ?4, timeline_end_frame = ?5,
                         timeline_start_sec = ?6, timeline_end_sec = ?7
                     WHERE cover_id = ?8",
                    params![
                        slot.slot_id,
                        slot.signature,
                        slot.slot_index,
                        slot.start_frame,
                        slot.end_frame,
                        timeline_sec(slot.start_frame, fps),
                        timeline_sec(slot.end_frame, fps),
                        cover_id
                    ],
                )
                .map_err(err)?;
            }
            None => {
                conn.execute(
                    "UPDATE story_covers
                     SET timeline_end_frame = timeline_start_frame, timeline_end_sec = timeline_start_sec
                     WHERE cover_id = ?1",
                    [cover_id],
                )
                .map_err(err)?;
            }
        }
    }
    Ok(())
}

/// v5 `normalize_selected_slot_id` / `normalize_selected_cover_id`.
pub(crate) fn normalize_story_selection(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "UPDATE story_state SET selected_slot_id = ''
         WHERE id = 1 AND selected_slot_id != ''
           AND selected_slot_id NOT IN (SELECT slot_id FROM story_marker_slots);
         UPDATE story_state SET selected_cover_id = ''
         WHERE id = 1 AND selected_cover_id != ''
           AND selected_cover_id NOT IN (SELECT cover_id FROM story_covers);",
    )
    .map_err(err)
}
