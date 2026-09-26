use super::*;
use qnc_source_dock::{show_timeline_dock, SourceTimeline};

/// The source dock is the public one; this form adds its own import actions to the header.
pub(super) fn render_source_dock(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let rect = ui.available_rect_before_wrap();
    let mut intent = None;
    let timeline_intent = show_timeline_dock(
        ui,
        rect,
        &dock_style(contracts, theme),
        source_dock_clip_label(contracts, view),
        |ui| {
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
                        let label = format!(
                            "{} ({poster_count})",
                            action_label(
                                contracts,
                                action_ids::INGEST_APPROVE_PROXY_POSTERS,
                                "Generiraj postere",
                            )
                        );
                        if action_button(
                            ui,
                            &label,
                            view.action_enabled(action_ids::INGEST_APPROVE_PROXY_POSTERS),
                            theme,
                        )
                        .clicked()
                        {
                            intent = Some(IngestIntent::empty(
                                action_ids::INGEST_APPROVE_PROXY_POSTERS,
                            ));
                        }
                    } else {
                        ui.label(muted("Nema postera na kartici", theme));
                    }
                    if primary_button(
                        ui,
                        action_label(contracts, action_ids::INGEST_IMPORT_SELECTED, "Uvezi"),
                        view.action_enabled(action_ids::INGEST_IMPORT_SELECTED),
                        theme,
                    )
                    .clicked()
                    {
                        intent = Some(IngestIntent::empty(action_ids::INGEST_IMPORT_SELECTED));
                    }
                    if action_button(
                        ui,
                        action_label(contracts, action_ids::INGEST_SELECT_ALL, "Odaberi sve"),
                        view.action_enabled(action_ids::INGEST_SELECT_ALL),
                        theme,
                    )
                    .clicked()
                    {
                        intent = Some(IngestIntent::empty(action_ids::INGEST_SELECT_ALL));
                    }
                    if action_button(
                        ui,
                        action_label(contracts, action_ids::INGEST_CLEAR_SELECTION, "Očisti"),
                        view.action_enabled(action_ids::INGEST_CLEAR_SELECTION),
                        theme,
                    )
                    .clicked()
                    {
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
        },
        SourceTimeline::from_assets(&view.timeline, timeline_theme(theme), &view.timeline_assets).with_timecode(view.source_timecode),
    );
    if intent.is_none() {
        intent = timeline_intent_to_ingest_intent(timeline_intent);
    }
    intent
}

fn action_label<'a>(contracts: &'a IngestContracts, action_id: &str, fallback: &'a str) -> &'a str {
    contracts
        .ingest
        .source_dock
        .actions_rtl
        .iter()
        .find(|action| action.action_id() == Some(action_id))
        .map(|action| action.label())
        .unwrap_or(fallback)
}
