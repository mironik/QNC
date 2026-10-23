use super::*;

impl ProgramSegments {
    /// Appends a segment at the end of the program; it becomes selected once saved.
    pub fn create(&mut self, segment: NewSegment) {
        let key = self.write_keyed(Operation::CreateSegment {
            project_id: self.project_id.clone(),
            kind: segment.kind.db().into(),
            clip_id: segment.clip_id,
            in_frame: segment.in_frame,
            out_frame: segment.out_frame,
            fps_num: segment.fps_num,
            fps_den: segment.fps_den,
            a1_source_channel: segment.a1_source_channel,
            a1_level_db: segment.a1_level_db,
            a1_off: segment.a1_off,
        });
        if key.is_some() {
            self.pending_create = key;
        }
    }

    /// Talking Head / Voice over from the marked source: the chosen clip between
    /// the confirmed IN/OUT, in the timebase the player confirmed. Missing parts end
    /// in a message, never in a guessed range or rate.
    pub fn create_from_source(&mut self, kind: SegmentKind) {
        let Some((clip_id, (in_frame, out_frame), (fps_num, fps_den))) = self.marked_source()
        else {
            return;
        };
        self.create(NewSegment {
            kind,
            clip_id,
            in_frame,
            out_frame,
            fps_num,
            fps_den,
            a1_source_channel: self.source.lanes[0].channel,
            a1_level_db: self.source.lanes[0].level_db,
            a1_off: self.source.lanes[0].off,
        });
    }

    /// v5 `quick_cover` / `overwrite_cover`: the marked source becomes the cover
    /// of the target slot (a B-roll virtual shot; the store replaces a cover there).
    pub fn cover_from_source(&mut self, overwrite: bool) {
        self.cover_pressed = Some((overwrite, std::time::Instant::now()));
        let slot = if overwrite {
            self.view.overwrite_cover_slot().map(str::to_string)
        } else {
            self.view
                .quick_cover_slot()
                .map(|slot| slot.slot_id.clone())
        };
        let slot_id = match slot {
            Ok(slot_id) => slot_id,
            Err(error) => return self.refresh_view(error),
        };
        let Some((clip_id, (in_frame, out_frame), (fps_num, fps_den))) = self.marked_source()
        else {
            return;
        };
        self.cover_end = self.cover_end_of(&slot_id, (in_frame, out_frame), (fps_num, fps_den));
        let clip_name = self.source.clip_name.clone();
        self.cover(
            NewCover {
                slot_id,
                clip_id,
                in_frame,
                out_frame,
                fps_num,
                fps_den,
                a2_source_channel: self.source.lanes[1].channel,
                a2_level_db: self.source.lanes[1].level_db,
                a2_off: self.source.lanes[1].off,
            },
            clip_name,
        );
    }

    /// The program frame a cover of the source range ends at in slot `slot_id`: its start
    /// and the source's length in story frames, at most the slot's end (a longer B-roll fills
    /// the slot, a shorter one splits it there, `qnc-program-db`).
    pub(crate) fn cover_end_of(&self, slot_id: &str, (in_frame, out_frame): (u64, u64), source_rate: (u32, u32)) -> Option<u64> {
        let slot = self.view.slots.iter().find(|slot| slot.slot_id == slot_id)?;
        let (num, den) = qnc_program_db::story_per_source(self.view.timebase?, source_rate).ok()?;
        let frames = out_frame.saturating_sub(in_frame) * num as u64 / den as u64;
        Some((slot.start_frame + frames).min(slot.end_frame))
    }

    pub(crate) fn marked_source(&mut self) -> Option<(String, (u64, u64), (u32, u32))> {
        let source = &self.source;
        let rate = |value: i64| u32::try_from(value).unwrap_or(0);
        let picked = match (&source.clip_id, source.marks, source.timebase) {
            (Some(clip_id), Some(marks), Some((num, den))) => {
                Some((clip_id.clone(), marks, (rate(num), rate(den))))
            }
            _ => None,
        };
        if picked.is_none() {
            self.refresh_view(qnc_ui_text::text("story.msg.pick_clip_and_marks").into());
        }
        picked
    }

    /// v5 `select_cover`: clears the selected marker; the playhead goes to the click.
    pub(crate) fn select_cover(&mut self, cover_id: String, frame: u64) {
        if !self
            .view
            .covers
            .iter()
            .any(|cover| cover.cover_id == cover_id)
        {
            return;
        }
        self.selected_marker = None;
        self.marker_edit = None;
        self.cover_taken = None;
        self.selected_cover = Some(cover_id.clone());
        self.write(Operation::SelectCover { cover_id });
        self.refresh_view(String::new());
        self.cue_at(frame);
    }

    /// A click that chooses something else (the playhead, a slot) lets go of the chosen cover
    /// (audit 2026-10-17: a cover chosen earlier stayed chosen in the base for good, its strip
    /// stayed drawn on its row and Delete would have taken it).
    pub(crate) fn drop_selected_cover(&mut self) {
        if self.selected_cover.take().is_some() {
            self.write(Operation::SelectCover { cover_id: String::new() });
        }
    }

    pub fn delete_selected(&mut self) {
        // Ctrl+M then Delete: the marker being edited goes (it is the selected one).
        self.marker_edit = None;
        if let Some(marker_id) = self.selected_marker.clone() {
            self.selected_marker = None;
            return self.write(Operation::DeleteMarker { marker_id });
        }
        // v5 `delete_selected_timeline_item`: the marker, then the cover, then the segment.
        if let Some(cover_id) = self.selected_cover.take() {
            return self.write(Operation::DeleteCover { cover_id });
        }
        let Some(segment_id) = self.selected.clone() else {
            return self.refresh_view(qnc_ui_text::text("story.msg.pick_segment").into());
        };
        self.write(Operation::DeleteSegment { segment_id });
    }

    pub fn move_selected(&mut self, up: bool) {
        let Some(segment_id) = self.selected.clone() else {
            return self.refresh_view(qnc_ui_text::text("story.msg.pick_segment").into());
        };
        self.write(Operation::MoveSegment { segment_id, up });
    }
}
