use super::*;

/// Ingest on the one desktop board: preview in the monitor, pool head in the head row,
/// the source browser in the body, the clip cards on the right, the source timeline in
/// the dock.
pub fn render_desktop(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let faces = qnc_board::BoardFaces {
        bg: theme.bg,
        left: theme.surface,
        right: theme.bg,
        divider: theme.border_soft,
    };
    qnc_board::show(ui, &contracts.board_sizes(), &faces, |ui, place, rect| match place {
        qnc_board::Place::Monitor => {
            render_preview(ui, rect, contracts, theme, view);
            None
        }
        qnc_board::Place::Head => render_pool_head(ui, contracts, theme),
        qnc_board::Place::Body => render_source_browser(ui, rect, contracts, theme, view),
        qnc_board::Place::Right => render_clip_grid(ui, contracts, theme, view),
        qnc_board::Place::Dock => render_source_dock(ui, contracts, theme, view),
    })
}
