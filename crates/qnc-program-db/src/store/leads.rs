//! Lead-in and lead-out (user 2026-10-24): short automatic segments before the first and after
//! the last segment of the story, as long as the project settings say (`export.lead_in_seconds`,
//! `export.tail_seconds`). Before an Off they are black and silence; before a Ton the picture of
//! its clip starts there (the frames before its IN) and the sound starts with the segment. After
//! the last segment the same, mirrored. Where the clip has fewer frames, the rest is black.
//! They are rows of `story_parts` marked `lead`, kept by the owner after every edit; the user
//! does not edit them, but slots and covers run over them like over any segment.

use super::*;

const LEAD_IN_SORT: i64 = -1_000_000;
const LEAD_OUT_SORT: i64 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq)]
struct Piece {
    part_id: String,
    lead: &'static str,
    sort_index: i64,
    kind: &'static str,
    clip_id: String,
    in_frame: i64,
    out_frame: i64,
    frames: i64,
    fps: (u32, u32),
    channel: i64,
}

struct Neighbour {
    kind: String,
    clip_id: String,
    in_frame: i64,
    out_frame: i64,
    fps: (u32, u32),
    channel: i64,
}

/// Brings the lead-in and lead-out in line with the first and last segment and the project
/// settings; rows are rewritten only when they differ. A changed lead-in moves the user
/// markers by its difference, so they stay on their pictures.
pub(crate) fn sync_leads(conn: &Connection) -> Result<()> {
    let real = real_parts(conn)?;
    let (Some(first), Some(last)) = (real.first(), real.last()) else {
        conn.execute("DELETE FROM story_parts WHERE lead != ''", []).map_err(err)?;
        return Ok(());
    };
    let rate = story_rate(conn)?.ok_or_else(|| qnc_ui_text::text("story.timeline_fps_invalid_story_program_valid"))?;
    let (lead_in_seconds, lead_out_seconds) = lead_seconds(conn)?;
    let frames = |seconds: f64| (seconds * f64::from(rate.0) / f64::from(rate.1)).round() as i64;
    let mut wanted = pieces(conn, first, frames(lead_in_seconds), rate, true)?;
    wanted.extend(pieces(conn, last, frames(lead_out_seconds), rate, false)?);
    let stored = stored_pieces(conn)?;
    if stored == wanted {
        return Ok(());
    }
    let lead_in = |pieces: &[Piece]| pieces.iter().filter(|piece| piece.lead == "in").map(|piece| piece.frames).sum::<i64>();
    let delta = lead_in(&wanted) - lead_in(&stored);
    conn.execute("DELETE FROM story_parts WHERE lead != ''", []).map_err(err)?;
    let now = story_now();
    for piece in &wanted {
        let fps = f64::from(piece.fps.0) / f64::from(piece.fps.1);
        let story_fps = f64::from(rate.0) / f64::from(rate.1);
        conn.execute(
            "INSERT INTO story_parts (
                part_id, kind, sort_index, title, text, clip_id, virtual_shot_id,
                in_tc, out_tc, in_seconds, out_seconds, fps, source_fps_num, source_fps_den,
                in_frame, out_frame, duration_frames, duration_label, duration_color_key,
                active, created_at, updated_at, a1_source_channel, a1_level_db, a1_off, lead
             ) VALUES (?1, ?2, ?3, '', '', ?4, '', ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                       ?12, ?13, ?14, ?15, ?16, 1, ?17, ?17, ?18, 0, 1, ?19)",
            params![
                piece.part_id,
                piece.kind,
                piece.sort_index,
                piece.clip_id,
                frame_timecode(piece.in_frame, fps),
                frame_timecode(piece.out_frame, fps),
                piece.in_frame as f64 / fps,
                piece.out_frame as f64 / fps,
                fps,
                piece.fps.0,
                piece.fps.1,
                piece.in_frame,
                piece.out_frame,
                piece.frames,
                frames_label(piece.frames, story_fps),
                duration_color_key(piece.frames, story_fps),
                now,
                piece.channel,
                piece.lead
            ],
        )
        .map_err(err)?;
    }
    if delta != 0 {
        shift_markers_from(conn, 0, delta, require_story_fps(conn)?)?;
    }
    Ok(())
}

/// The lead-in or lead-out next to `next` (the first segment for the lead-in, the last for the
/// lead-out): its picture part from the clip of a Ton, as many frames as the clip has, and
/// black for the rest.
fn pieces(conn: &Connection, next: &Neighbour, frames: i64, rate: (u32, u32), before: bool) -> Result<Vec<Piece>> {
    if frames <= 0 {
        return Ok(Vec::new());
    }
    let (num, den) = story_per_source(rate, next.fps)?;
    let duration = qnc_content_read::clip_timing_on(conn, &next.clip_id)?
        .map(|(frames, _)| frames as i64)
        .ok_or_else(|| qnc_ui_text::text("story.clip_timing_unknown"))?;
    let lead = if before { "in" } else { "out" };
    let piece = |index: usize, kind: &'static str, in_frame: i64, out_frame: i64, frames: i64| Piece {
        part_id: format!("lead_{lead}_{index}"),
        lead,
        sort_index: if before { LEAD_IN_SORT } else { LEAD_OUT_SORT } + index as i64,
        kind,
        clip_id: next.clip_id.clone(),
        in_frame,
        out_frame,
        frames,
        fps: next.fps,
        channel: next.channel,
    };
    let picture = if next.kind == "tonovi" {
        let room = if before { next.in_frame } else { duration - next.out_frame }.max(0);
        // Whole source frames that make whole story frames.
        (frames * den / num).min(room) / den * den
    } else {
        0
    };
    let picture_frames = picture * num / den;
    let black = frames - picture_frames;
    // The black part keeps a range of the clip, though nothing of it is seen or heard.
    let black_source = ((black * den + num - 1) / num).min(duration).max(0);
    let black_in = next.in_frame.min(duration - black_source).max(0);
    let mut pieces = Vec::new();
    let push_black = |pieces: &mut Vec<Piece>| {
        if black > 0 {
            let index = pieces.len();
            pieces.push(piece(index, "offovi", black_in, black_in + black_source, black));
        }
    };
    if before {
        push_black(&mut pieces);
        if picture > 0 {
            pieces.push(piece(pieces.len(), "tonovi", next.in_frame - picture, next.in_frame, picture_frames));
        }
    } else {
        if picture > 0 {
            pieces.push(piece(0, "tonovi", next.out_frame, next.out_frame + picture, picture_frames));
        }
        push_black(&mut pieces);
    }
    Ok(pieces)
}

/// The lead-in and lead-out seconds of the project; missing or unlikely is a controlled error.
fn lead_seconds(conn: &Connection) -> Result<(f64, f64)> {
    let settings: Option<String> = conn
        .query_row("SELECT settings_json FROM public_project_settings LIMIT 1", [], |row| row.get(0))
        .optional()
        .ok()
        .flatten();
    let settings: serde_json::Value = settings
        .and_then(|text| serde_json::from_str(&text).ok())
        .ok_or_else(|| qnc_ui_text::text("story.lead_settings_missing"))?;
    let seconds = |key: &str| {
        settings
            .get("export")
            .and_then(|export| export.get(key))
            .and_then(serde_json::Value::as_f64)
            .filter(|seconds| seconds.is_finite() && (0.0..=600.0).contains(seconds))
            .ok_or_else(|| qnc_ui_text::text("story.lead_settings_missing"))
    };
    Ok((seconds("lead_in_seconds")?, seconds("tail_seconds")?))
}

fn real_parts(conn: &Connection) -> Result<Vec<Neighbour>> {
    let mut statement = conn
        .prepare(
            "SELECT kind, clip_id, in_frame, out_frame, source_fps_num, source_fps_den, a1_source_channel
             FROM story_parts WHERE active = 1 AND lead = '' ORDER BY sort_index, rowid",
        )
        .map_err(err)?;
    let rows = statement
        .query_map([], |row| {
            Ok(Neighbour {
                kind: row.get(0)?,
                clip_id: row.get(1)?,
                in_frame: row.get(2)?,
                out_frame: row.get(3)?,
                fps: (row.get(4)?, row.get(5)?),
                channel: row.get(6)?,
            })
        })
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err);
    rows
}

fn stored_pieces(conn: &Connection) -> Result<Vec<Piece>> {
    let mut statement = conn
        .prepare(
            "SELECT part_id, lead, sort_index, kind, clip_id, in_frame, out_frame, duration_frames,
                    source_fps_num, source_fps_den, a1_source_channel
             FROM story_parts WHERE lead != '' ORDER BY sort_index",
        )
        .map_err(err)?;
    let rows = statement
        .query_map([], |row| {
            let lead: String = row.get(1)?;
            let kind: String = row.get(3)?;
            Ok(Piece {
                part_id: row.get(0)?,
                lead: if lead == "in" { "in" } else { "out" },
                sort_index: row.get(2)?,
                kind: if kind == "tonovi" { "tonovi" } else { "offovi" },
                clip_id: row.get(4)?,
                in_frame: row.get(5)?,
                out_frame: row.get(6)?,
                frames: row.get(7)?,
                fps: (row.get(8)?, row.get(9)?),
                channel: row.get(10)?,
            })
        })
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err);
    rows
}

/// A lead-in or lead-out is kept by the owner, never edited by the user.
pub(crate) fn refuse_lead(conn: &Connection, part_id: &str) -> Result<()> {
    let lead: Option<String> = conn
        .query_row("SELECT lead FROM story_parts WHERE part_id = ?1", [part_id.trim()], |row| row.get(0))
        .optional()
        .map_err(err)?;
    if lead.is_some_and(|lead| !lead.is_empty()) {
        return Err(qnc_ui_text::text("story.lead_is_automatic").into());
    }
    Ok(())
}
