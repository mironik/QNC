//! The S-Story (user 2026-10-31: "in the S-Story treat every clip as a slot; everything we do
//! with a slot applies to the offs and the tons"): its story has an M marker on every border of
//! two clips, so every clip is one slot, and what a slot's cover does there the clip does. The
//! slot's own edits keep their names; in a draft story they write the clip, through the edits
//! a join of two clips already has:
//! - a marker moved whole is a roll of the two clips (`roll_join`),
//! - a slot edge makes its clip longer or shorter and the story with it (`trim_segment`; user:
//!   a longer slot makes the story longer, a shorter one shorter),
//! - a slot moved whole keeps its clip as it is; the clip before plays on, the one after starts
//!   later (as a cover's slot does).

use super::*;

/// The S-Story: the story is a draft (no lead-in or lead-out, every clip a slot).
pub(crate) fn clips_are_slots(conn: &Connection) -> bool {
    qnc_db_broker::is_draft(conn)
}

/// The borders of two active segments, the segment starting at each.
fn borders(conn: &Connection) -> Result<Vec<(i64, String)>> {
    let parts = active_part_spans(conn)?;
    let mut frame = 0;
    let mut borders = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            borders.push((frame, part.part_id.clone()));
        }
        frame += part.frames;
    }
    Ok(borders)
}

/// The markers of a draft story are its clips' borders: a missing one is made, one elsewhere
/// goes with its slot.
pub(crate) fn ensure_clip_borders(conn: &Connection) -> Result<()> {
    if !clips_are_slots(conn) {
        return Ok(());
    }
    let fps = require_story_fps(conn)?;
    let borders = borders(conn)?;
    let elsewhere: Vec<String> = user_markers_from(conn, 0)?
        .into_iter()
        .filter(|(_, frame)| !borders.iter().any(|(border, _)| border == frame))
        .map(|(id, _)| id)
        .collect();
    delete_markers_with_slots(conn, &elsewhere)?;
    let now = story_now();
    for (frame, part_id) in borders {
        let there: bool = conn
            .query_row("SELECT EXISTS(SELECT 1 FROM story_markers WHERE timeline_frame = ?1)", [frame], |row| row.get(0))
            .map_err(err)?;
        if there {
            continue;
        }
        let tc = frame_timecode(frame, fps);
        conn.execute(
            "INSERT INTO story_markers
                (marker_id, timeline_frame, timeline_sec, tc, label, sort_index,
                 origin_part_id, origin_local_frame, origin_local_sec, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4, 0, ?5, 0, 0, ?6, ?6)",
            params![new_marker_id(), frame, timeline_sec(frame, fps), tc, part_id, now],
        )
        .map_err(err)?;
    }
    Ok(())
}

/// The segment starting (`start`) or ending at a program frame.
fn segment_at_border(conn: &Connection, frame: i64, start: bool) -> Result<Option<String>> {
    let mut at = 0;
    for part in active_part_spans(conn)? {
        let end = at + part.frames;
        if (start && at == frame) || (!start && end == frame) {
            return Ok(Some(part.part_id));
        }
        at = end;
    }
    Ok(None)
}

/// A segment's source IN and OUT and `story` frames as frames of its own source.
fn source_range(conn: &Connection, segment_id: &str, story: i64) -> Result<(i64, i64, i64)> {
    let rate = story_rate(conn)?.ok_or(qnc_ui_text::text("story.story_segments"))?;
    let (in_frame, out_frame, num, den): (i64, i64, u32, u32) = conn
        .query_row(
            "SELECT source_in_frame, source_out_frame, source_fps_num, source_fps_den FROM story_items
             WHERE role = 'segment' AND item_id = ?1 AND active = 1",
            [segment_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(err)?;
    let (num, den) = story_per_source(rate, (num, den))?;
    Ok((in_frame, out_frame, story * den / num))
}

fn source_frame(frame: i64) -> Result<u64> {
    u64::try_from(frame).map_err(|_| qnc_ui_text::text("story.range_shorter_than_one_picture_story").to_string())
}

impl Store {
    /// A marker moved whole in an S-Story: the roll of the two clips it is between. `None` when
    /// the story is not one or the marker is on no border.
    pub(crate) fn move_clip_border(&mut self, marker_id: &str, program_frame: u64) -> Option<Result<Data>> {
        if !clips_are_slots(&self.conn) {
            return None;
        }
        let duration = match program_length(&self.conn) {
            Ok(duration) => duration,
            Err(error) => return Some(Err(error)),
        };
        if let Err(error) = locked_check(&self.conn, marker_id, duration) {
            return Some(Err(error));
        }
        let old: i64 = self
            .conn
            .query_row("SELECT timeline_frame FROM story_markers WHERE marker_id = ?1", [marker_id], |row| row.get(0))
            .ok()?;
        let after = segment_at_border(&self.conn, old, true).ok()??;
        Some(self.roll_join(&after, program_frame as i64 - old))
    }

    /// The picture of a border alone moved in an S-Story (the zoom, as a cover's picture cut): the
    /// picture J of the clip after it (earlier) or the picture L of the clip before (later); the
    /// sound stays at the border.
    pub(crate) fn move_clip_border_picture(&mut self, marker_id: &str, program_frame: u64) -> Option<Result<Data>> {
        if !clips_are_slots(&self.conn) {
            return None;
        }
        let old: i64 = self
            .conn
            .query_row("SELECT timeline_frame FROM story_markers WHERE marker_id = ?1", [marker_id], |row| row.get(0))
            .ok()?;
        let after = segment_at_border(&self.conn, old, true).ok()??;
        let shift = program_frame as i64 - old;
        let (tail, lead) = if shift > 0 { (shift as u32, 0) } else { (0, (-shift) as u32) };
        Some(self.set_join_picture(&after, tail, lead))
    }

    /// A slot edge in an S-Story: its clip `frames` longer (negative: shorter) at its start or
    /// its end, and the story with it.
    pub(crate) fn trim_clip_slot(&mut self, slot_id: &str, start: bool, frames: i64) -> Option<Result<Data>> {
        if !clips_are_slots(&self.conn) {
            return None;
        }
        Some(self.trim_clip_slot_now(slot_id, start, frames))
    }

    fn trim_clip_slot_now(&mut self, slot_id: &str, start: bool, frames: i64) -> Result<Data> {
        if frames == 0 {
            return Ok(Data::Changed);
        }
        let (slot_start, slot_end) = slot_frames(&self.conn, slot_id)?;
        let segment = segment_at_border(&self.conn, if start { slot_start } else { slot_end }, start)?
            .ok_or_else(|| format!("slot is not one clip: {slot_id}"))?;
        let (in_frame, out_frame, source) = source_range(&self.conn, &segment, frames)?;
        let range = if start { (source_frame(in_frame - source)?, out_frame as u64) } else { (in_frame as u64, source_frame(out_frame + source)?) };
        self.trim_segment(&segment, range)
    }

    /// A slot moved whole in an S-Story: its clip stays as it is, the clip before plays on and
    /// the one after starts later (or the other way).
    pub(crate) fn move_clip_slot(&mut self, slot_id: &str, start_frame: u64) -> Option<Result<Data>> {
        if !clips_are_slots(&self.conn) {
            return None;
        }
        Some(self.move_clip_slot_now(slot_id, start_frame))
    }

    fn move_clip_slot_now(&mut self, slot_id: &str, start_frame: u64) -> Result<Data> {
        let (slot_start, slot_end) = slot_frames(&self.conn, slot_id)?;
        let shift = start_frame as i64 - slot_start;
        if shift == 0 {
            return Ok(Data::Changed);
        }
        let (Some(before), Some(after)) = (segment_at_border(&self.conn, slot_start, false)?, segment_at_border(&self.conn, slot_end, true)?) else {
            return Err(qnc_ui_text::text("story.slot_moves_to_neighbour").into());
        };
        let (before_in, before_out, before_shift) = source_range(&self.conn, &before, shift)?;
        let (after_in, after_out, after_shift) = source_range(&self.conn, &after, shift)?;
        self.trim_segment(&before, (before_in as u64, source_frame(before_out + before_shift)?))?;
        self.trim_segment(&after, (source_frame(after_in + after_shift)?, after_out as u64))
    }
}

/// A slot's first and end program frame.
fn slot_frames(conn: &Connection, slot_id: &str) -> Result<(i64, i64)> {
    conn.query_row("SELECT start_frame, end_frame FROM story_marker_slots WHERE slot_id = ?1", [slot_id.trim()], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()
        .map_err(err)?
        .ok_or_else(|| format!("slot not found: {slot_id}"))
}
