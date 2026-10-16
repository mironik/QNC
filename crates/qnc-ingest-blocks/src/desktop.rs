use super::*;

/// Ingest on the one desktop board: preview in the monitor, pool head in the head row,
/// the source browser in the body, the clip cards on the right, the source timeline in
/// the dock; drawn in `frame` as one layout tree with it.
pub fn render_desktop(
    ui: &mut Ui,
    frame: &mut qnc_board::Frame<'_>,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let (sizes, faces, names) = (contracts.board_sizes(), theme.board_faces(), contracts.board_names());
    let board = sizes.standard_layout(&names);
    // The panel the keyboard is on has a thin frame (the dock buttons are part of the source timeline).
    let framed = |intent, ui: &Ui, rect, panels: &[&str]| {
        qnc_focus_walk::paint_focus(ui, rect, panels.iter().any(|panel| view.focus.is(panel)), theme.focus);
        intent
    };
    qnc_board::show_in_frame(ui, frame, &board, |name| faces.named(name), |ui, block, rect| match block {
        "preview" => render_preview(ui, rect, contracts, theme, view),
        "pool-head" => framed(render_pool_head(ui, contracts, theme, view), ui, rect, &[qnc_focus_walk::POOL_TABS]),
        "source-browser" => framed(render_source_browser(ui, rect, contracts, theme, view), ui, rect, &[qnc_focus_walk::SOURCE_KINDS, qnc_focus_walk::BROWSER, qnc_focus_walk::BROWSER_BUTTONS]),
        "clip-cards" => framed(render_clip_grid(ui, contracts, theme, view), ui, rect, &[qnc_focus_walk::CARDS]),
        "source-dock" => framed(render_source_dock(ui, contracts, theme, view), ui, rect, &[qnc_focus_walk::SOURCE_TIMELINE, qnc_focus_walk::SOURCE_BUTTONS]),
        _ => None,
    })
}
