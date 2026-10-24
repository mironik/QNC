use super::*;

impl Store {
    /// Appends a Ton or Off segment (docs/93 R8-R10). The range is copied from the
    /// source IN/OUT; a story has one timebase, a different one is refused.
    pub(crate) fn create_segment(
        &mut self,
        project_id: &str,
        kind: &str,
        clip_id: &str,
        (in_frame, out_frame): (u64, u64),
        (fps_num, fps_den): (u32, u32),
        (a1_source_channel, a1_level_db, a1_off): (u16, i8, bool),
    ) -> Result<Data> {
        let kind = story_kind(kind)?;
        qnc_media_records::valid_id(clip_id).map_err(err)?;
        if out_frame <= in_frame {
            return Err(qnc_ui_text::text("story.out_after_in").into());
        }
        if fps_num == 0 || fps_den == 0 {
            return Err(qnc_ui_text::text("story.segment_valid_source_frame_rate").into());
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
        // The first segment sets the rate of the story; later ones keep it.
        let rate = match story_rate(&tx)? {
            Some(rate) => rate,
            None => {
                keep_story_rate(&tx, (fps_num, fps_den))?;
                (fps_num, fps_den)
            }
        };
        let per_source = story_per_source(rate, (fps_num, fps_den))?;
        let story_fps = f64::from(rate.0) / f64::from(rate.1);
        let next: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(sort_index) + 1, 0) FROM story_parts WHERE lead = ''",
                [],
                |row| row.get(0),
            )
            .map_err(err)?;
        let created = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let segment_id = format!("part_{created:x}");
        let now = story_now();
        // A last slot with a cover keeps its length (user 2026-10-18: a new segment was
        // refused because it made the last cover's slot longer than its clip): its end
        // marker stays where it is as a plain M marker, the new end comes after the new
        // segment, which gets an empty slot. Without a cover the end moves, as in v5.
        tx.execute(
            "UPDATE story_markers SET system_role = ''
             WHERE system_role = 'program_end'
               AND EXISTS (SELECT 1 FROM story_covers
                           WHERE slot_id LIKE '%|' || story_markers.marker_id)",
            [],
        )
        .map_err(err)?;
        // v5 `segment_source_from_clip_frames`; the length is in story frames.
        let in_frame = in_frame as i64;
        let (out_frame, duration) = story_length(in_frame, out_frame as i64, per_source)?;
        let fps = f64::from(fps_num) / f64::from(fps_den);
        tx.execute(
            "INSERT INTO story_parts (
                part_id, kind, sort_index, title, text, clip_id, virtual_shot_id,
                in_tc, out_tc, in_seconds, out_seconds, fps, source_fps_num, source_fps_den,
                in_frame, out_frame, duration_frames, duration_label, duration_color_key,
                active, created_at, updated_at, a1_source_channel, a1_level_db, a1_off
             ) VALUES (?1, ?2, ?3, '', '', ?4, '', ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                       ?12, ?13, ?14, ?15, ?16, 1, ?17, ?17, ?18, ?19, ?20)",
            params![
                segment_id,
                kind,
                next,
                clip_id,
                frame_timecode(in_frame, fps),
                frame_timecode(out_frame, fps),
                in_frame as f64 / fps,
                out_frame as f64 / fps,
                fps,
                fps_num,
                fps_den,
                in_frame,
                out_frame,
                duration,
                frames_label(duration, story_fps),
                duration_color_key(duration, story_fps),
                now,
                a1_source_channel,
                a1_level_db,
                a1_off
            ],
        )
        .map_err(err)?;
        finalize_story(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Created(segment_id))
    }

    /// v5 `delete_part`: the segment becomes inactive (`active = 0`); markers inside
    /// its program window go and later ones move left by its length
    /// (`shift_markers_after_part_removal_frames`); the selection moves to the
    /// nearest segment; the others close the gap.
    pub(crate) fn delete_segment(&mut self, segment_id: &str) -> Result<Data> {
        let segment_id = segment_id.trim();
        if segment_id.is_empty() {
            return Err("part_id required".into());
        }
        let tx = self.conn.savepoint().map_err(err)?;
        let deleted_sort: i64 = tx
            .query_row(
                "SELECT sort_index FROM story_parts WHERE part_id = ?1 AND active = 1",
                [segment_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(err)?
            .ok_or_else(|| format!("part not found: {segment_id}"))?;
        let fps = require_story_fps(&tx).ok();
        let window = segment_window(&tx, segment_id)?;
        tx.execute(
            "UPDATE story_parts SET active = 0, updated_at = ?2 WHERE part_id = ?1",
            params![segment_id, story_now()],
        )
        .map_err(err)?;
        if let (Some(fps), Some((start, end))) = (fps, window) {
            let (start, end) = (start as i64, end as i64);
            let inside = {
                let mut statement = tx
                    .prepare(
                        "SELECT marker_id FROM story_markers
                         WHERE timeline_frame > ?1 AND timeline_frame < ?2",
                    )
                    .map_err(err)?;
                let ids = statement
                    .query_map(params![start, end], |row| row.get::<_, String>(0))
                    .map_err(err)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(err)?;
                ids
            };
            delete_markers_with_slots(&tx, &inside)?;
            let shifted = {
                let mut statement = tx
                    .prepare("SELECT marker_id, timeline_frame FROM story_markers WHERE timeline_frame >= ?1")
                    .map_err(err)?;
                let rows = statement
                    .query_map([end], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                    })
                    .map_err(err)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(err)?;
                rows
            };
            let now = story_now();
            for (marker_id, frame) in shifted {
                let frame = (frame - (end - start)).max(0);
                tx.execute(
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
        }
        let selected: String = tx
            .query_row(
                "SELECT selected_part_id FROM story_state WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .map_err(err)?;
        if selected == segment_id {
            let neighbour: String = tx
                .query_row(
                    "SELECT part_id FROM story_parts WHERE part_id != ?1 AND active != 0
                     ORDER BY ABS(sort_index - ?2) ASC, sort_index ASC LIMIT 1",
                    params![segment_id, deleted_sort],
                    |row| row.get(0),
                )
                .optional()
                .map_err(err)?
                .unwrap_or_default();
            tx.execute(
                "UPDATE story_state SET selected_part_id = ?1 WHERE id = 1",
                [neighbour],
            )
            .map_err(err)?;
        }
        renumber_segments(&tx)?;
        merge_coincident_markers(&tx)?;
        finalize_story(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// Puts an excluded segment back at its place in the order: the program grows by
    /// its length there and the markers after it move right by as much (the inverse
    /// of `delete_part`; markers that were inside it are gone). It becomes selected.
    pub(crate) fn include_segment(&mut self, segment_id: &str) -> Result<Data> {
        let tx = self.conn.savepoint().map_err(err)?;
        let changed = tx
            .execute(
                "UPDATE story_parts SET active = 1, updated_at = ?2 WHERE part_id = ?1 AND active = 0",
                params![segment_id, story_now()],
            )
            .map_err(err)?;
        if changed == 0 {
            return Err(qnc_ui_text::fill("story.segment_not_excluded", &[("segment", &segment_id.to_string())]));
        }
        let fps = require_story_fps(&tx)?;
        let (start, end) = segment_window(&tx, segment_id)?
            .ok_or_else(|| format!("part not found: {segment_id}"))?;
        let (start, length) = (start as i64, (end - start) as i64);
        let shifted = {
            let mut statement = tx
                .prepare(
                    "SELECT marker_id, timeline_frame FROM story_markers
                     WHERE timeline_frame >= ?1 AND system_role != 'program_start'",
                )
                .map_err(err)?;
            let rows = statement
                .query_map([start], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })
                .map_err(err)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(err)?;
            rows
        };
        let now = story_now();
        for (marker_id, frame) in shifted {
            let frame = frame + length;
            tx.execute(
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
        tx.execute(
            "UPDATE story_state SET selected_part_id = ?1, updated_at = ?2 WHERE id = 1",
            params![segment_id, now],
        )
        .map_err(err)?;
        finalize_story(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// Replace: the segment takes the marked source as its base layer and its
    /// length (user rule 2026-09-25); the program around it follows the change.
    pub(crate) fn replace_segment(
        &mut self,
        segment_id: &str,
        clip_id: &str,
        (in_frame, out_frame): (u64, u64),
        (fps_num, fps_den): (u32, u32),
    ) -> Result<Data> {
        qnc_media_records::valid_id(clip_id).map_err(err)?;
        if out_frame <= in_frame {
            return Err(qnc_ui_text::text("story.out_after_in").into());
        }
        if fps_num == 0 || fps_den == 0 {
            return Err(qnc_ui_text::text("story.source_valid_frame_rate").into());
        }
        require_imported_clip(&self.conn, clip_id)?;
        require_source_range(&self.conn, clip_id, (in_frame, out_frame), (fps_num, fps_den))?;
        let tx = self.conn.savepoint().map_err(err)?;
        // The story keeps its rate while other segments have it; the only segment of a
        // story gives it the rate of its new source.
        let others: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM story_parts WHERE active = 1 AND part_id != ?1)",
                [segment_id],
                |row| row.get(0),
            )
            .map_err(err)?;
        let rate = match story_rate(&tx)? {
            Some(rate) if others => rate,
            _ => {
                keep_story_rate(&tx, (fps_num, fps_den))?;
                (fps_num, fps_den)
            }
        };
        let per_source = story_per_source(rate, (fps_num, fps_den))?;
        let story_fps = f64::from(rate.0) / f64::from(rate.1);
        let (start, end) = segment_window(&tx, segment_id)?
            .ok_or_else(|| format!("part not found: {segment_id}"))?;
        let (start, end) = (start as i64, end as i64);
        let in_frame = in_frame as i64;
        let (out_frame, length) = story_length(in_frame, out_frame as i64, per_source)?;
        let new_end = start + length;
        let fps = f64::from(fps_num) / f64::from(fps_den);
        if new_end < end {
            // Shorter: the markers of the cut part go, with their slots and covers.
            let cut = {
                let mut statement = tx
                    .prepare(
                        "SELECT marker_id FROM story_markers
                         WHERE timeline_frame > ?1 AND timeline_frame < ?2 AND system_role = ''",
                    )
                    .map_err(err)?;
                let ids = statement
                    .query_map(params![new_end, end], |row| row.get::<_, String>(0))
                    .map_err(err)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(err)?;
                ids
            };
            delete_markers_with_slots(&tx, &cut)?;
        }
        if new_end != end {
            shift_markers_from(&tx, end, new_end - end, story_fps)?;
        }
        let now = story_now();
        tx.execute(
            "UPDATE story_parts SET clip_id = ?2, virtual_shot_id = '', in_tc = ?3, out_tc = ?4,
                in_seconds = ?5, out_seconds = ?6, fps = ?7, source_fps_num = ?8,
                source_fps_den = ?9, in_frame = ?10, out_frame = ?11, duration_frames = ?12,
                duration_label = ?13, duration_color_key = ?14, a1_source_channel = 0, a1_level_db = 0, a1_off = 0,
                updated_at = ?15
             WHERE part_id = ?1 AND active = 1",
            params![
                segment_id,
                clip_id,
                frame_timecode(in_frame, fps),
                frame_timecode(out_frame, fps),
                in_frame as f64 / fps,
                out_frame as f64 / fps,
                fps,
                fps_num,
                fps_den,
                in_frame,
                out_frame,
                length,
                frames_label(length, story_fps),
                duration_color_key(length, story_fps),
                now
            ],
        )
        .map_err(err)?;
        finalize_story(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// Edit (user rule 2026-10-01): a new source IN/OUT of the same clip. Markers
    /// inside the segment keep their place in the picture (program frame = start +
    /// source frame - new IN); those outside the new range go with their slots and
    /// covers; the markers after the segment move by the change of length.
    pub(crate) fn trim_segment(&mut self, segment_id: &str, (in_frame, out_frame): (u64, u64)) -> Result<Data> {
        if out_frame <= in_frame {
            return Err(qnc_ui_text::text("story.out_after_in").into());
        }
        let tx = self.conn.savepoint().map_err(err)?;
        let (old_in, fps_num, fps_den, clip_id): (i64, u32, u32, String) = tx
            .query_row(
                "SELECT in_frame, source_fps_num, source_fps_den, clip_id FROM story_parts
                 WHERE part_id = ?1 AND active = 1",
                [segment_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(err)?
            .ok_or_else(|| format!("part not found: {segment_id}"))?;
        require_source_range(&tx, &clip_id, (in_frame, out_frame), (fps_num, fps_den))?;
        let (start, end) = segment_window(&tx, segment_id)?
            .ok_or_else(|| format!("part not found: {segment_id}"))?;
        let (start, end) = (start as i64, end as i64);
        let rate = story_rate(&tx)?.ok_or(qnc_ui_text::text("story.story_segments"))?;
        let (num, den) = story_per_source(rate, (fps_num, fps_den))?;
        let story_fps = f64::from(rate.0) / f64::from(rate.1);
        let in_frame = in_frame as i64;
        let (out_frame, length) = story_length(in_frame, out_frame as i64, (num, den))?;
        let fps = f64::from(fps_num) / f64::from(fps_den);
        let markers = user_markers_from(&tx, start + 1)?;
        let mut cut = Vec::new();
        let mut moved = Vec::new();
        for (marker_id, frame) in markers {
            if frame >= end {
                moved.push((marker_id, frame + start + length - end));
                continue;
            }
            // The picture under the marker, in source frames, stays under it.
            let source = old_in + (frame - start) * den / num;
            if source <= in_frame || source >= out_frame {
                cut.push(marker_id);
            } else {
                moved.push((marker_id, start + (source - in_frame) * num / den));
            }
        }
        delete_markers_with_slots(&tx, &cut)?;
        for (marker_id, frame) in moved {
            set_marker_frame(&tx, &marker_id, frame, story_fps)?;
        }
        tx.execute(
            "UPDATE story_parts SET in_tc = ?2, out_tc = ?3, in_seconds = ?4, out_seconds = ?5,
                in_frame = ?6, out_frame = ?7, duration_frames = ?8, duration_label = ?9,
                duration_color_key = ?10, updated_at = ?11
             WHERE part_id = ?1 AND active = 1",
            params![
                segment_id,
                frame_timecode(in_frame, fps),
                frame_timecode(out_frame, fps),
                in_frame as f64 / fps,
                out_frame as f64 / fps,
                in_frame,
                out_frame,
                length,
                frames_label(length, story_fps),
                duration_color_key(length, story_fps),
                story_now()
            ],
        )
        .map_err(err)?;
        finalize_story(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// Removes an excluded segment for good (it is no longer in the program).
    pub(crate) fn purge_segment(&mut self, segment_id: &str) -> Result<Data> {
        let tx = self.conn.savepoint().map_err(err)?;
        let removed = tx
            .execute(
                "DELETE FROM story_parts WHERE part_id = ?1 AND active = 0",
                [segment_id],
            )
            .map_err(err)?;
        if removed == 0 {
            return Err("Brise se samo iskljuceni segment.".into());
        }
        renumber_segments(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// Swaps with the neighbouring active segment; at the edge nothing changes
    /// (docs/93 R13). Excluded segments keep their place in the order.
    pub(crate) fn move_segment(&mut self, segment_id: &str, up: bool) -> Result<Data> {
        let tx = self.conn.savepoint().map_err(err)?;
        let mut order = all_segment_order(&tx)?;
        let index = order
            .iter()
            .position(|(id, active)| id == segment_id && *active)
            .ok_or(qnc_ui_text::text("story.segment_found"))?;
        let other = if up {
            order[..index].iter().rposition(|(_, active)| *active)
        } else {
            order[index + 1..]
                .iter()
                .position(|(_, active)| *active)
                .map(|offset| index + 1 + offset)
        };
        if let Some(other) = other {
            order.swap(index, other);
            let ids: Vec<String> = order.into_iter().map(|(id, _)| id).collect();
            write_segment_order(&tx, &ids)?;
            finalize_story(&tx)?;
        }
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }
}
