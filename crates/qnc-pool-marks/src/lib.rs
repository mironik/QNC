//! The marks of a media pool as a piece of a form (user 2026-10-23: "Ingest into blocks",
//! way B): the tick of one clip, all visible clips ticked or none, and the chosen pool tab.
//! A tick is only a mark of the view: no thread, no database; the database sees the
//! selection when the import starts. The action ids are the form's, given at composition.

use qnc_form_pieces::{Answer, Now, Piece};

pub const MODULE_ID: &str = "qnc.module.pool-marks";

/// The action ids of the form this piece takes.
#[derive(Debug, Clone)]
pub struct PoolMarks {
    pub toggle: &'static str,
    pub all: &'static str,
    pub clear: &'static str,
    /// The pool tabs: choosing one changes only the chosen tab (v5).
    pub tabs: &'static [&'static str],
}

impl Piece for PoolMarks {
    fn act(&mut self, action_id: &str, payload: Option<&str>, now: &mut Now<'_>) -> Option<Answer> {
        if self.tabs.contains(&action_id) {
            *now.tab = Some(action_id.to_string());
            return Some(Answer::changed());
        }
        let (ids, selected): (Vec<String>, bool) = match action_id {
            id if id == self.toggle => {
                let Some(clip_id) = payload else { return Some(Answer::refused(qnc_ui_text::text("ingest.msg.missing_clip_id"))) };
                let Some(clip) = now.clips.iter().find(|clip| clip.clip_id == clip_id) else {
                    return Some(Answer::refused(qnc_ui_text::text("ingest.msg.clip_not_found")));
                };
                (vec![clip.clip_id.clone()], !clip.selected)
            }
            id if id == self.all || id == self.clear => (now.visible().map(|clip| clip.clip_id.clone()).collect(), id == self.all),
            _ => return None,
        };
        for clip in now.clips.iter_mut().filter(|clip| ids.contains(&clip.clip_id)) {
            clip.selected = selected;
        }
        Some(Answer::changed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_clip_row::ClipRow;

    const MARKS: PoolMarks = PoolMarks { toggle: "toggle", all: "all", clear: "clear", tabs: &["tab_all", "tab_sub"] };

    #[test]
    fn a_tick_marks_one_clip_and_all_or_none_marks_the_visible_ones() {
        let row = |id: &str, seen| ClipRow { clip_id: id.into(), previously_seen: seen, ..Default::default() };
        let (mut clips, mut tab) = (vec![row("a", false), row("b", true)], None);
        let mut marks = MARKS;
        let mut now = Now { clips: &mut clips, tab: &mut tab, only_new: true, chosen: None, marks: None, project: None };
        assert!(marks.act("toggle", Some("b"), &mut now).is_some_and(|answer| !answer.refused));
        assert!(marks.act("toggle", Some("x"), &mut now).is_some_and(|answer| answer.refused));
        marks.act("all", None, &mut now);
        assert!(now.clips.iter().all(|clip| clip.selected), "b was ticked, a is the visible one");
        marks.act("clear", None, &mut now);
        assert_eq!(now.clips.iter().map(|clip| clip.selected).collect::<Vec<_>>(), [false, true], "only the visible one");
        marks.act("tab_sub", None, &mut now);
        assert_eq!(now.tab.as_deref(), Some("tab_sub"));
        assert!(marks.act("other", None, &mut now).is_none());
    }
}
