use eframe::egui::{
    self, Align, Align2, Button, Color32, CornerRadius, FontId, Label, Layout, Rect, RichText,
    ScrollArea, Sense, Stroke, StrokeKind, Ui, Vec2,
};

use qnc_ingest_components::{
    action_ids, ClipView, IngestIntent, IngestPayload, IngestViewModel, LocationEntry, SourceKind,
};
use qnc_ui_kit::FormActionBarStyle;

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

fn render_left_column(
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

fn preview_height(rect: Rect, contracts: &IngestContracts) -> f32 {
    let preview = &contracts.ingest.preview;
    let preview_width = (rect.width() - 32.0).max(240.0);
    let aspect_height = preview_width / preview.aspect_ratio();
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

    let mut intent = None;
    show_chrome_row(ui, rect, theme, theme.surface, true, |ui| {
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
    });

    intent
}

fn render_location_browser(
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
                    view.source_kind == SourceKind::Local,
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

fn render_location_action_bar(
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
                && !view.work_settings_loading;
            let response = qnc_ui_kit::show_form_action_bar(
                ui,
                &form_action_bar_style(theme),
                &contracts.ingest.dir_browser.confirm_label,
                can_confirm,
                &contracts.ingest.dir_browser.cancel_label,
            );
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
                render_root_disk_grid(ui, view, theme, intent);
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
                ui.horizontal(|ui| {
                    ui.add_space(path_tree_offset());
                    ui.label(muted("Nema podmapa.", theme));
                });
            } else {
                ScrollArea::vertical().show(ui, |ui| {
                    for entry in &view.browser_entries {
                        ui.horizontal(|ui| {
                            ui.add_space(path_tree_offset());
                            if browser_entry_button(ui, entry, theme).clicked() {
                                *intent = Some(IngestIntent::new(
                                    action_ids::INGEST_DIR_OPEN,
                                    IngestPayload::LocationUri(entry.qnc_uri.clone()),
                                ));
                            }
                        });
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
    ui.add(Label::new(RichText::new(&entry.name).color(theme.text)).sense(Sense::click()))
}

fn browser_nav_height(view: &IngestViewModel, theme: &Theme) -> f32 {
    if view.source_kind == SourceKind::Local && view.browser_roots {
        let rows = view.browser_entries.len().max(1) as f32;
        (theme.chrome_control_height * rows) + (4.0 * (rows - 1.0))
    } else {
        theme.chrome_control_height
    }
}

fn render_root_disk_grid(
    ui: &mut Ui,
    view: &IngestViewModel,
    theme: &Theme,
    intent: &mut Option<IngestIntent>,
) {
    egui::Grid::new("qnc_ingest_root_disk_table")
        .num_columns(3)
        .spacing(Vec2::new(14.0, 4.0))
        .striped(false)
        .show(ui, |ui| {
            for entry in &view.browser_entries {
                let mut clicked = false;
                clicked |= root_disk_cell(ui, &entry.name, theme).clicked();
                clicked |= root_disk_cell(ui, &entry.serial_number, theme).clicked();
                clicked |= root_disk_cell(ui, &entry.volume_name, theme).clicked();
                ui.end_row();

                if clicked {
                    *intent = Some(IngestIntent::new(
                        action_ids::INGEST_DIR_OPEN,
                        IngestPayload::LocationUri(entry.qnc_uri.clone()),
                    ));
                }
            }
        });
}

fn root_disk_cell(ui: &mut Ui, text: &str, theme: &Theme) -> egui::Response {
    ui.add(
        Label::new(RichText::new(text).color(theme.text).size(theme.font_ui))
            .sense(Sense::click())
            .selectable(false),
    )
}

fn path_tree_offset() -> f32 {
    42.0 + 12.0 + 58.0 + 12.0
}

fn render_clip_grid(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let outer = ui.available_rect_before_wrap();
    ui.painter().rect_filled(outer, 0.0, theme.bg);
    let rect = outer.shrink(contracts.ingest.board.block_pad);

    if view.clips.is_empty() {
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(24.0);
                let message = view.work_settings_error.as_deref().unwrap_or_else(|| {
                    if view.work_settings_loading {
                        "Citanje radnih postavki..."
                    } else {
                        &contracts.ingest.clip_grid.empty_message
                    }
                });
                ui.label(muted(message, theme));
            });
        });
        return None;
    }

    let metrics = grid_metrics(rect.width(), view.clips.len(), contracts);
    let card_width = metrics.card_width;
    let card_height = metrics.card_height;
    let gap = metrics.gap;
    let columns = metrics.columns;
    let mut intent = None;

    ScrollArea::vertical().show_viewport(ui, |ui, _| {
        for row in view.clips.chunks(columns) {
            ui.horizontal(|ui| {
                for clip in row {
                    let card =
                        render_clip_card(ui, clip, Vec2::new(card_width, card_height), theme);
                    if card.clicked() {
                        let checkbox_click = card
                            .interact_pointer_pos()
                            .is_some_and(|pos| selection_check_hit_rect(card.rect).contains(pos));
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

fn render_clip_card(ui: &mut Ui, clip: &ClipView, size: Vec2, theme: &Theme) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let stroke = if clip.selected {
        Stroke::new(2.0, theme.danger)
    } else {
        Stroke::new(1.0, theme.border)
    };
    ui.painter().rect_filled(rect, 0.0, theme.surface_alt);
    ui.painter()
        .rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);
    let image_rect = Rect::from_min_size(
        rect.left_top(),
        Vec2::new(rect.width(), rect.width() * 9.0 / 16.0),
    );
    ui.painter()
        .rect_filled(image_rect.shrink(1.0), 0.0, theme.surface);
    ui.painter().text(
        image_rect.center(),
        Align2::CENTER_CENTER,
        "...",
        FontId::proportional(theme.font_ui),
        theme.text_muted,
    );
    paint_selection_check(ui, image_rect, clip.selected, theme);
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
        truncate(
            &clip.name,
            ((rect.width() - 76.0) / 7.0).floor().clamp(8.0, 42.0) as usize,
        ),
        FontId::proportional(theme.font_ui - 1.0),
        theme.text,
    );
    ui.painter().text(
        egui::pos2(rect.right() - 8.0, image_rect.bottom() + 8.0),
        Align2::RIGHT_TOP,
        format_duration(clip.duration_seconds),
        FontId::proportional(theme.font_ui - 1.0),
        theme.text_muted,
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
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if contracts.ingest.source_dock.show_import_actions {
                let status = view.status_label();
                if !status.is_empty() {
                    ui.label(muted(&status, theme));
                }
                if action_button(ui, "Osvježi", !view.command_busy, theme).clicked() {
                    intent = Some(IngestIntent::empty(action_ids::INGEST_RELOAD));
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

    render_timeline_placeholder(ui, timeline_rect, theme, view);

    intent
}

fn show_chrome_row(
    ui: &mut Ui,
    rect: Rect,
    theme: &Theme,
    fill: Color32,
    draw_bottom_rule: bool,
    add_contents: impl FnOnce(&mut Ui),
) {
    ui.painter().rect_filled(rect, 0.0, fill);
    if draw_bottom_rule {
        ui.painter().hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            Stroke::new(1.0, theme.border),
        );
    }
    let inner = Rect::from_min_max(
        egui::pos2(
            rect.left() + theme.chrome_pad_x,
            rect.top() + theme.chrome_pad_y,
        ),
        egui::pos2(
            rect.right() - theme.chrome_pad_x,
            rect.bottom() - theme.chrome_pad_y,
        ),
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.set_clip_rect(rect);
            ui.set_min_height(theme.chrome_control_height);
            ui.spacing_mut().button_padding = Vec2::new(8.0, 2.0);
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
            add_contents(ui);
        },
    );
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

struct GridMetrics {
    columns: usize,
    card_width: f32,
    card_height: f32,
    gap: f32,
}

fn grid_metrics(available_width: f32, count: usize, contracts: &IngestContracts) -> GridMetrics {
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

fn timeline_placeholder_height() -> f32 {
    15.0 + 3.0 + 64.0 + 3.0 + 15.0 + 2.0
}

fn render_timeline_placeholder(ui: &mut Ui, rect: Rect, theme: &Theme, view: &IngestViewModel) {
    let response = ui.interact(
        rect,
        ui.make_persistent_id("qnc_ingest_timeline_placeholder"),
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
        ("A1", rect.top() + 1.0, 15.0, theme.surface),
        ("V", rect.top() + 19.0, 64.0, theme.surface),
        ("A2", rect.top() + 86.0, 15.0, theme.bg),
    ];
    for (label, top, lane_h, fill) in lanes {
        let lane = Rect::from_min_size(
            egui::pos2(rect.left() + 28.0, top),
            Vec2::new((rect.width() - 28.0).max(10.0), lane_h),
        );
        let label_rect = Rect::from_min_size(egui::pos2(rect.left(), top), Vec2::new(28.0, lane_h));
        ui.painter().rect_filled(label_rect, 0.0, theme.surface_alt);
        ui.painter().text(
            label_rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(theme.font_ui - 1.0),
            theme.text_muted,
        );
        ui.painter().rect_filled(lane, 0.0, fill);
        ui.painter().rect_stroke(
            lane,
            0.0,
            Stroke::new(1.0, theme.border_soft),
            StrokeKind::Inside,
        );
    }
    let _ = view;

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
    let label = if selected {
        RichText::new(text)
            .color(color)
            .strong()
            .size(theme.font_ui)
    } else {
        RichText::new(text).color(color).size(theme.font_ui)
    };
    let response = ui.add(Label::new(label).sense(Sense::click()).selectable(false));
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

fn fixed_text_link(
    ui: &mut Ui,
    text: &str,
    enabled: bool,
    width: f32,
    theme: &Theme,
) -> egui::Response {
    ui.allocate_ui_with_layout(
        Vec2::new(width, theme.chrome_control_height),
        Layout::left_to_right(Align::Center),
        |ui| text_link(ui, text, enabled, theme),
    )
    .inner
}

fn small_button(ui: &mut Ui, text: &str, enabled: bool, theme: &Theme) -> egui::Response {
    ui.add_enabled(
        enabled,
        Button::new(RichText::new(text).color(theme.text))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, theme.border))
            .corner_radius(CornerRadius::same(0))
            .min_size(Vec2::new(40.0, theme.chrome_control_height)),
    )
}

fn action_button(ui: &mut Ui, text: &str, enabled: bool, theme: &Theme) -> egui::Response {
    ui.add_enabled(
        enabled,
        Button::new(RichText::new(text).color(theme.text))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, theme.border))
            .corner_radius(CornerRadius::same(0))
            .min_size(Vec2::new(0.0, theme.chrome_control_height)),
    )
}

fn primary_button(ui: &mut Ui, text: &str, enabled: bool, theme: &Theme) -> egui::Response {
    ui.add_enabled(
        enabled,
        Button::new(RichText::new(text).color(Color32::WHITE).strong())
            .fill(theme.accent)
            .stroke(Stroke::new(0.0, Color32::TRANSPARENT))
            .corner_radius(CornerRadius::same(0))
            .min_size(Vec2::new(0.0, theme.chrome_control_height)),
    )
}

fn form_action_bar_style(theme: &Theme) -> FormActionBarStyle {
    FormActionBarStyle::new(
        theme.text,
        theme.accent,
        theme.border,
        theme.font_ui,
        theme.chrome_control_height,
    )
}

fn selection_check_hit_rect(card_rect: Rect) -> Rect {
    selection_check_rect(card_rect).expand(4.0)
}

fn selection_check_rect(card_rect: Rect) -> Rect {
    let thumb_height = (card_rect.height() - 34.0).max(72.0);
    let size = 16.0;
    let pad = 6.0;
    Rect::from_min_size(
        egui::pos2(
            card_rect.left() + pad,
            card_rect.top() + thumb_height - pad - size,
        ),
        Vec2::splat(size),
    )
}

fn paint_selection_check(ui: &Ui, thumb_rect: Rect, checked: bool, theme: &Theme) {
    let check_rect = selection_check_rect(Rect::from_min_max(
        thumb_rect.left_top(),
        egui::pos2(thumb_rect.right(), thumb_rect.bottom() + 34.0),
    ));
    if checked {
        let fill = Color32::from_rgb(0xff, 0x95, 0x00);
        ui.painter().rect_filled(check_rect, 3.0, fill);
        ui.painter()
            .rect_stroke(check_rect, 3.0, Stroke::new(1.5, fill), StrokeKind::Inside);
        let c = check_rect.center();
        let dark = Color32::from_rgb(0x1a, 0x1a, 0x1a);
        ui.painter().line_segment(
            [egui::pos2(c.x - 3.5, c.y), egui::pos2(c.x - 1.0, c.y + 3.0)],
            Stroke::new(2.0, dark),
        );
        ui.painter().line_segment(
            [
                egui::pos2(c.x - 1.0, c.y + 3.0),
                egui::pos2(c.x + 4.0, c.y - 3.0),
            ],
            Stroke::new(2.0, dark),
        );
    } else {
        ui.painter().rect_filled(
            check_rect,
            3.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, 90),
        );
        ui.painter().rect_stroke(
            check_rect,
            3.0,
            Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 255, 255, 140)),
            StrokeKind::Inside,
        );
    }
    let _ = theme;
}

fn truncate(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let head = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{head}...")
    } else {
        head
    }
}

fn format_duration(seconds: f64) -> String {
    if !seconds.is_finite() || seconds <= 0.0 {
        return "00:00".to_string();
    }
    let total = seconds.round() as i64;
    let minutes = total / 60;
    let secs = total % 60;
    format!("{minutes:02}:{secs:02}")
}

fn muted(text: &str, theme: &Theme) -> RichText {
    RichText::new(text).color(theme.text_muted)
}
