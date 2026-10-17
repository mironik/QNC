use super::*;

/// The source dock is the one source timeline (piece `qnc-source-timeline`); this form
/// adds only its own import actions to the header.
pub(super) fn render_source_dock(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let rect = ui.available_rect_before_wrap();
    let mut intent = None;
    let dock = &contracts.ingest.source_dock;
    let timeline_intent = qnc_source_timeline::show(
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
                    let mut style = form_action_bar_style(theme);
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
                            .map(qnc_ingest_layout::rgb),
                        &style,
                    ) {
                        intent = Some(IngestIntent::new(
                            action_ids::INGEST_SET_CLIP_FILTER,
                            IngestPayload::ClipFilter(modes[index]),
                        ));
                    }
                    // Uvezi (the strong one), Odaberi sve, Ocisti, drawn right to left.
                    for action_id in [action_ids::INGEST_IMPORT_SELECTED, action_ids::INGEST_SELECT_ALL, action_ids::INGEST_CLEAR_SELECTION] {
                        let (label, enabled) = (action_label(contracts, action_id), view.action_enabled(action_id));
                        let response = if action_id == action_ids::INGEST_IMPORT_SELECTED { primary_button(ui, label, enabled, theme) } else { action_button(ui, label, enabled, theme) };
                        ring(ui, &response, view, action_id, theme);
                        if response.clicked() {
                            intent = Some(IngestIntent::empty(action_id));
                        }
                    }
                }
                if contracts.ingest.source_dock.show_edit_actions {
                    for label in &dock.edit_actions {
                        let _ = action_button(ui, label, false, theme);
                    }
                }
                if view.archive_original_available {
                    let mut archive = view.archive_original;
                    if ui
                        .checkbox(
                            &mut archive,
                            RichText::new(&dock.archive_original_label)
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
                            RichText::new(&dock.ai_mining_label)
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
        (&view.timeline, timeline_theme(theme), &view.timeline_assets),
        &view.source_timeline,
    );
    if intent.is_none() {
        intent = timeline_intent_to_ingest_intent(timeline_intent);
    }
    intent
}

/// The frame of the dock button the keyboard is on.
fn ring(ui: &Ui, response: &egui::Response, view: &IngestViewModel, action_id: &str, theme: &Theme) {
    qnc_focus_walk::paint_focus(ui, response.rect.expand(2.0), view.focus.button.as_deref() == Some(action_id), theme.focus);
}

/// The label of a dock action from the layout contract (its action id when missing).
fn action_label<'a>(contracts: &'a IngestContracts, action_id: &'a str) -> &'a str {
    contracts
        .ingest
        .source_dock
        .actions_rtl
        .iter()
        .find(|action| action.action_id() == Some(action_id))
        .map(|action| action.label())
        .unwrap_or(action_id)
}
