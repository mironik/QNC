//! Media card and virtualized card grid. Passive paint (mirrors qnc_v4
//! `qnc_media_card` and `editorial::media_pool::show_card_grid`). One card, one
//! grid for every application: the caller supplies neutral rows, colours,
//! metrics and the feature flags from the UI contract, and gets back one
//! intent. The module knows no application and keeps no state.

use std::collections::HashMap;

use eframe::egui::{self, Color32, Rect, TextureHandle, Vec2};

/// How the status dots next to the file name are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusDotsMode {
    Off,
    /// Proxy + original dots once import has started.
    Pipeline,
    /// One green dot when the import is done.
    ImportedOnly,
}

impl StatusDotsMode {
    /// Parses the `status_dots` value of the UI contract.
    pub fn from_contract(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "pipeline" => Some(Self::Pipeline),
            "imported_only" => Some(Self::ImportedOnly),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MediaCardFeatures {
    /// Bottom-left check mark on the thumbnail.
    pub selection_check: bool,
    pub status_dots: StatusDotsMode,
}

/// Metrics from the UI contract (`media_card`).
#[derive(Debug, Clone, Copy)]
pub struct CardMetrics {
    pub min_card_width: f32,
    pub card_text_height: f32,
    pub grid_gap: f32,
}

impl CardMetrics {
    pub fn min_card_height(&self) -> f32 {
        self.min_card_width * 9.0 / 16.0 + self.card_text_height
    }
}

/// Theme colours the card paints with.
#[derive(Debug, Clone, Copy)]
pub struct CardStyle {
    pub raised: Color32,
    pub surface: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    /// Focus ring colour.
    pub select_red: Color32,
}

#[derive(Debug, Clone, Copy)]
pub struct GridMetrics {
    pub cols: usize,
    pub card_w: f32,
    pub card_h: f32,
    pub gap: f32,
}

pub fn grid_metrics(available_w: f32, count: usize, metrics: &CardMetrics) -> GridMetrics {
    let count = count.max(1);
    let usable_w = (available_w - 8.0).max(metrics.min_card_width);
    let cols = (((usable_w + metrics.grid_gap) / (metrics.min_card_width + metrics.grid_gap))
        .floor() as usize)
        .max(1)
        .min(count);
    let card_w = (usable_w - metrics.grid_gap * cols.saturating_sub(1) as f32) / cols as f32;
    let card_h = card_w * 9.0 / 16.0 + metrics.card_text_height;
    GridMetrics {
        cols,
        card_w,
        card_h,
        gap: metrics.grid_gap,
    }
}

/// Neutral card model.
pub struct MediaCardInput<'a> {
    pub title: &'a str,
    pub duration_sec: f64,
    /// Preferred over `duration_sec` when non-empty.
    pub duration_label: &'a str,
    pub import_status: &'a str,
    pub status_proxy: &'a str,
    pub status_original: &'a str,
    /// Preview / focus ring. Independent of the check mark.
    pub focused: bool,
    /// Only painted when `features.selection_check`.
    pub checked: bool,
    pub features: MediaCardFeatures,
    pub thumb: Option<&'a TextureHandle>,
    pub tc: &'a dyn Fn(f64) -> String,
}

pub fn paint_media_card(
    ui: &egui::Ui,
    rect: Rect,
    style: &CardStyle,
    metrics: &CardMetrics,
    input: &MediaCardInput<'_>,
) {
    let painter = ui.painter_at(rect);
    let stroke = egui::Stroke::new(
        if input.focused { 2.0 } else { 1.0 },
        if input.focused {
            style.select_red
        } else {
            style.border
        },
    );
    painter.rect_filled(rect, 0.0, style.raised);
    painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);

    let thumb_rect = thumb_rect(rect, metrics);
    painter.rect_filled(thumb_rect, 0.0, style.surface);
    if let Some(tex) = input.thumb {
        let size = tex.size_vec2();
        if size.x > 0.0 && size.y > 0.0 {
            let scale = (thumb_rect.width() / size.x).max(thumb_rect.height() / size.y);
            let image_rect = Rect::from_center_size(thumb_rect.center(), size * scale);
            ui.painter_at(thumb_rect).image(
                tex.id(),
                image_rect,
                Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    } else {
        painter.text(
            thumb_rect.center(),
            egui::Align2::CENTER_CENTER,
            "…",
            egui::TextStyle::Body.resolve(ui.style()),
            style.muted,
        );
    }

    if input.features.selection_check {
        paint_selection_check(ui, thumb_rect, input.checked);
    }

    let dur = if !input.duration_label.trim().is_empty() {
        input.duration_label.to_string()
    } else {
        (input.tc)(input.duration_sec)
    };
    let meta = Rect::from_min_max(egui::pos2(rect.left(), thumb_rect.bottom()), rect.max);
    painter.rect_filled(meta, 0.0, style.raised);
    let text_y = meta.center().y;
    let small = egui::TextStyle::Small.resolve(ui.style());

    let dots = status_dots_layout(input);
    let dots_w = match dots {
        StatusDotsLayout::None => 0.0,
        StatusDotsLayout::One => 12.0,
        StatusDotsLayout::Two => 22.0,
    };
    let name_max_w = (rect.width() - 16.0 - 52.0 - dots_w).max(40.0);
    let name = truncate(
        input.title,
        (name_max_w / 7.0).floor().clamp(8.0, 42.0) as usize,
    );
    let name_pos = egui::pos2(rect.left() + 8.0, text_y);
    let name_w = ui.fonts(|f| {
        f.layout_no_wrap(name.clone(), small.clone(), style.text)
            .size()
            .x
    });
    painter.text(
        name_pos,
        egui::Align2::LEFT_CENTER,
        name,
        small.clone(),
        style.text,
    );

    let dx = rect.left() + 8.0 + name_w + 8.0;
    match dots {
        StatusDotsLayout::None => {}
        StatusDotsLayout::One => {
            painter.circle_filled(egui::pos2(dx, text_y), 3.5, DOT_READY_GREEN);
        }
        StatusDotsLayout::Two => {
            painter.circle_filled(
                egui::pos2(dx, text_y),
                3.5,
                proxy_dot_color(input.status_proxy),
            );
            painter.circle_filled(
                egui::pos2(dx + 10.0, text_y),
                3.5,
                original_dot_color(input.status_original),
            );
        }
    }

    painter.text(
        egui::pos2(rect.right() - 8.0, text_y),
        egui::Align2::RIGHT_CENTER,
        dur,
        small,
        style.muted,
    );
}

/// Hit zone of the check mark (independent of the card activation zone).
pub fn selection_check_hit_rect(card_rect: Rect, metrics: &CardMetrics) -> Rect {
    selection_check_rect(thumb_rect(card_rect, metrics)).expand(4.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatusDotsLayout {
    None,
    One,
    Two,
}

fn status_dots_layout(input: &MediaCardInput<'_>) -> StatusDotsLayout {
    match input.features.status_dots {
        StatusDotsMode::Off => StatusDotsLayout::None,
        StatusDotsMode::Pipeline if import_started(input.import_status) => StatusDotsLayout::Two,
        StatusDotsMode::Pipeline => StatusDotsLayout::None,
        StatusDotsMode::ImportedOnly if is_imported_status(input.import_status) => {
            StatusDotsLayout::One
        }
        StatusDotsMode::ImportedOnly => StatusDotsLayout::None,
    }
}

// Status colours are semantic (ready / pending / error), not theme colours.
const DOT_READY_GREEN: Color32 = Color32::from_rgb(0x30, 0xd1, 0x58);

fn paint_selection_check(ui: &egui::Ui, thumb_rect: Rect, checked: bool) {
    let check_rect = selection_check_rect(thumb_rect);
    let painter = ui.painter_at(thumb_rect);
    if checked {
        let fill = Color32::from_rgb(0xff, 0x95, 0x00);
        painter.rect_filled(check_rect, 3.0, fill);
        painter.rect_stroke(
            check_rect,
            3.0,
            egui::Stroke::new(1.5, fill),
            egui::StrokeKind::Inside,
        );
        let c = check_rect.center();
        let dark = Color32::from_rgb(0x1a, 0x1a, 0x1a);
        painter.line_segment(
            [egui::pos2(c.x - 3.5, c.y), egui::pos2(c.x - 1.0, c.y + 3.0)],
            egui::Stroke::new(2.0, dark),
        );
        painter.line_segment(
            [
                egui::pos2(c.x - 1.0, c.y + 3.0),
                egui::pos2(c.x + 4.0, c.y - 3.0),
            ],
            egui::Stroke::new(2.0, dark),
        );
    } else {
        painter.rect_filled(
            check_rect,
            3.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, 90),
        );
        painter.rect_stroke(
            check_rect,
            3.0,
            egui::Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 255, 255, 140)),
            egui::StrokeKind::Inside,
        );
    }
}

fn thumb_rect(card_rect: Rect, metrics: &CardMetrics) -> Rect {
    let thumb_h = (card_rect.height() - metrics.card_text_height).max(72.0);
    Rect::from_min_size(card_rect.min, Vec2::new(card_rect.width(), thumb_h))
}

fn selection_check_rect(thumb_rect: Rect) -> Rect {
    let size = 16.0;
    let pad = 6.0;
    Rect::from_min_size(
        egui::pos2(thumb_rect.left() + pad, thumb_rect.bottom() - pad - size),
        Vec2::splat(size),
    )
}

fn proxy_dot_color(status: &str) -> Color32 {
    match status.trim().to_ascii_lowercase().as_str() {
        "ready" => DOT_READY_GREEN,
        "pending" => Color32::from_rgb(0xff, 0xd6, 0x0a),
        _ => Color32::from_rgb(0xff, 0x45, 0x3a),
    }
}

fn original_dot_color(status: &str) -> Color32 {
    match status.trim().to_ascii_lowercase().as_str() {
        "ready" => Color32::from_rgb(0x0a, 0x84, 0xff),
        "pending" => Color32::from_rgb(0xff, 0xd6, 0x0a),
        _ => Color32::from_rgb(0xff, 0x45, 0x3a),
    }
}

fn is_imported_status(import_status: &str) -> bool {
    matches!(
        import_status.trim().to_ascii_lowercase().as_str(),
        "imported" | "done"
    )
}

fn import_started(import_status: &str) -> bool {
    matches!(
        import_status.trim().to_ascii_lowercase().as_str(),
        "queued"
            | "processing"
            | "original_ready"
            | "generating_proxy"
            | "imported"
            | "done"
            | "error"
    )
}

/// Cuts `s` to `max` characters and appends an ellipsis when it was longer.
pub fn truncate(s: &str, max: usize) -> String {
    let mut chars = s.chars();
    let head: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

/// One row of the grid (neutral: identity, thumbnail key, labels, statuses).
pub struct CardRow<'a> {
    /// Click / focus identity.
    pub id: &'a str,
    /// Key into `thumb_textures`.
    pub thumb_id: &'a str,
    pub title: &'a str,
    pub duration_sec: f64,
    pub duration_label: &'a str,
    pub import_status: &'a str,
    pub status_proxy: &'a str,
    pub status_original: &'a str,
    pub checked: bool,
}

pub struct CardGridInput<'a> {
    pub height: f32,
    pub selected_id: &'a str,
    pub focused_id: &'a str,
    pub panel_focused: bool,
    pub cards: &'a [CardRow<'a>],
    pub thumb_textures: &'a HashMap<String, TextureHandle>,
    pub tc: &'a dyn Fn(f64) -> String,
    pub features: MediaCardFeatures,
    pub empty_message: &'a str,
    pub id_salt: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardGridAction {
    Activate(String),
    ToggleSelection(String),
}

/// Virtualized vertical card grid. Returns the last card click, if any.
pub fn show_card_grid(
    ui: &mut egui::Ui,
    style: &CardStyle,
    metrics: &CardMetrics,
    input: &CardGridInput<'_>,
) -> Option<CardGridAction> {
    let mut clicked: Option<CardGridAction> = None;
    let cards = input.cards;
    let muted = style.muted;
    let available_w = ui.available_width().max(metrics.min_card_width);
    // Prefer the live available height inside a content panel; fall back to input.
    let available_h = ui
        .available_height()
        .min(if input.height > 0.0 {
            input.height
        } else {
            f32::MAX
        })
        .max(metrics.min_card_height());
    let grid = grid_metrics(available_w, cards.len(), metrics);

    egui::ScrollArea::vertical()
        .id_salt(input.id_salt)
        .auto_shrink([false, false])
        .max_height(available_h)
        .show_viewport(ui, |ui, viewport| {
            ui.set_min_width(available_w);
            if cards.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(24.0);
                    ui.colored_label(muted, input.empty_message);
                });
                return;
            }

            let total_rows = cards.len().div_ceil(grid.cols);
            let row_stride = grid.card_h + grid.gap;
            let first_row = (viewport.top() / row_stride).floor().max(0.0) as usize;
            let last_row = ((viewport.bottom() / row_stride).ceil() as usize + 1).min(total_rows);
            let focus_id = if input.panel_focused && !input.focused_id.trim().is_empty() {
                input.focused_id
            } else {
                input.selected_id
            };

            ui.add_space(first_row as f32 * row_stride);
            for row_idx in first_row..last_row {
                let start = row_idx * grid.cols;
                let end = (start + grid.cols).min(cards.len());
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = grid.gap;
                    for card in &cards[start..end] {
                        let focused = card.id == focus_id || card.thumb_id == focus_id;
                        let thumb = input.thumb_textures.get(card.thumb_id).cloned();
                        let (rect, resp) = ui.allocate_exact_size(
                            Vec2::new(grid.card_w, grid.card_h),
                            egui::Sense::click(),
                        );
                        paint_media_card(
                            ui,
                            rect,
                            style,
                            metrics,
                            &MediaCardInput {
                                title: card.title,
                                duration_sec: card.duration_sec,
                                duration_label: card.duration_label,
                                import_status: card.import_status,
                                status_proxy: card.status_proxy,
                                status_original: card.status_original,
                                focused,
                                checked: card.checked,
                                features: input.features,
                                thumb: thumb.as_ref(),
                                tc: input.tc,
                            },
                        );
                        if resp.clicked() {
                            let is_checkbox_click = input.features.selection_check
                                && resp.interact_pointer_pos().is_some_and(|pos| {
                                    selection_check_hit_rect(rect, metrics).contains(pos)
                                });
                            clicked = Some(if is_checkbox_click {
                                CardGridAction::ToggleSelection(card.id.to_string())
                            } else {
                                CardGridAction::Activate(card.id.to_string())
                            });
                        }
                    }
                });
                ui.add_space(grid.gap);
            }
            let rendered_rows = last_row.saturating_sub(first_row);
            let remaining_rows = total_rows.saturating_sub(first_row + rendered_rows);
            ui.add_space(remaining_rows as f32 * row_stride);
        });

    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values of `contracts/ui/editorial.layout.json` (`media_card`).
    fn metrics() -> CardMetrics {
        CardMetrics {
            min_card_width: 160.0,
            card_text_height: 34.0,
            grid_gap: 10.0,
        }
    }

    fn tc(_: f64) -> String {
        String::new()
    }

    fn input(import_status: &'static str, mode: StatusDotsMode) -> MediaCardInput<'static> {
        MediaCardInput {
            title: "clip",
            duration_sec: 0.0,
            duration_label: "",
            import_status,
            status_proxy: "pending",
            status_original: "pending",
            focused: false,
            checked: false,
            features: MediaCardFeatures {
                selection_check: true,
                status_dots: mode,
            },
            thumb: None,
            tc: &tc,
        }
    }

    #[test]
    fn grid_fits_columns_to_width() {
        let g = grid_metrics(800.0, 10, &metrics());
        assert_eq!(g.cols, 4);
        assert!((g.card_w - 190.5).abs() < 0.01);
        assert!((g.card_h - (190.5 * 9.0 / 16.0 + 34.0)).abs() < 0.01);
        assert_eq!(grid_metrics(300.0, 10, &metrics()).cols, 1);
    }

    #[test]
    fn grid_never_has_more_columns_than_cards() {
        assert_eq!(grid_metrics(2000.0, 2, &metrics()).cols, 2);
        assert_eq!(grid_metrics(2000.0, 0, &metrics()).cols, 1);
    }

    #[test]
    fn imported_only_shows_one_dot_only_when_done() {
        assert_eq!(
            status_dots_layout(&input("queued", StatusDotsMode::ImportedOnly)),
            StatusDotsLayout::None
        );
        assert_eq!(
            status_dots_layout(&input("processing", StatusDotsMode::ImportedOnly)),
            StatusDotsLayout::None
        );
        assert_eq!(
            status_dots_layout(&input("imported", StatusDotsMode::ImportedOnly)),
            StatusDotsLayout::One
        );
    }

    #[test]
    fn pipeline_shows_two_dots_once_import_started() {
        assert_eq!(
            status_dots_layout(&input("detected", StatusDotsMode::Pipeline)),
            StatusDotsLayout::None
        );
        assert_eq!(
            status_dots_layout(&input("queued", StatusDotsMode::Pipeline)),
            StatusDotsLayout::Two
        );
        assert_eq!(
            status_dots_layout(&input("queued", StatusDotsMode::Off)),
            StatusDotsLayout::None
        );
    }

    #[test]
    fn status_dots_mode_parses_the_contract_values() {
        assert_eq!(StatusDotsMode::from_contract("pipeline"), Some(StatusDotsMode::Pipeline));
        assert_eq!(
            StatusDotsMode::from_contract("imported_only"),
            Some(StatusDotsMode::ImportedOnly)
        );
        assert_eq!(StatusDotsMode::from_contract("nope"), None);
    }

    #[test]
    fn check_hit_zone_sits_inside_the_thumbnail_corner() {
        let card = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(190.0, 141.0));
        let hit = selection_check_hit_rect(card, &metrics());
        let thumb = thumb_rect(card, &metrics());
        assert!(hit.left() >= thumb.left() - 4.0);
        assert!(hit.bottom() <= thumb.bottom() + 4.0);
        assert!(!hit.contains(egui::pos2(150.0, 20.0)));
    }

    #[test]
    fn truncate_adds_an_ellipsis_only_when_cut() {
        assert_eq!(truncate("abc", 5), "abc");
        assert_eq!(truncate("abcdef", 3), "abc…");
    }
}
