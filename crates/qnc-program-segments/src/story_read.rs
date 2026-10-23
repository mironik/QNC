use super::*;

impl ProgramSegments {
    /// Rereads the program from the database, on a thread of its own; `poll` takes the
    /// result. A reload asked while one runs reads once more after it.
    pub fn reload(&mut self) {
        let Some(target) = self.target.clone() else {
            return;
        };
        if self.reading.is_some() {
            self.read_again = true;
            return;
        }
        let (send, receive) = std::sync::mpsc::channel();
        // The clips counted before are not read again; the read gives the new ones.
        let (content, known) = (self.prerender.clone(), counted(&self.clip_channels));
        let started = std::thread::Builder::new().name("qnc-story-read".into()).spawn(move || {
            // One consistent read of the story (external audit 2026-10-17), the same the
            // player builds its program from.
            let read = StoryReader::open(&target).and_then(|mut client| {
                let story = client.program_snapshot()?;
                let clips = story.segments.iter().map(|segment| &segment.clip_id).chain(story.covers.iter().map(|cover| &cover.clip_id));
                let channels = clips
                    .filter(|clip| !known.contains(*clip))
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    // Only a count read is kept: a clip whose record is not final yet, or a read
                    // that failed, is read again with the next read of the story (third audit
                    // 2026-10-17: a missed count stayed missing for good).
                    .filter_map(|clip| {
                        let count = qnc_source_channels::read_clip(content.as_ref()?, clip).ok()??;
                        Some((clip.clone(), Some(count)))
                    })
                    .collect();
                Ok((story.segments, story.markers, story.slots, story.covers, story.selection, channels))
            });
            let _ = send.send(read);
        });
        match started {
            Ok(_) => self.reading = Some(receive),
            Err(error) => self.refresh_view(error.to_string()),
        }
    }

    /// Whether a read of the story or of its waves is still running.
    pub fn reading(&self) -> bool {
        self.reading.is_some() || self.waves.reading() || self.filmstrips.reading()
    }

    /// Takes a finished read of the story; true if one was applied.
    pub(crate) fn take_read(&mut self) -> bool {
        let Some(receive) = &self.reading else {
            return false;
        };
        let read = match receive.try_recv() {
            Ok(read) => read,
            Err(std::sync::mpsc::TryRecvError::Empty) => return false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err("story read stopped".into()),
        };
        self.reading = None;
        self.apply_read(read);
        if std::mem::take(&mut self.read_again) {
            self.reload();
        }
        true
    }

    pub(crate) fn apply_read(&mut self, read: qnc_program_db::Result<StoryRead>) {
        match read {
            Ok((stored, markers, slots, covers, selection, channels)) => {
                self.clip_channels.extend(channels);
                if stored != self.stored || covers != self.stored_covers {
                    self.program_changed = true;
                    self.waker.wake(self.prerender.as_ref());
                }
                self.stored = stored;
                self.stored_covers = covers;
                self.stored_markers = markers;
                self.stored_slots = slots;
                self.history = (selection.undo_depth, selection.redo_depth);
                self.story_rate = selection.story_fps;
                if std::mem::take(&mut self.adopt_selection) {
                    let some = |id: String| (!id.is_empty()).then_some(id);
                    self.selected = some(selection.selected_part_id);
                    self.selected_slot = some(selection.selected_slot_id);
                    self.selected_cover = some(selection.selected_cover_id);
                }
                self.refresh_view(String::new());
                self.resolve_sync();
            }
            Err(error) => self.refresh_view(error),
        }
    }
}
