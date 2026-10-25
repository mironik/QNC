//! Sync/B-roll capture of a story cover (QNC v5 `components/sync_cover_capture.rs`
//! and its host in `story.rs`).
//!
//! With Sync on, IN on the source arms it; the source plays on its own player from IN (user
//! 2026-10-20: IN is only the number the cover starts at, no player is opened), the slot
//! starts at the M marker at or before the Wrap playhead and the draft follows the playing
//! source frame (`source_window_frame`); the story's A1 under it plays in the same player.
//! User rule (2026-09-25): it runs to the source OUT or to the first M marker after that
//! marker, whichever comes first, and stops there; O stops it earlier. The source OUT of the
//! slot is IN plus the frames played. Enter confirms the cover once the slot is stored.
//!
//! This component owns only the session state and its frame math. It knows no
//! form, no player, no database and no project: the program model gives it the
//! markers, slots and the source, and writes what it asks for.

pub const MODULE_ID: &str = "qnc.module.sync-cover";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The source the Sync plays: the clip, its IN and OUT marks, its length and rate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncSource {
    pub clip_id: String,
    pub clip_name: String,
    pub source_in: u64,
    /// The source OUT mark (the clip end when none is set).
    pub source_out: u64,
    pub duration_frames: u64,
    pub timebase: (u32, u32),
}

/// A Sync play: the slot `[in, out)` on the program and the source from its IN (v5
/// `build_preview`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPreview {
    pub window: (u64, u64),
    pub clip_id: String,
    pub source_in: u64,
}

/// A closed Sync slot: program `[start, end)` and the source `[in, out)` played
/// over it (v5 `SyncCoverPendingSlot`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncSlot {
    pub start: u64,
    pub end: u64,
    pub source: SyncSource,
    pub source_out: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Session {
    anchor: u64,
    /// Program frame where the source runs out (v5 `auto_finish_program_frame`).
    auto_finish: u64,
    total_frames: u64,
    source: SyncSource,
    /// Story frames per source frames (`1:1`, `2:1` or `1:2`).
    per_source: (u64, u64),
    /// Last window frame seen: the player returns to the window start at its
    /// end, so a frame going back means the end was reached between repaints.
    last: u64,
    /// The program frame the window being played starts at (the anchor, or the draft
    /// end when a stopped Sync plays on).
    play_from: u64,
    /// The draft end of the slot (exclusive): it follows the play, the arrows move it
    /// (user 2026-10-13: only Enter writes the cover, Delete drops the draft).
    end: u64,
    /// The play stopped (the end reached, or the end moved by an arrow): Space plays
    /// on from the draft end in a new window.
    restart: bool,
}

#[derive(Debug, Default)]
pub struct SyncCover {
    enabled: bool,
    /// Source IN pressed while Sync is on (v5 `arm_source_in`).
    armed: Option<SyncSource>,
    active: Option<Session>,
    pending: Option<SyncSlot>,
    ready: Option<(String, SyncSlot)>,
    committing: bool,
    /// Source frame under the Sync play, then the source OUT of the closed slot
    /// (v5 `set_source_playhead_frame`).
    source_frame: Option<u64>,
    /// The play just reached the next M marker or the source OUT (told once: the source
    /// stops there once, live 2026-10-20: stopping it at every poll blocked Space).
    reached_end: bool,
}

impl SyncCover {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    /// A closed slot waits for its markers or for Enter, or its cover is being written.
    pub fn holds_enter(&self) -> bool {
        self.pending.is_some() || self.ready.is_some() || self.committing
    }

    /// Sync on or off (v5 `set_enabled`); off drops the session. Returns whether a
    /// Sync play was stopped.
    pub fn toggle(&mut self) -> bool {
        let was_active = self.active.is_some();
        *self = Self {
            enabled: !self.enabled,
            ..Self::default()
        };
        was_active
    }

    /// Source IN while Sync is on arms the next Space (v5 `arm_source_in`).
    pub fn arm(&mut self, source: SyncSource) {
        if self.enabled {
            self.armed = Some(source);
            self.pending = None;
            self.ready = None;
            self.committing = false;
        }
    }

    /// Whether Space starts a Sync play: Sync on, a source armed, no play running.
    pub fn wants_space(&self) -> bool {
        self.enabled && self.armed.is_some() && self.active.is_none()
    }

    /// The rate of the armed source, for the caller to tell how its frames count in
    /// the story.
    pub fn armed_timebase(&self) -> Option<(u32, u32)> {
        self.armed.as_ref().map(|source| source.timebase)
    }

    /// Space: the program window from the marker at or before the playhead to
    /// where the source runs out (v5 `build_preview`). `markers` are the program
    /// frames of the M markers. `per_source` is story frames per source frames:
    /// `(1, 1)`, or `(2, 1)` / `(1, 2)` for a source of half or twice the story rate
    /// (user 2026-10-09); the caller decides it from the story rule.
    pub fn start(
        &mut self,
        markers: &[u64],
        playhead: u64,
        total_frames: u64,
        per_source: (u64, u64),
    ) -> Result<SyncPreview, String> {
        let source = self
            .armed
            .clone()
            .ok_or(qnc_ui_text::text("story.msg.sync_no_in"))?;
        if !matches!(per_source, (1, 1) | (2, 1) | (1, 2)) {
            return Err(qnc_ui_text::text("story.msg.sync_rate").into());
        }
        let (num, den) = per_source;
        if total_frames == 0 {
            return Err(qnc_ui_text::text("story.msg.program_empty").into());
        }
        let anchor = markers
            .iter()
            .copied()
            .filter(|frame| *frame <= playhead)
            .max()
            .ok_or(qnc_ui_text::text("story.msg.sync_no_marker"))?
            .min(total_frames - 1);
        let duration = source.duration_frames.max(1);
        let source_in = source.source_in.min(duration - 1);
        // User rule (2026-09-25): Sync runs from the source IN to the source OUT or
        // to the first M marker after the anchor, whichever comes first.
        let available =
            ((source.source_out.min(duration).max(source_in + 1)) - source_in) * num / den;
        let next_marker = markers
            .iter()
            .copied()
            .filter(|frame| *frame > anchor)
            .min();
        let end = (anchor + available.max(1))
            .min(next_marker.unwrap_or(u64::MAX))
            .min(total_frames)
            .max(anchor + 1);
        let preview = SyncPreview {
            window: (anchor, end),
            clip_id: source.clip_id.clone(),
            source_in,
        };
        self.armed = None;
        self.pending = None;
        self.ready = None;
        self.source_frame = Some(source_in);
        self.active = Some(Session {
            anchor,
            auto_finish: end,
            total_frames,
            source: SyncSource {
                source_in,
                ..source
            },
            per_source,
            last: 0,
            play_from: anchor,
            end: anchor + 1,
            restart: false,
        });
        Ok(preview)
    }

    /// A frame of the window being played (from `source_window_frame`), as a program frame (v5
    /// `absolute_program_frame_from_playback`). Reaching the end of the source
    /// closes the slot (v5 `should_auto_finish`). Outside a Sync play the frame
    /// is returned as it is.
    pub fn program_frame(&mut self, window_frame: Option<u64>) -> Option<u64> {
        let Some(session) = self.active.as_mut() else {
            return window_frame;
        };
        if session.restart {
            // Stopped: the Wrap playhead shows the last picture of the draft.
            return Some(session.end.saturating_sub(1).max(session.anchor));
        }
        let frame = window_frame?;
        // Back to the window start is the end of the play (the player returns there); a
        // pause confirming a frame or two before the last is not (live 2026-10-13: that
        // wrote the cover at every pause).
        let wrapped = frame + 3 < session.last;
        session.last = session.last.max(frame);
        let program = (session.play_from + frame).min(session.auto_finish);
        let (num, den) = session.per_source;
        if program + 1 >= session.auto_finish || wrapped {
            // It stops there and waits: Enter writes the cover, Delete drops it.
            session.end = session.auto_finish;
            session.restart = true;
            self.reached_end = true;
            self.source_frame = Some(session.source.source_in + ((session.end - session.anchor) * den).div_ceil(num));
            return Some(session.end.saturating_sub(1));
        }
        session.end = program + 1;
        self.source_frame = Some(session.source.source_in + (program - session.anchor) * den / num);
        Some(program)
    }

    /// The source frame the source player plays, as a frame of the window being played
    /// (user 2026-10-20: "the player plays, IN is only the number the cover starts at";
    /// Sync follows the playing source, it opens no player). A frame before the play's
    /// first source frame (the source ran out and went back to its start) is 0, which
    /// ends the play as the window's return to its start does.
    pub fn source_window_frame(&self, source_frame: u64) -> Option<u64> {
        let session = self.active.as_ref()?;
        let (num, den) = session.per_source;
        let first = session.source.source_in + (session.play_from - session.anchor) * den / num;
        Some(source_frame.saturating_sub(first) * num / den)
    }

    /// Whether the play just reached its end (once).
    pub fn take_reached_end(&mut self) -> bool {
        std::mem::take(&mut self.reached_end)
    }

    /// The program span of a running or stopped Sync's draft, `[anchor, end)` (user
    /// 2026-10-18: the cover being written is drawn as it grows).
    pub fn draft_span(&self) -> Option<(u64, u64)> {
        self.active.as_ref().map(|session| (session.anchor, session.end.max(session.anchor + 1)))
    }

    /// The draft end of a running or stopped Sync (exclusive).
    pub fn draft_end(&self) -> Option<u64> {
        self.active.as_ref().map(|session| session.end)
    }

    /// An arrow on a stopped Sync: its end `frames` later or earlier, within the source
    /// and up to the marker or source end it may reach; false without a Sync.
    pub fn nudge(&mut self, frames: i64) -> bool {
        let Some(session) = self.active.as_mut() else {
            return false;
        };
        let end = session.end.saturating_add_signed(frames).clamp(session.anchor + 1, session.auto_finish);
        session.end = end;
        session.restart = true;
        let (num, den) = session.per_source;
        self.source_frame = Some(session.source.source_in + ((end - session.anchor) * den).div_ceil(num));
        true
    }

    /// O stopped the play (user 2026-10-19): the draft keeps its end; the arrows or the
    /// mouse move it, Space plays on, Enter writes, Delete drops.
    pub fn stop(&mut self) -> bool {
        self.active.as_mut().map(|session| session.restart = true).is_some()
    }

    /// A Sync stopped with its draft (not playing).
    pub fn stopped(&self) -> bool {
        self.active.as_ref().is_some_and(|session| session.restart)
    }

    /// The end of a stopped Sync at the program frame that plays `source_out` (the OUT
    /// dragged on the source timeline), kept within the slot as an arrow keeps it.
    pub fn drag_end(&mut self, source_out: u64) -> bool {
        let Some(session) = self.active.as_ref() else {
            return false;
        };
        let (num, den) = session.per_source;
        let end = session.anchor + source_out.saturating_sub(session.source.source_in) * num / den.max(1);
        let frames = end as i64 - session.end as i64;
        frames != 0 && self.nudge(frames)
    }

    /// Space on a stopped Sync: the window from its draft end on, the source from where
    /// it got to; `None` when it is not stopped or nothing is left to play.
    pub fn resume(&mut self) -> Option<SyncPreview> {
        let session = self.active.as_mut()?;
        if !session.restart || session.end >= session.auto_finish {
            return None;
        }
        let (num, den) = session.per_source;
        session.restart = false;
        session.play_from = session.end;
        session.last = 0;
        Some(SyncPreview {
            window: (session.end, session.auto_finish),
            clip_id: session.source.clip_id.clone(),
            source_in: session.source.source_in + (session.end - session.anchor) * den / num,
        })
    }

    /// Whether a stopped Sync has reached the end it may reach (Enter keeps it).
    pub fn at_end(&self) -> bool {
        self.active.as_ref().is_some_and(|session| session.restart && session.end >= session.auto_finish)
    }

    /// Delete before Enter: the Sync and its draft go, nothing was written.
    pub fn discard(&mut self) -> bool {
        let had = self.active.take().is_some() | self.pending.take().is_some() | self.ready.take().is_some();
        self.source_frame = None;
        self.committing = false;
        had
    }

    /// O, or the end of the source: the slot from the anchor to `end` (v5
    /// `pending_slot`). The Sync play stops.
    pub fn finish(&mut self, end: u64) -> Result<&SyncSlot, String> {
        let session = self.active.take().ok_or(qnc_ui_text::text("story.msg.sync_not_playing"))?;
        let start = session.anchor;
        let end = end.max(start + 1).min(session.total_frames.max(start + 1));
        let (num, den) = session.per_source;
        let source_out = (session.source.source_in + ((end - start) * den).div_ceil(num))
            .min(session.source.duration_frames);
        if source_out <= session.source.source_in {
            return Err(qnc_ui_text::text("story.msg.sync_out_after_in").into());
        }
        self.source_frame = Some(source_out);
        Ok(self.pending.insert(SyncSlot {
            start,
            end,
            source: session.source,
            source_out,
        }))
    }

    /// A closed slot still waits for its stored M-M slot.
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// The program frame that still needs its M marker (v5 `marker_at_head` after OUT).
    pub fn missing_marker(&self, markers: &[u64]) -> Option<u64> {
        let pending = self.pending.as_ref()?;
        (!markers.contains(&pending.end)).then_some(pending.end)
    }

    /// Matches the closed slot with a stored slot `(slot_id, start, end)` (v5
    /// `slot_plan`, `ready_cover`). Returns the slot to select.
    pub fn resolve<'a>(
        &mut self,
        mut slots: impl Iterator<Item = (&'a str, u64, u64)>,
    ) -> Option<String> {
        let pending = self.pending.as_ref()?;
        let (slot_id, _, _) =
            slots.find(|(_, start, end)| *start == pending.start && *end == pending.end)?;
        let slot_id = slot_id.to_string();
        let pending = self.pending.take()?;
        self.ready = Some((slot_id.clone(), pending));
        Some(slot_id)
    }

    /// Enter: the cover to write (v5 `commit_sync_cover_ready`). Reaching the next
    /// marker or the source OUT only stops; the user confirms with Enter.
    pub fn take_commit(&mut self, enter: bool) -> Option<(String, SyncSlot)> {
        if !enter {
            return None;
        }
        let ready = self.ready.take()?;
        self.committing = true;
        Some(ready)
    }

    /// A closed slot waits for Enter (not written yet): Escape drops it.
    pub fn waits(&self) -> bool {
        self.pending.is_some() || self.ready.is_some()
    }

    /// The cover write landed (or failed).
    pub fn landed(&mut self) {
        self.committing = false;
    }

    /// What the source timeline shows (v5 `sync_playhead_from_player_frame`):
    /// the source frame under the Sync play, played in parallel with the program,
    /// and once the slot is closed its source OUT with the IN/OUT of the slot.
    pub fn source_view(&self) -> Option<(u64, Option<(u64, u64)>)> {
        if let Some(session) = &self.active {
            // The source IN to the draft end are the cover's IN and OUT: the OUT follows the
            // play and the arrows (user 2026-10-16).
            let frame = self.source_frame.unwrap_or(session.source.source_in);
            let source_in = session.source.source_in;
            return Some((frame, Some((source_in, frame.max(source_in + 1)))));
        }
        let slot = self
            .pending
            .as_ref()
            .or(self.ready.as_ref().map(|(_, slot)| slot))?;
        Some((
            slot.source_out,
            Some((slot.source.source_in, slot.source_out)),
        ))
    }

    /// The clip the Sync's IN and OUT belong to (its play, closed slot or cover waiting).
    pub fn source_clip(&self) -> Option<&str> {
        let source = match &self.active {
            Some(session) => &session.source,
            None => &self.pending.as_ref().or(self.ready.as_ref().map(|(_, slot)| slot))?.source,
        };
        Some(source.clip_id.as_str())
    }

    /// The user pointed elsewhere on the program: a running Sync play stops.
    pub fn cancel(&mut self) -> bool {
        self.active.take().is_some()
    }
}

#[cfg(test)]
mod source_follow_tests;

#[cfg(test)]
mod tests;
