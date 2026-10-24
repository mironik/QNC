//! Sync/B-roll on the program (v5 `story.rs` host of `sync_cover_capture`): the
//! session math lives in `qnc-sync-cover`; here it meets the program model. IN
//! on the source arms it, Space in the Source view starts it from the marker at
//! or before the Wrap playhead, O or the end of the source closes the slot (its
//! end marker is written when missing), the stored slot is selected and Enter
//! writes the cover; reaching the next marker or the source OUT only stops. The program player only gets the
//! window to play; nothing here plays or opens media.

use qnc_program_db::Operation;
use qnc_sync_cover::{SyncPreview, SyncSource};

use crate::{NewCover, ProgramSegments};

/// What Space does (v5 `playback_transport_toggle_intent`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncSpace {
    /// Play this program window with the source over it (v5 `PlayProgram`).
    Start(SyncPreview),
    /// Sync could not start; the reason is in the view message.
    Blocked,
    /// Not a Sync start: plain play or pause.
    Play,
}

impl ProgramSegments {
    /// Source IN pressed while Sync is on arms it (v5 `mark_in_action`).
    pub(crate) fn arm_sync_on_new_in(&mut self, previous_in: Option<u64>) {
        let source = &self.source;
        let (Some(clip_id), Some(source_in), Some((num, den))) =
            (&source.clip_id, source.in_mark, source.timebase)
        else {
            return;
        };
        let pressed = std::mem::take(&mut self.sync_in_pressed);
        let flight = std::mem::take(&mut self.sync_flight);
        if (previous_in == Some(source_in) && !pressed) || !self.sync.enabled() {
            return;
        }
        self.sync_flight_ready = flight;
        // An IN on the fly while a Sync already plays starts it again from this IN (live
        // 2026-10-24: the IN reached the running Sync as Space, paused the source and left a
        // cover of one frame).
        if flight && self.sync.is_active() && !self.sync.stopped() {
            self.sync.cancel();
        }
        let rate = |value: i64| u32::try_from(value).unwrap_or(0);
        self.sync.arm(SyncSource {
            clip_id: clip_id.clone(),
            clip_name: source.clip_name.clone(),
            source_in,
            source_out: source.marks.map_or(source.duration_frames, |(_, out)| out),
            duration_frames: source.duration_frames,
            timebase: (rate(num), rate(den)),
        });
    }

    /// IN was pressed on the source: every press arms Sync, also on the same
    /// frame (v5 `mark_in_action` -> `arm_source_in`); taken with the next source.
    pub fn arm_sync(&mut self, in_flight: bool) {
        self.sync_in_pressed = true;
        self.sync_flight = in_flight && self.sync.enabled();
    }

    /// O stopped a running Sync: its draft waits for the arrows, the mouse, Enter or Delete.
    pub fn sync_stop(&mut self) -> bool {
        if !self.sync.stop() {
            return false;
        }
        self.refresh_view(qnc_ui_text::text("story.msg.sync_stopped").into());
        true
    }

    /// An IN pressed while the source played has armed Sync: it starts now (once).
    pub fn take_sync_flight(&mut self) -> bool {
        self.sync_flight_ready && std::mem::take(&mut self.sync_flight_ready)
    }

    /// The OUT of a stopped Sync dragged to `source_out` on the source timeline.
    pub fn sync_drag_end(&mut self, source_out: u64) -> bool {
        if !self.sync.drag_end(source_out) {
            return false;
        }
        self.refresh_view(qnc_ui_text::text("story.msg.sync_stopped").into());
        true
    }

    /// Space: a Sync play starts in the Source view once IN armed it.
    pub fn sync_space(&mut self, wrap_active: bool) -> SyncSpace {
        // A stopped Sync plays on from its draft end (user 2026-10-13).
        if let Some(preview) = self.sync.resume() {
            self.refresh_view(qnc_ui_text::text("story.msg.sync_resumes").into());
            return SyncSpace::Start(preview);
        }
        if self.sync.at_end() {
            // The slot is full: Space plays the source on, the draft waits for Enter or Delete
            // (live 2026-10-20: refusing Space here looked like a blocked player).
            self.refresh_view(qnc_ui_text::text("story.msg.sync_at_end").into());
            return SyncSpace::Play;
        }
        if wrap_active || !self.sync.wants_space() {
            return SyncSpace::Play;
        }
        let markers: Vec<u64> = self.view.markers.iter().map(|pin| pin.frame).collect();
        let playhead = self.playhead.unwrap_or(0);
        let total = self.view.total_frames;
        // The story rule decides how the source counts: the same, twice or half the
        // story rate (user 2026-10-09).
        let per_source = match (self.view.timebase, self.sync.armed_timebase()) {
            (Some(story), Some(source)) => qnc_program_db::story_per_source(story, source)
                .map(|(num, den)| (num as u64, den as u64)),
            _ => Err(qnc_ui_text::text("story.msg.program_empty").into()),
        };
        match per_source.and_then(|per_source| self.sync.start(&markers, playhead, total, per_source)) {
            // A slot that has its B-roll is said at once, not after the play (live
            // 2026-10-19: the story ended with that slot, Enter then refused it).
            Ok(preview) if self.stored_slots.iter().any(|slot| slot.start_frame == preview.window.0 && slot.has_cover) => {
                self.sync.cancel();
                self.refresh_view(qnc_ui_text::text("story.msg.sync_slot_has_broll").into());
                SyncSpace::Blocked
            }
            Ok(preview) => {
                self.selected_marker = None;
                self.selected_slot = None;
                self.selected_cover = None;
                self.refresh_view(qnc_ui_text::text("story.msg.sync_play").into());
                SyncSpace::Start(preview)
            }
            Err(error) => {
                self.refresh_view(error);
                SyncSpace::Blocked
            }
        }
    }

    /// A frame the program player confirmed, as a program frame; during a Sync
    /// play the window frame is moved to its anchor and the end of the source
    /// closes the slot.
    /// Reaching an M marker or the source OUT, or a pause, only stops it (user
    /// 2026-10-13: only Enter writes the cover, Delete drops the draft).
    /// `program`: the frame the program player confirmed; `clip`: the frame the source
    /// player confirmed, which a Sync follows (user 2026-10-20: it opens no player).
    pub fn sync_frame(&mut self, program: Option<u64>, clip: Option<u64>) -> Option<u64> {
        let frame = match self.sync.is_active() {
            true => self.sync.program_frame(clip.and_then(|frame| self.sync.source_window_frame(frame))),
            false => program,
        };
        self.view.sync_draft = self.sync.draft_span();
        frame
    }

    /// The window the next Sync would play, while Sync is on and none runs: from the M
    /// marker at or before the Wrap playhead to the next one (or the end of the story). The
    /// story's A1 under it is prepared before IN (user 2026-10-20).
    pub fn sync_window_ahead(&self) -> Option<(u64, u64)> {
        if !self.sync.enabled() || self.sync.is_active() || self.view.total_frames == 0 {
            return None;
        }
        let playhead = self.playhead.unwrap_or(0);
        let markers = self.view.markers.iter().map(|pin| pin.frame);
        let anchor = markers.clone().filter(|frame| *frame <= playhead).max()?;
        let end = markers.filter(|frame| *frame > anchor).min().unwrap_or(self.view.total_frames);
        Some((anchor, end.min(self.view.total_frames).max(anchor + 1)))
    }

    /// The Sync play just reached the next M marker or the source OUT (told once).
    pub fn take_sync_reached_end(&mut self) -> bool {
        self.sync.take_reached_end()
    }

    /// A Sync stopped with its draft (O, the next M marker or the source OUT).
    pub fn sync_stopped(&self) -> bool {
        self.sync.stopped()
    }

    /// Whether a Sync is running or stopped with a draft that Enter writes, Delete drops.
    /// Sync on and an IN armed it: Space starts the Sync play.
    pub fn sync_armed(&self) -> bool {
        self.sync.wants_space()
    }

    pub fn sync_drafting(&self) -> bool {
        self.sync.is_active()
    }

    /// An arrow on a stopped Sync moves its end a frame.
    pub fn sync_nudge(&mut self, frames: i64) -> bool {
        if !self.sync.nudge(frames) {
            return false;
        }
        self.refresh_view(qnc_ui_text::text("story.msg.sync_stopped").into());
        true
    }

    /// Delete before Enter: the Sync draft goes, nothing was written; the player takes
    /// the whole program again.
    pub fn discard_sync(&mut self) -> bool {
        if !self.sync.discard() {
            return false;
        }
        self.program_changed = true;
        self.refresh_view(qnc_ui_text::text("story.msg.sync_discarded").into());
        true
    }

    /// O during a Sync play closes the slot at the Wrap playhead (v5
    /// `finish_sync_cover_with_out`). False when no Sync play runs.
    pub fn finish_sync(&mut self) -> bool {
        if !self.sync.is_active() {
            return false;
        }
        let end = self.sync.draft_end().unwrap_or(self.playhead.unwrap_or(0));
        match self.sync.finish(end) {
            Ok(slot) => {
                // The Wrap playhead on the slot's end marker: the next Sync starts there,
                // not at the slot just made (live 2026-10-13: four started at 0 in a row).
                let end = slot.end;
                self.after_sync_finish(qnc_ui_text::text("story.msg.sync_slot_chosen"));
                self.cue_program(end);
            }
            Err(error) => self.refresh_view(error),
        }
        true
    }

    /// The source frame the source timeline shows during Sync, and the IN/OUT of
    /// a closed Sync slot (v5 `set_source_playhead_frame`).
    /// Only on the Sync's own clip (live 2026-10-24: a Sync waiting for Enter put its IN/OUT
    /// on the source timeline of every clip chosen in the pool).
    pub fn sync_source(&self) -> Option<(u64, Option<(u64, u64)>)> {
        let own = self.sync.source_clip().is_some_and(|clip| self.source.clip_id.as_deref() == Some(clip));
        own.then(|| self.sync.source_view()).flatten()
    }

    /// Enter is taken by a running Sync play, a closed Sync slot or its cover write.
    pub fn sync_holds_enter(&self) -> bool {
        self.sync.is_active()
            || self.sync.holds_enter()
            || self.marker_edit.is_some()
            || self.marker_committing
            || self.editing.is_some()
            || self.source.holds_enter
    }

    pub(crate) fn toggle_sync(&mut self) {
        if self.view.rows.is_empty() && !self.sync.enabled() {
            return self.refresh_view(qnc_ui_text::text("story.msg.broll_needs_segments").into());
        }
        // A running Sync play gives the player back to the whole program.
        self.program_changed |= self.sync.toggle();
        // Sync on: the Wrap playhead on the nearest M marker, where the cover starts (user
        // 2026-10-19).
        if self.sync.enabled() {
            let playhead = self.playhead.unwrap_or(0);
            let total = self.view.total_frames;
            let nearest = self.view.markers.iter().map(|pin| pin.frame).filter(|frame| *frame < total).min_by_key(|frame| frame.abs_diff(playhead));
            if let Some(frame) = nearest {
                self.cue_program(frame);
            }
        }
        let message = if self.sync.enabled() {
            qnc_ui_text::text("story.msg.sync_on")
        } else {
            qnc_ui_text::text("story.msg.sync_off")
        };
        self.refresh_view(message.into());
    }

    /// The player takes the whole program again; the end marker is written when
    /// it is missing (v5 `marker_at_head`), else the slot is taken at once.
    fn after_sync_finish(&mut self, message: &str) {
        self.program_changed = true;
        let markers: Vec<u64> = self.view.markers.iter().map(|pin| pin.frame).collect();
        match self.sync.missing_marker(&markers) {
            Some(frame) => match self.view.segment_at(frame) {
                Some(segment) => {
                    let operation = Operation::CreateMarker {
                        part_id: segment.segment_id.clone(),
                        local_frame: frame - segment.start_frame,
                    };
                    self.write(operation);
                    self.refresh_view(qnc_ui_text::text("story.msg.sync_out_saved").into());
                }
                None => self.refresh_view(qnc_ui_text::text("story.msg.no_wrap_for_sync_end").into()),
            },
            None => {
                self.refresh_view(message.into());
                self.resolve_sync();
            }
        }
    }

    /// The closed slot is stored: select it (v5 `select_pending_sync_slot`); the
    /// cover waits for Enter.
    pub(crate) fn resolve_sync(&mut self) {
        let slots = self.stored_slots.iter();
        let found = slots.map(|slot| (slot.slot_id.as_str(), slot.start_frame, slot.end_frame));
        let Some(slot_id) = self.sync.resolve(found) else {
            // v5 `slot_plan`: another M marker inside the Sync range splits it.
            if self.sync.has_pending() {
                self.refresh_view(qnc_ui_text::text("story.msg.sync_slot_not_ready").into());
            }
            return;
        };
        self.selected_slot = Some(slot_id.clone());
        self.selected_cover = None;
        self.write(Operation::SelectSlot { slot_id });
        self.refresh_view(qnc_ui_text::text("story.msg.sync_slot_chosen").into());
        let commit = std::mem::take(&mut self.sync_commit);
        self.commit_sync(commit);
    }

    /// Enter: the cover of the closed slot (v5
    /// `commit_sync_cover_ready`); a slot that got a cover meanwhile is kept.
    pub(crate) fn commit_sync(&mut self, enter: bool) {
        let Some((slot_id, slot)) = self.sync.take_commit(enter) else {
            return;
        };
        if self
            .stored_slots
            .iter()
            .any(|stored| stored.slot_id == slot_id && stored.has_cover)
        {
            self.sync.landed();
            return self.refresh_view(qnc_ui_text::text("story.msg.sync_slot_has_broll").into());
        }
        self.cover(
            NewCover {
                slot_id,
                clip_id: slot.source.clip_id,
                in_frame: slot.source.source_in,
                out_frame: slot.source_out,
                fps_num: slot.source.timebase.0,
                fps_den: slot.source.timebase.1,
                a2_source_channel: self.source.lanes[1].channel,
                a2_level_db: self.source.lanes[1].level_db,
                a2_off: self.source.lanes[1].off,
            },
            slot.source.clip_name,
        );
        self.refresh_view(qnc_ui_text::text("story.msg.sync_broll_added").into());
    }
}

impl ProgramSegments {
    /// Enter during a Sync play closes the slot at the Wrap playhead and writes its
    /// cover once the slot is stored (user rule 2026-10-01: Enter or reaching an M
    /// marker writes it). False when no Sync play runs.
    pub(crate) fn finish_sync_with_cover(&mut self) -> bool {
        self.sync_commit = self.sync.is_active();
        self.finish_sync() || std::mem::take(&mut self.sync_commit)
    }
}
