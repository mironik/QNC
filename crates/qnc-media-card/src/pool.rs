//! The media pool grid (user 2026-10-20: the Ingest and Story forms each turned their clips
//! into card rows and set up the same grid). A clip, a short or a B-roll shot of any
//! application says what its card shows (`PoolItem`); the grid makes the card rows, the
//! status dots and the marker itself and returns the click. It knows no application.

use eframe::egui::{self, Color32, Rect};

use crate::{
    minutes_seconds, pipeline_statuses, show_card_grid_focused, CardGridAction, CardGridInput, CardMetrics, CardRow,
    CardStyle, GridStep, MediaCardFeatures, RgbaThumb,
};

/// The dot in the top-right corner of a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolMarker {
    None,
    Imported,
    NotImported,
}

/// A line over the picture while the card's record is being written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolOverlay {
    None,
    Saving,
    SaveFailed,
}

/// What one card shows. The application's own clip or shot says it; the grid paints it.
pub trait PoolItem {
    fn card_id(&self) -> &str;
    fn card_title(&self) -> &str;
    /// Where the poster is and the poster once it is loaded.
    fn card_poster(&self) -> (Option<&str>, Option<&qnc_image_assets::RgbaImage>);
    fn card_seconds(&self) -> f64 {
        0.0
    }
    /// A duration already written as text (shown instead of the seconds).
    fn card_duration_label(&self) -> &str {
        ""
    }
    /// The catalog status and the imported media, for the status dots.
    fn card_import(&self) -> (&str, &str) {
        ("", "")
    }
    fn card_marker(&self) -> PoolMarker {
        PoolMarker::None
    }
    fn card_overlay(&self) -> PoolOverlay {
        PoolOverlay::None
    }
    /// The check mark: `None` is "the chosen card".
    fn card_checked(&self) -> Option<bool> {
        None
    }
}

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
    let rows: Vec<CardRow<'_>> = items.iter().map(|item| card_row(*item, grid.chosen_id, look)).collect();
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

fn card_row<'a, T: PoolItem>(item: &'a T, chosen_id: &str, look: &PoolLook<'a>) -> CardRow<'a> {
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
        checked: item.card_checked().unwrap_or(item.card_id() == chosen_id),
        rgba_thumb: poster_uri.zip(poster).map(|(uri, image)| RgbaThumb {
            uri,
            content_key: image.content_key,
            size: image.size,
            rgba: &image.pixels,
        }),
    }
}
