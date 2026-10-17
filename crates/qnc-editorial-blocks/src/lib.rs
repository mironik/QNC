//! The Media Assist and Story blocks on the desktop board (moved unchanged out of the
//! editorial form, user rule 2026-09-30: forms are boards of blocks).

// Copied 1:1 from qnc-ingest-desktop/src/widgets.rs. Left out on purpose: the
// right clip grid and the directory browser (its area stays empty and is
// filled by later group functions). Painting, metrics and helpers are unchanged.
use eframe::egui::{
    self, Align, Layout, Rect, Ui,
};

use qnc_monitor::{MonitorChrome, MonitorPicture, MonitorPoster, MonitorSurface};
use qnc_source_dock::show_chrome_row;
use qnc_timeline::TimelineIntent;

use qnc_editorial_application::{EditorialIntent, EditorialView, LibraryTab};
use qnc_panel_focus::{paint_focus, Panel};

use qnc_editorial_layout::{self as layout, theme::Theme, EditorialContracts};

/// Media Assist and Story on the one desktop board: preview in the monitor, pool head
/// in the head row, the clip cards in the body, the group's right panel (Segmenti for
/// Story) on the right, the source timeline in the dock; drawn in `frame` as one layout
/// tree with it.
pub fn render_desktop(
    ui: &mut Ui,
    frame: &mut qnc_board::Frame<'_>,
    contracts: &EditorialContracts,
    theme: &Theme,
    view: &EditorialView,
) -> Option<EditorialIntent> {
    let (sizes, faces, names) = (contracts.board_sizes(), theme.board_faces(), contracts.board_names());
    let board = sizes.standard_layout(&names);
    qnc_board::show_in_frame(ui, frame, &board, |name| faces.named(name), |ui, block, rect| qnc_dev_diagnostics::timed_block(block, || match block {
        "preview" => render_preview(ui, rect, contracts, theme, view),
        "pool-head" => {
            let intent = render_pool_head(ui, contracts, theme, view);
            paint_focus(ui, rect, view.focus.panel == Panel::PoolTabs, theme.focus);
            intent
        }
        "clip-cards" => {
            // Clip menu (qnc_v5 media pool): the card grid fills the column under the
            // pool head, down to the dock, on the panel background.
            let intent = render_clip_grid(ui, contracts, theme, view);
            paint_focus(ui, rect, view.focus.panel == Panel::Pool, theme.focus);
            intent
        }
        "right-panel" if contracts.composition().right_panel == "segment_panel" => {
            let intent = qnc_segment_panel::show(ui, &view.segments, layout::timeline_theme(theme), &contracts.editorial.cover_lengths, view.preview.output_levels)
                .map(|command| qnc_panel_focus::wheel_action(&command).map_or(EditorialIntent::Segment(command), EditorialIntent::action));
            paint_focus(ui, rect, view.focus.panel == Panel::Segments, theme.focus);
            intent
        }
        "source-dock" => {
            let intent = render_source_dock(ui, contracts, theme, view);
            paint_focus(ui, rect, matches!(view.focus.panel, Panel::SourceTimeline | Panel::SourceButtons), theme.focus);
            intent
        }
        // Right panel of the other groups: empty, reserved for their functions.
        _ => None,
    }))
}

fn render_preview(
    ui: &mut Ui,
    rect: Rect,
    contracts: &EditorialContracts,
    theme: &Theme,
    view: &EditorialView,
) -> Option<EditorialIntent> {
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
            frame_map: frame.frame_map.as_ref(),
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
    let label = view.current_clip_label().unwrap_or(contracts.editorial.preview.empty_label.as_str());
    qnc_trim_overlay::show(ui.ctx(), &view.segments); // the trim zoom, while something is taken
    qnc_monitor::paint_source_monitor(
        ui,
        rect,
        MonitorSurface {
            id: egui::Id::new(("qnc-monitor", "editorial-source")),
            chrome,
            picture,
            message: view.preview.monitor_message.as_deref(),
            yield_to_external: view.preview.monitor_auto,
        },
        poster,
        label,
    );
    None // the monitor gives no intent
}

fn render_pool_head(
    ui: &mut Ui,
    contracts: &EditorialContracts,
    theme: &Theme,
    view: &EditorialView,
) -> Option<EditorialIntent> {
    let rect = ui.available_rect_before_wrap();
    let tabs_left = contracts.pool_tabs();
    let tabs: Vec<_> = tabs_left
        .iter()
        .map(|tab| qnc_media_pool_head::RowTab {
            label: tab.label(),
            selected: tab.action_id().is_some_and(|id| view.tab_selected(id)),
            enabled: tab.action_id().is_some_and(|id| view.action_enabled(id)),
        })
        .collect();
    let commands: Vec<_> = contracts.pool_transport().iter().filter(|c| c.action_id().is_some()).collect();
    let labels: Vec<_> = commands
        .iter()
        .map(|command| qnc_media_pool_head::RowCommand {
            label: command.label(),
            on: command.action_id().is_some_and(|id| view.preview.monitor_mode_on(id)),
        })
        .collect();
    let style = qnc_media_pool_head::RowStyle {
        text: theme.text,
        muted: theme.text_muted,
        accent: theme.accent,
        border: theme.border,
        font_ui: theme.font_ui,
        control_height: theme.chrome_control_height,
        tab_gap: 10.0,
    };
    let mut intent = None;
    show_chrome_row(ui, rect, &contracts.dock_style(theme), theme.surface, true, |ui| {
        intent = match qnc_media_pool_head::show_row(ui, &style, &tabs, &labels) {
            Some(qnc_media_pool_head::RowClick::Tab(index)) => tabs_left[index].action_id(),
            Some(qnc_media_pool_head::RowClick::Command(index)) => commands[index].action_id(),
            None => None,
        }
        .map(EditorialIntent::action);
    });
    intent
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
    let intent = qnc_source_timeline::show(
        ui,
        rect,
        &contracts.dock_style(theme),
        (view.current_clip_label()).unwrap_or(&contracts.editorial.source_dock.clip_label_fallback),
        |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                for action in &contracts.composition().source_dock.actions_rtl {
                    let enabled = action
                        .action_id()
                        .is_some_and(|action_id| view.action_enabled(action_id));
                    let on = action.action_id().is_some_and(|action_id| view.segments.action_selected(action_id));
                    let response = qnc_ui_kit::chrome_button(ui, action.label(), enabled, on, layout::button_style(theme));
                    paint_focus(ui, response.rect.expand(2.0), action.action_id().is_some_and(|id| view.focus.button.as_deref() == Some(id)), theme.focus);
                    if response.clicked() {
                        if let Some(action_id) = action.action_id() {
                            header_intent = Some(EditorialIntent::action(action_id));
                        }
                    }
                }
            });
        },
        (&view.preview.timeline, layout::timeline_theme(theme), &view.preview.assets),
        &view.source_timeline,
    );
    // A click on A1/A2 and the wheel are the source timeline's own catalog actions.
    match (intent, header_intent) {
        (TimelineIntent::None, header) => header,
        (choose @ (TimelineIntent::ChooseA1Channel(_) | TimelineIntent::ChooseA2Channel(_)), _) => Some(EditorialIntent::Timeline(choose)),
        (intent, _) => Some(match qnc_source_timeline::SourceTimeline::action_of(&intent) { Some((action, None)) => EditorialIntent::action(action), _ => EditorialIntent::Timeline(intent) }), // with a value: the intent
    }
}

fn render_clip_grid(
    ui: &mut Ui,
    contracts: &EditorialContracts,
    theme: &Theme,
    view: &EditorialView,
) -> Option<EditorialIntent> {
    let outer = qnc_ui_kit::fill_block(ui, theme.bg);
    let rect = outer.shrink(contracts.editorial.board.block_pad);
    if view.library_tab == LibraryTab::Segment {
        let mut command = None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            let segments = &view.segments;
            command = qnc_segment_panel::show_segment_list(
                ui,
                segments,
                layout::timeline_theme(theme),
                theme.danger,
            );
        });
        return command.map(EditorialIntent::Segment);
    }
    let empty_message = if view.loading {
        contracts.editorial.media_card.loading_message.as_str()
    } else if view.library_tab == LibraryTab::Virtual {
        contracts.editorial.media_card.virtual_empty_message.as_str()
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
        action = qnc_media_card::show_card_grid_focused(
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
                panel_focused: view.focus.panel == Panel::Pool,
                cards: &rows,
                thumb_textures: &no_textures,
                tc: &qnc_media_card::minutes_seconds,
                features,
                empty_message,
                id_salt: "editorial_clip_grid",
            },
            (view.focus.pool_step, theme.focus),
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
