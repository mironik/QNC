use super::*;

pub(super) fn render_location_action_bar(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let outer = ui.max_rect();
    ui.allocate_rect(outer, Sense::hover());
    let rect = outer.shrink(contracts.ingest.board.block_pad);
    let mut intent = None;
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::right_to_left(Align::Center)),
        |ui| {
            let can_confirm = !view.browser_roots
                && !view.browser_path_label.is_empty()
                && !view.work_settings_loading
                && !view.browser_busy
                && !view.command_busy;
            let response = qnc_ui_kit::show_form_action_bar(
                ui,
                &form_action_bar_style(theme),
                &contracts.ingest.dir_browser.confirm_label,
                can_confirm,
                &contracts.ingest.dir_browser.cancel_label,
            );
            let chosen = |action_id| view.focus.is(qnc_focus_walk::BROWSER_BUTTONS) && view.focus.chosen(action_id);
            for (rect, action_id) in [(response.confirm_rect, action_ids::INGEST_DIR_CONFIRM), (response.cancel_rect, action_ids::INGEST_DIR_CANCEL)] {
                qnc_focus_walk::paint_focus(ui, rect.unwrap_or(egui::Rect::NOTHING).expand(2.0), chosen(action_id), theme.focus);
            }
            if response.confirm_clicked {
                if let Some(uri) = &view.browser_current_uri {
                    intent = Some(IngestIntent::new(
                        action_ids::INGEST_DIR_CONFIRM,
                        IngestPayload::LocationUri(uri.clone()),
                    ));
                }
            }
            if response.cancel_clicked {
                intent = Some(IngestIntent::empty(action_ids::INGEST_DIR_CANCEL));
            }
        },
    );
    intent
}
