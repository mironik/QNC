use super::*;
use qnc_media_pool_head::{RowColors, RowCommand, RowTab};

/// The pool head is the public row: the first contract tab is selected and only names
/// the pool; the transport buttons carry their action ids.
pub(super) fn render_pool_head(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
) -> Option<IngestIntent> {
    let head = &contracts.ingest.pool_head;
    let tabs: Vec<RowTab> = head
        .tabs_left
        .iter()
        .enumerate()
        .map(|(index, tab)| RowTab { label: tab, selected: index == 0, action_id: None, enabled: true })
        .collect();
    let transport: Vec<RowCommand> = head
        .transport_right
        .iter()
        .filter_map(|command| Some(RowCommand { label: command.label(), action_id: command.action_id()? }))
        .collect();
    let colors = RowColors { fill: theme.surface, muted: theme.text_muted, accent: theme.accent };
    qnc_media_pool_head::show_row(ui, &dock_style(contracts, theme), colors, &tabs, &transport)
        .map(IngestIntent::empty)
}
