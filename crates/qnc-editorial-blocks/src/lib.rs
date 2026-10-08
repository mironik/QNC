//! The Media Assist and Story blocks on the desktop board (moved unchanged out of the
//! editorial form, user rule 2026-09-30: forms are boards of blocks).

// Copied 1:1 from qnc-ingest-desktop/src/widgets.rs. Left out on purpose: the
// right clip grid and the directory browser (its area stays empty and is
// filled by later group functions). Painting, metrics and helpers are unchanged.
use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, Layout, Rect, RichText, Stroke, Ui,
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
    qnc_board::show_in_frame(ui, frame, &board, |name| faces.named(name), |ui, block, rect| block_time::timed(block, || match block {
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
    }))
}

/// Diagnostics only: what each block of the board costs, written once a second.
mod block_time {
    use std::{cell::RefCell, collections::BTreeMap, time::Instant};
    thread_local! {
        static TIMES: RefCell<(Option<Instant>, BTreeMap<String, f64>)> = RefCell::new((None, BTreeMap::new()));
    }
    pub(super) fn timed<T>(block: &str, draw: impl FnOnce() -> T) -> T {
        if !qnc_dev_diagnostics::player_diagnostics_enabled() {
            return draw();
        }
        let started = Instant::now();
        let result = draw();
        let spent = started.elapsed().as_secs_f64() * 1000.0;
        TIMES.with(|times| {
            let (since, sums) = &mut *times.borrow_mut();
            *sums.entry(block.to_string()).or_default() += spent;
            let since = since.get_or_insert_with(Instant::now);
            if since.elapsed().as_secs_f32() >= 1.0 {
                let parts: Vec<String> = sums.iter().map(|(name, ms)| format!("{name}={ms:.0}")).collect();
                qnc_dev_diagnostics::log_line(
                    qnc_dev_diagnostics::DiagnosticsStream::Player,
                    format!("ui-block-ms {}", parts.join(" ")),
                );
                sums.clear();
                *since = Instant::now();
            }
        });
        result
    }
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
            yield_to_external: view.preview.monitor_auto,
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
    show_chrome_row(ui, rect, &dock_style(contracts, theme), theme.surface, true, |ui| {
        intent = match qnc_media_pool_head::show_row(ui, &style, &tabs, &labels) {
            Some(qnc_media_pool_head::RowClick::Tab(index)) => tabs_left[index].action_id(),
            Some(qnc_media_pool_head::RowClick::Command(index)) => commands[index].action_id(),
            None => None,
        }
        .map(EditorialIntent::action);
    });
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
                    let on = action.action_id().is_some_and(|action_id| view.segments.action_selected(action_id));
                    if switch_button(ui, action.label(), enabled, on, theme).clicked() {
                        if let Some(action_id) = action.action_id() {
                            header_intent = Some(EditorialIntent::action(action_id));
                        }
                    }
                }
            });
        },
        SourceTimeline::from_assets(&view.preview.timeline, timeline_theme(theme), &view.preview.assets)
            .with_timecode(view.source_timecode)
            .with_a1_channel(lane_choice(&view.segments, 0))
            .with_a2_channel(lane_choice(&view.segments, 1))
            .with_wave_over_video(wave_over_video(&view.segments)),
    );
    match intent {
        TimelineIntent::None => header_intent,
        // A click on A1/A2 is the same catalog action as Ctrl+1/Ctrl+2.
        TimelineIntent::TakeAudioLane(qnc_timeline::AudioLane::A2) => Some(EditorialIntent::action("select_audio_a2")),
        TimelineIntent::TakeAudioLane(_) => Some(EditorialIntent::action("select_audio_a1")),
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

/// A button that may be on (the switch Pokrivalice | Sync/B-roll): on, its text and
/// border take the focus colour, as the Sync/B-roll button did in the segment bar.
fn switch_button(ui: &mut Ui, text: &str, enabled: bool, on: bool, theme: &Theme) -> egui::Response {
    if !on {
        return action_button(ui, text, enabled, theme);
    }
    ui.add_enabled(
        enabled,
        Button::new(RichText::new(text).color(theme.focus))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, theme.focus))
            .corner_radius(CornerRadius::same(0))
            .min_size(Vec2::new(0.0, theme.chrome_control_height)),
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

/// The channel picker of A1 (lane 0) or A2: a lane taken with the keyboard shows its
/// draft with the picker open.
fn lane_choice(segments: &qnc_program_segments::SegmentsView, lane: u8) -> Option<qnc_timeline::ChannelChoice> {
    let (selected, count) = if lane == 0 { segments.a1_choice } else { segments.a2_choice }?;
    let draft = segments.lane_taken.filter(|(taken, _)| *taken == lane).map(|(_, draft)| draft);
    Some(qnc_timeline::ChannelChoice { selected: draft.unwrap_or(selected), count, open: draft.is_some() })
}

/// The wave shown over the video row: the taken lane's, else A1's, else A2's.
fn wave_over_video(segments: &qnc_program_segments::SegmentsView) -> Option<(qnc_timeline::AudioLane, u8)> {
    let lanes = [qnc_timeline::AudioLane::A1, qnc_timeline::AudioLane::A2];
    let taken = segments.lane_taken.map(|(lane, _)| usize::from(lane));
    taken
        .into_iter()
        .chain([0, 1])
        .find(|lane| segments.wave_zoom[*lane] > 0)
        .map(|lane| (lanes[lane], segments.wave_zoom[lane]))
}
