//! Editorial shell: left media column | divider | right panel, plus the monitor
//! preview slot. Passive paint only (mirrors qnc_v4 `qnc_ui::{editorial_shell,
//! media_column_monitor, preview, content_panel}`). Colours and metrics come from
//! the caller (the UI contract), the module keeps no state and knows no
//! application.

use eframe::egui::{self, Color32, Rect, Response, Sense, TextureHandle, Vec2};

/// Shell metrics from `contracts/ui/editorial.layout.json` (`shell`, `preview`).
#[derive(Debug, Clone, Copy)]
pub struct ShellGeometry {
    pub left_ratio: f32,
    pub divider_width: f32,
    pub left_min_width: f32,
    pub right_min_width: f32,
    /// Height of one chrome row (tabs / transport strip).
    pub chrome_row_height: f32,
    pub preview_reserve_below: f32,
    pub preview_min_height: f32,
    pub preview_width_inset: f32,
    pub preview_min_width: f32,
}

/// Colours the shell paints with.
#[derive(Debug, Clone, Copy)]
pub struct ShellStyle {
    pub bg: Color32,
    /// Face of the left (media) column.
    pub left_face: Color32,
    /// Divider colour.
    pub border: Color32,
    pub preview_black: Color32,
    pub muted: Color32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
pub struct ShellMetrics {
    pub left_w: f32,
    pub right_w: f32,
    pub height: f32,
}

impl ShellMetrics {
    pub fn from_avail(avail: Vec2, geometry: &ShellGeometry) -> Self {
        let left_w = (avail.x * geometry.left_ratio).max(geometry.left_min_width);
        let right_w = (avail.x - left_w - geometry.divider_width).max(geometry.right_min_width);
        Self {
            left_w,
            right_w,
            height: avail.y,
        }
    }

    /// Preview height: 16:9 of the column width, capped by the height left
    /// above the reserve, never below the minimum.
    pub fn preview_h(&self, geometry: &ShellGeometry) -> f32 {
        let max_h = (self.height - geometry.preview_reserve_below).max(geometry.preview_min_height);
        let preview_w = (self.left_w - geometry.preview_width_inset).max(geometry.preview_min_width);
        (preview_w * 9.0 / 16.0)
            .min(max_h)
            .max(geometry.preview_min_height)
    }

    /// Body height under the preview and one chrome row. Never invents height
    /// above the column: overflow would paint over the dock.
    pub fn body_h(&self, preview_h: f32, geometry: &ShellGeometry) -> f32 {
        (self.height - preview_h - geometry.chrome_row_height).max(0.0)
    }
}

/// Left | divider | right. One callback, called once per side.
pub fn editorial_shell(
    ui: &mut egui::Ui,
    geometry: &ShellGeometry,
    style: &ShellStyle,
    mut paint: impl FnMut(&mut egui::Ui, &ShellMetrics, ShellSide),
) {
    let rect = ui.available_rect_before_wrap();
    ui.allocate_exact_size(rect.size(), Sense::hover());
    ui.set_clip_rect(rect);
    ui.painter().rect_filled(rect, 0.0, style.bg);

    let div_w = geometry.divider_width;
    let mut m = ShellMetrics::from_avail(rect.size(), geometry);
    m.right_w = (rect.width() - m.left_w - div_w).max(geometry.right_min_width);
    // Re-clamp so the columns fit exactly.
    if m.left_w + div_w + m.right_w > rect.width() {
        m.right_w = (rect.width() - m.left_w - div_w).max(0.0);
    }
    m.height = rect.height();

    let left_rect = Rect::from_min_size(rect.min, Vec2::new(m.left_w, m.height));
    let div_rect = Rect::from_min_size(
        egui::pos2(left_rect.right(), rect.top()),
        Vec2::new(div_w, m.height),
    );
    let right_rect = Rect::from_min_max(egui::pos2(div_rect.right(), rect.top()), rect.max);

    ui.painter().rect_filled(div_rect, 0.0, style.border);

    ui.allocate_new_ui(
        egui::UiBuilder::new()
            .max_rect(left_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.set_clip_rect(left_rect);
            ui.painter().rect_filled(left_rect, 0.0, style.left_face);
            paint(ui, &m, ShellSide::Left);
        },
    );
    ui.allocate_new_ui(
        egui::UiBuilder::new()
            .max_rect(right_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.set_clip_rect(right_rect);
            paint(ui, &m, ShellSide::Right);
        },
    );
}

/// Left media column: monitor slot, then whatever fits below it.
/// `after_preview` receives the exact height remaining under the preview.
pub fn media_column_monitor(
    ui: &mut egui::Ui,
    m: &ShellMetrics,
    geometry: &ShellGeometry,
    mut paint_monitor: impl FnMut(&mut egui::Ui, f32),
    mut after_preview: impl FnMut(&mut egui::Ui, f32),
) {
    let preview_h = m.preview_h(geometry).min(ui.available_height().max(0.0));
    paint_monitor(ui, preview_h);
    let rest = ui.available_height().max(0.0);
    after_preview(ui, rest);
}

pub struct PreviewInput<'a> {
    pub height: f32,
    pub texture: Option<&'a TextureHandle>,
    pub empty_label: &'a str,
    pub empty_font_size: f32,
    pub sense: Sense,
}

/// Monitor preview: contain + centered, or the empty label.
pub fn preview(ui: &mut egui::Ui, style: &ShellStyle, input: PreviewInput<'_>) -> Response {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, input.height), input.sense);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, style.preview_black);
    if let Some(tex) = input.texture {
        let size = tex.size_vec2();
        let scale = (rect.width() / size.x).min(rect.height() / size.y);
        let draw = size * scale;
        let offset = (rect.size() - draw) * 0.5;
        let img = Rect::from_min_size(rect.min + offset, draw);
        painter.image(
            tex.id(),
            img,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            input.empty_label,
            egui::FontId::proportional(input.empty_font_size),
            style.muted,
        );
    }
    resp
}

/// Padded content block under chrome (fixed height, clipped, solid face).
pub fn content_panel(
    ui: &mut egui::Ui,
    face: Color32,
    block_pad: i8,
    height: f32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let width = ui.available_width();
    let height = height.max(0.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, face);
    ui.allocate_new_ui(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.set_clip_rect(rect);
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(block_pad, block_pad))
                .show(ui, |ui| {
                    add_contents(ui);
                });
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values of `contracts/ui/editorial.layout.json`.
    fn geometry() -> ShellGeometry {
        ShellGeometry {
            left_ratio: 0.365,
            divider_width: 5.0,
            left_min_width: 280.0,
            right_min_width: 200.0,
            chrome_row_height: 28.0,
            preview_reserve_below: 190.0,
            preview_min_height: 160.0,
            preview_width_inset: 32.0,
            preview_min_width: 240.0,
        }
    }

    #[test]
    fn wide_window_splits_by_ratio() {
        let m = ShellMetrics::from_avail(Vec2::new(1920.0, 900.0), &geometry());
        assert!((m.left_w - 700.8).abs() < 0.01);
        assert!((m.right_w - 1214.2).abs() < 0.01);
    }

    #[test]
    fn narrow_window_keeps_left_minimum() {
        let m = ShellMetrics::from_avail(Vec2::new(600.0, 900.0), &geometry());
        assert_eq!(m.left_w, 280.0);
        assert_eq!(m.right_w, 315.0);
    }

    #[test]
    fn preview_is_sixteen_nine_of_column_width_capped_by_height() {
        let g = geometry();
        let tall = ShellMetrics::from_avail(Vec2::new(1920.0, 900.0), &g);
        assert!((tall.preview_h(&g) - 376.2).abs() < 0.01);
        let short = ShellMetrics::from_avail(Vec2::new(1920.0, 400.0), &g);
        assert_eq!(short.preview_h(&g), 210.0);
        let tiny = ShellMetrics::from_avail(Vec2::new(1920.0, 200.0), &g);
        assert_eq!(tiny.preview_h(&g), 160.0);
    }

    #[test]
    fn body_height_never_goes_negative() {
        let g = geometry();
        let m = ShellMetrics::from_avail(Vec2::new(1920.0, 100.0), &g);
        assert_eq!(m.body_h(160.0, &g), 0.0);
    }
}
