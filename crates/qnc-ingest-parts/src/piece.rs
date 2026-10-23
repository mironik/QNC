//! The parts as a piece of a form (way B of 2026-10-23): Add SubClip saves the IN/OUT the
//! source timeline shows as a part of the clip on the preview, the parts tab shows the
//! parts as cards, ticked for Uvezi (user 2026-10-23: a new part is ticked, the user may
//! untick it); a part not cut yet shows the poster of its clip and opens its clip with its
//! IN/OUT, a part cut and imported is shown as its own clip of the parts card. Delete removes
//! the chosen part. The action ids are the form's, given at composition.

use crate::IngestParts;
use qnc_clip_row::ClipRow;
use qnc_form_pieces::{Answer, Now, Piece};
use std::{
    collections::{HashMap, HashSet},
    sync::{mpsc, Arc},
};

type Still = (String, Result<qnc_image_assets::RgbaImage, String>);

/// The action ids of the form the parts piece takes.
#[derive(Debug, Clone)]
pub struct PartActions {
    pub save: &'static str,
    pub delete: &'static str,
    /// A click on a card (its payload is the card id).
    pub open: &'static str,
    /// The pool tab that shows the parts.
    pub tab: &'static str,
    /// The tick of a card (its payload is the card id).
    pub toggle: &'static str,
    /// Uvezi: ready while a ticked part waits to be cut.
    pub import: &'static str,
}

pub struct PartsPiece {
    parts: IngestParts,
    actions: PartActions,
    chosen: Option<String>,
    /// The picture at the IN of each part not cut yet (user 2026-10-23: the card showed the
    /// start of the clip), decoded off the form thread.
    stills: HashMap<String, Arc<qnc_image_assets::RgbaImage>>,
    asked: HashSet<String>,
    made: (mpsc::Sender<Still>, mpsc::Receiver<Still>),
}

impl std::fmt::Debug for PartsPiece {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PartsPiece").field("parts", &self.parts).field("stills", &self.stills.len()).finish()
    }
}

impl PartsPiece {
    pub fn new(actions: PartActions) -> Self {
        Self { parts: IngestParts::default(), actions, chosen: None, stills: HashMap::new(), asked: HashSet::new(), made: mpsc::channel() }
    }

    /// The pictures at the IN of the parts not cut yet: those made are taken, the others asked
    /// for, each once (a failed one keeps the poster of its clip).
    fn follow_stills(&mut self, locator: Option<&qnc_still_locator::StillLocator>) -> bool {
        let mut changed = false;
        while let Ok((part, still)) = self.made.1.try_recv() {
            if let Ok(still) = still {
                self.stills.insert(part, Arc::new(still));
                changed = true;
            }
        }
        let Some(locator) = locator else { return changed };
        for part in self.parts.parts().iter().filter(|part| part.cut_name.is_none()) {
            if !self.asked.insert(part.part_id.clone()) {
                continue;
            }
            let (send, locator, part) = (self.made.0.clone(), locator.clone(), part.clone());
            let _ = std::thread::Builder::new().name("qnc-ingest-part-still".into()).spawn(move || {
                let still = qnc_shot_stills::decode_still(&locator, &part.clip_id, part.in_frame).map(|image| {
                    let size = [image.width() as usize, image.height() as usize];
                    let content_key = part.part_id.bytes().fold(part.in_frame, |key, byte| key.rotate_left(5) ^ u64::from(byte));
                    qnc_image_assets::RgbaImage { size, pixels: image.into_raw(), content_key }
                });
                let _ = send.send((part.part_id, still));
            });
        }
        changed
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
        if action_id == actions.toggle {
            let part = self.parts.parts().iter().find(|part| Some(part.part_id.as_str()) == payload && part.cut_name.is_none())?;
            let (id, selected) = (part.part_id.clone(), !part.selected);
            return Some(self.parts.select(&id, selected).map_or_else(Answer::refused, |()| Answer::changed()));
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
        let changed = self.parts.poll() | self.follow_stills(now.stills.as_ref());
        Answer { changed, message: self.parts.take_message(), ..Answer::default() }
    }

    fn ready(&self) -> Vec<&'static str> {
        let waiting = self.parts.parts().iter().any(|part| part.selected && part.cut_name.is_none());
        if waiting { vec![self.actions.import] } else { Vec::new() }
    }

    /// The parts as cards: a part cut is its own clip of the parts card; a part not cut yet
    /// has the name and length of the part, the poster of its clip and its tick.
    fn rows(&self, tab: &str, clips: &[ClipRow]) -> Option<Vec<ClipRow>> {
        (tab == self.actions.tab).then(|| {
            let rows = self.parts.parts().iter().filter_map(|part| {
                if let Some(cut) = &part.cut_name {
                    return clips.iter().find(|clip| &clip.name == cut).cloned();
                }
                let clip = clips.iter().find(|clip| clip.clip_id == part.clip_id)?;
                let (frames, seconds) = (part.out_frame - part.in_frame, part.seconds);
                Some(ClipRow {
                    clip_id: part.part_id.clone(),
                    name: part.name.clone(),
                    duration_seconds: seconds,
                    duration_frames: frames,
                    selected: part.selected,
                    imported: false,
                    thumb_image: self.stills.get(&part.part_id).cloned().or_else(|| clip.thumb_image.clone()),
                    ..clip.clone()
                })
            });
            rows.collect()
        })
    }
}
