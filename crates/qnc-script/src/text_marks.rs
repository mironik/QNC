//! The M markers in the text of an OFF (user 2026-11-08: "setting the M markers for cutting the
//! B-roll in the text of the OFF"). A right click between two words (right click is Ctrl+) sets one
//! there, on one that is there takes it; Delete removes the one taken, Esc lets it go.

use qnc_source_preview::SourcePreview;

use crate::Script;

/// A right click at a place of a text (target: `block:word`, the word the place is before).
pub const MARK_AT: &str = "script_mark_at";

impl Script {
    pub(crate) fn mark_key(&mut self, action_id: &str, target: Option<&str>, preview: &SourcePreview) -> Option<bool> {
        match action_id {
            MARK_AT => {
                let (id, word) = target?.split_once(':').and_then(|(id, word)| Some((id.parse::<i64>().ok()?, word.parse::<u32>().ok()?)))?;
                let mut marks = self.blocks.iter().find(|block| block.id == id)?.marks.clone();
                if marks.contains(&word) {
                    self.view.taken_mark = Some((id, word));
                    return Some(true);
                }
                marks.push(word);
                self.view.taken_mark = None;
                Some(self.write(preview, move |target| qnc_script_db::set_marks(target, id, marks).map(|_| None)))
            }
            "delete_marker" => {
                let (id, word) = self.view.taken_mark.take()?;
                let marks: Vec<u32> = self.blocks.iter().find(|block| block.id == id)?.marks.iter().copied().filter(|mark| *mark != word).collect();
                Some(self.write(preview, move |target| qnc_script_db::set_marks(target, id, marks).map(|_| None)))
            }
            "clear_focus" => self.view.taken_mark.take().map(|_| true),
            _ => None,
        }
    }
}
