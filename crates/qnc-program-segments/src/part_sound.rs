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
                .rows
                .iter()
                .find(|row| row.start_frame <= frame && frame < row.end_frame)
                .or_else(|| self.view.rows.iter().find(|row| row.selected))
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
        // The channel count of the clip from its stored wave (one lane per channel).
        let channels = self.waves.channels(&clip_id);
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

    /// Up/down on the taken part: its draft channel.
    pub fn part_sound_draft(&mut self, delta: i64) -> bool {
        let moved = self.part_sound.draft(delta);
        self.refresh_part_sound();
        moved
    }

    /// Left (`off`) or right on the taken part.
    pub fn part_sound_off(&mut self, off: bool) -> bool {
        let changed = self.part_sound.set_off(off);
        self.refresh_part_sound();
        changed
    }

    /// Ctrl+left/right on the taken part: its level.
    pub fn part_sound_level(&mut self, delta_db: i8) -> bool {
        let changed = self.part_sound.nudge_level(delta_db);
        self.refresh_part_sound();
        changed
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

    /// The taken part and its draft in the view and the status, in the words of a lane.
    fn refresh_part_sound(&mut self) {
        let Some((part, draft)) = self.part_sound.taken() else {
            return;
        };
        let name = if part.lane == 0 { "A1" } else { "A2" };
        let message = if draft.off {
            qnc_ui_text::fill("story.msg.lane_off", &[("lane", &name.to_string())])
        } else {
            qnc_ui_text::fill(
                "story.msg.lane_level",
                &[("lane", &name.to_string()), ("channel", &(draft.channel + 1).to_string()), ("level", &format!("{:+}", draft.level_db))],
            )
        };
        self.refresh_view(message);
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
