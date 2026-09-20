// Copied 1:1 from qnc-ingest-desktop/src/widgets.rs. Left out on purpose: the
// right clip grid and the directory browser (its area stays empty and is
// filled by later group functions). Painting, metrics and helpers are unchanged.
use eframe::egui::{
    self, Align, Align2, Button, Color32, CornerRadius, FontId, Label, Layout, Rect, RichText,
    ScrollArea, Sense, Stroke, StrokeKind, Ui,
    Vec2,
};

use qnc_monitor::{MonitorChrome, MonitorPaint, MonitorPicture, MonitorSurface};
use qnc_timeline::{TimelineIntent, TimelineTheme};

use qnc_editorial_application::{action_ids, EditorialClip, EditorialIntent, EditorialView};

use crate::{
    layout_contract::EditorialContracts,
    theme::Theme,
};

pub fn render_desktop(
    ui: &mut Ui,
    contracts: &EditorialContracts,
    theme: &Theme,
    view: &EditorialView,
) -> Option<EditorialIntent> {
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
    contracts: &EditorialContracts,
    theme: &Theme,
    view: &EditorialView,
) -> Option<EditorialIntent> {
    let metrics = &contracts.editorial.board;
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
    // Right panel: empty, reserved for the functions of the group.
    ui.painter().rect_filled(right_rect, 0.0, theme.bg);

    let mut intent = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
        intent = render_left_column(ui, contracts, theme, view);
    });
    intent
}

fn render_left_column(
    ui: &mut Ui,
    contracts: &EditorialContracts,
    theme: &Theme,
    view: &EditorialView,
) -> Option<EditorialIntent> {
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

    render_preview(ui, preview_rect, contracts, theme, view);

    let mut intent = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(head_rect), |ui| {
        intent = render_pool_head(ui, contracts, theme);
    });
    // Clip menu (qnc_v5 media pool): the card grid fills the column under the
    // pool head, down to the dock, on the panel background.
    if intent.is_none() {
        ui.scope_builder(egui::UiBuilder::new().max_rect(browser_rect), |ui| {
            intent = render_clip_grid(ui, contracts, theme, view);
        });
    }
    intent
}

fn preview_height(rect: Rect, contracts: &EditorialContracts) -> f32 {
    let preview = &contracts.editorial.preview;
    let preview_width = (rect.width() - 32.0).max(240.0);
    let aspect_height = preview_width / preview.aspect_ratio();
    let available = (rect.height() - preview.reserve_below).max(preview.min_height);
    aspect_height.clamp(preview.min_height, available)
}

fn render_preview(
    ui: &mut Ui,
    rect: Rect,
    contracts: &EditorialContracts,
    theme: &Theme,
    view: &EditorialView,
) {
    let chrome = MonitorChrome {
        fill: theme.black,
        border: theme.border,
        muted: theme.text_muted,
        font_size: theme.font_ui,
    };
    let picture = view
        .preview
        .monitor_frame
        .as_ref()
        .filter(|_| view.preview.video_visible)
        .map(|frame| MonitorPicture {
            session_id: &frame.session_id,
            generation: frame.generation,
            sequence: frame.sequence,
            size: [frame.width, frame.height],
            rgba: &frame.rgba,
        });
    match qnc_monitor::paint_monitor(
        ui,
        rect,
        MonitorSurface {
            id: egui::Id::new(("qnc-monitor", "editorial-source")),
            chrome,
            picture,
            message: view.preview.monitor_message.as_deref(),
        },
    ) {
        MonitorPaint::Picture | MonitorPaint::Message => return,
        MonitorPaint::Empty => {}
    }
    let label = view
        .current_clip_label()
        .unwrap_or(contracts.editorial.preview.empty_label.as_str());
    qnc_monitor::paint_placeholder(ui, rect, chrome, label);
}

fn render_pool_head(
    ui: &mut Ui,
    contracts: &EditorialContracts,
    theme: &Theme,
) -> Option<EditorialIntent> {
    let rect = ui.available_rect_before_wrap();

    let mut intent = None;
    show_chrome_row(ui, rect, theme, theme.surface, true, |ui| {
        ui.spacing_mut().button_padding = Vec2::new(8.0, 2.0);
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
        for (index, tab) in contracts.editorial.pool_head.tabs_left.iter().enumerate() {
            let selected = index == 0;
            let _ = text_tab(ui, tab, selected, theme);
            ui.add_space(10.0);
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            for command in contracts.editorial.pool_head.transport_right.iter().rev() {
                let action_id = match command.as_str() {
                    ">" => action_ids::PLAY_PAUSE,
                    "[" => action_ids::STEP_BACK_FRAME,
                    "]" => action_ids::STEP_FORWARD_FRAME,
                    _ => continue,
                };
                if small_button(ui, command, true, theme).clicked() {
                    intent = Some(EditorialIntent::Action(action_id));
                }
            }
        });
    });

    intent
}

fn render_source_dock(
    ui: &mut Ui,
    contracts: &EditorialContracts,
    theme: &Theme,
    view: &EditorialView,
) -> Option<EditorialIntent> {
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
            header_rect.bottom() + contracts.editorial.source_dock.header_timeline_gap,
        ),
        Vec2::new(
            inner.width().max(0.0),
            timeline_placeholder_height().min(
                (inner.bottom()
                    - header_rect.bottom()
                    - contracts.editorial.source_dock.header_timeline_gap)
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
            // Group actions (contract `actions_rtl`): shown, not wired yet.
            for label in &contracts.composition().source_dock.actions_rtl {
                let _ = action_button(ui, label, false, theme);
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

fn source_dock_clip_label<'a>(
    contracts: &'a EditorialContracts,
    view: &'a EditorialView,
) -> &'a str {
    match view.current_clip_label() {
        Some(label) => label,
        None => &contracts.editorial.source_dock.clip_label_fallback,
    }
}

fn timeline_placeholder_height() -> f32 {
    qnc_timeline::source_player_timeline_height()
}

fn render_player_timeline(
    ui: &mut Ui,
    rect: Rect,
    theme: &Theme,
    view: &EditorialView,
) -> Option<EditorialIntent> {
    let intent = qnc_timeline::show_source_player_timeline_with_artifacts(
        ui,
        rect,
        &view.preview.timeline,
        timeline_theme(theme),
        view.preview.assets.filmstrip_background(),
        view.preview.assets.a1_peaks(),
        view.preview.assets.a2_peaks(),
        view.preview.assets.a3_peaks(),
        view.preview.assets.a4_peaks(),
    );
    match intent {
        TimelineIntent::None => None,
        other => Some(EditorialIntent::Timeline(other)),
    }
}

fn timeline_theme(theme: &Theme) -> TimelineTheme {
    TimelineTheme::from_qnc_theme(
        theme.bg,
        theme.surface,
        theme.surface_alt,
        theme.border_soft,
        theme.text,
        theme.text_muted,
        theme.accent,
    )
}

struct GridMetrics {
    columns: usize,
    card_width: f32,
    card_height: f32,
    gap: f32,
}

/// Same arithmetic as the Ingest clip grid (usable width is the panel width less 8).
fn grid_metrics(available_width: f32, count: usize, contracts: &EditorialContracts) -> GridMetrics {
    let card = &contracts.editorial.media_card;
    let count = count.max(1);
    let usable_width = (available_width - 8.0).max(card.min_card_width);
    let columns = (((usable_width + card.grid_gap) / (card.min_card_width + card.grid_gap)).floor()
        as usize)
        .max(1)
        .min(count);
    let card_width = (usable_width - card.grid_gap * columns.saturating_sub(1) as f32) / columns as f32;
    let card_height = card_width * 9.0 / 16.0 + card.card_text_height;
    GridMetrics {
        columns,
        card_width,
        card_height,
        gap: card.grid_gap,
    }
}

/// The clip menu: a virtualized card grid of the project clips. Click on a card
/// chooses the clip for the preview. Presentation as in Ingest and the qnc_v5
/// media pool: chosen card has a 2 px red outline, others a 1 px border.
fn render_clip_grid(
    ui: &mut Ui,
    contracts: &EditorialContracts,
    theme: &Theme,
    view: &EditorialView,
) -> Option<EditorialIntent> {
    let outer = ui.available_rect_before_wrap();
    ui.painter().rect_filled(outer, 0.0, theme.bg);
    let rect = outer.shrink(contracts.editorial.board.block_pad);
    let clips = &view.clips;

    if clips.is_empty() {
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(24.0);
                let message = if view.loading {
                    "Citam projektni katalog..."
                } else if !view.message.is_empty() {
                    view.message.as_str()
                } else {
                    contracts.editorial.media_card.empty_message.as_str()
                };
                ui.label(RichText::new(message).color(theme.text_muted));
            });
        });
        return None;
    }

    let metrics = grid_metrics(rect.width(), clips.len(), contracts);
    let show_check = contracts.composition().media_card.selection_check;
    let dots = qnc_media_card::StatusDotsMode::from_contract(&contracts.composition().media_card.status_dots)
        .unwrap_or(qnc_media_card::StatusDotsMode::Off);
    let row_stride = metrics.card_height + metrics.gap;
    let total_rows = clips.len().div_ceil(metrics.columns);
    let mut intent = None;

    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ScrollArea::vertical()
            .id_salt("editorial_clip_grid")
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, viewport| {
                let first_row = (viewport.top() / row_stride).floor().max(0.0) as usize;
                let last_row =
                    ((viewport.bottom() / row_stride).ceil() as usize + 1).min(total_rows);
                ui.add_space(first_row as f32 * row_stride);
                for row_index in first_row..last_row {
                    let start = row_index * metrics.columns;
                    let end = (start + metrics.columns).min(clips.len());
                    ui.horizontal(|ui| {
                        for clip in &clips[start..end] {
                            let chosen = view.chosen_clip_id() == Some(clip.clip_id.as_str());
                            let card = render_clip_card(
                                ui,
                                clip,
                                chosen,
                                show_check,
                                dots,
                                Vec2::new(metrics.card_width, metrics.card_height),
                                theme,
                            );
                            if card.clicked() {
                                intent = Some(EditorialIntent::PreviewClip(clip.clip_id.clone()));
                            }
                            ui.add_space(metrics.gap);
                        }
                    });
                    ui.add_space(metrics.gap);
                }
                let rendered = last_row.saturating_sub(first_row);
                let remaining = total_rows.saturating_sub(first_row + rendered);
                ui.add_space(remaining as f32 * row_stride);
            });
    });

    intent
}

/// Card painting as in the Ingest clip grid: the poster once it is loaded, the "..." placeholder until then.
fn render_clip_card(
    ui: &mut Ui,
    clip: &EditorialClip,
    chosen: bool,
    show_check: bool,
    dots: qnc_media_card::StatusDotsMode,
    size: Vec2,
    theme: &Theme,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let stroke = if chosen {
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
    let painted = match (&clip.thumb_uri, &clip.thumb_image) {
        (Some(uri), Some(image)) => qnc_ui_kit::paint_rgba_image(
            ui,
            image_rect.shrink(1.0),
            uri,
            image.content_key,
            image.size,
            &image.pixels,
        ),
        _ => false,
    };
    if !painted {
        ui.painter().text(
            image_rect.center(),
            Align2::CENTER_CENTER,
            "...",
            FontId::proportional(theme.font_ui),
            theme.text_muted,
        );
    }
    if show_check {
        // Media Assist mirrors the chosen clip in the check mark (qnc_v5).
        paint_selection_check(ui, image_rect, chosen);
    }
    let name_font = FontId::proportional(theme.font_ui - 1.0);
    let name = truncate(
        &clip.name,
        ((rect.width() - 76.0 - 22.0) / 7.0).floor().clamp(8.0, 42.0) as usize,
    );
    let name_width = ui.fonts(|f| f.layout_no_wrap(name.clone(), name_font.clone(), theme.text).size().x);
    let name_top = image_rect.bottom() + 8.0;
    ui.painter().text(
        egui::pos2(rect.left() + 8.0, name_top),
        Align2::LEFT_TOP,
        name,
        name_font,
        theme.text,
    );
    // The status dots beside the name come from the public card module: the form only passes
    // what the project database says about the clip.
    qnc_media_card::paint_clip_status(
        ui.painter(),
        egui::pos2(rect.left() + 8.0 + name_width + 8.0, name_top + (theme.font_ui - 1.0) * 0.6),
        dots,
        &clip.import_status,
        &clip.imported_media_uri,
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

fn selection_check_rect(image_rect: Rect) -> Rect {
    let size = 16.0;
    let pad = 6.0;
    Rect::from_min_size(
        egui::pos2(image_rect.left() + pad, image_rect.bottom() - pad - size),
        Vec2::splat(size),
    )
}

fn paint_selection_check(ui: &Ui, image_rect: Rect, checked: bool) {
    let check_rect = selection_check_rect(image_rect);
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
