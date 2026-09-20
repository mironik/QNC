use super::*;

pub(super) fn render_source_dock(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(rect, 0.0, theme.panel_alt);
    ui.painter().line_segment(
        [rect.left_top(), rect.right_top()],
        Stroke::new(1.0, theme.border),
    );

    let mut intent = None;
    let inner = Rect::from_min_max(
        egui::pos2(rect.left() + 8.0, rect.top()),
        egui::pos2(rect.right() - 8.0, rect.bottom()),
    );
    let header_rect = Rect::from_min_size(
        inner.left_top(),
        Vec2::new(inner.width().max(0.0), theme.chrome_row_height),
    );
    let timeline_rect = Rect::from_min_size(
        egui::pos2(
            inner.left(),
            header_rect.bottom() + contracts.ingest.source_dock.header_timeline_gap,
        ),
        Vec2::new(
            inner.width().max(0.0),
            timeline_placeholder_height().min(
                (inner.bottom()
                    - header_rect.bottom()
                    - contracts.ingest.source_dock.header_timeline_gap)
                    .max(0.0),
            ),
        ),
    );

    show_chrome_row(ui, header_rect, theme, theme.panel_alt, true, |ui| {
        ui.label(
            RichText::new(source_dock_clip_label(contracts, view))
                .color(theme.text)
                .strong()
                .size(theme.font_ui),
        );
        ui.add_space(10.0);
        if contracts.ingest.source_dock.show_import_actions {
            let status = view.status_label();
            if !status.is_empty() {
                ui.label(muted(&status, theme)).on_hover_text(&view.message);
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if contracts.ingest.source_dock.show_import_actions {
                let mut style = FormActionBarStyle::new(
                    theme.text,
                    theme.accent,
                    theme.border,
                    theme.font_ui,
                    theme.chrome_control_height,
                );
                style.button_width = contracts.ingest.source_dock.clip_filter_width / 2.0;
                let labels = &contracts.ingest.source_dock.clip_filter_labels;
                let modes = [ClipFilter::New, ClipFilter::All];
                if let Some(index) = qnc_ui_kit::show_two_way_switch(
                    ui,
                    [&labels[0], &labels[1]],
                    usize::from(view.clip_filter == ClipFilter::All),
                    contracts
                        .ingest
                        .source_dock
                        .clip_filter_colors
                        .map(|[r, g, b]| Color32::from_rgb(r, g, b)),
                    &style,
                ) {
                    intent = Some(IngestIntent::new(
                        action_ids::INGEST_SET_CLIP_FILTER,
                        IngestPayload::ClipFilter(modes[index]),
                    ));
                }
                let poster_count = view.proxy_poster_approval_count();
                if poster_count > 0 {
                    let label = format!("Generiraj postere ({poster_count})");
                    if action_button(ui, &label, !view.command_busy, theme).clicked() {
                        intent = Some(IngestIntent::empty(
                            action_ids::INGEST_APPROVE_PROXY_POSTERS,
                        ));
                    }
                    ui.label(muted("Nema postera na kartici", theme));
                }
                if primary_button(ui, "Uvezi", action_enabled("Uvezi", view), theme).clicked() {
                    intent = Some(IngestIntent::empty(action_ids::INGEST_IMPORT_SELECTED));
                }
                if action_button(
                    ui,
                    "Odaberi sve",
                    action_enabled("Odaberi sve", view),
                    theme,
                )
                .clicked()
                {
                    intent = Some(IngestIntent::empty(action_ids::INGEST_SELECT_ALL));
                }
                if action_button(ui, "Očisti", action_enabled("Očisti", view), theme).clicked() {
                    intent = Some(IngestIntent::empty(action_ids::INGEST_CLEAR_SELECTION));
                }
            }
            if contracts.ingest.source_dock.show_edit_actions {
                let _ = action_button(ui, "Export", false, theme);
                let _ = action_button(ui, "B", false, theme);
            }
            if view.archive_original_available {
                let mut archive = view.archive_original;
                if ui
                    .checkbox(
                        &mut archive,
                        RichText::new("Kopiraj original")
                            .color(theme.text)
                            .size(theme.font_ui),
                    )
                    .changed()
                {
                    intent = Some(IngestIntent::new(
                        action_ids::INGEST_SET_ARCHIVE,
                        IngestPayload::Bool(archive),
                    ));
                }
            }
            let mut ai = view.ai_mining;
            if ui
                .add_enabled(
                    false,
                    egui::Checkbox::new(
                        &mut ai,
                        RichText::new("AI mining")
                            .color(theme.text)
                            .size(theme.font_ui),
                    ),
                )
                .changed()
            {
                intent = Some(IngestIntent::new(
                    action_ids::INGEST_SET_AI_MINING,
                    IngestPayload::Bool(ai),
                ));
            }
        });
    });

    if intent.is_none() {
        intent = render_player_timeline(ui, timeline_rect, theme, view);
    } else {
        render_player_timeline(ui, timeline_rect, theme, view);
    }

    intent
}
