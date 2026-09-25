//! Sync/B-roll on the program (v5 `story.rs` host of `sync_cover_capture`): the
//! session math lives in `qnc-sync-cover`; here it meets the program model. IN
//! on the source arms it, Space in the Source view starts it from the marker at
//! or before the Wrap playhead, O or the end of the source closes the slot (its
//! end marker is written when missing), the stored slot is selected and Enter
//! writes the cover; reaching the next marker or the source OUT only stops. The program player only gets the
//! window to play; nothing here plays or opens media.

use qnc_content_store::Operation;
use qnc_sync_cover::{SyncPreview, SyncSource};

use crate::ProgramSegments;

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
        if (previous_in == Some(source_in) && !pressed) || !self.sync.enabled() {
            return;
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
    pub fn arm_sync(&mut self) {
        self.sync_in_pressed = true;
    }

    /// Space: a Sync play starts in the Source view once IN armed it.
    pub fn sync_space(&mut self, wrap_active: bool) -> SyncSpace {
        if wrap_active || !self.sync.wants_space() {
            return SyncSpace::Play;
        }
        let markers: Vec<u64> = self.view.markers.iter().map(|pin| pin.frame).collect();
        let playhead = self.playhead.unwrap_or(0);
        let total = self.view.total_frames;
        match self
            .sync
            .start(&markers, playhead, total, self.view.timebase)
        {
            Ok(preview) => {
                self.selected_marker = None;
                self.selected_slot = None;
                self.selected_cover = None;
                self.refresh_view("Sync play · OUT završava slot".into());
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
    pub fn sync_frame(&mut self, confirmed: Option<u64>) -> Option<u64> {
        let was_active = self.sync.is_active();
        let frame = self.sync.program_frame(confirmed);
        if was_active && !self.sync.is_active() {
            self.after_sync_finish("Sync slot zatvoren · Enter dodaje pokrivalicu");
        }
        frame
    }

    /// O during a Sync play closes the slot at the Wrap playhead (v5
    /// `finish_sync_cover_with_out`). False when no Sync play runs.
    pub fn finish_sync(&mut self) -> bool {
        if !self.sync.is_active() {
            return false;
        }
        match self.sync.finish(self.playhead.unwrap_or(0)) {
            Ok(_) => self.after_sync_finish("Sync slot odabran · Enter dodaje pokrivalicu"),
            Err(error) => self.refresh_view(error),
        }
        true
    }

    /// The source frame the source timeline shows during Sync, and the IN/OUT of
    /// a closed Sync slot (v5 `set_source_playhead_frame`).
    pub fn sync_source(&self) -> Option<(u64, Option<(u64, u64)>)> {
        self.sync.source_view()
    }

    /// Enter is taken by a closed Sync slot or its cover write.
    pub fn sync_holds_enter(&self) -> bool {
        self.sync.holds_enter()
    }

    pub(crate) fn toggle_sync(&mut self) {
        // A running Sync play gives the player back to the whole program.
        self.program_changed |= self.sync.toggle();
        let message = if self.sync.enabled() {
            "Sync uključen · Source IN armira Sync play"
        } else {
            "Sync isključen"
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
                    self.refresh_view("Sync OUT spremljen · čekam novi M-M slot".into());
                }
                None => self.refresh_view("Nema Wrap segmenta za kraj Sync slota.".into()),
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
                self.refresh_view("Sync slot još nije materijaliziran".into());
            }
            return;
        };
        self.selected_slot = Some(slot_id.clone());
        self.selected_cover = None;
        self.write(Operation::SelectSlot { slot_id });
        self.refresh_view("Sync slot odabran · Enter dodaje pokrivalicu".into());
        self.commit_sync(false);
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
            return self.refresh_view("Sync slot već ima pokrivalicu".into());
        }
        self.write(Operation::CreateCover {
            project_id: self.project_id.clone(),
            slot_id,
            clip_id: slot.source.clip_id,
            clip_name: slot.source.clip_name,
            in_frame: slot.source.source_in,
            out_frame: slot.source_out,
            fps_num: slot.source.timebase.0,
            fps_den: slot.source.timebase.1,
        });
        self.refresh_view("Sync pokrivalica dodana u slot · spremam...".into());
    }
}
