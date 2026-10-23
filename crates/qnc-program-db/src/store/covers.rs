use super::*;

impl Store {
    pub(crate) fn delete_cover(&mut self, cover_id: &str) -> Result<Data> {
        let cover_id = cover_id.trim();
        let now = story_now();
        let tx = self.conn.savepoint().map_err(err)?;
        let deleted = tx
            .execute("DELETE FROM story_covers WHERE cover_id = ?1", [cover_id])
            .map_err(err)?;
        if deleted == 0 {
            return Err(format!("cover not found: {cover_id}"));
        }
        tx.execute(
            "UPDATE story_state SET selected_cover_id = CASE
                WHEN selected_cover_id = ?1 THEN '' ELSE selected_cover_id END,
                draft_updated_at = ?2, updated_at = ?2 WHERE id = 1",
            params![cover_id, now],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// v5 `select_cover`: an existing cover, or an empty id to clear (a segment
    /// chosen on the program clears it, v5 `selected_cover_id.clear()`).
    pub(crate) fn select_cover(&mut self, cover_id: &str) -> Result<Data> {
        let cover_id = cover_id.trim();
        let exists: bool = cover_id.is_empty()
            || self
                .conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM story_covers WHERE cover_id = ?1)",
                    [cover_id],
                    |row| row.get(0),
                )
                .map_err(err)?;
        if !exists {
            return Err(format!("cover not found: {cover_id}"));
        }
        self.conn
            .execute(
                "UPDATE story_state SET selected_cover_id = ?1, draft_updated_at = ?2,
                    updated_at = ?2 WHERE id = 1",
                params![cover_id, story_now()],
            )
            .map_err(err)?;
        Ok(Data::Changed)
    }


    /// v5 `create_cover` from source frames (`/api/story/cover/create` with
    /// `in_frame`/`out_frame`): the B-roll virtual shot of the source IN/OUT (made
    /// first, v5 `add_virtual_shot_from_frames`) becomes the cover of the slot; a
    /// cover already in the slot is replaced. The source
    /// is not cut to the slot (the playlist plays min(slot, source)).
    pub(crate) fn create_cover(
        &mut self,
        project_id: &str,
        slot_id: &str,
        (clip_id, shot_id): (&str, &str),
        (in_frame, out_frame): (u64, u64),
        (fps_num, fps_den): (u32, u32),
        (a2_source_channel, a2_level_db, a2_off): (u16, i8, bool),
    ) -> Result<Data> {
        qnc_media_records::valid_id(clip_id).map_err(err)?;
        qnc_media_records::valid_id(shot_id).map_err(err)?;
        if out_frame <= in_frame {
            return Err(qnc_ui_text::text("story.out_after_in").into());
        }
        if fps_num == 0 || fps_den == 0 {
            return Err(qnc_ui_text::text("story.b_roll_valid_source_frame_rate").into());
        }
        let project_matches: bool = self
            .conn
            .query_row(
                "SELECT count(*)=1 AND min(project_id)=?1 FROM public_project_settings",
                [project_id],
                |row| row.get(0),
            )
            .map_err(err)?;
        if !project_matches {
            return Err(qnc_ui_text::text("db.project_database_belongs_another_project").into());
        }
        require_imported_clip(&self.conn, clip_id)?;
        require_source_range(&self.conn, clip_id, (in_frame, out_frame), (fps_num, fps_den))?;
        let tx = self.conn.savepoint().map_err(err)?;
        let rate = story_rate(&tx)?.ok_or(qnc_ui_text::text("story.story_segments"))?;
        let per_source = story_per_source(rate, (fps_num, fps_den))?;
        // A B-roll shorter than its slot (user 2026-10-18: a short B-roll in the one slot of
        // a new story left a hole and was refused): an M marker at its end splits the slot,
        // as Sync does; the cover takes the first part, the rest is an empty slot.
        let (_, cover_frames) = story_length(in_frame as i64, out_frame as i64, per_source)?;
        let slot_id = &split_slot_for(&tx, slot_id, cover_frames)?;
        type SlotRow = (i64, i64, f64, f64, String, i64);
        let slot: Option<SlotRow> = tx
            .query_row(
                "SELECT start_frame, end_frame, start_sec, end_sec, slot_signature, slot_index
                 FROM story_marker_slots WHERE slot_id = ?1",
                [slot_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .optional()
            .map_err(err)?;
        let Some((start, end, start_sec, end_sec, signature, slot_index)) = slot else {
            return Err(format!("slot not found: {slot_id}"));
        };
        let created = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        tx.execute("DELETE FROM story_covers WHERE slot_id = ?1", [slot_id])
            .map_err(err)?;
        let cover_id = format!("cover_{created:x}");
        let now = story_now();
        let (in_frame, out_frame) = (in_frame as i64, out_frame as i64);
        let fps = f64::from(fps_num) / f64::from(fps_den);
        tx.execute(
            "INSERT INTO story_covers (
                cover_id, slot_id, timeline_start_frame, timeline_end_frame,
                timeline_start_sec, timeline_end_sec, slot_signature, slot_index,
                clip_id, virtual_shot_id, title, note, in_tc, out_tc, in_seconds, out_seconds,
                source_in_frame, source_out_frame, source_fps, source_fps_num, source_fps_den,
                sort_index, a2_source_channel, created_at, updated_at, a2_level_db, a2_off
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, '', '', ?11, ?12, ?13, ?14,
                       ?15, ?16, ?17, ?18, ?19, 0, ?21, ?20, ?20, ?22, ?23)",
            params![
                cover_id,
                slot_id,
                start,
                end,
                start_sec,
                end_sec,
                signature,
                slot_index,
                clip_id,
                shot_id,
                frame_timecode(in_frame, fps),
                frame_timecode(out_frame, fps),
                timeline_sec(in_frame, fps),
                timeline_sec(out_frame, fps),
                in_frame,
                out_frame,
                fps,
                fps_num,
                fps_den,
                now,
                a2_source_channel,
                a2_level_db,
                a2_off
            ],
        )
        .map_err(err)?;
        tx.execute(
            "UPDATE story_state SET selected_cover_id = ?1, selected_shot_id = ?2,
                draft_updated_at = ?3, updated_at = ?3 WHERE id = 1",
            params![cover_id, shot_id, now],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Created(cover_id))
    }
}
