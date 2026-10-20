//! The media pool panel (user 2026-10-20, rule 11): the card grid of the pool on its place
//! of the board, in the look of the one theme. The form gives only its contract's measures
//! and texts, what the pool shows now and the items; the panel paints the place, the cards,
//! the ticks, the markers and the dots (pieces `qnc-media-card`, `qnc-clip-row`) and returns
//! the click. It knows no form.

use eframe::egui::{Color32, Ui};
use qnc_clip_row::PoolItem;
use qnc_media_card::{CardGridAction, CardMetrics, GridStep, MediaCardFeatures, PoolGrid, PoolLook, StatusDotsMode, Ticks};

pub const MODULE_ID: &str = "qnc.module.media-pool-panel";

/// What the form's layout contract says of its pool.
pub struct PoolSpec<'a> {
    /// The padding of the place.
    pub pad: f32,
    pub metrics: CardMetrics,
    pub ticks: Ticks,
    pub status_dots: StatusDotsMode,
    /// The colour of an imported clip's marker; `None`: no markers.
    pub imported_marker: Option<Color32>,
    /// The line over a card while its record is written, and when that failed.
    pub saving: &'a str,
    pub save_failed: &'a str,
}

/// What the pool shows now (from the form's view).
pub struct PoolNow<'a> {
    pub chosen_id: &'a str,
    pub panel_focused: bool,
    pub step: Option<(GridStep, u64)>,
    pub empty_message: &'a str,
    pub id_salt: &'a str,
}

pub fn show<T: PoolItem>(ui: &mut Ui, theme: &qnc_desktop_theme::Theme, spec: &PoolSpec<'_>, now: &PoolNow<'_>, items: &[&T]) -> Option<CardGridAction> {
    let place = qnc_ui_kit::fill_block(ui, theme.bg);
    let look = PoolLook {
        style: qnc_theme_styles::cards(theme),
        metrics: spec.metrics,
        features: MediaCardFeatures {
            ticks: spec.ticks,
            top_right_marker: spec.imported_marker.is_some(),
            status_dots: spec.status_dots,
        },
        imported_marker: spec.imported_marker.unwrap_or(theme.text_muted),
        not_imported_marker: theme.text_muted,
        saving: spec.saving,
        save_failed: spec.save_failed,
        focus: theme.focus,
    };
    let grid = PoolGrid {
        rect: place.shrink(spec.pad),
        chosen_id: now.chosen_id,
        panel_focused: now.panel_focused,
        step: now.step,
        empty_message: now.empty_message,
        id_salt: now.id_salt,
    };
    qnc_media_card::show_pool(ui, items, &grid, &look)
}
