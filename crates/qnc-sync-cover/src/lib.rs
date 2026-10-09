//! Sync/B-roll capture of a story cover (QNC v5 `components/sync_cover_capture.rs`
//! and its host in `story.rs`).
//!
//! With Sync on, IN on the source arms it; Space in the Source view plays the
//! program from the M marker at or before the Wrap playhead with the source from
//! its IN over it as a transient cover (picture and A2, A1 of the story stays).
//! User rule (2026-09-25): it runs to the source OUT or to the first M marker
//! after that marker, whichever comes first, and stops there; O stops it earlier.
//! The source OUT of the slot is IN plus the frames played. Enter confirms the
//! cover once the slot is stored.
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

/// What the program player plays: the program window `[in, out)` with the source
/// from its IN as a transient cover (v5 `build_preview`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPreview {
    pub window: (u64, u64),
    pub clip_id: String,
    pub source_in: u64,
    pub timebase: (u32, u32),
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
            .ok_or("Source IN nije postavljen za Sync.")?;
        if !matches!(per_source, (1, 1) | (2, 1) | (1, 2)) {
            return Err("Sync: izvor mora imati isti, dvostruki ili upola manji fps od priče.".into());
        }
        let (num, den) = per_source;
        if total_frames == 0 {
            return Err("Playlist input je prazan".into());
        }
        let anchor = markers
            .iter()
            .copied()
            .filter(|frame| *frame <= playhead)
            .max()
            .ok_or("Nema prethodnog M markera za Sync/B-roll")?
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
            timebase: source.timebase,
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
        });
        Ok(preview)
    }

    /// A frame the window program confirmed, as a program frame (v5
    /// `absolute_program_frame_from_playback`). Reaching the end of the source
    /// closes the slot (v5 `should_auto_finish`). Outside a Sync play the frame
    /// is returned as it is.
    pub fn program_frame(&mut self, window_frame: Option<u64>) -> Option<u64> {
        let Some(session) = self.active.as_mut() else {
            return window_frame;
        };
        let frame = window_frame?;
        let wrapped = frame < session.last;
        session.last = frame;
        let program = (session.anchor + frame).min(session.auto_finish);
        let (num, den) = session.per_source;
        self.source_frame = Some(session.source.source_in + (program - session.anchor) * den / num);
        if program + 1 >= session.auto_finish || wrapped {
            let end = session.auto_finish;
            // User rule: it stops there; Enter confirms the cover.
            let _ = self.finish(end);
            return Some(end);
        }
        Some(program)
    }

    /// O, or the end of the source: the slot from the anchor to `end` (v5
    /// `pending_slot`). The Sync play stops.
    pub fn finish(&mut self, end: u64) -> Result<&SyncSlot, String> {
        let session = self.active.take().ok_or("Sync play nije aktivan.")?;
        let start = session.anchor;
        let end = end.max(start + 1).min(session.total_frames.max(start + 1));
        let (num, den) = session.per_source;
        let source_out = (session.source.source_in + ((end - start) * den).div_ceil(num))
            .min(session.source.duration_frames);
        if source_out <= session.source.source_in {
            return Err("Sync OUT mora biti poslije Source IN".into());
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

    /// The cover write landed (or failed).
    pub fn landed(&mut self) {
        self.committing = false;
    }

    /// What the source timeline shows (v5 `sync_playhead_from_player_frame`):
    /// the source frame under the Sync play, played in parallel with the program,
    /// and once the slot is closed its source OUT with the IN/OUT of the slot.
    pub fn source_view(&self) -> Option<(u64, Option<(u64, u64)>)> {
        if let Some(session) = &self.active {
            let frame = self.source_frame.unwrap_or(session.source.source_in);
            return Some((frame, None));
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

    /// The user pointed elsewhere on the program: a running Sync play stops.
    pub fn cancel(&mut self) -> bool {
        self.active.take().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(source_in: u64, duration: u64) -> SyncSource {
        SyncSource {
            clip_id: "c".into(),
            clip_name: "Clip".into(),
            source_in,
            source_out: duration,
            duration_frames: duration,
            timebase: (50, 1),
        }
    }

    fn armed() -> SyncCover {
        let mut sync = SyncCover::new();
        sync.toggle();
        sync.arm(source(10, 40));
        sync
    }

    #[test]
    fn space_plays_from_the_marker_before_the_playhead_to_the_end_of_the_source() {
        let mut sync = armed();
        assert!(sync.wants_space());
        let preview = sync.start(&[0, 20, 60], 25, 100, (1, 1)).unwrap();
        assert_eq!(preview.window, (20, 50), "anchor 20, 30 source frames left");
        assert_eq!(preview.source_in, 10);
        assert!(!sync.wants_space(), "IN is used once (v5 set_active)");
    }

    #[test]
    fn sync_stops_at_the_source_out_or_the_next_marker_whichever_comes_first() {
        let mut sync = armed();
        let preview = sync.start(&[0, 20, 35], 25, 100, (1, 1)).unwrap();
        assert_eq!(preview.window, (20, 35), "the next marker closes it");
        let mut sync = SyncCover::new();
        sync.toggle();
        sync.arm(SyncSource {
            source_out: 18,
            ..source(10, 40)
        });
        let preview = sync.start(&[0, 20, 35], 25, 100, (1, 1)).unwrap();
        assert_eq!(preview.window, (20, 28), "the source OUT closes it");
    }

    #[test]
    fn sync_needs_a_marker_before_the_playhead_and_one_timebase() {
        let mut sync = armed();
        assert!(sync.start(&[30], 25, 100, (1, 1)).is_err());
        assert!(sync.start(&[0], 25, 100, (3, 1)).is_err());
        let mut off = SyncCover::new();
        off.arm(source(0, 10));
        assert!(!off.wants_space(), "IN arms only while Sync is on");
    }

    #[test]
    fn frames_follow_the_window_and_o_closes_the_slot() {
        let mut sync = armed();
        sync.start(&[0, 20], 25, 100, (1, 1)).unwrap();
        assert_eq!(
            sync.program_frame(Some(1)),
            Some(21),
            "the window may start on frame 1 (live player log)"
        );
        assert_eq!(sync.program_frame(Some(12)), Some(32));
        assert_eq!(
            sync.source_view(),
            Some((22, None)),
            "the source plays in parallel"
        );
        let slot = sync.finish(32).unwrap().clone();
        assert_eq!((slot.start, slot.end, slot.source_out), (20, 32, 22));
        assert_eq!(
            sync.source_view(),
            Some((22, Some((10, 22)))),
            "OUT and the slot marks"
        );
        assert_eq!(sync.missing_marker(&[0, 20]), Some(32));
        assert_eq!(sync.missing_marker(&[0, 20, 32]), None);
        assert_eq!(
            sync.resolve([("a|b", 0, 20), ("b|c", 20, 32)].into_iter()),
            Some("b|c".into())
        );
        assert!(sync.take_commit(false).is_none(), "O waits for Enter");
        let (slot_id, slot) = sync.take_commit(true).unwrap();
        assert_eq!(
            (slot_id.as_str(), slot.source.source_in, slot.source_out),
            ("b|c", 10, 22)
        );
        assert!(sync.holds_enter(), "until the write lands");
        sync.landed();
        assert!(!sync.holds_enter());
    }

    #[test]
    fn the_end_of_the_source_stops_and_waits_for_enter() {
        let mut sync = armed();
        sync.start(&[0, 20], 25, 100, (1, 1)).unwrap();
        assert_eq!(sync.program_frame(Some(29)), Some(50));
        assert!(!sync.is_active());
        sync.resolve([("b|c", 20, 50)].into_iter());
        assert!(
            sync.take_commit(false).is_none(),
            "it stops; Enter confirms"
        );
        assert!(sync.take_commit(true).is_some());
    }

    #[test]
    fn a_window_end_missed_between_repaints_still_closes_the_slot() {
        let mut sync = armed();
        sync.start(&[0, 20, 35], 25, 100, (1, 1)).unwrap();
        assert_eq!(sync.program_frame(Some(5)), Some(25));
        assert_eq!(
            sync.program_frame(Some(0)),
            Some(35),
            "the player went back to the window start: its end was reached"
        );
        assert!(!sync.is_active());
        assert!(
            sync.missing_marker(&[0, 20, 35]).is_none(),
            "the next marker closes it"
        );
    }

    #[test]
    fn a_source_of_half_or_twice_the_story_rate_counts_in_story_frames() {
        // XDCAM 1080i50 (25) in a 50p story: every field is a story frame.
        let mut sync = armed();
        let preview = sync.start(&[0, 20], 25, 200, (2, 1)).unwrap();
        assert_eq!(preview.window, (20, 80), "30 source frames are 60 story frames");
        assert_eq!(sync.program_frame(Some(11)), Some(31));
        assert_eq!(sync.source_view(), Some((15, None)));
        let slot = sync.finish(31).unwrap().clone();
        assert_eq!(slot.source_out, 16, "11 story frames need 6 source pictures");
        // 50p in a 25 story: every other picture.
        let mut sync = armed();
        let preview = sync.start(&[0, 20], 25, 200, (1, 2)).unwrap();
        assert_eq!(preview.window, (20, 35), "30 source frames are 15 story frames");
        let slot = sync.finish(30).unwrap().clone();
        assert_eq!(slot.source_out, 30);
    }

    #[test]
    fn switching_sync_off_drops_everything() {
        let mut sync = armed();
        sync.start(&[0], 5, 100, (1, 1)).unwrap();
        assert!(sync.toggle(), "a running play was stopped");
        assert!(!sync.enabled() && !sync.is_active() && !sync.holds_enter());
    }
}
