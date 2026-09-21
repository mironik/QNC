use super::*;

pub(super) fn render_clip_grid(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let outer = ui.available_rect_before_wrap();
    ui.painter().rect_filled(outer, 0.0, theme.bg);
    let rect = outer.shrink(contracts.ingest.board.block_pad);

    let clips = view.visible_clips().collect::<Vec<_>>();
    if clips.is_empty() {
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(24.0);
                let message = view.work_settings_error.as_deref().unwrap_or_else(|| {
                    if view.work_settings_loading {
                        "Citanje radnih postavki..."
                    } else if view.clip_filter == ClipFilter::New {
                        &contracts.ingest.clip_grid.empty_new_message
                    } else if view.command_busy || view.selected_source_uri.is_some() {
                        &view.message
                    } else {
                        &contracts.ingest.clip_grid.empty_message
                    }
                });
                ui.label(muted(message, theme));
            });
        });
        return None;
    }

    let metrics = grid_metrics(rect.width(), clips.len(), contracts);
    let card_width = metrics.card_width;
    let card_height = metrics.card_height;
    let gap = metrics.gap;
    let columns = metrics.columns;
    let mut intent = None;

    ScrollArea::vertical()
        .id_salt(("ingest_grid", view.clip_filter))
        .show_viewport(ui, |ui, _| {
            for row in clips.chunks(columns) {
                ui.horizontal(|ui| {
                    for clip in row {
                        let card = render_clip_card(
                            ui,
                            clip,
                            view.preview_clip_id.as_deref() == Some(clip.clip_id.as_str()),
                            Vec2::new(card_width, card_height),
                            theme,
                        );
                        if card.clicked() {
                            let checkbox_click = card.interact_pointer_pos().is_some_and(|pos| {
                                selection_check_hit_rect(card.rect).contains(pos)
                            });
                            let action_id = if checkbox_click {
                                action_ids::INGEST_CLIP_TOGGLE
                            } else {
                                action_ids::INGEST_PREVIEW_FOCUS
                            };
                            intent = Some(IngestIntent::new(
                                action_id,
                                IngestPayload::ClipId(clip.clip_id.clone()),
                            ));
                        }
                        ui.add_space(gap);
                    }
                });
                ui.add_space(gap);
            }
        });

    intent
}

pub(super) struct GridMetrics {
    pub(super) columns: usize,
    pub(super) card_width: f32,
    pub(super) card_height: f32,
    pub(super) gap: f32,
}

pub(super) fn grid_metrics(
    available_width: f32,
    count: usize,
    contracts: &IngestContracts,
) -> GridMetrics {
    let min_card_width = contracts.ingest.clip_grid.min_card_width;
    let gap = contracts.ingest.clip_grid.grid_gap;
    let count = count.max(1);
    let usable_width = (available_width - 8.0).max(min_card_width);
    let columns = (((usable_width + gap) / (min_card_width + gap)).floor() as usize)
        .max(1)
        .min(count);
    let card_width = (usable_width - gap * columns.saturating_sub(1) as f32) / columns as f32;
    let card_height = card_width * 9.0 / 16.0 + contracts.ingest.clip_grid.card_text_height;
    GridMetrics {
        columns,
        card_width,
        card_height,
        gap,
    }
}
