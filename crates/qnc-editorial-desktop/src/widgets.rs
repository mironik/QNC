// Copied 1:1 from qnc-ingest-desktop/src/widgets.rs. Left out on purpose: the
// right clip grid and the directory browser (its area stays empty and is
// filled by later group functions). Painting, metrics and helpers are unchanged.
use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, Label, Layout, Rect, RichText, Sense, Stroke, Ui,
    Vec2,
};

use qnc_monitor::{MonitorChrome, MonitorPicture, MonitorPoster, MonitorSurface};
use qnc_source_dock::{show_chrome_row, show_timeline_dock, SourceTimeline, TimelineDockStyle};
use qnc_timeline::{TimelineIntent, TimelineTheme};

use qnc_editorial_application::{action_ids, EditorialIntent, EditorialView, LibraryTab};

use crate::{layout_contract::EditorialContracts, theme::Theme};

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
        intent = render_pool_head(ui, contracts, theme, view);
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
    let poster = view
        .chosen_clip_id()
        .and_then(|id| view.clips.iter().find(|clip| clip.clip_id == id))
        .and_then(|clip| match (&clip.thumb_uri, &clip.thumb_image) {
            (Some(uri), Some(image)) => Some(MonitorPoster {
                uri,
                content_key: image.content_key,
                size: image.size,
                rgba: &image.pixels,
            }),
            _ => None,
        });
    let label = view
        .current_clip_label()
        .unwrap_or(contracts.editorial.preview.empty_label.as_str());
    qnc_monitor::paint_source_monitor(
        ui,
        rect,
        MonitorSurface {
            id: egui::Id::new(("qnc-monitor", "editorial-source")),
            chrome,
            picture,
            message: view.preview.monitor_message.as_deref(),
        },
        poster,
        label,
    );
}

fn render_pool_head(
    ui: &mut Ui,
    contracts: &EditorialContracts,
    theme: &Theme,
    view: &EditorialView,
) -> Option<EditorialIntent> {
    let rect = ui.available_rect_before_wrap();

    let mut intent = None;
    show_chrome_row(
        ui,
        rect,
        &dock_style(contracts, theme),
        theme.surface,
        true,
        |ui| {
            ui.spacing_mut().button_padding = Vec2::new(8.0, 2.0);
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
            for tab in &contracts.editorial.pool_head.tabs_left {
                let library_tab = match tab.as_str() {
                    "Virtual" => Some(LibraryTab::Virtual),
                    "All" => Some(LibraryTab::All),
                    _ => None,
                };
                let Some(library_tab) = library_tab else {
                    let _ = text_tab(ui, tab, false, theme);
                    ui.add_space(10.0);
                    continue;
                };
                let selected = view.library_tab == library_tab;
                if text_tab(ui, tab, selected, theme).clicked() {
                    intent = Some(EditorialIntent::SwitchLibraryTab(library_tab));
                }
                ui.add_space(10.0);
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                for command in contracts.editorial.pool_head.transport_right.iter().rev() {
                    let Some(action_id) = command.action_id() else {
                        continue;
                    };
                    if small_button(ui, command.label(), true, theme).clicked() {
                        intent = Some(EditorialIntent::action(action_id));
                    }
                }
            });
        },
    );

    intent
}

fn dock_style(contracts: &EditorialContracts, theme: &Theme) -> TimelineDockStyle {
    TimelineDockStyle {
        fill: theme.panel_alt,
        border: theme.border,
        text: theme.text,
        font_ui: theme.font_ui,
        chrome_row_height: theme.chrome_row_height,
        chrome_control_height: theme.chrome_control_height,
        chrome_pad_x: theme.chrome_pad_x,
        chrome_pad_y: theme.chrome_pad_y,
        header_timeline_gap: contracts.editorial.source_dock.header_timeline_gap,
    }
}

/// The source dock is the public one; this form only says which clip it shows and which
/// action buttons its group has.
fn render_source_dock(
    ui: &mut Ui,
    contracts: &EditorialContracts,
    theme: &Theme,
    view: &EditorialView,
) -> Option<EditorialIntent> {
    let rect = ui.available_rect_before_wrap();
    let mut header_intent = None;
    let intent = show_timeline_dock(
        ui,
        rect,
        &dock_style(contracts, theme),
        source_dock_clip_label(contracts, view),
        |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                for label in &contracts.composition().source_dock.actions_rtl {
                    let save_short = label == "Add virtual clip";
                    let enabled = save_short && view.chosen_clip_id().is_some();
                    if action_button(ui, label, enabled, theme).clicked() && save_short {
                        header_intent =
                            Some(EditorialIntent::action(action_ids::SAVE_VIRTUAL_SHOT));
                    }
                }
            });
        },
        SourceTimeline {
            projection: &view.preview.timeline,
            theme: timeline_theme(theme),
            filmstrip: view.preview.assets.filmstrip_background(),
            peaks: [
                view.preview.assets.a1_peaks(),
                view.preview.assets.a2_peaks(),
                view.preview.assets.a3_peaks(),
                view.preview.assets.a4_peaks(),
            ],
        },
    );
    match intent {
        TimelineIntent::None => header_intent,
        other => Some(EditorialIntent::Timeline(other)),
    }
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

fn render_clip_grid(
    ui: &mut Ui,
    contracts: &EditorialContracts,
    theme: &Theme,
    view: &EditorialView,
) -> Option<EditorialIntent> {
    let outer = ui.available_rect_before_wrap();
    ui.painter().rect_filled(outer, 0.0, theme.bg);
    let rect = outer.shrink(contracts.editorial.board.block_pad);
    let empty_message = if view.loading {
        "Citam projektni katalog..."
    } else if view.library_tab == LibraryTab::Virtual {
        "Nema virtualnih — Spremi virtualni kadar."
    } else if !view.message.is_empty() {
        view.message.as_str()
    } else {
        contracts.editorial.media_card.empty_message.as_str()
    };
    let short_rows;
    let clip_rows;
    let rows: Vec<qnc_media_card::CardRow<'_>> = if view.library_tab == LibraryTab::Virtual {
        short_rows = view
            .shorts
            .iter()
            .map(|shot| {
                let (status_proxy, status_original) = qnc_media_card::pipeline_statuses(
                    &shot.import_status,
                    &shot.imported_media_uri,
                );
                qnc_media_card::CardRow {
                    id: shot.shot_id.as_str(),
                    thumb_id: shot.shot_id.as_str(),
                    title: shot.name.as_str(),
                    duration_sec: 0.0,
                    duration_label: shot.duration_label.as_str(),
                    import_status: shot.import_status.as_str(),
                    status_proxy,
                    status_original,
                    checked: view.chosen_shot_id.as_deref() == Some(shot.shot_id.as_str()),
                    rgba_thumb: shot
                        .poster_uri
                        .as_deref()
                        .zip(shot.poster_image.as_deref())
                        .map(|(uri, image)| qnc_media_card::RgbaThumb {
                            uri,
                            content_key: image.content_key,
                            size: image.size,
                            rgba: &image.pixels,
                        }),
                }
            })
            .collect();
        short_rows
    } else {
        clip_rows = view
            .clips
            .iter()
            .map(|clip| {
                let (status_proxy, status_original) = qnc_media_card::pipeline_statuses(
                    &clip.import_status,
                    &clip.imported_media_uri,
                );
                qnc_media_card::CardRow {
                    id: clip.clip_id.as_str(),
                    thumb_id: clip.thumb_uri.as_deref().unwrap_or(clip.clip_id.as_str()),
                    title: clip.name.as_str(),
                    duration_sec: clip.duration_seconds,
                    duration_label: "",
                    import_status: clip.import_status.as_str(),
                    status_proxy,
                    status_original,
                    checked: view.chosen_clip_id() == Some(clip.clip_id.as_str()),
                    rgba_thumb: clip
                        .thumb_uri
                        .as_deref()
                        .zip(clip.thumb_image.as_deref())
                        .map(|(uri, image)| qnc_media_card::RgbaThumb {
                            uri,
                            content_key: image.content_key,
                            size: image.size,
                            rgba: &image.pixels,
                        }),
                }
            })
            .collect();
        clip_rows
    };
    let style = qnc_media_card::CardStyle {
        raised: theme.surface_alt,
        surface: theme.surface,
        border: theme.border,
        text: theme.text,
        muted: theme.text_muted,
        select_red: theme.danger,
    };
    let metrics = qnc_media_card::CardMetrics {
        min_card_width: contracts.editorial.media_card.min_card_width,
        card_text_height: contracts.editorial.media_card.card_text_height,
        grid_gap: contracts.editorial.media_card.grid_gap,
    };
    let features = qnc_media_card::MediaCardFeatures {
        selection_check: contracts.composition().media_card.selection_check,
        status_dots: qnc_media_card::StatusDotsMode::from_contract(
            &contracts.composition().media_card.status_dots,
        )
        .unwrap_or(qnc_media_card::StatusDotsMode::Off),
    };
    let no_textures = std::collections::HashMap::new();
    let mut action = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        action = qnc_media_card::show_card_grid(
            ui,
            &style,
            &metrics,
            &qnc_media_card::CardGridInput {
                height: rect.height(),
                selected_id: if view.library_tab == LibraryTab::Virtual {
                    view.chosen_shot_id.as_deref().unwrap_or("")
                } else {
                    view.chosen_clip_id().unwrap_or("")
                },
                focused_id: "",
                panel_focused: false,
                cards: &rows,
                thumb_textures: &no_textures,
                tc: &format_duration,
                features,
                empty_message,
                id_salt: "editorial_clip_grid",
            },
        );
    });
    match action {
        Some(qnc_media_card::CardGridAction::Activate(id))
        | Some(qnc_media_card::CardGridAction::ToggleSelection(id)) => {
            if view.library_tab == LibraryTab::Virtual {
                Some(EditorialIntent::PreviewShort(id))
            } else {
                Some(EditorialIntent::PreviewClip(id))
            }
        }
        None => None,
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
