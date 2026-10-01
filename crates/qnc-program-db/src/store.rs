//! The Story tables of a project database and their requests (moved unchanged from
//! the former project content store; v5 `story/*.rs`).

use crate::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::time::{SystemTime, UNIX_EPOCH};

/// The tables this module owns; it writes no other.
const OWNED: [&str; 7] = [
    "story_parts",
    "story_markers",
    "story_marker_slots",
    "story_covers",
    "story_state",
    "story_object_history",
    "story_undo",
];

pub(crate) struct Store {
    conn: Connection,
    schema_ready: bool,
}

impl Store {
    /// Creates the Story tables if missing (read-write) and serves them; a read-only
    /// connection of a project without a story reads an empty one.
    pub(crate) fn attach(mut conn: Connection, access: Access) -> Result<Self> {
        if access == Access::ReadWrite {
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(err)?;
            ensure_story_schema(&tx)?;
            crate::undo::ensure_schema(&tx)?;
            tx.commit().map_err(err)?;
        }
        let schema_ready: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='story_state')
                    AND EXISTS(SELECT 1 FROM sqlite_master WHERE name='story_undo')",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        conn.authorizer(Some(move |ctx: rusqlite::hooks::AuthContext<'_>| {
            use rusqlite::hooks::{AuthAction as A, Authorization as R};
            match ctx.action {
                A::Insert { table_name }
                | A::Delete { table_name }
                | A::Update { table_name, .. }
                    if access == Access::ReadWrite && OWNED.contains(&table_name) =>
                {
                    R::Allow
                }
                A::Read { .. }
                | A::Select
                | A::Transaction { .. }
                | A::Savepoint { .. }
                | A::Function { .. }
                | A::Recursive => R::Allow,
                _ => R::Deny,
            }
        }));
        Ok(Self { conn, schema_ready })
    }

    pub(crate) fn execute(&mut self, operation: &Operation) -> Result<Data> {
        if !self.schema_ready {
            return match operation {
                Operation::ListSegments => Ok(Data::Segments(Vec::new())),
                Operation::ListMarkers => Ok(Data::Markers(Vec::new())),
                Operation::ListSlots => Ok(Data::Slots(Vec::new())),
                Operation::ListCovers => Ok(Data::Covers(Vec::new())),
                Operation::ReadStorySelection => {
                    Ok(Data::StorySelection(StorySelection::default()))
                }
                _ => Err("Prica projekta nije inicijalizirana.".into()),
            };
        }
        if operation.edits_story() {
            let before = crate::undo::capture(&self.conn)?;
            let data = self.dispatch(operation)?;
            crate::undo::record(&self.conn, &before)?;
            return Ok(data);
        }
        self.dispatch(operation)
    }

    fn dispatch(&mut self, operation: &Operation) -> Result<Data> {
        match operation {
            Operation::UndoStory => {
                crate::undo::step(&self.conn, true)?;
                Ok(Data::Changed)
            }
            Operation::RedoStory => {
                crate::undo::step(&self.conn, false)?;
                Ok(Data::Changed)
            }
            Operation::CreateSegment {
                project_id,
                kind,
                clip_id,
                in_frame,
                out_frame,
                fps_num,
                fps_den,
                a1_source_channel,
            } => self.create_segment(
                project_id,
                kind,
                clip_id,
                (*in_frame, *out_frame),
                (*fps_num, *fps_den),
                *a1_source_channel,
            ),
            Operation::DeleteSegment { segment_id } => self.delete_segment(segment_id),
            Operation::IncludeSegment { segment_id } => self.include_segment(segment_id),
            Operation::ReplaceSegment {
                segment_id,
                clip_id,
                in_frame,
                out_frame,
                fps_num,
                fps_den,
            } => self.replace_segment(
                segment_id,
                clip_id,
                (*in_frame, *out_frame),
                (*fps_num, *fps_den),
            ),
            Operation::TrimSegment {
                segment_id,
                in_frame,
                out_frame,
            } => self.trim_segment(segment_id, (*in_frame, *out_frame)),
            Operation::PurgeSegment { segment_id } => self.purge_segment(segment_id),
            Operation::MoveSegment { segment_id, up } => self.move_segment(segment_id, *up),
            Operation::ListSegments => self.list_segments(),
            Operation::CreateMarker {
                part_id,
                local_frame,
            } => self.create_marker(part_id, *local_frame),
            Operation::MoveMarker {
                marker_id,
                program_frame,
            } => self.move_marker(marker_id, *program_frame),
            Operation::DeleteMarker { marker_id } => self.delete_marker(marker_id),
            Operation::ListMarkers => self.list_markers(),
            Operation::ListSlots => self.list_slots(),
            Operation::ListCovers => self.list_covers(),
            Operation::ReadStorySelection => self.read_story_selection(),
            Operation::SelectPart { part_id } => self.select_part(part_id),
            Operation::SelectSlot { slot_id } => self.select_slot(slot_id),
            Operation::CreateCover {
                project_id,
                slot_id,
                clip_id,
                virtual_shot_id,
                in_frame,
                out_frame,
                fps_num,
                fps_den,
                a2_source_channel,
            } => self.create_cover(
                project_id,
                slot_id,
                (clip_id, virtual_shot_id),
                (*in_frame, *out_frame),
                (*fps_num, *fps_den),
                *a2_source_channel,
            ),
            Operation::DeleteCover { cover_id } => self.delete_cover(cover_id),
            Operation::SelectCover { cover_id } => self.select_cover(cover_id),
        }
    }

    /// Appends a Ton or Off segment (docs/93 R8-R10). The range is copied from the
    /// source IN/OUT; a story has one timebase, a different one is refused.
    fn create_segment(
        &mut self,
        project_id: &str,
        kind: &str,
        clip_id: &str,
        (in_frame, out_frame): (u64, u64),
        (fps_num, fps_den): (u32, u32),
        a1_source_channel: u16,
    ) -> Result<Data> {
        let kind = story_kind(kind)?;
        qnc_media_records::valid_id(clip_id).map_err(err)?;
        if out_frame <= in_frame {
            return Err("OUT mora biti najmanje jedan frame nakon IN.".into());
        }
        if fps_num == 0 || fps_den == 0 {
            return Err("Segment nema valjan source fps.".into());
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
            return Err("Projektna baza pripada drugom projektu.".into());
        }
        require_imported_clip(&self.conn, clip_id)?;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let story_timebase: Option<(u32, u32)> = tx
            .query_row(
                "SELECT source_fps_num, source_fps_den FROM story_parts
                 WHERE active = 1 ORDER BY sort_index LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(err)?;
        if let Some((num, den)) = story_timebase {
            // Same rate even when written differently (50/1 and 100/2).
            if u64::from(num) * u64::from(fps_den) != u64::from(fps_num) * u64::from(den) {
                return Err(format!(
                    "Klip ima {fps_num}/{fps_den} fps, a prica {num}/{den}; mijesani fps nije dopusten."
                ));
            }
        }
        let next: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(sort_index) + 1, 0) FROM story_parts",
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
        // v5 `segment_source_from_clip_frames`.
        let (in_frame, out_frame) = (in_frame as i64, out_frame as i64);
        let duration = out_frame - in_frame;
        let fps = f64::from(fps_num) / f64::from(fps_den);
        tx.execute(
            "INSERT INTO story_parts (
                part_id, kind, sort_index, title, text, clip_id, virtual_shot_id,
                in_tc, out_tc, in_seconds, out_seconds, fps, source_fps_num, source_fps_den,
                in_frame, out_frame, duration_frames, duration_label, duration_color_key,
                active, created_at, updated_at, a1_source_channel
             ) VALUES (?1, ?2, ?3, '', '', ?4, '', ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                       ?12, ?13, ?14, ?15, ?16, 1, ?17, ?17, ?18)",
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
                frames_label(duration, fps),
                duration_color_key(duration, fps),
                now,
                a1_source_channel
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
    fn delete_segment(&mut self, segment_id: &str) -> Result<Data> {
        let segment_id = segment_id.trim();
        if segment_id.is_empty() {
            return Err("part_id required".into());
        }
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
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
        finalize_story(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// Puts an excluded segment back at its place in the order: the program grows by
    /// its length there and the markers after it move right by as much (the inverse
    /// of `delete_part`; markers that were inside it are gone). It becomes selected.
    fn include_segment(&mut self, segment_id: &str) -> Result<Data> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let changed = tx
            .execute(
                "UPDATE story_parts SET active = 1, updated_at = ?2 WHERE part_id = ?1 AND active = 0",
                params![segment_id, story_now()],
            )
            .map_err(err)?;
        if changed == 0 {
            return Err(format!("Segment nije iskljucen: {segment_id}"));
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
    fn replace_segment(
        &mut self,
        segment_id: &str,
        clip_id: &str,
        (in_frame, out_frame): (u64, u64),
        (fps_num, fps_den): (u32, u32),
    ) -> Result<Data> {
        qnc_media_records::valid_id(clip_id).map_err(err)?;
        if out_frame <= in_frame {
            return Err("OUT mora biti najmanje jedan frame nakon IN.".into());
        }
        if fps_num == 0 || fps_den == 0 {
            return Err("Izvor nema valjan source fps.".into());
        }
        require_imported_clip(&self.conn, clip_id)?;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let other_rate: Option<(u32, u32)> = tx
            .query_row(
                "SELECT source_fps_num, source_fps_den FROM story_parts
                 WHERE active = 1 AND part_id != ?1 ORDER BY sort_index LIMIT 1",
                [segment_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(err)?;
        if let Some((num, den)) = other_rate {
            if u64::from(num) * u64::from(fps_den) != u64::from(fps_num) * u64::from(den) {
                return Err(format!(
                    "Klip ima {fps_num}/{fps_den} fps, a prica {num}/{den}; mijesani fps nije dopusten."
                ));
            }
        }
        let (start, end) = segment_window(&tx, segment_id)?
            .ok_or_else(|| format!("part not found: {segment_id}"))?;
        let (start, end) = (start as i64, end as i64);
        let (in_frame, out_frame) = (in_frame as i64, out_frame as i64);
        let length = out_frame - in_frame;
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
            shift_markers_from(&tx, end, new_end - end, fps)?;
        }
        let now = story_now();
        tx.execute(
            "UPDATE story_parts SET clip_id = ?2, virtual_shot_id = '', in_tc = ?3, out_tc = ?4,
                in_seconds = ?5, out_seconds = ?6, fps = ?7, source_fps_num = ?8,
                source_fps_den = ?9, in_frame = ?10, out_frame = ?11, duration_frames = ?12,
                duration_label = ?13, duration_color_key = ?14, a1_source_channel = 0,
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
                frames_label(length, fps),
                duration_color_key(length, fps),
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
    fn trim_segment(&mut self, segment_id: &str, (in_frame, out_frame): (u64, u64)) -> Result<Data> {
        if out_frame <= in_frame {
            return Err("OUT mora biti najmanje jedan frame nakon IN.".into());
        }
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let (old_in, fps_num, fps_den): (i64, u32, u32) = tx
            .query_row(
                "SELECT in_frame, source_fps_num, source_fps_den FROM story_parts
                 WHERE part_id = ?1 AND active = 1",
                [segment_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(err)?
            .ok_or_else(|| format!("part not found: {segment_id}"))?;
        let (start, end) = segment_window(&tx, segment_id)?
            .ok_or_else(|| format!("part not found: {segment_id}"))?;
        let (start, end) = (start as i64, end as i64);
        let (in_frame, out_frame) = (in_frame as i64, out_frame as i64);
        let length = out_frame - in_frame;
        let fps = f64::from(fps_num) / f64::from(fps_den);
        let markers = user_markers_from(&tx, start + 1)?;
        let mut cut = Vec::new();
        let mut moved = Vec::new();
        for (marker_id, frame) in markers {
            if frame >= end {
                moved.push((marker_id, frame + start + length - end));
                continue;
            }
            let source = old_in + frame - start;
            if source <= in_frame || source >= out_frame {
                cut.push(marker_id);
            } else {
                moved.push((marker_id, start + source - in_frame));
            }
        }
        delete_markers_with_slots(&tx, &cut)?;
        for (marker_id, frame) in moved {
            set_marker_frame(&tx, &marker_id, frame, fps)?;
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
                frames_label(length, fps),
                duration_color_key(length, fps),
                story_now()
            ],
        )
        .map_err(err)?;
        finalize_story(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// Removes an excluded segment for good (it is no longer in the program).
    fn purge_segment(&mut self, segment_id: &str) -> Result<Data> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
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
    fn move_segment(&mut self, segment_id: &str, up: bool) -> Result<Data> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let mut order = all_segment_order(&tx)?;
        let index = order
            .iter()
            .position(|(id, active)| id == segment_id && *active)
            .ok_or("Segment nije pronadjen.")?;
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

    /// v5 `create_marker_from_part_frame`: M is placed on a Wrap segment at a frame
    /// inside it (`local_to_timeline_frame`, clamped to the segment); the program
    /// frame follows. A marker already on that frame is refreshed, never duplicated.
    fn create_marker(&mut self, part_id: &str, local_frame: u64) -> Result<Data> {
        let fps = require_story_fps(&self.conn)?;
        let part_id = part_id.trim();
        if part_id.is_empty() {
            return Err("part_id required".into());
        }
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
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
                marker_id
            }
        };
        finalize_story(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Created(marker_id))
    }

    /// v5 `update_marker_frame`: inside the story length, on a free frame; the start
    /// and the end marker are locked. The label is kept.
    fn move_marker(&mut self, marker_id: &str, program_frame: u64) -> Result<Data> {
        let fps = require_story_fps(&self.conn)?;
        let marker_id = marker_id.trim();
        if marker_id.is_empty() {
            return Err("marker_id required".into());
        }
        let frame = program_frame as i64;
        let duration = program_length(&self.conn)?;
        if frame > duration {
            return Err(format!(
                "M marker mora biti unutar trajanja storyja ({:.3} s).",
                timeline_sec(duration, fps)
            ));
        }
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
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

    /// v5 `delete_marker`: the start and the end marker are locked.
    fn delete_marker(&mut self, marker_id: &str) -> Result<Data> {
        require_story_fps(&self.conn)?;
        let marker_id = marker_id.trim();
        if marker_id.is_empty() {
            return Err("marker_id required".into());
        }
        let duration = program_length(&self.conn)?;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        locked_check(&tx, marker_id, duration)?;
        delete_markers_with_slots(&tx, &[marker_id.to_string()])?;
        finalize_story(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// v5 `list_markers`: every marker by program frame, the locked start and end
    /// included (`system_role`).
    fn list_markers(&self) -> Result<Data> {
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
    fn list_segments(&self) -> Result<Data> {
        if !object_exists(&self.conn, "view", "public_story_parts")? {
            return Ok(Data::Segments(Vec::new()));
        }
        let mut statement = self
            .conn
            .prepare(
                "SELECT segment_id, kind, sort_index, clip_id, in_frame, out_frame, fps_num,
                        fps_den, active, a1_source_channel
                 FROM public_story_parts ORDER BY sort_index, segment_id",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                Ok(ProgramSegment {
                    segment_id: row.get(0)?,
                    kind: row.get(1)?,
                    sort_index: row.get::<_, i64>(2)?.max(0) as u32,
                    clip_id: row.get(3)?,
                    in_frame: row.get::<_, i64>(4)?.max(0) as u64,
                    out_frame: row.get::<_, i64>(5)?.max(0) as u64,
                    fps_num: row.get(6)?,
                    fps_den: row.get(7)?,
                    active: row.get::<_, i64>(8)? != 0,
                    a1_source_channel: channel(row.get(9)?),
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok(Data::Segments(rows))
    }

    /// v5 `marker_slots_snapshot`: the stored slots in order, with `has_cover`.
    fn list_slots(&self) -> Result<Data> {
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
    fn list_covers(&self) -> Result<Data> {
        if !object_exists(&self.conn, "view", "public_story_covers")? {
            return Ok(Data::Covers(Vec::new()));
        }
        let mut statement = self
            .conn
            .prepare(
                "SELECT cover_id, slot_id, clip_id, virtual_shot_id, timeline_start_frame,
                        timeline_end_frame, source_in_frame, source_out_frame, fps_num, fps_den,
                        a2_source_channel
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
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok(Data::Covers(rows))
    }

    fn read_story_selection(&self) -> Result<Data> {
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
            ..selection
        }))
    }

    /// v5 `select_part`: an existing segment, or an empty id to clear.
    fn select_part(&mut self, part_id: &str) -> Result<Data> {
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
    fn select_slot(&mut self, slot_id: &str) -> Result<Data> {
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
    /// v5 `delete_cover`: the selection is cleared when it was this cover.
    fn delete_cover(&mut self, cover_id: &str) -> Result<Data> {
        let cover_id = cover_id.trim();
        let now = story_now();
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
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
    fn select_cover(&mut self, cover_id: &str) -> Result<Data> {
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
    fn create_cover(
        &mut self,
        project_id: &str,
        slot_id: &str,
        (clip_id, shot_id): (&str, &str),
        (in_frame, out_frame): (u64, u64),
        (fps_num, fps_den): (u32, u32),
        a2_source_channel: u16,
    ) -> Result<Data> {
        qnc_media_records::valid_id(clip_id).map_err(err)?;
        qnc_media_records::valid_id(shot_id).map_err(err)?;
        if out_frame <= in_frame {
            return Err("OUT mora biti najmanje jedan frame nakon IN.".into());
        }
        if fps_num == 0 || fps_den == 0 {
            return Err("Pokrivalica nema valjan source fps.".into());
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
            return Err("Projektna baza pripada drugom projektu.".into());
        }
        require_imported_clip(&self.conn, clip_id)?;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let story_timebase: Option<(u32, u32)> = tx
            .query_row(
                "SELECT source_fps_num, source_fps_den FROM story_parts
                 WHERE active = 1 ORDER BY sort_index LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(err)?;
        let Some((num, den)) = story_timebase else {
            return Err("Prica nema segmenata.".into());
        };
        if u64::from(num) * u64::from(fps_den) != u64::from(fps_num) * u64::from(den) {
            return Err(format!(
                "Klip ima {fps_num}/{fps_den} fps, a prica {num}/{den}; mijesani fps nije dopusten."
            ));
        }
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
                sort_index, a2_source_channel, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, '', '', ?11, ?12, ?13, ?14,
                       ?15, ?16, ?17, ?18, ?19, 0, ?21, ?20, ?20)",
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
                a2_source_channel
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

fn ensure_story_schema(conn: &Connection) -> Result<()> {
    // Development records are removed, not converted (AGENTS section 3):
    // program_segments / program_markers, story_covers from before the
    // marker-pair slot binding or the A2 channel, and a story from before the
    // A1 channel (its markers, slots and covers go with it).
    let part_columns = table_columns(conn, "story_parts")?;
    if !part_columns.is_empty() && !part_columns.iter().any(|c| c == "a1_source_channel") {
        conn.execute_batch(
            "DROP VIEW IF EXISTS public_story_parts;
             DROP VIEW IF EXISTS public_story_markers;
             DROP VIEW IF EXISTS public_story_marker_slots;
             DROP VIEW IF EXISTS public_story_covers;
             DROP TABLE IF EXISTS story_covers;
             DROP TABLE IF EXISTS story_marker_slots;
             DROP TABLE IF EXISTS story_markers;
             DROP TABLE story_parts;",
        )
        .map_err(err)?;
    }
    let cover_columns = table_columns(conn, "story_covers")?;
    if !cover_columns.is_empty()
        && !["slot_id", "a2_source_channel"]
            .iter()
            .all(|needed| cover_columns.iter().any(|column| column == needed))
    {
        conn.execute_batch("DROP VIEW IF EXISTS public_story_covers; DROP TABLE story_covers;")
            .map_err(err)?;
    }
    conn.execute_batch(
        "DROP VIEW IF EXISTS public_program_segments;
         DROP VIEW IF EXISTS public_program_markers;
         DROP TABLE IF EXISTS program_markers;
         DROP TABLE IF EXISTS program_segments;
         CREATE TABLE IF NOT EXISTS story_state (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            selected_part_id TEXT NOT NULL DEFAULT '',
            selected_shot_id TEXT NOT NULL DEFAULT '',
            selected_slot_id TEXT NOT NULL DEFAULT '',
            selected_cover_id TEXT NOT NULL DEFAULT '',
            draft_updated_at TEXT,
            committed_at TEXT,
            updated_at TEXT
         );
         CREATE TABLE IF NOT EXISTS story_parts (
            part_id TEXT PRIMARY KEY,
            kind TEXT NOT NULL CHECK (kind IN ('tonovi', 'offovi')),
            sort_index INTEGER NOT NULL,
            title TEXT NOT NULL DEFAULT '',
            text TEXT NOT NULL DEFAULT '',
            clip_id TEXT NOT NULL DEFAULT '',
            virtual_shot_id TEXT NOT NULL DEFAULT '',
            in_tc TEXT NOT NULL DEFAULT '',
            out_tc TEXT NOT NULL DEFAULT '',
            in_seconds REAL,
            out_seconds REAL,
            fps REAL NOT NULL DEFAULT 0,
            source_fps_num INTEGER NOT NULL DEFAULT 0,
            source_fps_den INTEGER NOT NULL DEFAULT 1,
            in_frame INTEGER NOT NULL DEFAULT 0,
            out_frame INTEGER NOT NULL DEFAULT 0,
            duration_frames INTEGER NOT NULL DEFAULT 0,
            duration_label TEXT NOT NULL DEFAULT '',
            duration_color_key TEXT NOT NULL DEFAULT '',
            active INTEGER NOT NULL DEFAULT 1,
            a1_source_channel INTEGER NOT NULL DEFAULT 0 CHECK (a1_source_channel >= 0),
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_story_parts_sort ON story_parts(sort_index);
         CREATE TABLE IF NOT EXISTS story_markers (
            marker_id TEXT PRIMARY KEY,
            timeline_frame INTEGER NOT NULL DEFAULT 0,
            timeline_sec REAL NOT NULL DEFAULT 0,
            tc TEXT NOT NULL DEFAULT '',
            label TEXT NOT NULL DEFAULT '',
            sort_index INTEGER NOT NULL DEFAULT 0,
            system_role TEXT NOT NULL DEFAULT '',
            origin_part_id TEXT NOT NULL DEFAULT '',
            origin_local_frame INTEGER,
            origin_local_sec REAL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_story_markers_frame ON story_markers(timeline_frame);
         CREATE TABLE IF NOT EXISTS story_marker_slots (
            slot_id TEXT PRIMARY KEY,
            slot_index INTEGER NOT NULL,
            start_frame INTEGER NOT NULL DEFAULT 0,
            end_frame INTEGER NOT NULL DEFAULT 0,
            duration_frames INTEGER NOT NULL DEFAULT 0,
            start_sec REAL NOT NULL,
            end_sec REAL NOT NULL,
            duration_sec REAL NOT NULL DEFAULT 0,
            start_marker_id TEXT NOT NULL DEFAULT '',
            end_marker_id TEXT NOT NULL DEFAULT '',
            slot_signature TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS story_covers (
            cover_id TEXT PRIMARY KEY,
            slot_id TEXT NOT NULL DEFAULT '',
            timeline_start_frame INTEGER NOT NULL DEFAULT 0,
            timeline_end_frame INTEGER NOT NULL DEFAULT 0,
            timeline_start_sec REAL NOT NULL DEFAULT 0,
            timeline_end_sec REAL NOT NULL DEFAULT 0,
            slot_signature TEXT NOT NULL DEFAULT '',
            slot_index INTEGER NOT NULL DEFAULT 0,
            clip_id TEXT NOT NULL DEFAULT '',
            virtual_shot_id TEXT NOT NULL DEFAULT '',
            title TEXT NOT NULL DEFAULT '',
            note TEXT NOT NULL DEFAULT '',
            in_tc TEXT NOT NULL DEFAULT '',
            out_tc TEXT NOT NULL DEFAULT '',
            in_seconds REAL,
            out_seconds REAL,
            source_in_frame INTEGER NOT NULL DEFAULT 0,
            source_out_frame INTEGER NOT NULL DEFAULT 0,
            source_fps REAL NOT NULL DEFAULT 0,
            source_fps_num INTEGER NOT NULL DEFAULT 0,
            source_fps_den INTEGER NOT NULL DEFAULT 1,
            sort_index INTEGER NOT NULL DEFAULT 0,
            a2_source_channel INTEGER NOT NULL DEFAULT 0 CHECK (a2_source_channel >= 0),
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS story_object_history (
            object_type TEXT NOT NULL,
            object_id TEXT NOT NULL,
            state TEXT NOT NULL DEFAULT '',
            snapshot_json TEXT NOT NULL DEFAULT '{}',
            updated_at TEXT NOT NULL,
            PRIMARY KEY (object_type, object_id)
         );
         DROP VIEW IF EXISTS public_story_parts;
         CREATE VIEW public_story_parts AS
         SELECT part_id AS segment_id, kind, sort_index, clip_id, in_frame, out_frame,
                source_fps_num AS fps_num, source_fps_den AS fps_den, active,
                duration_frames, duration_label, duration_color_key, a1_source_channel
         FROM story_parts;
         DROP VIEW IF EXISTS public_story_markers;
         CREATE VIEW public_story_markers AS
         SELECT marker_id, timeline_frame AS program_frame, system_role
         FROM story_markers;
         DROP VIEW IF EXISTS public_story_state;
         CREATE VIEW public_story_state AS
         SELECT selected_part_id, selected_shot_id, selected_slot_id, selected_cover_id,
                draft_updated_at, committed_at, updated_at
         FROM story_state;
         DROP VIEW IF EXISTS public_story_marker_slots;
         CREATE VIEW public_story_marker_slots AS
         SELECT s.slot_id, s.slot_index, s.start_frame, s.end_frame, s.duration_frames,
                s.start_marker_id, s.end_marker_id, s.slot_signature, s.updated_at,
                EXISTS(SELECT 1 FROM story_covers c WHERE c.slot_id = s.slot_id) AS has_cover
         FROM story_marker_slots s;
         DROP VIEW IF EXISTS public_story_covers;
         CREATE VIEW public_story_covers AS
         SELECT cover_id, slot_id, slot_signature, slot_index, timeline_start_frame,
                timeline_end_frame, clip_id, virtual_shot_id, source_in_frame, source_out_frame,
                source_fps_num AS fps_num, source_fps_den AS fps_den, a2_source_channel
         FROM story_covers;",
    )
    .map_err(err)?;
    // Cover identity is the marker pair. The seconds signature stays a column
    // from v5; this owner does not use it as the slot id.
    conn.execute(
        "INSERT INTO story_state (id, updated_at) VALUES (1, ?1)
         ON CONFLICT(id) DO NOTHING",
        [story_now()],
    )
    .map_err(err)?;
    Ok(())
}

/// v5 `now_str`.
pub(crate) fn story_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("epoch_{secs}")
}

/// v5 `validate_kind`: a story segment is `tonovi` or `offovi`.
fn story_kind(kind: &str) -> Result<&'static str> {
    match kind.trim() {
        "tonovi" => Ok("tonovi"),
        "offovi" => Ok("offovi"),
        other => Err(format!("Nepoznata vrsta segmenta: {other}.")),
    }
}

fn new_marker_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("marker_{nanos:x}")
}

/// v5 `require_current_story_program_source_fps`: the source rate of the first
/// active segment; markers and slots need it.
fn require_story_fps(conn: &Connection) -> Result<f64> {
    let rate: Option<(i64, i64)> = conn
        .query_row(
            "SELECT source_fps_num, source_fps_den FROM story_parts
             WHERE active = 1 AND source_fps_num > 0 AND source_fps_den > 0
             ORDER BY sort_index LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(err)?;
    rate.map(|(num, den)| num as f64 / den as f64)
        .ok_or_else(|| "timeline_fps_invalid: story program nema valjan source FPS".into())
}

/// v5 `round3(frame_to_seconds(frame, fps))`.
fn timeline_sec(frame: i64, fps: f64) -> f64 {
    if fps <= 0.0 {
        return 0.0;
    }
    ((frame.max(0) as f64 / fps) * 1000.0).round() / 1000.0
}

/// v5 `frame_to_timecode`: `hh:mm:ss:ff` with the rate rounded to whole frames.
fn frame_timecode(frame: i64, fps: f64) -> String {
    let fps = fps.round().max(1.0) as i64;
    let total = frame.max(0);
    let total_sec = total / fps;
    format!(
        "{:02}:{:02}:{:02}:{:02}",
        total_sec / 3600,
        (total_sec / 60) % 60,
        total_sec % 60,
        total % fps
    )
}

/// v5 `seconds_frames_label_from_frames`: `seconds:frames`.
fn frames_label(frames: i64, fps: f64) -> String {
    let fps = fps.round().max(1.0) as i64;
    let frames = frames.max(0);
    format!("{}:{:02}", frames / fps, frames % fps)
}

/// v5 `duration_color_key_from_frames` with the default 3, 5, 7 second marks.
fn duration_color_key(frames: i64, fps: f64) -> &'static str {
    if fps <= 0.0 {
        return "over_7";
    }
    match frames.max(0) as f64 / fps {
        seconds if seconds < 3.0 => "under_3",
        seconds if seconds < 5.0 => "under_5",
        seconds if seconds < 7.0 => "under_7",
        _ => "over_7",
    }
}

/// v5 lock of the start and the end marker on delete and move.
fn locked_check(conn: &Connection, marker_id: &str, duration: i64) -> Result<()> {
    let marker: Option<(i64, String)> = conn
        .query_row(
            "SELECT timeline_frame, system_role FROM story_markers WHERE marker_id = ?1",
            [marker_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(err)?;
    let (frame, role) = marker.ok_or_else(|| format!("marker not found: {marker_id}"))?;
    if frame == 0 || role == "program_start" {
        return Err("Početni M marker je zaključan.".into());
    }
    if role == "program_end" || frame == duration {
        return Err("Završni M marker je zaključan.".into());
    }
    Ok(())
}

struct StoryPartSpan {
    part_id: String,
    frames: i64,
}

/// Active segments in order with v5 `part_span_frames`.
fn active_part_spans(conn: &Connection) -> Result<Vec<StoryPartSpan>> {
    let mut statement = conn
        .prepare(
            "SELECT part_id, duration_frames, in_frame, out_frame FROM story_parts
             WHERE active = 1 ORDER BY sort_index",
        )
        .map_err(err)?;
    let rows = statement
        .query_map([], |row| {
            let duration: i64 = row.get(1)?;
            let (in_frame, out_frame): (i64, i64) = (row.get(2)?, row.get(3)?);
            let frames = if duration > 0 {
                duration
            } else {
                (out_frame - in_frame).max(0)
            };
            Ok(StoryPartSpan {
                part_id: row.get(0)?,
                frames,
            })
        })
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    Ok(rows)
}

fn first_marker(conn: &Connection, sql: &str, value: i64) -> Result<Option<String>> {
    conn.query_row(sql, [value], |row| row.get(0))
        .optional()
        .map_err(err)
}

/// v5 `ensure_start_marker` + `ensure_end_marker`: exactly one locked start
/// marker at frame 0 on the first segment and one locked end marker at the
/// program length on the last segment; no segments, no markers.
fn ensure_boundary_markers(conn: &Connection) -> Result<()> {
    let parts = active_part_spans(conn)?;
    let duration: i64 = parts.iter().map(|part| part.frames).sum();
    if duration <= 0 {
        conn.execute_batch(
            "DELETE FROM story_covers;
             DELETE FROM story_marker_slots;
             DELETE FROM story_markers;
             UPDATE story_state SET selected_slot_id = '' WHERE id = 1;",
        )
        .map_err(err)?;
        return Ok(());
    }
    let fps = require_story_fps(conn)?;
    let now = story_now();
    let first = parts
        .first()
        .map(|part| part.part_id.as_str())
        .unwrap_or("");
    let start_tc = frame_timecode(0, fps);
    let start = match first_marker(
        conn,
        "SELECT marker_id FROM story_markers WHERE system_role = 'program_start'
         AND ?1 = ?1 ORDER BY timeline_frame, marker_id LIMIT 1",
        0,
    )? {
        Some(id) => Some(id),
        None => first_marker(
            conn,
            "SELECT marker_id FROM story_markers WHERE timeline_frame = ?1
             ORDER BY marker_id LIMIT 1",
            0,
        )?,
    };
    match start {
        Some(id) => {
            conn.execute(
                "UPDATE story_markers
                 SET timeline_frame = 0, origin_part_id = ?1, origin_local_frame = 0,
                     origin_local_sec = 0, tc = ?2, label = ?2, system_role = 'program_start'
                 WHERE marker_id = ?3",
                params![first, start_tc, id],
            )
            .map_err(err)?;
            conn.execute(
                "UPDATE story_markers SET system_role = ''
                 WHERE system_role = 'program_start' AND marker_id != ?1",
                [&id],
            )
            .map_err(err)?;
        }
        None => {
            conn.execute(
                "INSERT INTO story_markers
                    (marker_id, timeline_frame, timeline_sec, tc, label, sort_index, system_role,
                     origin_part_id, origin_local_frame, origin_local_sec, created_at, updated_at)
                 VALUES (?1, 0, 0, ?2, ?2, 0, 'program_start', ?3, 0, 0, ?4, ?4)",
                params![new_marker_id(), start_tc, first, now],
            )
            .map_err(err)?;
        }
    }
    let at_duration = first_marker(
        conn,
        "SELECT marker_id FROM story_markers
         WHERE timeline_frame = ?1 AND system_role != 'program_start'
         ORDER BY marker_id LIMIT 1",
        duration,
    )?;
    let system_end = first_marker(
        conn,
        "SELECT marker_id FROM story_markers WHERE system_role = 'program_end'
         AND ?1 = ?1 ORDER BY timeline_frame, marker_id LIMIT 1",
        0,
    )?;
    if let (Some(system_end), Some(at_duration)) = (&system_end, &at_duration) {
        if system_end != at_duration {
            conn.execute(
                "DELETE FROM story_markers WHERE marker_id = ?1",
                [system_end],
            )
            .map_err(err)?;
        }
    }
    let last = parts.last().expect("duration > 0 has a segment");
    let end_sec = timeline_sec(duration, fps);
    let end_tc = frame_timecode(duration, fps);
    let local_sec = last.frames as f64 / fps;
    match at_duration.or(system_end) {
        Some(id) => {
            conn.execute(
                "UPDATE story_markers
                 SET timeline_frame = ?1, timeline_sec = ?2, tc = ?3, label = ?3,
                     sort_index = 0, system_role = 'program_end', origin_part_id = ?4,
                     origin_local_frame = ?5, origin_local_sec = ?6, updated_at = ?7
                 WHERE marker_id = ?8",
                params![
                    duration,
                    end_sec,
                    end_tc,
                    last.part_id,
                    last.frames,
                    local_sec,
                    now,
                    id
                ],
            )
            .map_err(err)?;
            conn.execute(
                "UPDATE story_markers SET system_role = ''
                 WHERE system_role = 'program_end' AND marker_id != ?1",
                [&id],
            )
            .map_err(err)?;
        }
        None => {
            conn.execute(
                "INSERT INTO story_markers
                    (marker_id, timeline_frame, timeline_sec, tc, label, sort_index, system_role,
                     origin_part_id, origin_local_frame, origin_local_sec, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4, 0, 'program_end', ?5, ?6, ?7, ?8, ?8)",
                params![
                    new_marker_id(),
                    duration,
                    end_sec,
                    end_tc,
                    last.part_id,
                    last.frames,
                    local_sec,
                    now
                ],
            )
            .map_err(err)?;
        }
    }
    Ok(())
}

/// v5 `finalize_story_mutation` under the v5 rule: boundary markers, slots from
/// adjacent markers, covers bound to their slot again, selection kept valid.
fn finalize_story(conn: &Connection) -> Result<()> {
    ensure_boundary_markers(conn)?;
    recompute_marker_slots(conn)?;
    rebind_covers(conn)?;
    normalize_story_selection(conn)
}

/// v5 `recompute_marker_slots`: one slot between every two adjacent markers,
/// empty spans skipped. Identity is the marker pair (v5 rule), not seconds.
fn recompute_marker_slots(conn: &Connection) -> Result<()> {
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
fn delete_markers_with_slots(conn: &Connection, marker_ids: &[String]) -> Result<()> {
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

/// Slot identity: the ids of its start and end marker.
fn slot_id(start_marker_id: &str, end_marker_id: &str) -> String {
    format!("{start_marker_id}|{end_marker_id}")
}

struct SlotRow {
    slot_id: String,
    slot_index: i64,
    start_frame: i64,
    end_frame: i64,
    start_marker_id: String,
    signature: String,
}

/// v5 rule: a cover is removed only by the user. Deleting a marker deletes its
/// slots and their covers (`delete_markers_with_slots`). Any other change keeps
/// the cover on its logical slot: the same marker pair (moved markers give it the
/// new frames), else the slot that keeps its start marker (a new marker split the
/// slot: the cover is trimmed). A cover without a slot keeps no frames.
fn rebind_covers(conn: &Connection) -> Result<()> {
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
fn normalize_story_selection(conn: &Connection) -> Result<()> {
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

/// Program length in frames: the segments one after another.
fn program_length(conn: &Connection) -> Result<i64> {
    conn.query_row(
        "SELECT COALESCE(SUM(out_frame - in_frame), 0) FROM story_parts WHERE active = 1",
        [],
        |row| row.get(0),
    )
    .map_err(err)
}

/// Program frames `[start, end)` of a segment in the stored order.
fn segment_window(conn: &Connection, segment_id: &str) -> Result<Option<(u64, u64)>> {
    let mut statement = conn
        .prepare(
            "SELECT part_id, out_frame - in_frame FROM story_parts
             WHERE active = 1 ORDER BY sort_index",
        )
        .map_err(err)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    let mut start = 0u64;
    for (id, frames) in rows {
        let end = start + frames.max(0) as u64;
        if id == segment_id {
            return Ok(Some((start, end)));
        }
        start = end;
    }
    Ok(None)
}

/// The user markers at or after a program frame, with their frames.
fn user_markers_from(conn: &Connection, from: i64) -> Result<Vec<(String, i64)>> {
    let mut statement = conn
        .prepare(
            "SELECT marker_id, timeline_frame FROM story_markers
             WHERE timeline_frame >= ?1 AND system_role = ''",
        )
        .map_err(err)?;
    let rows = statement
        .query_map([from], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err);
    rows
}

/// Puts one marker on a program frame.
fn set_marker_frame(conn: &Connection, marker_id: &str, frame: i64, fps: f64) -> Result<()> {
    let frame = frame.max(0);
    conn.execute(
        "UPDATE story_markers SET timeline_frame = ?1, timeline_sec = ?2, tc = ?3, updated_at = ?4
         WHERE marker_id = ?5",
        params![frame, timeline_sec(frame, fps), frame_timecode(frame, fps), story_now(), marker_id],
    )
    .map_err(err)?;
    Ok(())
}

/// Moves the user markers at or after a program frame by `delta` frames.
fn shift_markers_from(conn: &Connection, from: i64, delta: i64, fps: f64) -> Result<()> {
    let rows = {
        let mut statement = conn
            .prepare(
                "SELECT marker_id, timeline_frame FROM story_markers
                 WHERE timeline_frame >= ?1 AND system_role = ''",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([from], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        rows
    };
    let now = story_now();
    for (marker_id, frame) in rows {
        let frame = (frame + delta).max(0);
        conn.execute(
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
    Ok(())
}

/// Every segment, active or excluded, in its place: one order for both, so an
/// excluded segment comes back where it was.
fn all_segment_order(conn: &Connection) -> Result<Vec<(String, bool)>> {
    let mut statement = conn
        .prepare("SELECT part_id, active FROM story_parts ORDER BY sort_index, rowid")
        .map_err(err)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0))
        })
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    Ok(rows)
}

fn write_segment_order(conn: &Connection, ids: &[String]) -> Result<()> {
    let now = story_now();
    for (sort_index, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE story_parts SET sort_index = ?1, updated_at = ?3 WHERE part_id = ?2",
            params![sort_index as i64, id, now],
        )
        .map_err(err)?;
    }
    Ok(())
}

/// 0..n over all segments, keeping their order (active and excluded together).
fn renumber_segments(conn: &Connection) -> Result<()> {
    let ids: Vec<String> = all_segment_order(conn)?
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    write_segment_order(conn, &ids)
}
/// Only an imported clip goes into the story (docs/93 R10), read through the one
/// public reader of the clip catalog.
fn require_imported_clip(conn: &Connection, clip_id: &str) -> Result<()> {
    match qnc_content_read::imported_on(conn, clip_id)? {
        Some(true) => Ok(()),
        Some(false) => Err(format!("Klip '{clip_id}' nije uvezen.")),
        None => Err("Klip nije pronadjen.".into()),
    }
}

fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    conn.prepare(&format!("PRAGMA table_info({table})"))
        .and_then(|mut statement| {
            statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(err)
}

/// A stored source channel index; the column never holds a negative one.
fn channel(value: i64) -> u16 {
    u16::try_from(value.max(0)).unwrap_or(u16::MAX)
}


pub(crate) fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn object_exists(conn: &Connection, kind: &str, name: &str) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type=?1 AND name=?2)",
        params![kind, name],
        |r| r.get(0),
    )
    .map_err(err)
}
