//! The Media Assist and Story blocks on the desktop board (moved unchanged out of the
//! editorial form, user rule 2026-09-30: forms are boards of blocks).

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

use qnc_editorial_application::{EditorialIntent, EditorialView, LibraryTab};
use qnc_panel_focus::{paint_focus, Panel};

use qnc_editorial_layout::{theme::Theme, EditorialContracts};

/// Media Assist and Story on the one desktop board: preview in the monitor, pool head
/// in the head row, the clip cards in the body, the group's right panel (Segmenti for
/// Story) on the right, the source timeline in the dock.
pub fn render_desktop(
    ui: &mut Ui,
    contracts: &EditorialContracts,
    theme: &Theme,
    view: &EditorialView,
) -> Option<EditorialIntent> {
    let (sizes, faces, names) = (contracts.board_sizes(), theme.board_faces(), contracts.board_names());
    qnc_board::show(ui, &sizes, &faces, &names, |ui, block, rect| match block {
        "preview" => {
            render_preview(ui, rect, contracts, theme, view);
            None
        }
        "pool-head" => render_pool_head(ui, contracts, theme, view),
        "clip-cards" => {
            // Clip menu (qnc_v5 media pool): the card grid fills the column under the
            // pool head, down to the dock, on the panel background.
            let intent = render_clip_grid(ui, contracts, theme, view);
            paint_focus(ui, rect, view.focus == Panel::Pool, theme.focus);
            intent
        }
        "right-panel" if contracts.composition().right_panel == "segment_panel" => {
            let intent = qnc_segment_panel::show(ui, &view.segments, timeline_theme(theme))
                .map(EditorialIntent::Segment);
            paint_focus(ui, rect, view.focus == Panel::Segments, theme.focus);
            intent
        }
        "source-dock" => {
            let intent = render_source_dock(ui, contracts, theme, view);
            paint_focus(ui, rect, view.focus == Panel::SourceTimeline, theme.focus);
            intent
        }
        // Right panel of the other groups: empty, reserved for their functions.
        _ => None,
    })
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
            for tab in contracts.pool_tabs() {
                let Some(action_id) = tab.action_id() else {
                    let _ = text_tab(ui, tab.label(), false, theme);
                    ui.add_space(10.0);
                    continue;
                };
                let selected = view.tab_selected(action_id);
                if text_tab(ui, tab.label(), selected, theme).clicked()
                    && view.action_enabled(action_id)
                {
                    intent = Some(EditorialIntent::action(action_id));
                }
                ui.add_space(10.0);
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                for command in contracts.pool_transport().iter().rev() {
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
        (view.current_clip_label()).unwrap_or(&contracts.editorial.source_dock.clip_label_fallback),
        |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                for action in &contracts.composition().source_dock.actions_rtl {
                    let enabled = action
                        .action_id()
                        .is_some_and(|action_id| view.action_enabled(action_id));
                    if action_button(ui, action.label(), enabled, theme).clicked() {
                        if let Some(action_id) = action.action_id() {
                            header_intent = Some(EditorialIntent::action(action_id));
                        }
                    }
                }
            });
        },
        SourceTimeline::from_assets(&view.preview.timeline, timeline_theme(theme), &view.preview.assets)
            .with_timecode(view.source_timecode),
    );
    match intent {
        TimelineIntent::None => header_intent,
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

fn render_clip_grid(
    ui: &mut Ui,
    contracts: &EditorialContracts,
    theme: &Theme,
    view: &EditorialView,
) -> Option<EditorialIntent> {
    let outer = ui.available_rect_before_wrap();
    ui.painter().rect_filled(outer, 0.0, theme.bg);
    let rect = outer.shrink(contracts.editorial.board.block_pad);
    if view.library_tab == LibraryTab::Segment {
        let mut command = None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            let segments = &view.segments;
            command = qnc_segment_panel::show_segment_list(
                ui,
                segments,
                timeline_theme(theme),
                theme.danger,
            );
        });
        return command.map(EditorialIntent::Segment);
    }
    let empty_message = if view.loading {
        "Citam projektni katalog..."
    } else if view.library_tab == LibraryTab::Virtual {
        "Nema virtualnih — Spremi virtualni kadar."
    } else if view.library_tab == LibraryTab::Broll {
        contracts.editorial.media_card.broll_empty_message.as_str()
    } else if !view.message.is_empty() {
        view.message.as_str()
    } else {
        contracts.editorial.media_card.empty_message.as_str()
    };
    let short_rows;
    let clip_rows;
    // Virtual lists the shorts, B-roll the shots of the covers (v5 pool tabs).
    let shot_tab = matches!(view.library_tab, LibraryTab::Virtual | LibraryTab::Broll);
    let rows: Vec<qnc_media_card::CardRow<'_>> = if shot_tab {
        short_rows = view
            .shorts
            .iter()
            .filter(|shot| shot.b_roll == (view.library_tab == LibraryTab::Broll))
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
                    overlay_label: "",
                    top_right_marker: None,
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
                    overlay_label: "",
                    top_right_marker: None,
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
        top_right_marker: false,
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
                selected_id: if shot_tab {
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
            if shot_tab {
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
