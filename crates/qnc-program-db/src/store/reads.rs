use super::*;

impl Store {
    /// v5 `list_markers`: every marker by program frame, the locked start and end
    /// included (`system_role`).
    pub(crate) fn list_markers(&self) -> Result<Data> {
        if !object_exists(&self.conn, "view", "public_story_markers")? {
            return Ok(Data::Markers(Vec::new()));
        }
        let mut statement = self
            .conn
            .prepare(
                "SELECT marker_id, program_frame, system_role
                 FROM public_story_markers
                 ORDER BY program_frame, marker_id",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                Ok(ProgramMarker {
                    marker_id: row.get(0)?,
                    program_frame: row.get::<_, i64>(1)?.max(0) as u64,
                    system_role: row.get(2)?,
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok(Data::Markers(rows))
    }

    /// v5 `list_parts`: every segment, deleted ones included (`active = false`),
    /// by `sort_index`; the program is the active ones.
    pub(crate) fn list_segments(&self) -> Result<Data> {
        if !object_exists(&self.conn, "view", "public_story_parts")? {
            return Ok(Data::Segments(Vec::new()));
        }
        let mut statement = self
            .conn
            .prepare(
                "SELECT segment_id, kind, sort_index, clip_id, in_frame, out_frame, fps_num,
                        fps_den, active, a1_source_channel, duration_frames, a1_level_db, a1_off,
                        v_lead_frames
                 FROM public_story_parts ORDER BY sort_index, segment_id",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                let (in_frame, out_frame) = (row.get::<_, i64>(4)?.max(0) as u64, row.get::<_, i64>(5)?.max(0) as u64);
                let duration = row.get::<_, i64>(10)?.max(0) as u64;
                Ok(ProgramSegment {
                    segment_id: row.get(0)?,
                    kind: row.get(1)?,
                    sort_index: row.get::<_, i64>(2)?.max(0) as u32,
                    clip_id: row.get(3)?,
                    in_frame,
                    out_frame,
                    fps_num: row.get(6)?,
                    fps_den: row.get(7)?,
                    active: row.get::<_, i64>(8)? != 0,
                    a1_source_channel: channel(row.get(9)?),
                    a1_level_db: level(row.get(11)?),
                    a1_off: row.get::<_, i64>(12)? != 0,
                    // A story written before the length was kept in story frames: its range.
                    duration_frames: if duration > 0 { duration } else { out_frame.saturating_sub(in_frame) },
                    v_lead_frames: row.get::<_, i64>(13)?.clamp(0, i64::from(MAX_OVERLAP_FRAMES)) as u32,
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok(Data::Segments(rows))
    }

    /// v5 `marker_slots_snapshot`: the stored slots in order, with `has_cover`.
    pub(crate) fn list_slots(&self) -> Result<Data> {
        if !object_exists(&self.conn, "view", "public_story_marker_slots")? {
            return Ok(Data::Slots(Vec::new()));
        }
        let mut statement = self
            .conn
            .prepare(
                "SELECT slot_id, start_frame, end_frame, start_marker_id, end_marker_id, has_cover
                 FROM public_story_marker_slots ORDER BY slot_index",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                Ok(ProgramSlot {
                    slot_id: row.get(0)?,
                    start_frame: row.get::<_, i64>(1)?.max(0) as u64,
                    end_frame: row.get::<_, i64>(2)?.max(0) as u64,
                    start_marker_id: row.get(3)?,
                    end_marker_id: row.get(4)?,
                    has_cover: row.get::<_, i64>(5)? != 0,
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok(Data::Slots(rows))
    }

    /// v5 `list_covers`: every cover with its slot, frames and A2 source channel.
    pub(crate) fn list_covers(&self) -> Result<Data> {
        if !object_exists(&self.conn, "view", "public_story_covers")? {
            return Ok(Data::Covers(Vec::new()));
        }
        let mut statement = self
            .conn
            .prepare(
                "SELECT cover_id, slot_id, clip_id, virtual_shot_id, timeline_start_frame,
                        timeline_end_frame, source_in_frame, source_out_frame, fps_num, fps_den,
                        a2_source_channel, a2_level_db, a2_off, a2_lead_frames, a2_tail_frames
                 FROM public_story_covers ORDER BY timeline_start_frame, cover_id",
            )
            .map_err(err)?;
        let frame = |value: i64| value.max(0) as u64;
        let rows = statement
            .query_map([], |row| {
                Ok(ProgramCover {
                    cover_id: row.get(0)?,
                    slot_id: row.get(1)?,
                    clip_id: row.get(2)?,
                    virtual_shot_id: row.get(3)?,
                    program_start_frame: frame(row.get(4)?),
                    program_end_frame: frame(row.get(5)?),
                    source_in_frame: frame(row.get(6)?),
                    source_out_frame: frame(row.get(7)?),
                    fps_num: row.get(8)?,
                    fps_den: row.get(9)?,
                    a2_source_channel: channel(row.get(10)?),
                    a2_level_db: level(row.get(11)?),
                    a2_off: row.get::<_, i64>(12)? != 0,
                    a2_lead_frames: row.get::<_, i64>(13)?.clamp(0, i64::from(MAX_OVERLAP_FRAMES)) as u32,
                    a2_tail_frames: row.get::<_, i64>(14)?.clamp(0, i64::from(MAX_OVERLAP_FRAMES)) as u32,
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok(Data::Covers(rows))
    }

    pub(crate) fn read_story_selection(&self) -> Result<Data> {
        if !object_exists(&self.conn, "view", "public_story_state")? {
            return Ok(Data::StorySelection(StorySelection::default()));
        }
        let selection = self
            .conn
            .query_row(
                "SELECT selected_part_id, selected_slot_id, selected_cover_id
                 FROM public_story_state LIMIT 1",
                [],
                |row| {
                    Ok(StorySelection {
                        selected_part_id: row.get(0)?,
                        selected_slot_id: row.get(1)?,
                        selected_cover_id: row.get(2)?,
                        ..StorySelection::default()
                    })
                },
            )
            .optional()
            .map_err(err)?
            .unwrap_or_default();
        let (undo_depth, redo_depth) = if object_exists(&self.conn, "table", "story_undo")? {
            crate::undo::depth(&self.conn)?
        } else {
            (0, 0)
        };
        Ok(Data::StorySelection(StorySelection {
            undo_depth,
            redo_depth,
            story_fps: story_rate(&self.conn)?,
            ..selection
        }))
    }

    /// v5 `select_part`: an existing segment, or an empty id to clear.
    pub(crate) fn select_part(&mut self, part_id: &str) -> Result<Data> {
        let part_id = part_id.trim();
        if !part_id.is_empty() {
            let exists: bool = self
                .conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM story_parts WHERE part_id = ?1)",
                    [part_id],
                    |row| row.get(0),
                )
                .map_err(err)?;
            if !exists {
                return Err(format!("part not found: {part_id}"));
            }
        }
        self.conn
            .execute(
                "UPDATE story_state SET selected_part_id = ?1, draft_updated_at = ?2,
                    updated_at = ?2 WHERE id = 1",
                params![part_id, story_now()],
            )
            .map_err(err)?;
        Ok(Data::Changed)
    }

    /// v5 `select_marker_slot`: an existing slot, or an empty id to clear.
    pub(crate) fn select_slot(&mut self, slot_id: &str) -> Result<Data> {
        let slot_id = slot_id.trim();
        if !slot_id.is_empty() {
            let exists: bool = self
                .conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM story_marker_slots WHERE slot_id = ?1)",
                    [slot_id],
                    |row| row.get(0),
                )
                .map_err(err)?;
            if !exists {
                return Err(format!("slot not found: {slot_id}"));
            }
        }
        self.conn
            .execute(
                "UPDATE story_state SET selected_slot_id = ?1, draft_updated_at = ?2,
                    updated_at = ?2 WHERE id = 1",
                params![slot_id, story_now()],
            )
            .map_err(err)?;
        Ok(Data::Changed)
    }
}
