//! The sound assistant on the SubSegment (user 2026-11-07: "the AI sound assistant must be
//! universal, whatever the form"; there are no covers, so it evens out the speech of the parts):
//! while it is on, every part without a curve of the user's plays the curve `qnc-sound-assist`
//! proposes from the loudness measured with the waves, heard, drawn and in the wave as the parts'
//! own curves are. AI turns it on and off, KEEP writes the proposal as the user's curves.

use qnc_script_db::{Block, Piece};
use qnc_source_preview::SourcePreview;

use crate::{view, SubsegmentEdit};

/// What the proposal was made of: the length and rate, the waves read and every placement (with
/// whether it has a curve of its own).
pub(crate) type AssistKey = (u64, (u32, u32), u64, Vec<(qnc_program_waveform::Placement, bool)>);

impl SubsegmentEdit {
    /// AI on: the SubSegment's parts are evened out (`None` while no SubSegment is open).
    pub fn sound_assist_on(&self) -> Option<bool> {
        self.open_subsegment.map(|_| self.assist)
    }

    /// The assistant's curves of `block` (the stored parts; a part with a curve of its own, or
    /// whose curve is taken, gets none): the part and its points on its clip's frames.
    pub(crate) fn propose(&mut self, block: &Block) -> Vec<(usize, Vec<(u64, i16)>)> {
        if !self.assist {
            return Vec::new();
        }
        let pieces = view::block_pieces(block);
        let shown = view::subsegment_view(block, None);
        let placements = view::subsegment_placements(&shown, &pieces);
        let taken = self.level.as_ref().and_then(|line| line.id.parse::<usize>().ok());
        let items: Vec<qnc_sound_assist::Item> = placements
            .iter()
            .enumerate()
            .map(|(index, at)| qnc_sound_assist::Item { lane: 0, at: at.clone(), own_curve: !at.curve.is_empty() || taken == Some(index) })
            .collect();
        let rate = pieces.first().and_then(|piece| self.names.get(&piece.clip_id)).filter(|(_, frames, seconds)| *seconds > 0.0 && *frames > 0).map_or((25, 1), |(_, frames, seconds)| ((*frames as f64 / seconds).round() as u32, 1));
        let key: AssistKey = (shown.total_frames, rate, self.waves.revision(), items.iter().map(|item| (item.at.clone(), item.own_curve)).collect());
        if let Some((kept, curves)) = &self.assisted {
            if *kept == key {
                return curves.clone();
            }
        }
        let waves = &self.waves;
        let proposals = qnc_sound_assist::propose(shown.total_frames, rate, &items, |id| {
            waves.clip_loudness(id).map(|(channels, duration_frames)| qnc_program_loudness::ClipMeasure { channels, duration_frames })
        });
        // The placements play their parts' own clip frames, so the points are the parts' frames.
        let curves: Vec<(usize, Vec<(u64, i16)>)> = proposals.into_iter().map(|proposal| (proposal.index, proposal.points)).collect();
        self.assisted = Some((key, curves.clone()));
        curves
    }

    /// AI (Shift+N) and KEEP (Ctrl+Shift+N) while a SubSegment is open; `None` for any other key.
    pub(crate) fn assist_key(&mut self, action_id: &str, block: &Block, pieces: &[Piece], preview: &SourcePreview) -> Option<bool> {
        match action_id {
            "toggle_sound_assist" => {
                self.assist = !self.assist;
                let key = if self.assist { "story.msg.sound_assist_on" } else { "story.msg.sound_assist_off" };
                self.view.message = qnc_ui_text::text(key).into();
                Some(true)
            }
            "sound_assist_keep" => {
                if !self.assist {
                    self.view.message = qnc_ui_text::text("story.msg.sound_assist_keep_off").into();
                    return Some(false);
                }
                let curves = std::mem::take(&mut self.assist_curves);
                if curves.is_empty() {
                    self.view.message = qnc_ui_text::text("story.msg.sound_assist_keep_none").into();
                    return Some(false);
                }
                let count = curves.len();
                let written: Vec<(usize, Piece)> = curves
                    .into_iter()
                    .filter_map(|(index, keys)| {
                        let mut piece = pieces.get(index)?.clone();
                        piece.level_keys = keys;
                        Some((index, piece))
                    })
                    .collect();
                self.assisted = None;
                let id = block.id;
                // The whole proposal is one write and one step of undo (audit 2026-11-08 #15).
                let changes = written.into_iter().map(|(index, piece)| qnc_script_db::Operation::SetPiece { id, index, piece }).collect();
                self.write(preview, move |target| qnc_script_db::together(target, changes).map(|_| None));
                self.view.message = qnc_ui_text::fill("story.msg.sound_assist_kept", &[("count", &count.to_string())]).into();
                Some(true)
            }
            _ => None,
        }
    }
}
