use super::*;

/// The pool head: the public row of `qnc-media-pool-head` with the tabs and commands
/// of the Ingest layout contract; the chosen tab comes from the view (v5: Ingest shows
/// the same tabs as Story / Media Assist, a click only chooses one).
pub(super) fn render_pool_head(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let rect = ui.available_rect_before_wrap();
    let head = &contracts.ingest.pool_head;
    let tabs: Vec<_> = head
        .tabs_left
        .iter()
        .enumerate()
        .map(|(index, tab)| qnc_media_pool_head::RowTab {
            label: tab.label(),
            selected: view.pool_tab.as_deref().map_or(index == 0, |chosen| tab.action_id() == Some(chosen)),
            enabled: tab.action_id().is_some(),
        })
        .collect();
    let style = row_style(theme, head.tab_gap);
    let mut intent = None;
    qnc_source_dock::show_chrome_row(ui, rect, &dock_style(contracts, theme), theme.surface, true, |ui| {
        intent = match qnc_media_pool_head::show_row(ui, &style, &tabs, &[]) {
            Some(qnc_media_pool_head::RowClick::Tab(index)) => head.tabs_left[index].action_id(),
            _ => None,
        }
        .map(IngestIntent::empty);
    });
    intent
}

/// The transport panel in its own place (Play, IN, OUT and the monitor's Auto | On).
pub(super) fn render_transport(ui: &mut Ui, rect: Rect, contracts: &IngestContracts, theme: &Theme) -> Option<IngestIntent> {
    let head = &contracts.ingest.pool_head;
    let (row, chrome) = (row_style(theme, head.tab_gap), dock_style(contracts, theme));
    let look = qnc_transport_panel::TransportLook { row: &row, chrome: &chrome, fill: theme.surface };
    let listed = head.transport_right.iter().filter_map(|command| Some((command.label(), command.action_id()?)));
    qnc_transport_panel::show(ui, rect, look, listed).map(|action| IngestIntent::empty(&action))
}

/// The colours of the public pool head row from the Ingest theme.
pub(super) fn row_style(theme: &Theme, tab_gap: f32) -> qnc_media_pool_head::RowStyle {
    qnc_media_pool_head::RowStyle {
        text: theme.text,
        muted: theme.text_muted,
        accent: theme.accent,
        border: theme.border,
        font_ui: theme.font_ui,
        control_height: theme.chrome_control_height,
        tab_gap,
    }
}
