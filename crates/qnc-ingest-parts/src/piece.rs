//! The parts as a piece of a form (way B of 2026-10-23): Add SubClip saves the IN/OUT the
//! source timeline shows as a part of the clip on the preview, the parts tab shows the
//! parts as cards (the poster of their clip), a click on one opens its clip with its IN/OUT
//! and Delete removes the chosen one. The action ids are the form's, given at composition.

use crate::IngestParts;
use qnc_clip_row::ClipRow;
use qnc_form_pieces::{Answer, Now, Piece};

/// The action ids of the form the parts piece takes.
#[derive(Debug, Clone)]
pub struct PartActions {
    pub save: &'static str,
    pub delete: &'static str,
    /// A click on a card (its payload is the card id).
    pub open: &'static str,
    /// The pool tab that shows the parts.
    pub tab: &'static str,
}

#[derive(Debug)]
pub struct PartsPiece {
    parts: IngestParts,
    actions: PartActions,
    chosen: Option<String>,
}

impl PartsPiece {
    pub fn new(actions: PartActions) -> Self {
        Self { parts: IngestParts::default(), actions, chosen: None }
    }
}

impl Piece for PartsPiece {
    fn act(&mut self, action_id: &str, payload: Option<&str>, now: &mut Now<'_>) -> Option<Answer> {
        let actions = &self.actions;
        if action_id == actions.save {
            let clip = now.chosen.and_then(|id| now.clips.iter().find(|clip| clip.clip_id == id));
            let (Some(clip), Some(marks)) = (clip, now.marks) else {
                return Some(Answer::refused(qnc_ui_text::text("ingest.msg.subclip_needs_clip")));
            };
            return Some(match self.parts.save(&clip.clip_id, &clip.name, marks) {
                Ok(()) => {
                    *now.tab = Some(actions.tab.to_string());
                    Answer::said(qnc_ui_text::text("ingest.msg.subclip_saved"))
                }
                Err(error) => Answer::refused(error),
            });
        }
        if action_id == actions.delete && now.tab.as_deref() == Some(actions.tab) {
            let part = self.chosen.take()?;
            return Some(self.parts.delete(&part).map_or_else(Answer::refused, |()| Answer::changed()));
        }
        if action_id == actions.open {
            let part = self.parts.parts().iter().find(|part| Some(part.part_id.as_str()) == payload)?;
            self.chosen = Some(part.part_id.clone());
            return Some(Answer { open: Some((part.clip_id.clone(), (part.in_frame, part.out_frame))), ..Answer::changed() });
        }
        None
    }

    fn poll(&mut self, now: &mut Now<'_>) -> Answer {
        self.parts.configure(now.project);
        let changed = self.parts.poll();
        Answer { changed, message: self.parts.take_message(), ..Answer::default() }
    }

    /// The parts as cards: the name and length of the part, the poster of its clip.
    fn rows(&self, tab: &str, clips: &[ClipRow]) -> Option<Vec<ClipRow>> {
        (tab == self.actions.tab).then(|| {
            let rows = self.parts.parts().iter().filter_map(|part| {
                let clip = clips.iter().find(|clip| clip.clip_id == part.clip_id)?;
                let frames = part.out_frame - part.in_frame;
                let seconds = clip.duration_seconds * frames as f64 / clip.duration_frames.max(1) as f64;
                Some(ClipRow {
                    clip_id: part.part_id.clone(),
                    name: part.name.clone(),
                    duration_seconds: seconds,
                    duration_frames: frames,
                    selected: false,
                    ..clip.clone()
                })
            });
            rows.collect()
        })
    }
}
