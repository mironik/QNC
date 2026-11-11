//! The right panel of an editorial board (user 2026-10-26: each group says in its layout
//! contract what its right panel is). It paints the public panel the kind names and gives its
//! answer; it knows no form. A kind it does not know stays empty.
//!
//! A group may give the right panel tabs on top (user 2026-10-27, AI Media Assist: Audio AI and
//! Video AI), each with its panel kind. Which tab is open is a view choice of this process only
//! (like a collapsed section), kept in the UI memory; nothing is written and nothing else reads it.

use eframe::egui;
use qnc_action_event::ActionEvent;
use qnc_editorial_layout::RightPanelTab;

pub const MODULE_ID: &str = "qnc.module.right-panel";
/// The kinds of right panels are the side pieces' (`qnc-side-pieces`): their pieces are made per kind.
use qnc_side_pieces::{DRAFT_SEGMENT_PANEL, SCRIPT_CLIP_BAR, SCRIPT_PANEL, SCRIPT_WRITER_PANEL, SEGMENT_PANEL, TRANSCRIPT_PANEL, TRANSCRIPT_SEARCH_PANEL, VIDEO_AI_PANEL};

/// What the right panels show.
pub struct Shown<'a> {
    pub segments: &'a qnc_program_view::SegmentsView,
    pub cover_lengths: &'a [qnc_segment_panel::CoverLengthBand],
    pub transcript: &'a qnc_transcript::TranscriptView,
    /// The shots of the clip on the preview (Video AI).
    pub shots: &'a qnc_clip_shots::ShotsView,
    /// The script of the story (Media Assist Audio).
    pub script: &'a qnc_script::ScriptView,
    /// The shown clip's source timecode.
    pub timecode: Option<&'a qnc_source_timecode::SourceTimecode>,
    /// What the project's AI features allow: a tab of a feature it has off is grey.
    pub ai: &'a qnc_ai_features::Gate,
}

/// The right panel of a group: its tabs on top when it has them, then the panel of the open tab;
/// without tabs the panel of `kind`. Gives the answer and whether it is the segment panel (it
/// takes the keyboard focus).
pub fn show_tabbed(ui: &mut egui::Ui, kind: &str, tabs: &[RightPanelTab], theme: &qnc_desktop_theme::Theme, shown: &Shown<'_>) -> (Option<ActionEvent>, bool) {
    if tabs.is_empty() {
        return show(ui, kind, theme, shown);
    }
    let memory = egui::Id::new("qnc-right-panel-tab");
    let open: Option<String> = ui.ctx().data(|data| data.get_temp(memory));
    // A tab of a feature the project has off is grey and never open while another one is not.
    let allowed = |id: &str| shown.ai.allows(id);
    let open = open
        .filter(|id| tabs.iter().any(|tab| &tab.action_id == id) && allowed(id))
        .or_else(|| tabs.iter().find(|tab| allowed(&tab.action_id)).map(|tab| tab.action_id.clone()))
        .unwrap_or_else(|| tabs[0].action_id.clone());
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), theme.chrome_row_height), egui::Sense::hover());
    let row: Vec<_> = tabs.iter().map(|tab| qnc_pool_tabs_panel::PoolTab { label: &tab.label, action_id: Some(tab.action_id.as_str()) }).collect();
    if let Some(chosen) = qnc_pool_tabs_panel::show(ui, rect, (theme, 10.0), &row, |id| id == open, allowed) {
        ui.ctx().data_mut(|data| data.insert_temp(memory, chosen.to_string()));
    }
    let panel = tabs.iter().find(|tab| tab.action_id == open).map_or(kind, |tab| tab.panel.as_str());
    show(ui, panel, theme, shown)
}

/// The panel of `kind`, and whether it is the segment panel (it takes the keyboard focus).
pub fn show(ui: &mut egui::Ui, kind: &str, theme: &qnc_desktop_theme::Theme, shown: &Shown<'_>) -> (Option<ActionEvent>, bool) {
    match kind {
        // The draft story's segment panel is the same panel (S-Story).
        SEGMENT_PANEL | DRAFT_SEGMENT_PANEL => (qnc_segment_panel::show(ui, shown.segments, theme, shown.cover_lengths), true),
        // A statement of the script open as a SubSegment: its own text there, the same block as
        // in the script (user 2026-11-04); otherwise the transcript of the clip shown.
        TRANSCRIPT_PANEL => match qnc_script_panel::show_open_statement(ui, shown.script, theme) {
            Some(asked) => (asked, false),
            None => (qnc_transcript_panel::show(ui, shown.transcript, theme, shown.timecode), false),
        },
        TRANSCRIPT_SEARCH_PANEL => match qnc_script_panel::show_open_statement(ui, shown.script, theme) {
            Some(asked) => (asked, false),
            None => (qnc_transcript_panel::show_searched(ui, shown.transcript, theme, shown.timecode), false),
        },
        VIDEO_AI_PANEL => (qnc_shot_panel::show(ui, shown.shots, theme, shown.timecode, shown.cover_lengths), false),
        SCRIPT_PANEL => (qnc_script_panel::show(ui, shown.script, shown.transcript, theme), false),
        SCRIPT_WRITER_PANEL => (qnc_script_writer_panel::show(ui, shown.script, theme), false),
        SCRIPT_CLIP_BAR => (qnc_script_writer_panel::show_clip_bar(ui, shown.script, &shown.transcript.search, theme), false),
        _ => (None, false),
    }
}
