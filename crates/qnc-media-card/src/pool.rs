//! The media pool grid (user 2026-10-20: the Ingest and Story forms each turned their clips
//! into card rows and set up the same grid). A clip, a short or a B-roll shot says what its
//! card shows (`PoolItem`, piece `qnc-clip-row`); the grid makes the card rows, the status
//! dots and the marker itself and returns the click. It knows no application.

use eframe::egui::{self, Color32, Rect};

pub use qnc_clip_row::{PoolItem, PoolMarker, PoolOverlay};

use crate::{
    minutes_seconds, pipeline_statuses, show_card_grid_focused, CardGridAction, CardGridInput, CardMetrics, CardRow,
    CardStyle, GridStep, MediaCardFeatures, RgbaThumb,
};

/// How the pool looks (the form's contract and theme).
pub struct PoolLook<'a> {
    pub style: CardStyle,
    pub metrics: CardMetrics,
    pub features: MediaCardFeatures,
    pub imported_marker: Color32,
    pub not_imported_marker: Color32,
    pub saving: &'a str,
    pub save_failed: &'a str,
    /// The frame of the card the keyboard is on.
    pub focus: Color32,
}

/// What the pool shows now.
pub struct PoolGrid<'a> {
    pub rect: Rect,
    pub chosen_id: &'a str,
    pub panel_focused: bool,
    pub step: Option<(GridStep, u64)>,
    pub empty_message: &'a str,
    pub id_salt: &'a str,
}

pub fn show_pool<T: PoolItem>(ui: &mut egui::Ui, items: &[&T], grid: &PoolGrid<'_>, look: &PoolLook<'_>) -> Option<CardGridAction> {
    let rows: Vec<CardRow<'_>> = items.iter().map(|item| card_row(*item, grid, look)).collect();
    let no_textures = std::collections::HashMap::new();
    let input = CardGridInput {
        height: grid.rect.height(),
        selected_id: grid.chosen_id,
        focused_id: "",
        panel_focused: grid.panel_focused,
        cards: &rows,
        thumb_textures: &no_textures,
        tc: &minutes_seconds,
        features: look.features,
        empty_message: grid.empty_message,
        id_salt: grid.id_salt,
    };
    ui.scope_builder(egui::UiBuilder::new().max_rect(grid.rect), |ui| {
        show_card_grid_focused(ui, &look.style, &look.metrics, &input, (grid.step, look.focus))
    })
    .inner
}

fn card_row<'a, T: PoolItem>(item: &'a T, grid: &PoolGrid<'_>, look: &PoolLook<'a>) -> CardRow<'a> {
    let (import_status, imported_media) = item.card_import();
    let (status_proxy, status_original) = pipeline_statuses(import_status, imported_media);
    let (poster_uri, poster) = item.card_poster();
    CardRow {
        id: item.card_id(),
        thumb_id: poster_uri.unwrap_or(item.card_id()),
        title: item.card_title(),
        duration_sec: item.card_seconds(),
        duration_label: item.card_duration_label(),
        import_status,
        status_proxy,
        status_original,
        overlay_label: match item.card_overlay() {
            PoolOverlay::None => "",
            PoolOverlay::Saving => look.saving,
            PoolOverlay::SaveFailed => look.save_failed,
        },
        top_right_marker: match item.card_marker() {
            PoolMarker::None => None,
            PoolMarker::Imported => Some(look.imported_marker),
            PoolMarker::NotImported => Some(look.not_imported_marker),
        },
        checked: if look.features.ticks == crate::Ticks::Selected { item.card_selected() } else { item.card_id() == grid.chosen_id },
        rgba_thumb: poster_uri.zip(poster).map(|(uri, image)| RgbaThumb {
            uri,
            content_key: image.content_key,
            size: image.size,
            rgba: &image.pixels,
        }),
    }
}
