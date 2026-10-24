//! The sound of a stored segment (A1) or cover (A2) changed on the Wrap row (user
//! 2026-10-16): the part under the Wrap playhead (else the chosen one) is taken with the
//! same keys as a lane on the source timeline. The state is the piece `qnc-part-sound`;
//! here the program model finds the part, tells the user and writes the change.

use qnc_part_sound::{Lane, Part};
use qnc_program_db::Operation;
use qnc_program_view::PartSoundView;

use crate::ProgramSegments;

impl ProgramSegments {
    /// Ctrl+1 (lane 0: the segment) or Ctrl+2 (lane 1: its cover) on the segment panel,
    /// at the program frame `frame`.
    pub fn take_part_sound(&mut self, lane: u8, frame: u64) -> bool {
        let found = if lane == 0 {
            self.view
                .user_segment_at(frame)
                .or_else(|| self.view.rows.iter().find(|row| !row.lead && row.selected))
                .map(|row| {
                    let kept = Lane { channel: row.a1_source_channel, off: row.a1_off, level_db: row.a1_level_db };
                    (row.segment_id.clone(), row.clip_id.clone(), kept)
                })
        } else {
            self.view
                .covers
                .iter()
                .find(|cover| cover.start_frame <= frame && frame < cover.end_frame)
                .or_else(|| self.view.covers.iter().find(|cover| cover.selected))
                .map(|cover| {
                    let kept = Lane { channel: cover.a2_source_channel, off: cover.a2_off, level_db: cover.a2_level_db };
                    (cover.cover_id.clone(), cover.clip_id.clone(), kept)
                })
        };
        let Some((id, clip_id, kept)) = found else {
            let key = if lane == 0 { "story.msg.no_segment_at_playhead" } else { "story.msg.no_cover_at_playhead" };
            self.refresh_view(qnc_ui_text::text(key).into());
            return false;
        };
        // The channel count of the clip from its stored media record (one lane per channel);
        // not read yet (the record was not final), it is asked again now.
        // Another part's sound on the same lane taken by its slot field: the one taken is kept
        // first, as Enter (user 2026-10-22: the A2 zoom's strips switch from cover to cover).
        if self.part_sound.taken().is_some_and(|(part, _)| part.lane == lane && part.id != id) {
            self.commit_part_sound();
        }
        if self.refused(crate::confirm::Take::Sound(lane, id.clone())) {
            return false;
        }
        let channels = self.clip_channels.get(&clip_id).copied().flatten();
        if channels.is_none() {
            self.reload();
        }
        self.let_go_edits(true);
        if !self.part_sound.take(Part { lane, id }, kept, channels) {
            self.refresh_view(qnc_ui_text::text("story.msg.no_audio_channels").into());
            return false;
        }
        self.refresh_part_sound();
        true
    }

    pub fn part_sound_taken(&self) -> bool {
        self.part_sound.taken().is_some()
    }

    /// The taken part (lane, id) and its stored sound (as it was taken).
    pub fn part_sound_stored(&self) -> Option<(u8, String, Lane)> {
        let (part, _) = self.part_sound.taken()?;
        Some((part.lane, part.id.clone(), self.part_sound.kept()?))
    }

    /// The taken part (lane, id) and its draft, which the program hears before Enter
    /// (user 2026-10-16).
    pub fn part_sound_draft_now(&self) -> Option<(u8, String, Lane)> {
        let (part, draft) = self.part_sound.taken()?;
        Some((part.lane, part.id.clone(), draft))
    }

    /// The lanes of the taken part, for the keys every timeline gives its lanes
    /// (`qnc_timeline::lane_key`); the caller says what it did with [`Self::say`].
    pub fn part_sound_lanes(&mut self) -> Option<&mut qnc_lane_choice::LaneChoice> {
        self.part_sound.lanes_mut()
    }

    /// Enter: the change is written (one UNDO step); the program plays it once stored.
    pub(crate) fn commit_part_sound(&mut self) -> bool {
        let Some(change) = self.part_sound.commit() else {
            return false;
        };
        let (channel, level_db, off) = (change.sound.channel, change.sound.level_db, change.sound.off);
        let operation = if change.part.lane == 0 {
            Operation::SetSegmentSound { segment_id: change.part.id, channel, level_db, off }
        } else {
            Operation::SetCoverSound { cover_id: change.part.id, channel, level_db, off }
        };
        self.write(operation);
        self.refresh_view(String::new());
        true
    }

    /// Escape: the taken part is let go, nothing written.
    pub fn release_part_sound(&mut self) -> bool {
        let released = self.part_sound.release();
        if released {
            self.refresh_view(String::new());
        }
        released
    }

    /// The taken part and its draft in the view.
    fn refresh_part_sound(&mut self) {
        if self.part_sound.taken().is_some() {
            self.refresh_view(self.view.message.clone());
        }
    }

    /// What the view shows of the taken part.
    pub(crate) fn part_sound_view(&self) -> Option<PartSoundView> {
        let (part, draft) = self.part_sound.taken()?;
        Some(PartSoundView {
            lane: part.lane,
            id: part.id.clone(),
            channel: draft.channel,
            channels: self.part_sound.channels()?,
            off: draft.off,
            level_db: draft.level_db,
        })
    }
}
