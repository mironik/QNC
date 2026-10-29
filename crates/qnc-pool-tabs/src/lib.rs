//! The tabs of the editorial pool, by their catalog actions. Neutral: it knows no clip, no
//! database and no form; a form's layout contract says which of them it shows.

pub const MODULE_ID: &str = "qnc.module.pool-tabs";

pub const ALL: &str = "editorial_tab_all";
pub const VIRTUAL: &str = "editorial_tab_virtual";
pub const BROLL: &str = "editorial_tab_broll";
pub const SEGMENT: &str = "editorial_tab_segment";
/// The clips that have a kept transcript (user 2026-10-28, Media Assist Audio AI).
pub const TRANSCRIPT: &str = "editorial_tab_transcript";
/// The SubSegments of the script: statements put together from several parts, as virtual clips
/// (user 2026-10-29, Media Assist Audio).
pub const SUBSEGMENT: &str = "editorial_tab_subsegment";

/// Ingest's one tab of the pool: its clips.
pub const INGEST_ALL: &str = "ingest_tab_all";

/// How much each tab of a pool would show. A tab with nothing in it is grey and cannot be
/// chosen, in every form (user 2026-10-28).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Contents {
    pub clips: usize,
    pub shorts: usize,
    pub b_roll: usize,
    pub segments: usize,
    pub transcribed: usize,
    pub subsegments: usize,
}

impl Contents {
    /// Whether the tab of `action_id` has something to show; `None` when it is not a tab.
    pub fn has(&self, action_id: &str) -> Option<bool> {
        let count = match LibraryTab::from_action(action_id)? {
            LibraryTab::All => self.clips,
            LibraryTab::Virtual => self.shorts,
            LibraryTab::Broll => self.b_roll,
            LibraryTab::Segment => self.segments,
            LibraryTab::Transcript => self.transcribed,
            LibraryTab::Subsegment => self.subsegments,
        };
        Some(count > 0)
    }
}

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
    Subsegment,
}

impl LibraryTab {
    pub fn from_action(action_id: &str) -> Option<Self> {
        match action_id {
            ALL | INGEST_ALL => Some(Self::All),
            VIRTUAL => Some(Self::Virtual),
            BROLL => Some(Self::Broll),
            SEGMENT => Some(Self::Segment),
            TRANSCRIPT => Some(Self::Transcript),
            SUBSEGMENT => Some(Self::Subsegment),
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
        for (action, tab) in [(ALL, LibraryTab::All), (VIRTUAL, LibraryTab::Virtual), (BROLL, LibraryTab::Broll), (SEGMENT, LibraryTab::Segment), (TRANSCRIPT, LibraryTab::Transcript), (SUBSEGMENT, LibraryTab::Subsegment)] {
            assert_eq!(LibraryTab::from_action(action), Some(tab));
        }
        assert_eq!(LibraryTab::from_action("play_pause"), None);
        assert!(LibraryTab::Virtual.lists_shots() && LibraryTab::Broll.lists_shots());
        assert!(!LibraryTab::Transcript.lists_shots() && !LibraryTab::All.lists_shots());
    }

    #[test]
    fn a_tab_with_nothing_in_it_cannot_be_chosen() {
        let contents = Contents { clips: 3, transcribed: 1, ..Contents::default() };
        assert_eq!(contents.has(ALL), Some(true));
        assert_eq!(contents.has(INGEST_ALL), Some(true));
        assert_eq!(contents.has(TRANSCRIPT), Some(true));
        assert_eq!(contents.has(VIRTUAL), Some(false));
        assert_eq!(contents.has(BROLL), Some(false));
        assert_eq!(contents.has(SEGMENT), Some(false));
        assert_eq!(contents.has(SUBSEGMENT), Some(false));
        assert_eq!(Contents { subsegments: 1, ..contents }.has(SUBSEGMENT), Some(true));
        assert_eq!(contents.has("play_pause"), None);
    }
}
