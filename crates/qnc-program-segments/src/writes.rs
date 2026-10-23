use super::*;

impl ProgramSegments {
    pub(crate) fn write(&mut self, operation: Operation) {
        self.write_keyed(operation);
    }

    /// Sends a story write without waiting; its key comes back.
    pub(crate) fn write_keyed(&mut self, operation: Operation) -> Option<String> {
        let Some(target) = self.target.clone() else {
            self.refresh_view(qnc_ui_text::text("story.msg.db_unavailable").into());
            return None;
        };
        if self.writer.is_none() {
            match StoryWriter::spawn(target) { // never waits on the form thread
                Ok(writer) => self.writer = Some(writer),
                Err(error) => {
                    self.refresh_view(error);
                    return None;
                }
            }
        }
        let key = self.next_key("write");
        match self.writer.as_ref()?.submit(&operation) {
            Ok(pending) => {
                self.pending.push((key.clone(), pending));
                Some(key)
            }
            Err(error) => {
                self.refresh_view(error);
                None
            }
        }
    }

    /// v5 `create_cover` from source frames: the B-roll virtual shot of the source
    /// IN/OUT is made first (`qnc-virtual-shots`); the cover of the slot follows once
    /// it is saved.
    pub(crate) fn cover(&mut self, cover: NewCover, clip_name: String) {
        let Some(target) = self.target.clone() else {
            return self.refresh_view(qnc_ui_text::text("story.msg.db_unavailable").into());
        };
        if self.shots.is_none() {
            match VirtualShotsWriter::spawn(target) { // never waits on the form thread
                Ok(shots) => self.shots = Some(shots),
                Err(error) => return self.refresh_view(error),
            }
        }
        let sent = self.shots.as_ref().map(|shots| {
            shots.submit(&ShotOperation::CreateCoverShot {
                project_id: self.project_id.clone(),
                clip_id: cover.clip_id.clone(),
                clip_name,
                in_frame: cover.in_frame,
                out_frame: cover.out_frame,
            })
        });
        match sent {
            Some(Ok(pending)) => {
                self.cover_marks = Some((cover.clip_id.clone(), cover.in_frame));
                self.covers_waiting.push((pending, cover));
            }
            Some(Err(error)) => self.refresh_view(error),
            None => {}
        }
    }

    /// Applies finished writes and rereads the program. Returns whether it changed.
    pub fn poll(&mut self) -> bool {
        let read = self.take_read();
        let mut message = String::new();
        let mut landed = false;
        // Covers whose B-roll virtual shot is saved go into their slot.
        let mut place = Vec::new();
        for (pending, cover) in std::mem::take(&mut self.covers_waiting) {
            match pending.try_take() {
                None => self.covers_waiting.push((pending, cover)),
                Some(result) => {
                    landed = true;
                    match result {
                        Ok(ShotData::Created(shot_id)) => place.push((cover, shot_id)),
                        Ok(_) => message = qnc_ui_text::text("story.msg.bad_broll_reply").into(),
                        Err(error) => message = error,
                    }
                }
            }
        }
        for (cover, virtual_shot_id) in place {
            self.created_cover_shots.push((virtual_shot_id.clone(), cover.clip_id.clone(), cover.in_frame));
            let shot = virtual_shot_id.clone();
            let key = self.write_keyed(Operation::CreateCover {
                project_id: self.project_id.clone(),
                slot_id: cover.slot_id,
                clip_id: cover.clip_id,
                virtual_shot_id,
                in_frame: cover.in_frame,
                out_frame: cover.out_frame,
                fps_num: cover.fps_num,
                fps_den: cover.fps_den,
                a2_source_channel: cover.a2_source_channel,
                a2_level_db: cover.a2_level_db,
                a2_off: cover.a2_off,
            });
            match key {
                Some(key) => self.cover_writes.push((key, shot)),
                None => self.forget_cover_shot(shot),
            }
        }
        let mut created = None;
        let mut waiting = Vec::new();
        for (key, pending) in std::mem::take(&mut self.pending) {
            match pending.try_take() {
                None => waiting.push((key, pending)),
                Some(result) => {
                    landed = true;
                    // The cover is in the story: its B-roll shot is kept, and the Wrap playhead
                    // goes to its end for the next B.
                    let cover = self.cover_writes.iter().any(|(written, _)| *written == key);
                    if result.is_ok() {
                        if let Some((_, shot)) = self.cover_writes.iter().find(|(written, _)| *written == key) {
                            if let Some(shots) = &self.shots {
                                let _ = shots.submit(&ShotOperation::ConfirmCoverShot { shot_id: shot.clone() });
                            }
                        }
                        if let Some(end) = self.cover_end.take().filter(|_| cover) {
                            self.cue_program(end);
                        }
                    } else if cover {
                        self.cover_end = None;
                    }
                    match result {
                        Ok(StoryData::Created(segment_id))
                            if self.pending_create.as_deref() == Some(key.as_str()) =>
                        {
                            created = Some(segment_id);
                        }
                        Ok(_) => {}
                        Err(error) => {
                            message = error;
                            // A refused cover leaves no B-roll shot behind.
                            if let Some(index) = self.cover_writes.iter().position(|(written, _)| *written == key) {
                                let (_, shot) = self.cover_writes.remove(index);
                                self.forget_cover_shot(shot);
                            }
                        }
                    }
                    self.cover_writes.retain(|(written, _)| *written != key);
                }
            }
        }
        self.pending = waiting;
        if !landed {
            return read;
        }
        if let Some(segment_id) = created {
            // A new segment becomes the selection, in the database too.
            self.selected = Some(segment_id.clone());
            self.write(Operation::SelectPart {
                part_id: segment_id,
            });
        }
        // The store may have moved the selection (v5 `delete_part`): take it once
        // every write of this component has landed.
        self.adopt_selection = !self.has_pending_work();
        if self.adopt_selection {
            self.sync.landed();
            self.marker_committing = false;
        }
        self.reload();
        if !message.is_empty() {
            self.refresh_view(message);
        }
        true
    }

    /// The B-roll shot made for a refused cover goes again (its poster is not made).
    pub(crate) fn forget_cover_shot(&mut self, shot_id: String) {
        self.created_cover_shots.retain(|(shot, _, _)| *shot != shot_id);
        if let Some(shots) = &self.shots {
            let _ = shots.submit(&ShotOperation::ForgetCoverShot { shot_id });
        }
    }

    pub(crate) fn next_key(&mut self, what: &str) -> String {
        self.sequence += 1;
        format!("program_segment:{what}:{}", self.sequence)
    }
}
