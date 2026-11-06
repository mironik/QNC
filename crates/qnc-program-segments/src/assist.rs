//! The sound assistant on the Wrap rows (user 2026-11-06): its curves (`qnc-sound-assist`) for
//! the segments and covers without a curve of the user's, from the loudness read with the
//! waves, drawn and heard in the wave as the player plays them.

use qnc_program_view::SegmentsView;
use qnc_program_waveform::Placement;

use crate::ProgramSegments;

impl ProgramSegments {
    /// The assistant's curves put on `view` and on the placements of its wave.
    pub(crate) fn assist(&mut self, view: &mut SegmentsView, segments: &mut [Placement], covers: &mut [Placement]) {
        let lanes = [segments.len(), covers.len()];
        let items: Vec<qnc_sound_assist::Item> = segments
            .iter()
            .zip(&view.rows)
            .map(|(at, row)| qnc_sound_assist::Item { lane: 0, at: at.clone(), own_curve: !at.curve.is_empty() || row.lead })
            .chain(covers.iter().zip(&view.covers).map(|(at, _)| qnc_sound_assist::Item { lane: 1, at: at.clone(), own_curve: !at.curve.is_empty() }))
            .collect();
        let rate = view.timebase.unwrap_or((0, 1));
        let key: AssistKey = (view.total_frames, rate, self.waves.revision(), items.iter().map(|item| (item.lane, item.at.clone(), item.own_curve)).collect());
        let proposals = match &self.assisted {
            Some((kept, proposals)) if *kept == key => proposals.clone(),
            _ => {
                let waves = &self.waves;
                let proposals = qnc_sound_assist::propose(view.total_frames, rate, &items, |id| {
                    waves.clip_loudness(id).map(|(channels, duration_frames)| qnc_program_loudness::ClipMeasure { channels, duration_frames })
                });
                self.assisted = Some((key, proposals.clone()));
                proposals
            }
        };
        self.assist_curves.clear();
        for proposal in proposals {
            let keys: Vec<qnc_program_db::LevelKey> = proposal.points.iter().map(|(frame, db10)| qnc_program_db::LevelKey { frame: *frame, db10: *db10 }).collect();
            let id = if proposal.index < lanes[0] {
                segments[proposal.index].curve = proposal.points.clone();
                view.rows[proposal.index].level_keys = keys.clone();
                view.rows[proposal.index].segment_id.clone()
            } else {
                let index = proposal.index - lanes[0];
                covers[index].curve = proposal.points.clone();
                view.covers[index].level_keys = keys.clone();
                view.covers[index].cover_id.clone()
            };
            self.assist_curves.push((id, keys));
        }
    }

    /// The assistant's proposal kept as the user's curves (user 2026-11-06), all at once, one Undo
    /// step: from then on they are edited as any curve and the assistant leaves those parts alone.
    pub fn keep_sound_assist(&mut self) -> bool {
        let (kept, message) = if !self.sound_assist {
            (false, qnc_ui_text::text("story.msg.sound_assist_keep_off").to_string())
        } else if self.assist_curves.is_empty() {
            (false, qnc_ui_text::text("story.msg.sound_assist_keep_none").to_string())
        } else {
            let count = self.assist_curves.len();
            let curves = std::mem::take(&mut self.assist_curves);
            self.write(qnc_program_db::Operation::SetLevelKeysMany { curves });
            self.assisted = None;
            (true, qnc_ui_text::fill("story.msg.sound_assist_kept", &[("count", &count.to_string())]))
        };
        self.refresh_view(message);
        kept
    }
}

/// What the assistant's proposals were made of: the program's length and rate, the waves read
/// and every item (lane, placement, own curve).
pub(crate) type AssistKey = (u64, (u32, u32), u64, Vec<(u8, Placement, bool)>);
