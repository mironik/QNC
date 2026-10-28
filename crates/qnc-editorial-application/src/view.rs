//! Data the editorial form paints and the intents it sends back. Neutral: no
//! player, database or store type leaks through it.

pub use qnc_source_preview::{MonitorFrame, PreviewView};
pub use qnc_virtual_short_cards::VirtualShortCard as EditorialShort;

/// The pool tab (piece `qnc-pool-tabs`).
pub use qnc_pool_tabs::LibraryTab;

/// One row of the clip list: the clip row every form shows (piece `qnc-clip-row`).
pub use qnc_clip_row::ClipRow as EditorialClip;

#[derive(Clone, Default)]
pub struct EditorialView {
    pub library_tab: LibraryTab,
    pub clips: Vec<EditorialClip>,
    /// Short virtual shots for the Virtual tab, oldest first.
    pub shorts: Vec<EditorialShort>,
    /// The short card that is selected on the Virtual tab.
    pub chosen_shot_id: Option<String>,
    /// True while the project catalog is being read.
    pub loading: bool,
    /// Catalog status or controlled error text.
    pub message: String,
    /// The source preview: monitor picture, timeline, artifacts, chosen clip.
    pub preview: PreviewView,
    /// Ton and Off segments of the story program.
    pub segments: qnc_program_segments::SegmentsView,
    /// The panel the keyboard acts on (and the dock button it is on).
    pub focus: qnc_panel_focus::FocusView,
    /// What the one source timeline shows besides the clip: its A1/A2 lanes, large wave
    /// and the clip's original timecode (piece `qnc-source-timeline`).
    pub source_timeline: qnc_source_timeline::SourceTimelineView,
}

impl EditorialView {
    /// Id of the clip in the monitor, if one is chosen.
    pub fn chosen_clip_id(&self) -> Option<&str> {
        self.preview.clip_id.as_deref()
    }

    /// The clip in the monitor, if one is chosen.
    pub fn shown_clip(&self) -> Option<&EditorialClip> {
        let id = self.chosen_clip_id()?;
        self.clips.iter().find(|clip| clip.clip_id == id)
    }

    /// Name of the clip in the monitor, if one is chosen.
    pub fn current_clip_label(&self) -> Option<&str> {
        self.shown_clip().map(|clip| clip.name.as_str())
    }

    pub fn action_enabled(&self, action_id: &str) -> bool {
        match action_id {
            action_ids::SAVE_VIRTUAL_SHOT | action_ids::ADD_TON_SEGMENT | action_ids::ADD_OFF_SEGMENT => self.chosen_clip_id().is_some(),
            "transcribe_marks" => self.chosen_clip_id().is_some() || self.focus.transcript.ticked > 0,
            action_ids::DELETE_SEGMENT | action_ids::DELETE_PART => self.segments.selected().is_some(),
            tab if LibraryTab::from_action(tab).is_some() => qnc_pool_tabs::Contents { clips: self.clips.len(), shorts: self.shorts.iter().filter(|s| !s.b_roll).count(), b_roll: self.shorts.iter().filter(|s| s.b_roll).count(), segments: self.segments.parts.len(), transcribed: self.focus.transcript.pool_clips(true, &self.clips).len() }.has(tab).unwrap_or(false),
            action if self.segments.handles(action) => self.segments.action_enabled(action),
            action_ids::PLAY_PAUSE
            | action_ids::STEP_BACK_FRAME
            | action_ids::STEP_FORWARD_FRAME
            | action_ids::MARK_IN
            | action_ids::MARK_OUT | "transcribe_fast" | "transcribe_best" => true,
            _ => false,
        }
    }

    pub fn tab_selected(&self, action_id: &str) -> bool {
        LibraryTab::from_action(action_id) == Some(self.library_tab)
    }
}

/// Neutral intent. `Action` carries an `action_id` from
/// `contracts/qnc-keyboard-shortcuts.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorialIntent {
    PreviewClip(String),
    /// Open the parent clip and show this short's IN/OUT.
    PreviewShort(String),
    /// One system action from any input device: a key, a click, the wheel (user 2026-10-18).
    Action(qnc_action_event::ActionEvent),
}

impl EditorialIntent {
    pub fn action(action: impl Into<qnc_action_event::ActionEvent>) -> Self {
        Self::Action(action.into())
    }
}

pub mod action_ids {
    pub use qnc_pool_tabs::{ALL as EDITORIAL_TAB_ALL, BROLL as EDITORIAL_TAB_BROLL, SEGMENT as EDITORIAL_TAB_SEGMENT, VIRTUAL as EDITORIAL_TAB_VIRTUAL};
    pub const STEP_PREV_PART: &str = "step_prev_part";
    pub const STEP_NEXT_PART: &str = "step_next_part";
    pub const DELETE_PART: &str = "delete_part";
    pub const PLAY_PAUSE: &str = "play_pause";
    pub const STEP_BACK_FRAME: &str = "step_back_frame";
    pub const STEP_FORWARD_FRAME: &str = "step_forward_frame";
    pub const MARK_IN: &str = "mark_in";
    pub const MARK_OUT: &str = "mark_out";
    pub const SAVE_VIRTUAL_SHOT: &str = "save_virtual_shot";
    pub const ADD_TON_SEGMENT: &str = "add_ton_segment";
    pub const ADD_OFF_SEGMENT: &str = "add_off_segment";
    pub const DELETE_SEGMENT: &str = "delete_segment";
}
