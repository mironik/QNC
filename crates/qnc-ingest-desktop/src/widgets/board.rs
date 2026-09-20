use super::*;

pub(super) fn render_board(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let metrics = &contracts.ingest.board;
    let rect = ui.available_rect_before_wrap();
    ui.allocate_rect(rect, Sense::hover());

    let usable_width = rect
        .width()
        .max(metrics.left_min_width + metrics.right_min_width);
    let split = (usable_width * metrics.left_ratio).clamp(
        metrics.left_min_width,
        usable_width - metrics.right_min_width,
    );
    let left_rect = Rect::from_min_size(rect.left_top(), Vec2::new(split, rect.height()));
    let divider_rect = Rect::from_min_size(
        egui::pos2(left_rect.right(), rect.top()),
        Vec2::new(metrics.divider_width, rect.height()),
    );
    let right_rect = Rect::from_min_max(
        egui::pos2(divider_rect.right(), rect.top()),
        rect.right_bottom(),
    );

    ui.painter().rect_filled(left_rect, 0.0, theme.surface);
    ui.painter()
        .rect_filled(divider_rect, 0.0, theme.border_soft);
    ui.painter().rect_filled(right_rect, 0.0, theme.bg);

    let mut intent = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
        intent = render_left_column(ui, contracts, theme, view);
    });
    if intent.is_none() {
        ui.scope_builder(egui::UiBuilder::new().max_rect(right_rect), |ui| {
            intent = render_clip_grid(ui, contracts, theme, view);
        });
    }
    intent
}

pub(super) fn render_left_column(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let rect = ui.available_rect_before_wrap();
    ui.allocate_rect(rect, Sense::hover());

    let preview_h = preview_height(rect, contracts);
    let preview_rect = Rect::from_min_size(rect.left_top(), Vec2::new(rect.width(), preview_h));
    let head_rect = Rect::from_min_size(
        egui::pos2(rect.left(), preview_rect.bottom()),
        Vec2::new(rect.width(), theme.chrome_row_height),
    );
    let browser_rect = Rect::from_min_max(
        egui::pos2(rect.left(), head_rect.bottom()),
        rect.right_bottom(),
    );
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

    render_preview(ui, preview_rect, contracts, theme, view);

    let mut intent = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(head_rect), |ui| {
        intent = render_pool_head(ui, contracts, theme);
    });
    if intent.is_none() {
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(browser_content_rect),
            |ui| {
                intent = render_location_browser(ui, contracts, theme, view);
            },
        );
    }
    if intent.is_none() {
        ui.scope_builder(egui::UiBuilder::new().max_rect(action_rect), |ui| {
            intent = render_location_action_bar(ui, contracts, theme, view);
        });
    }
    intent
}
