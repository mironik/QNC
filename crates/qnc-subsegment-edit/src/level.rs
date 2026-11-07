//! The level curve of a part's A1 on the SubSegment's timeline (user 2026-11-07: "the level line
//! and the AI sound assistant must be universal, whatever the form"), the same tool as a
//! segment's on a Wrap row: Ctrl+Shift+1 or a right click on the line takes it, the line of the
//! playhead is the point controlled (up/down, Ctrl+arrows and Ctrl+wheel a dB, Delete removes
//! it), every step is heard at once, Enter writes it, a click writes it and places the playhead
//! (the curve stays taken), Esc gives it up. The curve and its draft are the piece
//! `qnc-level-curve`; here the part is found and its curve written with the part.

use qnc_level_curve::{LevelLine, Span};
use qnc_program_db::LevelKey;
use qnc_script_db::{Block, Piece};
use qnc_source_preview::SourcePreview;

use crate::{view, SubsegmentEdit};

/// A click on the timeline while the curve is taken (its frame): the curve is written and stays
/// taken, the playhead goes there (the next point controlled).
pub const LEVEL_KEEP_AT: &str = "subsegment_level_keep_at";

/// The curve of a part as the story keeps it.
pub(crate) fn keys_of(piece: &Piece) -> Vec<LevelKey> {
    piece.level_keys.iter().map(|(frame, db10)| LevelKey { frame: *frame, db10: *db10 }).collect()
}

impl SubsegmentEdit {
    pub(crate) fn level_taken(&self) -> bool {
        self.level.is_some()
    }

    /// Where part `index` lies on the SubSegment's axis and the frames of its clip it plays.
    fn span_of(&self, index: usize, piece: &Piece) -> Option<Span> {
        let part = self.view.subsegment.as_ref()?.parts.get(index)?;
        Some(Span { start: part.start, end: part.end, source_in: piece.from_frame, source_out: piece.to_frame })
    }

    /// The level keys; `None` for a key that is not the curve's.
    pub(crate) fn level_key(&mut self, action_id: &str, target: Option<&str>, block: &Block, pieces: &[Piece], preview: &SourcePreview) -> Option<bool> {
        let at = self.view.subsegment.as_ref().and_then(|view| view.playhead).unwrap_or(0);
        if action_id == "select_level_a1" {
            // The part the right click was on, else the chosen slot, else the part in hand.
            let index = target.and_then(|index| index.parse::<usize>().ok()).filter(|index| *index < pieces.len()).unwrap_or_else(|| self.part_in_hand());
            let piece = pieces.get(index)?;
            let span = self.span_of(index, piece)?;
            (self.trim, self.sound, self.join_sound, self.slide, self.slip_part) = (None, None, None, None, None);
            self.level = Some(LevelLine::new(0, index.to_string(), span, keys_of(piece)));
            self.say_level(at);
            return Some(true);
        }
        let line = self.level.as_mut()?;
        let changed = match action_id {
            "step_prev_part" => line.nudge(at, 1),
            "step_next_part" => line.nudge(at, -1),
            "delete_marker" => line.remove_at(at),
            "activate_focused_item" => return Some(self.keep_level(block, pieces, preview, None)),
            LEVEL_KEEP_AT => {
                let frame = target.and_then(|frame| frame.parse::<u64>().ok());
                return Some(self.keep_level(block, pieces, preview, frame));
            }
            "clear_focus" => {
                self.level = None;
                self.pieces_request = Some((view::piece_inputs(block), at));
                self.view.message.clear();
                return Some(true);
            }
            key => match qnc_lane_choice::LaneKey::of(key) {
                Some(qnc_lane_choice::LaneKey::Level(steps)) => line.nudge(at, i16::from(steps)),
                _ => return None,
            },
        };
        if changed {
            self.pieces_request = Some((view::piece_inputs(&self.drafted(block.clone())), at));
        }
        self.say_level(at);
        Some(true)
    }

    /// Enter (`at` none: the curve is let go) or a click at `at` (it stays taken, the playhead
    /// goes there): the drafted curve is written with its part.
    fn keep_level(&mut self, block: &Block, pieces: &[Piece], preview: &SourcePreview, at: Option<u64>) -> bool {
        let Some(line) = self.level.as_mut() else { return false };
        let index = line.id.parse::<usize>().ok();
        let keys = line.keep();
        if at.is_none() {
            self.level = None;
        }
        let playhead = at.or_else(|| self.view.subsegment.as_ref().and_then(|view| view.playhead)).unwrap_or(0);
        let written = index.zip(keys).and_then(|(index, keys)| {
            let keys = match LevelKey::checked(keys) {
                Ok(keys) => keys,
                Err(error) => {
                    self.view.message = error;
                    return None;
                }
            };
            let mut piece = pieces.get(index)?.clone();
            piece.level_keys = LevelKey::pairs(&keys);
            Some((index, piece))
        });
        match written {
            Some((index, piece)) => {
                let id = block.id;
                self.write(preview, move |target| qnc_script_db::set_piece(target, id, index, piece).map(|_| None));
            }
            None => self.pieces_request = Some((view::piece_inputs(&self.drafted(block.clone())), playhead)),
        }
        if at.is_some() {
            self.pieces_request = Some((view::piece_inputs(&self.drafted(block.clone())), playhead));
            self.say_level(playhead);
        }
        true
    }

    /// The status: the level of the curve on the playhead line and whether a point is there.
    fn say_level(&mut self, frame: u64) {
        let Some(line) = &self.level else { return };
        let level = format!("{:+.1}", f64::from(line.level_at(frame)) / 10.0);
        let key = if line.point_at(frame).is_some() { "story.msg.level_point" } else { "story.msg.level_line" };
        self.view.message = qnc_ui_text::fill(key, &[("lane", "A1"), ("level", &level)]).into();
    }

    /// The curves of the parts on the axis (the taken one as drafted), for drawing.
    pub(crate) fn level_curves(&self, shown: &view::SubsegmentView, pieces: &[Piece]) -> Vec<qnc_program_view::LevelCurve> {
        shown
            .parts
            .iter()
            .zip(pieces)
            .enumerate()
            .filter_map(|(index, (part, piece))| {
                let span = Span { start: part.start, end: part.end, source_in: piece.from_frame, source_out: piece.to_frame };
                let taken = self.level.as_ref().filter(|line| line.id == index.to_string());
                // The taken curve as drafted, else the part's own, else the assistant's (AI on).
                let assisted = || self.assist_curves.iter().find(|(at, _)| *at == index).map(|(_, keys)| keys.iter().map(|(frame, db10)| LevelKey { frame: *frame, db10: *db10 }).collect());
                let keys = match taken {
                    Some(line) => line.draft().to_vec(),
                    None if !piece.level_keys.is_empty() => keys_of(piece),
                    None => assisted().unwrap_or_default(),
                };
                // A part without a curve draws its flat line (0 dB), so a right click can take it.
                Some(qnc_program_view::LevelCurve { lane: 0, id: index.to_string(), span, placed: qnc_level_curve::place(span, &keys), taken: taken.is_some() })
            })
            .collect()
    }

    /// The drafted curve of the taken part, as the program plays it before Enter.
    pub(crate) fn level_draft(&self) -> Option<(usize, Vec<(u64, i16)>)> {
        let line = self.level.as_ref()?;
        Some((line.id.parse().ok()?, LevelKey::pairs(line.draft())))
    }
}
