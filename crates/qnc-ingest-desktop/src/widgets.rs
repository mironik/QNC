use eframe::egui::{
    self, Align, Align2, Button, Color32, CornerRadius, FontId, Frame, Label, Layout, Rect,
    RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui, Vec2,
};

use qnc_ingest_components::{
    action_ids, ClipView, IngestIntent, IngestPayload, IngestViewModel, LocationEntry, SourceKind,
};

use crate::{
    layout_contract::{IngestContracts, IngestDirBrowser},
    theme::Theme,
};

pub fn render_desktop(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let available = ui.available_rect_before_wrap();
    if available.width() <= 1.0 || available.height() <= 1.0 {
        return None;
    }

    ui.allocate_rect(available, Sense::hover());
    ui.painter().rect_filled(available, 0.0, theme.bg);

    let dock_height = contracts.dock_height().min(available.height() * 0.42);
    let dock_rect = Rect::from_min_max(
        egui::pos2(available.left(), available.bottom() - dock_height),
        available.right_bottom(),
    );
    let content_rect = Rect::from_min_max(
        available.left_top(),
        egui::pos2(available.right(), dock_rect.top()),
    );

    let mut intent = None;

    ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
        intent = render_board(ui, contracts, theme, view);
    });

    if intent.is_none() {
        ui.scope_builder(egui::UiBuilder::new().max_rect(dock_rect), |ui| {
            intent = render_source_dock(ui, contracts, theme, view);
        });
    }

    intent
}

fn render_board(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let metrics = &contracts.ingest.board;
    let rect = ui
        .available_rect_before_wrap()
        .shrink2(Vec2::new(metrics.shell_margin_x, 0.0));
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

    ui.painter().rect_filled(left_rect, 0.0, theme.bg);
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

fn render_left_column(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let pad = contracts.ingest.board.block_pad;
    let gap = contracts.ingest.board.gap;
    let rect = ui.available_rect_before_wrap().shrink(pad);
    ui.allocate_rect(rect, Sense::hover());

    let preview_h = preview_height(rect, contracts);
    let preview_rect = Rect::from_min_size(rect.left_top(), Vec2::new(rect.width(), preview_h));
    let head_rect = Rect::from_min_size(
        egui::pos2(rect.left(), preview_rect.bottom() + gap),
        Vec2::new(rect.width(), theme.chrome_row_height),
    );
    let browser_rect = Rect::from_min_max(
        egui::pos2(rect.left(), head_rect.bottom() + gap),
        rect.right_bottom(),
    );

    render_preview(ui, preview_rect, contracts, theme, view);

    let mut intent = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(head_rect), |ui| {
        intent = render_pool_head(ui, contracts, theme);
    });
    if intent.is_none() {
        ui.scope_builder(egui::UiBuilder::new().max_rect(browser_rect), |ui| {
            intent = render_location_browser(ui, contracts, theme, view);
        });
    }
    intent
}

fn preview_height(rect: Rect, contracts: &IngestContracts) -> f32 {
    let preview = &contracts.ingest.preview;
    let aspect_height = rect.width() / preview.aspect_ratio();
    let available = (rect.height() - preview.reserve_below).max(preview.min_height);
    aspect_height.clamp(preview.min_height, available)
}

fn render_preview(
    ui: &mut Ui,
    rect: Rect,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) {
    ui.allocate_rect(rect, Sense::hover());
    ui.painter().rect_filled(rect, 0.0, theme.black);
    ui.painter().rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, theme.border),
        StrokeKind::Inside,
    );
    let label = if view.preview_clip_id.is_some() {
        view.current_clip_label()
    } else {
        contracts.ingest.preview.empty_label.as_str()
    };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(theme.font_ui),
        theme.text_muted,
    );
}

fn render_pool_head(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
) -> Option<IngestIntent> {
    let rect = ui.available_rect_before_wrap();
    ui.allocate_rect(rect, Sense::hover());
    ui.painter().rect_filled(rect, 0.0, theme.surface);

    let mut intent = None;
    ui.horizontal(|ui| {
        for (index, tab) in contracts.ingest.pool_head.tabs_left.iter().enumerate() {
            let selected = index == 0;
            let response = text_tab(ui, tab, selected, theme);
            if response.clicked() && !selected {
                intent = Some(IngestIntent::empty(action_ids::INGEST_RELOAD));
            }
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            for command in &contracts.ingest.pool_head.transport_right {
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
    });

    intent
}

fn render_location_browser(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let rect = ui.available_rect_before_wrap();
    ui.allocate_rect(rect, Sense::hover());
    ui.painter().rect_filled(rect, 0.0, theme.bg);
    ui.painter().rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, theme.border_soft),
        StrokeKind::Inside,
    );

    let mut intent = None;
    let frame = Frame::NONE.inner_margin(egui::Margin {
        left: 0,
        right: 0,
        top: 6,
        bottom: 6,
    });
    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(muted(&contracts.ingest.dir_browser.sources_label, theme));
            for kind in [SourceKind::Local, SourceKind::Lan, SourceKind::Internet] {
                let selected = view.source_kind == kind;
                let label = source_kind_label(&contracts.ingest.dir_browser, kind);
                if text_tab(ui, label, selected, theme).clicked() && !selected {
                    intent = Some(IngestIntent::new(
                        kind.action_id(),
                        IngestPayload::SourceKind(kind),
                    ));
                }
            }
        });

        ui.add_space(5.0);
        ui.horizontal(|ui| {
            let up_enabled = view.browser_parent_available;
            if text_link(
                ui,
                &contracts.ingest.dir_browser.up_label,
                up_enabled,
                theme,
            )
            .clicked()
            {
                intent = Some(IngestIntent::empty(action_ids::INGEST_DIR_UP));
            }
            ui.label(
                RichText::new(format!("{}:", contracts.ingest.dir_browser.disks_label))
                    .color(theme.text),
            );
            render_browser_head_entries(
                ui,
                &contracts.ingest.dir_browser,
                theme,
                view,
                &mut intent,
            );
        });

        ui.add_space(4.0);
        let body_height = (ui.available_height() - theme.chrome_row_height - 8.0).max(20.0);
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), body_height),
            Layout::top_down(Align::Min),
            |ui| {
                render_browser_body(ui, &contracts.ingest.dir_browser, theme, view, &mut intent);
            },
        );

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if action_button(ui, &contracts.ingest.dir_browser.cancel_label, true, theme).clicked()
            {
                intent = Some(IngestIntent::empty(action_ids::INGEST_DIR_CANCEL));
            }
            let can_confirm = !view.browser_roots && !view.browser_path_label.is_empty();
            if primary_button(
                ui,
                &contracts.ingest.dir_browser.confirm_label,
                can_confirm,
                theme,
            )
            .clicked()
            {
                intent = Some(IngestIntent::new(
                    action_ids::INGEST_DIR_CONFIRM,
                    IngestPayload::LocationUri(view.browser_path_label.clone()),
                ));
            }
        });
    });

    intent
}

fn render_browser_head_entries(
    ui: &mut Ui,
    labels: &IngestDirBrowser,
    theme: &Theme,
    view: &IngestViewModel,
    intent: &mut Option<IngestIntent>,
) {
    match view.source_kind {
        SourceKind::Local if view.browser_roots => {
            if view.browser_entries.is_empty() {
                ui.label(muted("Nema diskova.", theme));
            } else {
                for entry in &view.browser_entries {
                    if browser_entry_button(ui, entry, theme).clicked() {
                        *intent = Some(IngestIntent::new(
                            action_ids::INGEST_DIR_OPEN,
                            IngestPayload::LocationUri(entry.qnc_uri.clone()),
                        ));
                    }
                }
            }
        }
        SourceKind::Local => {
            ui.label(RichText::new(&view.browser_path_label).color(theme.text));
        }
        SourceKind::Lan => {
            ui.label(muted(&labels.empty_lan, theme));
        }
        SourceKind::Internet => {
            ui.label(muted(&labels.empty_internet, theme));
        }
    }
}

fn render_browser_body(
    ui: &mut Ui,
    labels: &IngestDirBrowser,
    theme: &Theme,
    view: &IngestViewModel,
    intent: &mut Option<IngestIntent>,
) {
    if view.browser_busy {
        ui.label(muted("Učitavam...", theme));
        return;
    }
    if let Some(error) = &view.browser_error {
        ui.label(RichText::new(error).color(theme.danger));
        return;
    }
    match view.source_kind {
        SourceKind::Local if view.browser_roots => {}
        SourceKind::Local => {
            if view.browser_entries.is_empty() {
                ui.label(muted("Nema stavki.", theme));
            } else {
                ScrollArea::vertical().show(ui, |ui| {
                    for entry in &view.browser_entries {
                        if browser_entry_button(ui, entry, theme).clicked() {
                            *intent = Some(IngestIntent::new(
                                action_ids::INGEST_DIR_OPEN,
                                IngestPayload::LocationUri(entry.qnc_uri.clone()),
                            ));
                        }
                    }
                });
            }
        }
        SourceKind::Lan => {
            ui.label(muted(&labels.empty_lan, theme));
        }
        SourceKind::Internet => {
            ui.label(muted(&labels.empty_internet, theme));
        }
    }
}

fn browser_entry_button(ui: &mut Ui, entry: &LocationEntry, theme: &Theme) -> egui::Response {
    let mut line = entry.name.clone();
    if !entry.serial_number.is_empty() {
        line.push_str("   ");
        line.push_str(&entry.serial_number);
    }
    if !entry.volume_name.is_empty() {
        line.push_str("   ");
        line.push_str(&entry.volume_name);
    }
    ui.add(Label::new(RichText::new(line).color(theme.text)).sense(Sense::click()))
}

fn render_clip_grid(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let rect = ui
        .available_rect_before_wrap()
        .shrink(contracts.ingest.board.block_pad);
    ui.allocate_rect(rect, Sense::hover());
    ui.painter().rect_filled(rect, 0.0, theme.bg);

    if view.clips.is_empty() {
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            &contracts.ingest.clip_grid.empty_message,
            FontId::proportional(theme.font_ui),
            theme.text_muted,
        );
        return None;
    }

    let card_width = contracts.ingest.clip_grid.min_card_width;
    let card_height = (card_width * 9.0 / 16.0) + contracts.ingest.clip_grid.card_text_height;
    let gap = contracts.ingest.clip_grid.grid_gap;
    let columns = ((rect.width() + gap) / (card_width + gap)).floor().max(1.0) as usize;
    let mut intent = None;

    ScrollArea::vertical().show_viewport(ui, |ui, _| {
        for row in view.clips.chunks(columns) {
            ui.horizontal(|ui| {
                for clip in row {
                    if render_clip_card(ui, clip, Vec2::new(card_width, card_height), theme)
                        .clicked()
                    {
                        intent = Some(IngestIntent::new(
                            action_ids::INGEST_PREVIEW_FOCUS,
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

fn render_clip_card(ui: &mut Ui, clip: &ClipView, size: Vec2, theme: &Theme) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let stroke = if clip.selected {
        Stroke::new(1.0, theme.accent)
    } else {
        Stroke::new(1.0, theme.border)
    };
    ui.painter().rect_filled(rect, 0.0, theme.panel);
    ui.painter()
        .rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);
    let image_rect = Rect::from_min_size(
        rect.left_top(),
        Vec2::new(rect.width(), rect.width() * 9.0 / 16.0),
    );
    ui.painter()
        .rect_filled(image_rect.shrink(1.0), 0.0, theme.black);
    let marker = if clip.imported {
        Color32::from_rgb(55, 210, 145)
    } else {
        theme.text_muted
    };
    ui.painter().circle_filled(
        egui::pos2(rect.right() - 10.0, rect.top() + 10.0),
        4.0,
        marker,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 8.0, image_rect.bottom() + 8.0),
        Align2::LEFT_TOP,
        &clip.name,
        FontId::proportional(theme.font_ui - 1.0),
        theme.text,
    );
    response
}

fn render_source_dock(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let rect = ui.available_rect_before_wrap();
    ui.allocate_rect(rect, Sense::hover());
    ui.painter().rect_filled(rect, 0.0, theme.panel_alt);
    ui.painter().line_segment(
        [rect.left_top(), rect.right_top()],
        Stroke::new(1.0, theme.border),
    );

    let mut intent = None;
    let frame = Frame::NONE.inner_margin(egui::Margin {
        left: 8,
        right: 8,
        top: 4,
        bottom: 6,
    });
    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(source_dock_clip_label(contracts, view)).color(theme.text));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if contracts.ingest.source_dock.show_import_actions {
                    for action in &contracts.ingest.source_dock.actions_rtl {
                        let enabled = action_enabled(action, view);
                        let response = if action == "Uvezi" {
                            primary_button(ui, action, enabled, theme)
                        } else {
                            action_button(ui, action, enabled, theme)
                        };
                        if response.clicked() {
                            intent = Some(intent_for_dock_action(action));
                        }
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
                            RichText::new("Kopiraj original").color(theme.text),
                        )
                        .changed()
                    {
                        intent = Some(IngestIntent::new(
                            action_ids::INGEST_SET_ARCHIVE,
                            IngestPayload::Bool(archive),
                        ));
                    }
                } else {
                    ui.add_enabled(false, egui::Checkbox::new(&mut false, "Kopiraj original"));
                }
                let mut ai = view.ai_mining;
                if ui
                    .checkbox(&mut ai, RichText::new("AI mining").color(theme.text))
                    .changed()
                {
                    intent = Some(IngestIntent::new(
                        action_ids::INGEST_SET_AI_MINING,
                        IngestPayload::Bool(ai),
                    ));
                }
            });
        });

        ui.add_space(contracts.ingest.source_dock.header_timeline_gap);
        render_timeline_placeholder(ui, theme, view);
    });

    intent
}

fn action_enabled(action: &str, view: &IngestViewModel) -> bool {
    match action {
        "Uvezi" => view.selected_count() > 0 && !view.command_busy,
        "Očisti" => view.selected_count() > 0,
        "Odaberi sve" => view.total_count() > 0,
        "Generiraj postere" => view.total_count() > 0,
        _ => !view.command_busy,
    }
}

fn source_kind_label(labels: &IngestDirBrowser, kind: SourceKind) -> &str {
    let index = match kind {
        SourceKind::Local => 0,
        SourceKind::Lan => 1,
        SourceKind::Internet => 2,
    };
    labels
        .kinds
        .get(index)
        .map(String::as_str)
        .unwrap_or_else(|| kind.label())
}

fn source_dock_clip_label<'a>(
    contracts: &'a IngestContracts,
    view: &'a IngestViewModel,
) -> &'a str {
    if view.preview_clip_id.is_some() {
        view.current_clip_label()
    } else {
        &contracts.ingest.source_dock.clip_label_fallback
    }
}

fn intent_for_dock_action(action: &str) -> IngestIntent {
    match action {
        "Uvezi" => IngestIntent::empty(action_ids::INGEST_IMPORT_SELECTED),
        "Očisti" => IngestIntent::empty(action_ids::INGEST_CLEAR_SELECTION),
        "Odaberi sve" => IngestIntent::empty(action_ids::INGEST_SELECT_ALL),
        "Generiraj postere" => IngestIntent::empty(action_ids::INGEST_APPROVE_PROXY_POSTERS),
        "Osvježi" => IngestIntent::empty(action_ids::INGEST_RELOAD),
        _ => IngestIntent::empty(action_ids::INGEST_RELOAD),
    }
}

fn render_timeline_placeholder(ui: &mut Ui, theme: &Theme, view: &IngestViewModel) {
    let height = (ui.available_height() - 2.0).max(32.0);
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        Sense::click_and_drag(),
    );
    ui.painter().rect_filled(rect, 0.0, theme.bg);
    ui.painter().rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, theme.border_soft),
        StrokeKind::Inside,
    );

    let lanes = [
        ("V", rect.top() + 8.0, 24.0),
        ("A1", rect.top() + 40.0, 18.0),
        ("A2", rect.top() + 62.0, 18.0),
    ];
    for (label, top, lane_h) in lanes {
        let lane = Rect::from_min_size(
            egui::pos2(rect.left() + 42.0, top),
            Vec2::new((rect.width() - 50.0).max(10.0), lane_h),
        );
        ui.painter().text(
            egui::pos2(rect.left() + 10.0, top + lane_h * 0.5),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(theme.font_ui - 1.0),
            theme.text_muted,
        );
        ui.painter().rect_filled(lane, 0.0, theme.input_bg);
        ui.painter().rect_stroke(
            lane,
            0.0,
            Stroke::new(1.0, theme.border_soft),
            StrokeKind::Inside,
        );
    }
    ui.painter().text(
        egui::pos2(rect.right() - 10.0, rect.top() + 10.0),
        Align2::RIGHT_TOP,
        view.status_label(),
        FontId::proportional(theme.font_ui - 1.0),
        theme.text_muted,
    );

    if response.clicked() {
        ui.ctx().request_repaint();
    }
}

fn text_tab(ui: &mut Ui, text: &str, selected: bool, theme: &Theme) -> egui::Response {
    let color = if selected {
        theme.text
    } else {
        theme.text_muted
    };
    let response = ui.add(Label::new(RichText::new(text).color(color)).sense(Sense::click()));
    if selected {
        let y = response.rect.bottom() + 2.0;
        ui.painter().line_segment(
            [
                egui::pos2(response.rect.left(), y),
                egui::pos2(response.rect.right(), y),
            ],
            Stroke::new(2.0, theme.accent),
        );
    }
    response
}

fn text_link(ui: &mut Ui, text: &str, enabled: bool, theme: &Theme) -> egui::Response {
    ui.add_enabled(
        enabled,
        Label::new(RichText::new(text).color(if enabled {
            theme.text
        } else {
            theme.text_muted
        }))
        .sense(Sense::click()),
    )
}

fn small_button(ui: &mut Ui, text: &str, enabled: bool, theme: &Theme) -> egui::Response {
    ui.add_enabled(
        enabled,
        Button::new(RichText::new(text).color(theme.text))
            .fill(theme.input_bg)
            .stroke(Stroke::new(1.0, theme.border))
            .corner_radius(CornerRadius::same(0))
            .min_size(Vec2::new(32.0, theme.chrome_control_height)),
    )
}

fn action_button(ui: &mut Ui, text: &str, enabled: bool, theme: &Theme) -> egui::Response {
    ui.add_enabled(
        enabled,
        Button::new(RichText::new(text).color(theme.text))
            .fill(theme.input_bg)
            .stroke(Stroke::new(1.0, theme.border))
            .corner_radius(CornerRadius::same(0))
            .min_size(Vec2::new(88.0, theme.chrome_control_height)),
    )
}

fn primary_button(ui: &mut Ui, text: &str, enabled: bool, theme: &Theme) -> egui::Response {
    ui.add_enabled(
        enabled,
        Button::new(RichText::new(text).color(theme.text))
            .fill(theme.accent)
            .stroke(Stroke::new(1.0, theme.accent))
            .corner_radius(CornerRadius::same(0))
            .min_size(Vec2::new(88.0, theme.chrome_control_height)),
    )
}

fn muted(text: &str, theme: &Theme) -> RichText {
    RichText::new(text).color(theme.text_muted)
}
