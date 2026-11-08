//! Words picked in the text of a statement and cut out of its video (user 2026-11-05: "allow
//! deleting words in the text and cut that part of the video out of the timeline"; "that way
//! we take out the fillers"). A right click on a word or a letter takes it (user 2026-11-05: "a
//! right click is Ctrl+, in every action"), Shift+right click takes from the first one to it;
//! Delete cuts what is taken out of the part (what was
//! before them and what was after them stay, joined by a cut, user: "no crossfade"); Esc lets
//! them go. The picture and the sound go together: the part is the same clip, only shorter.

use crate::Script;
use qnc_script_db::Piece;
use qnc_source_preview::SourcePreview;

/// Shift+right click on a word or a letter of a statement: what is taken grows from the first
/// taken to it. `target` is `block:part:from:to`, as for a click.
pub const WORD_PICK: &str = "script_word_pick";
/// A right click on a word or a letter of a statement (user 2026-11-05: "a right click is
/// Ctrl+", the same in every action): takes it; Delete then cuts it out of the video.
pub const WORD_TAKE: &str = "script_word_take";
/// The edited text of a statement's part applied (user 2026-11-06: "what is deleted in the
/// text is deleted in the sound"): `target` is `block:part:text`.
pub const TEXT_CUT: &str = "script_text_cut";
/// The text of a statement's part typed (`target` is `block:part:text`): the script keeps it
/// as the draft until Enter or Esc (docs/99 step 4: not egui memory).
pub const TEXT_DRAFT: &str = "script_text_draft";
/// The edited text of a statement's part given back as said (Esc; `target` is `block:part`).
pub const TEXT_REVERT: &str = "script_text_revert";

/// Words picked in one part: clip frames `[from, to)` from the first word's start to the last
/// one's end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WordPick {
    pub block: i64,
    pub part: usize,
    pub from: u64,
    pub to: u64,
    /// The word the pick started on (Shift+click picks from it).
    anchor: (u64, u64),
}

/// `block:part:from:to` of a word.
fn parse(target: Option<&str>) -> Option<(i64, usize, u64, u64)> {
    let mut parts = target?.split(':');
    let id = parts.next()?.parse().ok()?;
    let index = parts.next()?.parse().ok()?;
    let from: u64 = parts.next()?.parse().ok()?;
    let to = parts.next().and_then(|to| to.parse().ok()).unwrap_or(from + 1);
    Some((id, index, from, to.max(from + 1)))
}

impl WordPick {
    /// One word picked.
    pub fn word(target: Option<&str>) -> Option<Self> {
        let (block, part, from, to) = parse(target)?;
        Some(Self { block, part, from, to, anchor: (from, to) })
    }

    /// The pick from its first word to the word `target` (a word of another part starts anew).
    pub fn grown(self, target: Option<&str>) -> Option<Self> {
        let (block, part, from, to) = parse(target)?;
        if (block, part) != (self.block, self.part) {
            return Self::word(target);
        }
        let (first, last) = (self.anchor.0.min(from), self.anchor.1.max(to));
        Some(Self { from: first, to: last, ..self })
    }
}

/// What is left of `piece` when `[from, to)` is cut out of it: before and after (clip frames).
pub fn left_of(piece: &Piece, from: u64, to: u64) -> (Option<(u64, u64)>, Option<(u64, u64)>) {
    let (from, to) = (from.max(piece.from_frame), to.min(piece.to_frame));
    if to <= from {
        return (Some((piece.from_frame, piece.to_frame)), None);
    }
    ((from > piece.from_frame).then_some((piece.from_frame, from)), (to < piece.to_frame).then_some((to, piece.to_frame)))
}

impl Script {
    /// What is taken in the text: a right click takes a word or a letter (Ctrl+), Shift+right
    /// click takes up to it, Delete cuts it out, Esc lets it go. A left click only cues (the
    /// playhead). `None` when the action is not about what is taken.
    pub(crate) fn pick_key(&mut self, action_id: &str, target: Option<&str>, preview: &SourcePreview) -> Option<bool> {
        if matches!(action_id, WORD_TAKE | WORD_PICK | "delete_marker" | "clear_focus" | "undo_object" | "redo_object") && qnc_dev_diagnostics::player_diagnostics_enabled() {
            qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("script-words action={action_id} target={target:?} taken={:?}", self.word_pick.map(|pick| (pick.block, pick.part, pick.from, pick.to))));
        }
        match action_id {
            WORD_TAKE => {
                self.word_pick = WordPick::word(target);
                self.view.picked = self.word_pick;
                Some(true)
            }
            WORD_PICK => {
                self.word_pick = match self.word_pick {
                    Some(pick) => pick.grown(target),
                    None => WordPick::word(target),
                };
                self.view.picked = self.word_pick;
                Some(true)
            }
            "clear_focus" if self.word_pick.is_some() => {
                (self.word_pick, self.view.picked) = (None, None);
                Some(true)
            }
            // Backspace (and Ctrl+Z, the Undo button) takes back the last change of the script,
            // Shift+Backspace takes it again (user 2026-11-06: "undo with Backspace does not work").
            "undo_object" | "redo_object" => {
                let undo = action_id == "undo_object";
                (self.word_pick, self.view.picked) = (None, None);
                self.edit.let_go();
                self.message_after_write = Some(qnc_ui_text::text(if undo { "script.undone" } else { "script.redone" }).into());
                Some(self.write(preview, move |target| {
                    let done = if undo { qnc_script_db::undo(target)? } else { qnc_script_db::redo(target)? };
                    if done { Ok(None) } else { Err(qnc_ui_text::text(if undo { "script.nothing_to_undo" } else { "script.nothing_to_redo" }).to_string()) }
                }))
            }
            TEXT_DRAFT => {
                let mut parts = target?.splitn(3, ':');
                let (id, index) = (parts.next()?.parse::<i64>().ok()?, parts.next()?.parse::<usize>().ok()?);
                let edited = parts.next().unwrap_or_default().to_string();
                let said = crate::text_of(self.view.blocks.iter().find(|row| row.block.id == id).and_then(|row| row.words.get(index)).map_or(&[][..], Vec::as_slice));
                if edited == said {
                    self.view.text_drafts.remove(&(id, index));
                } else {
                    self.view.text_drafts.insert((id, index), (said, edited));
                }
                Some(true)
            }
            TEXT_REVERT => {
                let mut parts = target?.splitn(2, ':');
                let (id, index) = (parts.next()?.parse::<i64>().ok()?, parts.next()?.parse::<usize>().ok()?);
                self.view.text_drafts.remove(&(id, index));
                Some(true)
            }
            TEXT_CUT => {
                let mut parts = target?.splitn(3, ':');
                let (id, index) = (parts.next()?.parse::<i64>().ok()?, parts.next()?.parse::<usize>().ok()?);
                let edited = parts.next().unwrap_or_default().to_string();
                self.view.text_drafts.remove(&(id, index));
                (self.word_pick, self.view.picked) = (None, None);
                Some(self.text_cut(id, index, &edited, preview))
            }
            "delete_marker" => {
                let pick = self.word_pick?;
                (self.word_pick, self.view.picked) = (None, None);
                Some(self.cut_out(pick, preview))
            }
            _ => None,
        }
    }

    /// The picked words cut out of their part: what was before and after them, each with its
    /// own words, timecodes and length.
    fn cut_out(&mut self, pick: WordPick, preview: &SourcePreview) -> bool {
        let Some(block) = self.block(Some(pick.block)).cloned() else { return true };
        let pieces = crate::view::block_pieces(&block);
        let Some(piece) = pieces.get(pick.part).cloned() else { return true };
        let (before, after) = left_of(&piece, pick.from, pick.to);
        if before == Some((piece.from_frame, piece.to_frame)) {
            return true; // nothing of the part picked
        }
        if before.is_none() && after.is_none() && pieces.len() < 2 {
            self.view.message = qnc_ui_text::text("db.script_last_piece").into();
            return true;
        }
        let words = self.view.blocks.iter().find(|row| row.block.id == pick.block).and_then(|row| row.words.get(pick.part)).cloned().unwrap_or_default();
        let said = |from: u64, to: u64| words.iter().filter(|word| (from..to).contains(&((word.from + word.to.max(word.from + 1)) / 2))).map(|word| word.text.as_str()).collect::<Vec<_>>().join(" ");
        let part = |range: Option<(u64, u64)>| {
            range.map(|(from, to)| Piece { text: said(from, to), ..qnc_subsegment_edit::trimmed_piece(&self.names, &piece, from, to) })
        };
        let (before, after) = (part(before), part(after));
        let (id, index) = (pick.block, pick.part);
        self.edit.let_go();
        self.message_after_write = Some(qnc_ui_text::text("script.words_cut_out").into());
        self.write(preview, move |target| qnc_script_db::cut_out(target, id, index, before, after).map(|_| None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(from: u64, to: u64) -> Piece {
        Piece { clip_id: "c".into(), clip_name: "c".into(), from_frame: from, to_frame: to, start_tc: String::new(), end_tc: String::new(), length: String::new(), seconds: 0.0, text: String::new(), gap_before: 0, cut: false, a1_channel: None, a1_level_db: 0, a1_off: false, a1_lead_frames: 0, a1_tail_frames: 0, v_lead_frames: 0, v_tail_frames: 0, level_keys: Vec::new(), smooth_frames: 0, smooth_clip: String::new() }
    }

    #[test]
    fn a_pick_grows_from_its_first_word_whichever_way_and_cuts_out_its_frames() {
        let pick = WordPick::word(Some("4:0:7449:7452")).unwrap();
        let grown = pick.grown(Some("4:0:7441:7449")).unwrap();
        assert_eq!((grown.from, grown.to), (7441, 7452), "back from the first word");
        let grown = grown.grown(Some("4:0:7465:7466")).unwrap();
        assert_eq!((grown.from, grown.to), (7449, 7466), "from the first word, not from the earlier pick");
        assert_eq!(grown.grown(Some("4:1:10:12")).map(|p| (p.part, p.from)), Some((1, 10)), "another part starts anew");
        let piece = piece(7400, 7500);
        assert_eq!(left_of(&piece, 7449, 7466), (Some((7400, 7449)), Some((7466, 7500))));
        assert_eq!(left_of(&piece, 7390, 7420), (None, Some((7420, 7500))), "at its start");
        assert_eq!(left_of(&piece, 7480, 7600), (Some((7400, 7480)), None), "at its end");
        assert_eq!(left_of(&piece, 7400, 7500), (None, None), "all of it");
    }
}

impl Script {
    /// Part `index` of block `id` keeps what its edited text still says (`text_edit`).
    fn text_cut(&mut self, id: i64, index: usize, edited: &str, preview: &SourcePreview) -> bool {
        let Some(block) = self.block(Some(id)).cloned() else { return true };
        let pieces = crate::view::block_pieces(&block);
        let Some(piece) = pieces.get(index).cloned() else { return true };
        let words = self.view.blocks.iter().find(|row| row.block.id == id).and_then(|row| row.words.get(index)).cloned().unwrap_or_default();
        if words.is_empty() {
            return true;
        }
        let kept = crate::text_edit::kept_ranges(&words, edited, (piece.from_frame, piece.to_frame));
        if kept == [(piece.from_frame, piece.to_frame)] {
            return true; // nothing said is gone
        }
        if kept.is_empty() && pieces.len() < 2 {
            self.view.message = qnc_ui_text::text("db.script_last_piece").into();
            return true;
        }
        let said = |from: u64, to: u64| words.iter().filter(|word| (from..to).contains(&((word.from + word.to.max(word.from + 1)) / 2))).map(|word| word.text.as_str()).collect::<Vec<_>>().join(" ");
        let kept: Vec<Piece> = kept
            .into_iter()
            .map(|(from, to)| Piece { text: said(from, to), ..qnc_subsegment_edit::trimmed_piece(&self.names, &piece, from, to) })
            .collect();
        self.edit.let_go();
        self.message_after_write = Some(qnc_ui_text::text("script.words_cut_out").into());
        self.write(preview, move |target| qnc_script_db::keep_of(target, id, index, kept).map(|_| None))
    }
}
