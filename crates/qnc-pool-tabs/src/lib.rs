//! The tabs of the editorial pool, by their catalog actions. Neutral: it knows no clip, no
//! database and no form; a form's layout contract says which of them it shows.

pub const MODULE_ID: &str = "qnc.module.pool-tabs";

pub const ALL: &str = "editorial_tab_all";
pub const VIRTUAL: &str = "editorial_tab_virtual";
pub const BROLL: &str = "editorial_tab_broll";
pub const SEGMENT: &str = "editorial_tab_segment";
/// The clips that have a kept transcript (user 2026-10-28, Media Assist Audio AI).
pub const TRANSCRIPT: &str = "editorial_tab_transcript";

/// Pool tab, as in v5: Virtual lists short shots only, B-roll the cover shots, Segment the
/// program segments; Transcript the clips already transcribed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibraryTab {
    #[default]
    All,
    Virtual,
    Broll,
    Segment,
    Transcript,
}

impl LibraryTab {
    pub fn from_action(action_id: &str) -> Option<Self> {
        match action_id {
            ALL => Some(Self::All),
            VIRTUAL => Some(Self::Virtual),
            BROLL => Some(Self::Broll),
            SEGMENT => Some(Self::Segment),
            TRANSCRIPT => Some(Self::Transcript),
            _ => None,
        }
    }

    /// Whether the tab lists shots (shorts or B-roll) rather than clips.
    pub fn lists_shots(self) -> bool {
        matches!(self, Self::Virtual | Self::Broll)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tab_comes_from_its_action_and_only_virtual_and_broll_list_shots() {
        for (action, tab) in [(ALL, LibraryTab::All), (VIRTUAL, LibraryTab::Virtual), (BROLL, LibraryTab::Broll), (SEGMENT, LibraryTab::Segment), (TRANSCRIPT, LibraryTab::Transcript)] {
            assert_eq!(LibraryTab::from_action(action), Some(tab));
        }
        assert_eq!(LibraryTab::from_action("play_pause"), None);
        assert!(LibraryTab::Virtual.lists_shots() && LibraryTab::Broll.lists_shots());
        assert!(!LibraryTab::Transcript.lists_shots() && !LibraryTab::All.lists_shots());
    }
}
