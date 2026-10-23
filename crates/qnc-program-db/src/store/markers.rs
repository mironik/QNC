use super::*;

impl Store {
    /// v5 `create_marker_from_part_frame`: M is placed on a Wrap segment at a frame
    /// inside it (`local_to_timeline_frame`, clamped to the segment); the program
    /// frame follows. A marker already on that frame is refreshed, never duplicated.
    pub(crate) fn create_marker(&mut self, part_id: &str, local_frame: u64) -> Result<Data> {
        let fps = require_story_fps(&self.conn)?;
        let part_id = part_id.trim();
        if part_id.is_empty() {
            return Err("part_id required".into());
        }
        let tx = self.conn.savepoint().map_err(err)?;
        let (start, end) =
            segment_window(&tx, part_id)?.ok_or_else(|| format!("part not found: {part_id}"))?;
        let local = (local_frame as i64).min((end - start) as i64).max(0);
        let frame = start as i64 + local;
        let tc = frame_timecode(frame, fps);
        let existing: Option<String> = tx
            .query_row(
                "SELECT marker_id FROM story_markers WHERE timeline_frame = ?1
                 ORDER BY marker_id LIMIT 1",
                [frame],
                |row| row.get(0),
            )
            .optional()
            .map_err(err)?;
        let marker_id = match existing {
            Some(marker_id) => {
                tx.execute(
                    "UPDATE story_markers
                     SET tc = ?1, origin_part_id = ?2, origin_local_frame = ?3,
                         origin_local_sec = ?4, updated_at = ?5
                     WHERE marker_id = ?6",
                    params![
                        tc,
                        part_id,
                        local,
                        local as f64 / fps,
                        story_now(),
                        marker_id
                    ],
                )
                .map_err(err)?;
                marker_id
            }
            None => {
                let marker_id = new_marker_id();
                let now = story_now();
                tx.execute(
                    "INSERT INTO story_markers
                        (marker_id, timeline_frame, timeline_sec, tc, label, sort_index,
                         origin_part_id, origin_local_frame, origin_local_sec, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?4, 0, ?5, ?6, ?7, ?8, ?8)",
                    params![
                        marker_id,
                        frame,
                        timeline_sec(frame, fps),
                        tc,
                        part_id,
                        local,
                        local as f64 / fps,
                        now
                    ],
                )
                .map_err(err)?;
                split_cover_at(&tx, &marker_id, frame)?;
                marker_id
            }
        };
        finalize_story(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Created(marker_id))
    }

    /// v5 `update_marker_frame`: inside the story length, on a free frame; the start
    /// and the end marker are locked. The label is kept.
    pub(crate) fn move_marker(&mut self, marker_id: &str, program_frame: u64) -> Result<Data> {
        let fps = require_story_fps(&self.conn)?;
        let marker_id = marker_id.trim();
        if marker_id.is_empty() {
            return Err("marker_id required".into());
        }
        let frame = program_frame as i64;
        let duration = program_length(&self.conn)?;
        if frame > duration {
            return Err(qnc_ui_text::fill("story.marker_inside_story", &[("seconds", &format!("{:.3}", timeline_sec(duration, fps)))]));
        }
        let tx = self.conn.savepoint().map_err(err)?;
        locked_check(&tx, marker_id, duration)?;
        let taken: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM story_markers
                 WHERE marker_id != ?1 AND timeline_frame = ?2)",
                params![marker_id, frame],
                |row| row.get(0),
            )
            .map_err(err)?;
        if taken {
            return Err(format!("marker already exists at timeline_frame={frame}"));
        }
        let old: i64 = tx
            .query_row("SELECT timeline_frame FROM story_markers WHERE marker_id = ?1", [marker_id], |row| row.get(0))
            .map_err(err)?;
        roll_cover(&tx, marker_id, frame - old)?;
        tx.execute(
            "UPDATE story_markers SET timeline_frame = ?1, timeline_sec = ?2, tc = ?3,
                updated_at = ?4
             WHERE marker_id = ?5",
            params![
                frame,
                timeline_sec(frame, fps),
                frame_timecode(frame, fps),
                story_now(),
                marker_id
            ],
        )
        .map_err(err)?;
        finalize_story(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// A slot moved (user 2026-10-10, not in v5): both its markers move by the same
    /// frames, so it keeps its length and its cover; a neighbouring slot grows or
    /// shrinks. The markers may not reach a neighbouring marker; the start and the end
    /// marker are locked. One edit, one UNDO step.
    pub(crate) fn move_slot(&mut self, slot_id: &str, start_frame: u64) -> Result<Data> {
        let fps = require_story_fps(&self.conn)?;
        let duration = program_length(&self.conn)?;
        let tx = self.conn.savepoint().map_err(err)?;
        let (start_id, end_id): (String, String) = tx
            .query_row(
                "SELECT start_marker_id, end_marker_id FROM story_marker_slots WHERE slot_id = ?1",
                [slot_id.trim()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(err)?
            .ok_or_else(|| format!("slot not found: {slot_id}"))?;
        locked_check(&tx, &start_id, duration)?;
        locked_check(&tx, &end_id, duration)?;
        let frame_of = |id: &str| -> Result<i64> {
            tx.query_row("SELECT timeline_frame FROM story_markers WHERE marker_id = ?1", [id], |row| row.get(0))
                .map_err(err)
        };
        let (start, end) = (frame_of(&start_id)?, frame_of(&end_id)?);
        let new_start = start_frame as i64;
        let new_end = end + (new_start - start);
        let (low, high) = (new_start.min(start), new_end.max(end));
        let blocked: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM story_markers
                 WHERE marker_id NOT IN (?1, ?2) AND timeline_frame BETWEEN ?3 AND ?4)",
                params![start_id, end_id, low, high],
                |row| row.get(0),
            )
            .map_err(err)?;
        if blocked || new_start <= 0 || new_end >= duration {
            return Err("Slot se pomice samo do susjednog M markera.".into());
        }
        // The slot keeps its cover as it is; the slot after it starts elsewhere (roll).
        roll_cover(&tx, &end_id, new_start - start)?;
        for (id, frame) in [(&start_id, new_start), (&end_id, new_end)] {
            tx.execute(
                "UPDATE story_markers SET timeline_frame = ?1, timeline_sec = ?2, tc = ?3,
                    updated_at = ?4
                 WHERE marker_id = ?5",
                params![frame, timeline_sec(frame, fps), frame_timecode(frame, fps), story_now(), id],
            )
            .map_err(err)?;
        }
        finalize_story(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// A slot trimmed or extended at one edge (user 2026-10-13, not in v5): the other
    /// slots keep their lengths, so every marker from the slot's end on moves by
    /// `frames` and the last slot of the story takes the difference (it keeps at least
    /// one frame; the locked end stays). At its start the slot keeps its place and its
    /// cover starts `frames` earlier in its source (as far as the source allows); at its
    /// end the cover plays on. The last slot ends with the story: its edges move with Ctrl+M.
    pub(crate) fn trim_slot(&mut self, slot_id: &str, start: bool, frames: i64) -> Result<Data> {
        let fps = require_story_fps(&self.conn)?;
        let duration = program_length(&self.conn)?;
        let tx = self.conn.savepoint().map_err(err)?;
        let (start_id, slot_start, slot_end): (String, i64, i64) = tx
            .query_row(
                "SELECT start_marker_id, start_frame, end_frame FROM story_marker_slots WHERE slot_id = ?1",
                [slot_id.trim()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(err)?
            .ok_or_else(|| format!("slot not found: {slot_id}"))?;
        if slot_end >= duration {
            return Err("Zadnji slot zavrsava s pricom: pomakni njegov pocetni M marker.".into());
        }
        if frames == 0 {
            return Ok(Data::Changed);
        }
        if slot_end - slot_start + frames < 1 {
            return Err(qnc_ui_text::text("story.msg.slot_one_picture").into());
        }
        // The first slot after it without a cover takes the difference (user 2026-10-13: an
        // empty slot does not push the slots after it); else the last slot of the story.
        let empty: Option<(i64, i64)> = tx
            .query_row(
                "SELECT start_frame, end_frame FROM story_marker_slots
                 WHERE start_frame >= ?1 AND slot_id NOT IN (SELECT slot_id FROM story_covers)
                 ORDER BY start_frame LIMIT 1",
                [slot_end],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(err)?;
        match empty {
            Some((empty_start, empty_end)) => {
                if empty_end - (empty_start + frames) < 1 {
                    return Err(qnc_ui_text::text("story.msg.empty_slot_one_picture").into());
                }
            }
            None => {
                let last = user_markers_from(&tx, slot_end)?.into_iter().map(|(_, frame)| frame).max().unwrap_or(slot_end);
                if last + frames >= duration {
                    return Err(qnc_ui_text::text("story.msg.last_slot_one_picture").into());
                }
            }
        }
        if start {
            // The slot keeps its place: its cover starts `frames` earlier in its source.
            roll_cover(&tx, &start_id, -frames)?;
        }
        match empty {
            Some((empty_start, _)) => shift_markers_between(&tx, slot_end, empty_start, frames, fps)?,
            None => shift_markers_from(&tx, slot_end, frames, fps)?,
        }
        finalize_story(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// v5 `delete_marker`: the start and the end marker are locked.
    pub(crate) fn delete_marker(&mut self, marker_id: &str) -> Result<Data> {
        require_story_fps(&self.conn)?;
        let marker_id = marker_id.trim();
        if marker_id.is_empty() {
            return Err("marker_id required".into());
        }
        let duration = program_length(&self.conn)?;
        let tx = self.conn.savepoint().map_err(err)?;
        locked_check(&tx, marker_id, duration)?;
        delete_markers_with_slots(&tx, &[marker_id.to_string()])?;
        finalize_story(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }
}
