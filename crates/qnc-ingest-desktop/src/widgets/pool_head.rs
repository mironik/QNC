use super::*;

pub(super) fn render_pool_head(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
) -> Option<IngestIntent> {
    let rect = ui.available_rect_before_wrap();

    let mut intent = None;
    qnc_source_dock::show_chrome_row(
        ui,
        rect,
        &dock_style(contracts, theme),
        theme.surface,
        true,
        |ui| {
            ui.spacing_mut().button_padding = Vec2::new(8.0, 2.0);
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
            for (index, tab) in contracts.ingest.pool_head.tabs_left.iter().enumerate() {
                let selected = index == 0;
                let _ = text_tab(ui, tab, selected, theme);
                ui.add_space(10.0);
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                for command in contracts.ingest.pool_head.transport_right.iter().rev() {
                    let action_id = match command.as_str() {
                        ">" => action_ids::PLAY_PAUSE,
                        "[" => action_ids::STEP_BACK_FRAME,
                        "]" => action_ids::STEP_FORWARD_FRAME,
                        _ => action_ids::INGEST_RELOAD,
                    };
                    if small_button(ui, command, true, theme).clicked() {
                        intent = Some(IngestIntent::empty(action_id));
                    }
                }
            });
        },
    );

    intent
}
