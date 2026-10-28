//! The rows of a settings panel (audit 2026-10-27: they lived in the Project form): the
//! title row, a section with its rule and title, a labelled row (label, field, button
//! column) and a row of buttons under the button column, and the three buttons of a panel.
//! The measures come from the form's layout contract, the colours from the one desktop
//! theme. Passive: it draws and reports clicks, it knows no form.

use eframe::egui::{self, Color32, RichText, Vec2};
use qnc_desktop_theme::Theme;
use serde::Deserialize;

/// The measures of a settings panel, from the form's layout contract.
#[derive(Debug, Clone, Deserialize)]
pub struct PanelMetrics {
    pub panel_pad: f32,
    pub title: String,
    pub subtitle: String,
    pub inner_pad_x: f32,
    pub inner_pad_y: f32,
    pub section_gap: f32,
    pub inline_label_width: f32,
    pub inline_button_width: f32,
    pub inline_column_gap: f32,
    pub field_min_width: f32,
    pub label_font_size: f32,
    pub group_title_font_size: f32,
    pub row_height: f32,
}

fn button(ui: &mut egui::Ui, label: &str, enabled: bool, look: qnc_ui_kit::ButtonLook) -> egui::Response {
    qnc_ui_kit::look_button(ui, label, enabled, look)
}

/// The main button of a row (Create, Save, Choose).
pub fn primary_btn(ui: &mut egui::Ui, label: &str, enabled: bool, theme: &Theme) -> egui::Response {
    let min_size = Vec2::new(0.0, theme.chrome_control_height);
    let look = qnc_ui_kit::ButtonLook { text: Color32::WHITE, fill: theme.accent, border: None, font_size: Some(theme.font_ui), strong: true, square: false, min_size };
    button(ui, label, enabled, look)
}

/// A plain button with a border (Cancel, New template).
pub fn action_btn(ui: &mut egui::Ui, label: &str, theme: &Theme) -> egui::Response {
    let min_size = Vec2::new(0.0, theme.chrome_control_height);
    let look = qnc_ui_kit::ButtonLook { text: theme.text, fill: Color32::TRANSPARENT, border: Some(theme.border), font_size: Some(theme.font_ui), strong: false, square: false, min_size };
    button(ui, label, true, look)
}

/// The accent button inside a row (e.g. Save as preset): no minimum height.
pub fn accent_btn(ui: &mut egui::Ui, label: &str, enabled: bool, theme: &Theme) -> egui::Response {
    let look = qnc_ui_kit::ButtonLook { text: Color32::WHITE, fill: theme.accent, border: None, font_size: Some(theme.font_ui), strong: false, square: false, min_size: Vec2::ZERO };
    button(ui, label, enabled, look)
}

/// The title row of a panel with a rule under it.
/// A list's title on the left of its row and a search field on the right (at least 120 wide),
/// with a hint while empty.
pub fn title_with_search(ui: &mut egui::Ui, title: egui::RichText, text: &mut String, width: f32) -> egui::Response {
    ui.label(title);
    let hint = qnc_ui_text::text("rows.search");
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.add(egui::TextEdit::singleline(text).desired_width(width.max(120.0)).hint_text(hint))
    })
    .inner
}

pub fn panel_title_row(ui: &mut egui::Ui, title: &str, theme: &Theme) -> egui::Response {
    let width = ui.available_width();
    let output = ui.allocate_ui_with_layout(Vec2::new(width, theme.chrome_row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.spacing_mut().button_padding = Vec2::new(8.0, 2.0);
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
        let (pad_x, pad_y) = (theme.chrome_pad_x as i8, theme.chrome_pad_y as i8);
        egui::Frame::NONE
            .fill(theme.bg)
            .inner_margin(egui::Margin { left: pad_x, right: pad_x, top: pad_y, bottom: pad_y })
            .show(ui, |ui| {
                ui.set_min_size(Vec2::new(ui.available_width(), theme.chrome_control_height));
                ui.set_max_height(theme.chrome_control_height);
                ui.label(RichText::new(title).size(theme.font_ui).strong().color(theme.text));
            });
    });
    let rect = output.response.rect;
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, egui::Stroke::new(1.0, theme.border));
    output.response
}

/// A section: a rule, its title in capitals, then its contents.
pub fn section(ui: &mut egui::Ui, width: f32, title: &str, metrics: &PanelMetrics, theme: &Theme, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(8.0);
    let (y, x0) = (ui.cursor().top(), ui.min_rect().left());
    ui.painter().hline(egui::Rangef::new(x0, x0 + width), y, egui::Stroke::new(1.0, theme.border));
    ui.add_space(10.0);
    ui.set_max_width(width);
    ui.label(RichText::new(title.to_uppercase()).size(metrics.group_title_font_size).strong().color(theme.text_muted));
    ui.add_space(metrics.section_gap);
    add_contents(ui);
}

/// A row: its label, the field `content` and the button column `trailing`.
pub fn inline_row(
    ui: &mut egui::Ui,
    content_w: f32,
    label: &str,
    metrics: &PanelMetrics,
    theme: &Theme,
    content: impl FnOnce(&mut egui::Ui, f32),
    trailing: impl FnOnce(&mut egui::Ui, f32),
) {
    let row_h = metrics.row_height;
    let (label_w, middle_w, button_w) = inline_cols(content_w, metrics);
    ui.horizontal(|ui| {
        ui.set_min_height(row_h);
        ui.set_max_width(content_w);
        ui.add_space(metrics.inner_pad_x);
        let text = RichText::new(label).size(metrics.label_font_size).strong().color(theme.text_muted);
        ui.add_sized(Vec2::new(label_w, row_h), egui::Label::new(text));
        ui.add_space(metrics.inline_column_gap);
        ui.allocate_ui_with_layout(Vec2::new(middle_w, row_h), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_max_width(middle_w);
            content(ui, middle_w);
        });
        ui.add_space(metrics.inline_column_gap);
        button_column(ui, button_w, row_h, |ui| trailing(ui, button_w));
    });
}

/// Buttons under the button column of the rows.
pub fn trailing_actions(ui: &mut egui::Ui, content_w: f32, metrics: &PanelMetrics, trailing: impl FnOnce(&mut egui::Ui)) {
    let row_h = metrics.row_height;
    let (label_w, middle_w, button_w) = inline_cols(content_w, metrics);
    ui.horizontal(|ui| {
        ui.set_min_height(row_h);
        ui.set_max_width(content_w);
        ui.add_space(metrics.inner_pad_x + label_w + metrics.inline_column_gap + middle_w + metrics.inline_column_gap);
        button_column(ui, button_w, row_h, |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            trailing(ui);
        });
    });
}

fn button_column(ui: &mut egui::Ui, button_w: f32, row_h: f32, add: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(Vec2::new(button_w, row_h), egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.set_min_width(button_w);
        ui.set_max_width(button_w);
        add(ui);
    });
}

/// Widths of the label, field and button columns in a row `content_w` wide.
pub fn inline_cols(content_w: f32, metrics: &PanelMetrics) -> (f32, f32, f32) {
    let inner = (content_w - metrics.inner_pad_x * 2.0).max(0.0);
    let (mut label_w, mut button_w) = (metrics.inline_label_width, metrics.inline_button_width);
    let gaps = metrics.inline_column_gap * 2.0;
    if label_w + button_w + gaps > inner {
        let scale = (inner - gaps).max(80.0) / (metrics.inline_label_width + metrics.inline_button_width);
        label_w = (metrics.inline_label_width * scale).max(88.0);
        button_w = (metrics.inline_button_width * scale).max(96.0);
    }
    let middle_w = (inner - label_w - button_w - gaps).max(48.0);
    (label_w, middle_w, button_w)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics() -> PanelMetrics {
        PanelMetrics {
            panel_pad: 20.0, title: String::new(), subtitle: String::new(), inner_pad_x: 8.0, inner_pad_y: 8.0,
            section_gap: 8.0, inline_label_width: 168.0, inline_button_width: 120.0, inline_column_gap: 8.0,
            field_min_width: 160.0, label_font_size: 12.0, group_title_font_size: 12.0, row_height: 24.0,
        }
    }

    #[test]
    fn a_wide_row_keeps_its_columns_and_a_narrow_one_shrinks_them() {
        let (label, middle, button) = inline_cols(800.0, &metrics());
        assert_eq!((label, button), (168.0, 120.0));
        assert_eq!(middle, 800.0 - 16.0 - 168.0 - 120.0 - 16.0);
        let (label, _, button) = inline_cols(200.0, &metrics());
        assert!(label < 168.0 && button < 120.0 && label >= 88.0 && button >= 96.0);
    }
}
