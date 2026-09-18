// Copied 1:1 from qnc-ingest-desktop/src/widgets.rs. Left out on purpose: the
// right clip grid and the directory browser (its area stays empty and is
// filled by later group functions). Painting, metrics and helpers are unchanged.
use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, Label, Layout, Rect, RichText, Sense, Stroke, Ui,
    Vec2,
};

use qnc_monitor::{MonitorChrome, MonitorPaint, MonitorPicture, MonitorSurface};
use qnc_timeline::{TimelineIntent, TimelineTheme};

use crate::{
    layout_contract::EditorialContracts,
    theme::Theme,
    view::{action_ids, EditorialIntent, EditorialView},
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
    let action_rect = Rect::from_min_size(
        egui::pos2(
            browser_rect.left(),
            (browser_rect.bottom()
                - theme.chrome_control_height
                - contracts.editorial.board.block_pad)
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
    // Where Ingest shows the directory browser: empty (clip selection later).
    ui.painter()
        .rect_filled(browser_content_rect, 0.0, theme.bg);
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
        .monitor_frame
        .as_ref()
        .filter(|_| view.video_visible)
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
            message: view.monitor_message.as_deref(),
        },
    ) {
        MonitorPaint::Picture | MonitorPaint::Message => return,
        MonitorPaint::Empty => {}
    }
    if let Some(poster) = &view.poster {
        if qnc_ui_kit::paint_rgba_image(
            ui,
            rect.shrink(1.0),
            &poster.uri,
            poster.content_key,
            poster.size,
            &poster.pixels,
        ) {
            return;
        }
    }
    let label = view
        .clip_label
        .as_deref()
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
    match &view.clip_label {
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
        &view.timeline,
        timeline_theme(theme),
        view.filmstrip.as_ref(),
        &view.a1_peaks,
        &view.a2_peaks,
        &view.a3_peaks,
        &view.a4_peaks,
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
