use super::*;

pub(super) fn render_clip_grid(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let outer = ui.available_rect_before_wrap();
    ui.painter().rect_filled(outer, 0.0, theme.bg);
    let rect = outer.shrink(contracts.ingest.board.block_pad);

    let empty_message = empty_grid_message(contracts, view);
    let clips = view.visible_clips().collect::<Vec<_>>();
    let rows = clips
        .iter()
        .map(|clip| {
            let marker = if clip.imported {
                Color32::from_rgb(55, 210, 145)
            } else {
                theme.text_muted
            };
            qnc_media_card::CardRow {
                id: clip.clip_id.as_str(),
                thumb_id: clip.thumb_uri.as_deref().unwrap_or(clip.clip_id.as_str()),
                title: clip.name.as_str(),
                duration_sec: clip.duration_seconds,
                duration_label: "",
                import_status: if clip.imported { "imported" } else { "" },
                status_proxy: "",
                status_original: "",
                overlay_label: save_state_label(clip),
                top_right_marker: Some(marker),
                checked: clip.selected,
                rgba_thumb: clip
                    .thumb_uri
                    .as_deref()
                    .zip(clip.thumb_image.as_ref())
                    .map(|(uri, image)| qnc_media_card::RgbaThumb {
                        uri,
                        content_key: image.content_key,
                        size: image.size,
                        rgba: &image.pixels,
                    }),
            }
        })
        .collect::<Vec<_>>();
    let style = qnc_media_card::CardStyle {
        raised: theme.surface_alt,
        surface: theme.surface,
        border: theme.border,
        text: theme.text,
        muted: theme.text_muted,
        select_red: theme.danger,
    };
    let metrics = qnc_media_card::CardMetrics {
        min_card_width: contracts.ingest.clip_grid.min_card_width,
        card_text_height: contracts.ingest.clip_grid.card_text_height,
        grid_gap: contracts.ingest.clip_grid.grid_gap,
    };
    let features = qnc_media_card::MediaCardFeatures {
        selection_check: true,
        top_right_marker: true,
        status_dots: qnc_media_card::StatusDotsMode::Off,
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
                selected_id: "",
                focused_id: view.preview_clip_id.as_deref().unwrap_or(""),
                panel_focused: true,
                cards: &rows,
                thumb_textures: &no_textures,
                tc: &format_duration,
                features,
                empty_message,
                id_salt: if view.clip_filter == ClipFilter::New {
                    "ingest_grid_new"
                } else {
                    "ingest_grid_all"
                },
            },
        );
    });
    match action {
        Some(qnc_media_card::CardGridAction::ToggleSelection(id)) => Some(IngestIntent::new(
            action_ids::INGEST_CLIP_TOGGLE,
            IngestPayload::ClipId(id),
        )),
        Some(qnc_media_card::CardGridAction::Activate(id)) => Some(IngestIntent::new(
            action_ids::INGEST_PREVIEW_FOCUS,
            IngestPayload::ClipId(id),
        )),
        None => None,
    }
}

fn empty_grid_message<'a>(contracts: &'a IngestContracts, view: &'a IngestViewModel) -> &'a str {
    view.work_settings_error.as_deref().unwrap_or_else(|| {
        if view.work_settings_loading {
            "Citanje radnih postavki..."
        } else if view.clip_filter == ClipFilter::New {
            &contracts.ingest.clip_grid.empty_new_message
        } else if view.command_busy || view.selected_source_uri.is_some() {
            &view.message
        } else {
            &contracts.ingest.clip_grid.empty_message
        }
    })
}

fn save_state_label(clip: &ClipView) -> &'static str {
    match clip.save_state {
        qnc_ingest_application::SaveState::Pending => "Spremanje...",
        qnc_ingest_application::SaveState::Failed => "Upis nije uspio",
        _ => "",
    }
}
