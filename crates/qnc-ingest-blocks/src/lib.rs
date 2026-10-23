//! The Ingest form on the desktop board (rule 11: a form is only a layout grid of panels).
//! Each place of the board gets its public panel with the Ingest layout's measures and
//! texts and the state of the Ingest view; the panel's answer becomes an Ingest intent.

use eframe::egui::{self, Rect, Ui};

use qnc_ingest_application::{
    action_ids, timeline_intent_to_ingest_intent, ClipFilter, IngestIntent, IngestPayload, IngestViewModel, SourceKind,
};
use qnc_ingest_layout::{theme::Theme, IngestContracts};

mod panels;

/// Ingest on the one desktop board: preview in the monitor, pool tabs and transport in the
/// head row, the source browser in the body, the clip cards on the right, the source
/// timeline in the dock; drawn in `frame` as one layout tree with it.
pub fn render_desktop(
    ui: &mut Ui,
    frame: &mut qnc_board::Frame<'_>,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let (faces, board) = (theme.board_faces(), contracts.board_layout());
    // The panel the keyboard is on has a thin frame (the dock buttons are part of the source timeline).
    let framed = |intent, ui: &Ui, rect, panels: &[&str]| {
        qnc_ui_kit::paint_focus(ui, rect, panels.iter().any(|panel| view.focus.is(panel)), theme.focus);
        intent
    };
    qnc_board::show_in_frame(ui, frame, &board, |name| faces.named(name), |ui, block, rect| match block {
        "preview" => panels::preview(ui, rect, contracts, theme, view),
        "pool-head" => framed(panels::pool_tabs(ui, rect, contracts, theme, view), ui, rect, &[qnc_focus_walk::POOL_TABS]),
        "transport" => qnc_transport_panel::show(ui, rect, theme, contracts.ingest.pool_head.tab_gap).map(|action| IngestIntent::empty(&action)),
        "source-browser" => framed(panels::browser(ui, rect, contracts, theme, view), ui, rect, &[qnc_focus_walk::SOURCE_KINDS, qnc_focus_walk::BROWSER, qnc_focus_walk::BROWSER_BUTTONS]),
        "clip-cards" => framed(panels::pool(ui, contracts, theme, view), ui, rect, &[qnc_focus_walk::CARDS]),
        "source-dock" => framed(panels::source_dock(ui, contracts, theme, view), ui, rect, &[qnc_focus_walk::SOURCE_TIMELINE, qnc_focus_walk::SOURCE_BUTTONS]),
        _ => None,
    })
}
