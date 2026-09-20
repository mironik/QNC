use super::*;

pub(super) fn render_location_browser(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let outer = ui.max_rect();
    ui.allocate_rect(outer, Sense::hover());
    ui.painter().rect_filled(outer, 0.0, theme.bg);
    let rect = outer.shrink(contracts.ingest.board.block_pad);

    let source_row_rect = Rect::from_min_size(
        rect.left_top(),
        Vec2::new(rect.width(), theme.chrome_control_height),
    );
    let nav_rect = Rect::from_min_size(
        egui::pos2(rect.left(), source_row_rect.bottom() + 8.0),
        Vec2::new(rect.width(), browser_nav_height(view, theme)),
    );
    let error_height = if view.browser_error.is_some() {
        theme.chrome_control_height + 4.0
    } else {
        0.0
    };
    let error_rect = Rect::from_min_size(
        egui::pos2(rect.left(), nav_rect.bottom() + 4.0),
        Vec2::new(rect.width(), error_height),
    );
    let body_top = if view.browser_error.is_some() {
        error_rect.bottom() + 6.0
    } else {
        nav_rect.bottom() + 6.0
    };
    let body_rect = Rect::from_min_max(
        egui::pos2(rect.left(), body_top),
        egui::pos2(rect.right(), rect.bottom().max(body_top)),
    );

    let mut intent = None;
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(source_row_rect)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.horizontal(|ui| {
                ui.label(muted(&contracts.ingest.dir_browser.sources_label, theme));
                ui.add_space(12.0);
                for kind in [SourceKind::Local, SourceKind::Lan, SourceKind::Internet] {
                    let selected = view.source_kind == kind;
                    let label = source_kind_label(&contracts.ingest.dir_browser, kind);
                    if text_tab(ui, label, selected, theme).clicked() && !selected {
                        intent = Some(IngestIntent::new(
                            kind.action_id(),
                            IngestPayload::SourceKind(kind),
                        ));
                    }
                    ui.add_space(10.0);
                }
            });
        },
    );

    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(nav_rect)
            .layout(Layout::left_to_right(Align::Min)),
        |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let up_enabled = view.browser_parent_available;
                if fixed_text_link(
                    ui,
                    &contracts.ingest.dir_browser.up_label,
                    up_enabled,
                    42.0,
                    theme,
                )
                .clicked()
                {
                    intent = Some(IngestIntent::empty(action_ids::INGEST_DIR_UP));
                }
                ui.add_space(12.0);
                if fixed_text_link(
                    ui,
                    &contracts.ingest.dir_browser.disks_label,
                    !view.browser_busy && !view.command_busy,
                    58.0,
                    theme,
                )
                .clicked()
                {
                    intent = Some(IngestIntent::empty(action_ids::INGEST_DIR_ROOTS));
                }
                ui.add_space(12.0);
                render_browser_head_entries(
                    ui,
                    &contracts.ingest.dir_browser,
                    theme,
                    view,
                    &mut intent,
                );
            });
        },
    );

    if let Some(error) = &view.browser_error {
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(error_rect)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                ui.label(RichText::new(error).color(theme.danger));
            },
        );
    }

    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(body_rect)
            .layout(Layout::top_down(Align::Min)),
        |ui| {
            render_browser_body(ui, &contracts.ingest.dir_browser, theme, view, &mut intent);
        },
    );

    intent
}

pub(super) fn browser_nav_height(view: &IngestViewModel, theme: &Theme) -> f32 {
    if view.browser_roots {
        let rows = view.browser_entries.len().max(1) as f32;
        (theme.chrome_control_height * rows) + (4.0 * (rows - 1.0))
    } else {
        theme.chrome_control_height
    }
}
