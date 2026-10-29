use super::*;

impl Store {
    /// v5 `delete_cover`: the selection is cleared when it was this cover.
    /// The source channel, level and on/off of A1 of a segment or A2 of a cover.
    pub(crate) fn set_sound(&mut self, table: &str, key: &str, lane: &str, id: &str, (channel, level_db, off): (u16, i8, bool)) -> Result<Data> {
        let now = story_now();
        let changed = self
            .conn
            .execute(
                &format!(
                    "UPDATE {table} SET {lane}_source_channel = ?1, {lane}_level_db = ?2, {lane}_off = ?3,
                        updated_at = ?4 WHERE {key} = ?5"
                ),
                params![channel, level_db, off, now, id.trim()],
            )
            .map_err(err)?;
        if changed == 0 {
            return Err(format!("{key} not found: {}", id.trim()));
        }
        Ok(Data::Changed)
    }

    /// The A2 sound of the cut at an M marker: the cover before it (`P`, its slot ends at the
    /// marker) and the cover after it (`N`, its slot starts there). Their offset is the L of P
    /// minus the J of N, story frames after the picture cut.
    pub(crate) fn cut_covers(&self, marker_id: &str) -> Result<(Option<String>, Option<String>, i64)> {
        let marker_id = marker_id.trim();
        let side = |filter: &str, column: &str| -> Result<Option<(String, i64)>> {
            self.conn
                .query_row(
                    &format!("SELECT cover_id, {column} FROM story_covers WHERE {filter} LIMIT 1"),
                    [marker_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(err)
        };
        let before = side("substr(slot_id, -length(?1) - 1) = '|' || ?1", "a2_tail_frames")?;
        let after = side("substr(slot_id, 1, length(?1) + 1) = ?1 || '|'", "a2_lead_frames")?;
        let offset = before.as_ref().map_or(0, |(_, tail)| *tail) - after.as_ref().map_or(0, |(_, lead)| *lead);
        Ok((before.map(|(id, _)| id), after.map(|(id, _)| id), offset))
    }

    /// Puts the A2 sound of the cut at `offset`: an L of the cover before (offset > 0) or a J
    /// of the cover after (offset < 0); a side with no cover cannot take it.
    pub(crate) fn set_cut_sound(&mut self, marker_id: &str, offset: i64) -> Result<Data> {
        let (before, after, _) = self.cut_covers(marker_id)?;
        let bound = i64::from(MAX_OVERLAP_FRAMES);
        if offset > 0 && before.is_none() || offset < 0 && after.is_none() {
            return Err(qnc_ui_text::text("story.cut_sound_no_cover").into());
        }
        let now = story_now();
        if let Some(cover) = before {
            self.conn
                .execute(
                    "UPDATE story_covers SET a2_tail_frames = ?1, updated_at = ?2 WHERE cover_id = ?3",
                    params![offset.clamp(0, bound), now, cover],
                )
                .map_err(err)?;
        }
        if let Some(cover) = after {
            self.conn
                .execute(
                    "UPDATE story_covers SET a2_lead_frames = ?1, updated_at = ?2 WHERE cover_id = ?3",
                    params![(-offset).clamp(0, bound), now, cover],
                )
                .map_err(err)?;
        }
        Ok(Data::Changed)
    }

    /// The L of the cover before the cut and the J of the cover after it, story frames; `None`
    /// for a side with no cover.
    pub(crate) fn cut_edges(&self, marker_id: &str) -> Result<(Option<u32>, Option<u32>)> {
        let marker_id = marker_id.trim();
        let side = |filter: &str, column: &str| -> Result<Option<i64>> {
            self.conn
                .query_row(&format!("SELECT {column} FROM story_covers WHERE {filter} LIMIT 1"), [marker_id], |row| row.get(0))
                .optional()
                .map_err(err)
        };
        let bound = |frames: i64| frames.clamp(0, i64::from(MAX_OVERLAP_FRAMES)) as u32;
        let tail = side("substr(slot_id, -length(?1) - 1) = '|' || ?1", "a2_tail_frames")?.map(bound);
        let lead = side("substr(slot_id, 1, length(?1) + 1) = ?1 || '|'", "a2_lead_frames")?.map(bound);
        Ok((tail, lead))
    }

    /// Both sides of the cut's A2 sound at once (user 2026-10-29): the L of the cover before
    /// and the J of the cover after; a side with no cover cannot take any.
    pub(crate) fn set_cut_sound_edges(&mut self, marker_id: &str, tail: u32, lead: u32) -> Result<Data> {
        if tail > MAX_OVERLAP_FRAMES || lead > MAX_OVERLAP_FRAMES {
            return Err(qnc_ui_text::fill("story.overlap_too_long", &[("frames", &MAX_OVERLAP_FRAMES.to_string())]));
        }
        let (before, after, _) = self.cut_covers(marker_id)?;
        if tail > 0 && before.is_none() || lead > 0 && after.is_none() {
            return Err(qnc_ui_text::text("story.cut_sound_no_cover").into());
        }
        let now = story_now();
        if let Some(cover) = before {
            self.conn
                .execute("UPDATE story_covers SET a2_tail_frames = ?1, updated_at = ?2 WHERE cover_id = ?3", params![tail, now, cover])
                .map_err(err)?;
        }
        if let Some(cover) = after {
            self.conn
                .execute("UPDATE story_covers SET a2_lead_frames = ?1, updated_at = ?2 WHERE cover_id = ?3", params![lead, now, cover])
                .map_err(err)?;
        }
        Ok(Data::Changed)
    }

    /// The picture cut moves, its A2 sound stays: where the sound of the cover before ends and
    /// where the sound of the cover after starts stay in the story (user 2026-10-29: J and L
    /// together); neither can be on the wrong side of the new picture cut, a side with no cover
    /// takes none.
    pub(crate) fn move_marker_picture(&mut self, marker_id: &str, program_frame: u64) -> Result<Data> {
        let from: i64 = self
            .conn
            .query_row("SELECT timeline_frame FROM story_markers WHERE marker_id = ?1", [marker_id.trim()], |row| row.get(0))
            .map_err(err)?;
        let (tail, lead) = self.cut_edges(marker_id)?;
        let (sound_end, sound_start) = (from + i64::from(tail.unwrap_or(0)), from - i64::from(lead.unwrap_or(0)));
        self.move_marker(marker_id, program_frame)?;
        let (before, after) = self.cut_edges(marker_id)?;
        let to = program_frame as i64;
        let tail = before.map_or(0, |_| (sound_end - to).max(0));
        let lead = after.map_or(0, |_| (to - sound_start).max(0));
        let bound = |frames: i64| frames.min(i64::from(MAX_OVERLAP_FRAMES)) as u32;
        self.set_cut_sound_edges(marker_id, bound(tail), bound(lead))
    }

    /// The picture J of a Ton segment, at most `MAX_OVERLAP_FRAMES`; an Off segment has no picture.
    pub(crate) fn set_picture_lead(&mut self, segment_id: &str, frames: u32) -> Result<Data> {
        if frames > MAX_OVERLAP_FRAMES {
            return Err(qnc_ui_text::fill("story.overlap_too_long", &[("frames", &MAX_OVERLAP_FRAMES.to_string())]));
        }
        let changed = self
            .conn
            .execute(
                "UPDATE story_parts SET v_lead_frames = ?1, updated_at = ?2 WHERE part_id = ?3 AND kind = 'tonovi'",
                params![frames, story_now(), segment_id.trim()],
            )
            .map_err(err)?;
        if changed == 0 {
            return Err(qnc_ui_text::text("story.picture_lead_ton_only").into());
        }
        Ok(Data::Changed)
    }

    /// The J and L of a cover's A2, at most `MAX_OVERLAP_FRAMES` each.
    pub(crate) fn set_cover_overlap(&mut self, cover_id: &str, lead: u32, tail: u32) -> Result<Data> {
        if lead > MAX_OVERLAP_FRAMES || tail > MAX_OVERLAP_FRAMES {
            return Err(qnc_ui_text::fill("story.overlap_too_long", &[("frames", &MAX_OVERLAP_FRAMES.to_string())]));
        }
        let changed = self
            .conn
            .execute(
                "UPDATE story_covers SET a2_lead_frames = ?1, a2_tail_frames = ?2, updated_at = ?3
                 WHERE cover_id = ?4",
                params![lead, tail, story_now(), cover_id.trim()],
            )
            .map_err(err)?;
        if changed == 0 {
            return Err(format!("cover_id not found: {}", cover_id.trim()));
        }
        Ok(Data::Changed)
    }
}
