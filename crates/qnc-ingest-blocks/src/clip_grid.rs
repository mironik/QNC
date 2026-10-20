use super::*;

use qnc_media_card::{CardGridAction, PoolGrid, PoolLook};

/// The media pool: the public pool grid of `qnc-media-card` on this form's place.
pub(super) fn render_clip_grid(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let outer = qnc_ui_kit::fill_block(ui, theme.bg);
    let grid = &contracts.ingest.clip_grid;
    let look = PoolLook {
        style: qnc_media_card::CardStyle {
            raised: theme.surface_alt,
            surface: theme.surface,
            border: theme.border,
            text: theme.text,
            muted: theme.text_muted,
            select_red: theme.danger,
        },
        metrics: qnc_media_card::CardMetrics {
            min_card_width: grid.min_card_width,
            card_text_height: grid.card_text_height,
            grid_gap: grid.grid_gap,
        },
        features: qnc_media_card::MediaCardFeatures {
            selection_check: true,
            top_right_marker: true,
            status_dots: qnc_media_card::StatusDotsMode::Off,
        },
        imported_marker: qnc_ingest_layout::rgb(grid.imported_marker_rgb),
        not_imported_marker: theme.text_muted,
        saving: &grid.saving_label,
        save_failed: &grid.save_failed_label,
        focus: theme.focus,
    };
    let pool = PoolGrid {
        rect: outer.shrink(contracts.ingest.board.block_pad),
        chosen_id: view.preview_clip_id.as_deref().unwrap_or(""),
        panel_focused: view.focus.is(qnc_focus_walk::CARDS),
        step: view.focus.step_of(qnc_focus_walk::CARDS),
        empty_message: empty_grid_message(contracts, view),
        id_salt: if view.clip_filter == ClipFilter::New { "ingest_grid_new" } else { "ingest_grid_all" },
    };
    let clips: Vec<&ClipView> = view.visible_clips().collect();
    match qnc_media_card::show_pool(ui, &clips, &pool, &look)? {
        CardGridAction::ToggleSelection(id) => Some(IngestIntent::new(action_ids::INGEST_CLIP_TOGGLE, IngestPayload::ClipId(id))),
        CardGridAction::Activate(id) => Some(IngestIntent::new(action_ids::INGEST_PREVIEW_FOCUS, IngestPayload::ClipId(id))),
    }
}

fn empty_grid_message<'a>(contracts: &'a IngestContracts, view: &'a IngestViewModel) -> &'a str {
    view.work_settings_error.as_deref().unwrap_or_else(|| {
        if view.work_settings_loading {
            &contracts.ingest.clip_grid.loading_settings_message
        } else if view.clip_filter == ClipFilter::New {
            &contracts.ingest.clip_grid.empty_new_message
        } else if view.command_busy || view.selected_source_uri.is_some() {
            &view.message
        } else {
            &contracts.ingest.clip_grid.empty_message
        }
    })
}
