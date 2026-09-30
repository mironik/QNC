use super::*;

/// The body of the left column: the source browser and its action bar at the bottom.
pub(super) fn render_source_browser(
    ui: &mut Ui,
    browser_rect: Rect,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let action_rect = Rect::from_min_size(
        egui::pos2(
            browser_rect.left(),
            (browser_rect.bottom()
                - theme.chrome_control_height
                - contracts.ingest.board.block_pad)
                .max(browser_rect.top()),
        ),
        Vec2::new(browser_rect.width(), theme.chrome_control_height),
    );
    let browser_content_rect = Rect::from_min_max(
        browser_rect.left_top(),
        egui::pos2(
            browser_rect.right(),
            (action_rect.top() - 8.0).max(browser_rect.top()),
        ),
    );

    let mut intent = None;
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(browser_content_rect),
        |ui| {
            intent = render_location_browser(ui, contracts, theme, view);
        },
    );
    if intent.is_none() {
        ui.scope_builder(egui::UiBuilder::new().max_rect(action_rect), |ui| {
            intent = render_location_action_bar(ui, contracts, theme, view);
        });
    }
    intent
}
