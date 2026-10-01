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
    qnc_board::show_in_frame(ui, frame, &board, |name| faces.named(name), |ui, block, rect| match block {
        "preview" => {
            render_preview(ui, rect, contracts, theme, view);
            None
        }
        "pool-head" => render_pool_head(ui, contracts, theme),
        "source-browser" => render_source_browser(ui, rect, contracts, theme, view),
        "clip-cards" => render_clip_grid(ui, contracts, theme, view),
        "source-dock" => render_source_dock(ui, contracts, theme, view),
        _ => None,
    })
}
