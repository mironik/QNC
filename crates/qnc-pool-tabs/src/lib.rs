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
/// The voice records of the script, one card per OFF (user 2026-10-30, Media Assist Audio).
pub const VOICE: &str = "editorial_tab_voice";
/// The versions of the S-Story (user 2026-10-31).
pub const STORIES: &str = "editorial_tab_stories";
/// Every usable part of every clip, grouped by what its pictures show (user 2026-11-10: Story,
/// S-Story, MA Video).
pub const AI_BROLL: &str = "editorial_tab_ai_broll";

/// The catalog actions of the X on a chosen card (target: the card's id): a statement, the
/// transcript of a clip, a composite statement (a block of the script).
pub const REMOVE_SHORT: &str = "short_remove";
pub const REMOVE_TRANSCRIPT: &str = "transcript_forget_clip";
pub const REMOVE_SUBSEGMENT: &str = "script_remove";

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
    pub voices: usize,
    pub stories: usize,
    pub ai_broll: usize,
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
            LibraryTab::Voice => self.voices,
            LibraryTab::Stories => self.stories,
            LibraryTab::AiBroll => self.ai_broll,
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
    Voice,
    Stories,
    AiBroll,
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
            VOICE => Some(Self::Voice),
            STORIES => Some(Self::Stories),
            AI_BROLL => Some(Self::AiBroll),
            _ => None,
        }
    }

    /// Whether the tab lists shots (shorts or B-roll) rather than clips.
    pub fn lists_shots(self) -> bool {
        matches!(self, Self::Virtual | Self::Broll)
    }

    /// What the X on a chosen card of this tab does (user 2026-11-09: what was made wrongly is
    /// deleted from its card); `None`: its cards have no X.
    pub fn remove_action(self) -> Option<&'static str> {
        match self {
            Self::Virtual => Some(REMOVE_SHORT),
            Self::Transcript => Some(REMOVE_TRANSCRIPT),
            Self::Subsegment => Some(REMOVE_SUBSEGMENT),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tab_comes_from_its_action_and_only_virtual_and_broll_list_shots() {
        for (action, tab) in [(ALL, LibraryTab::All), (VIRTUAL, LibraryTab::Virtual), (BROLL, LibraryTab::Broll), (SEGMENT, LibraryTab::Segment), (TRANSCRIPT, LibraryTab::Transcript), (SUBSEGMENT, LibraryTab::Subsegment), (VOICE, LibraryTab::Voice), (STORIES, LibraryTab::Stories), (AI_BROLL, LibraryTab::AiBroll)] {
            assert_eq!(LibraryTab::from_action(action), Some(tab));
        }
        assert_eq!(LibraryTab::from_action("play_pause"), None);
        assert!(LibraryTab::Virtual.lists_shots() && LibraryTab::Broll.lists_shots());
        assert!(!LibraryTab::Transcript.lists_shots() && !LibraryTab::All.lists_shots());
    }

    #[test]
    fn only_statements_transcripts_and_composite_statements_have_an_x() {
        assert_eq!(LibraryTab::Virtual.remove_action(), Some(REMOVE_SHORT));
        assert_eq!(LibraryTab::Transcript.remove_action(), Some(REMOVE_TRANSCRIPT));
        assert_eq!(LibraryTab::Subsegment.remove_action(), Some(REMOVE_SUBSEGMENT));
        for tab in [LibraryTab::All, LibraryTab::Broll, LibraryTab::Segment, LibraryTab::Voice, LibraryTab::Stories] {
            assert_eq!(tab.remove_action(), None);
        }
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
        assert_eq!(contents.has(VOICE), Some(false));
        assert_eq!(Contents { voices: 2, ..contents }.has(VOICE), Some(true));
        assert_eq!(contents.has(STORIES), Some(false));
        assert_eq!(Contents { stories: 1, ..contents }.has(STORIES), Some(true));
        assert_eq!(contents.has("play_pause"), None);
    }
}
