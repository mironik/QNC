use eframe::egui::{self, Color32, RichText, Vec2};
use qnc_ui_kit::{FormActionBarResponse, FormActionBarStyle};

use crate::{
    layout_contract::{SettingsPanelMetrics, ShellLayoutContract},
    theme::{self, Theme},
};

pub fn primary_btn(
    ui: &mut egui::Ui,
    label: &str,
    enabled: bool,
    shell: &ShellLayoutContract,
) -> egui::Response {
    let t = Theme::from_contract(&shell.colors);
    ui.add_enabled(
        enabled,
        egui::Button::new(
            RichText::new(label)
                .color(Color32::WHITE)
                .strong()
                .size(shell.theme_metrics.font_ui),
        )
        .min_size(Vec2::new(0.0, shell.theme_metrics.chrome_control_height))
        .fill(t.accent),
    )
}

pub fn action_btn(ui: &mut egui::Ui, label: &str, shell: &ShellLayoutContract) -> egui::Response {
    let t = Theme::from_contract(&shell.colors);
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .color(t.text)
                .size(shell.theme_metrics.font_ui),
        )
        .min_size(Vec2::new(0.0, shell.theme_metrics.chrome_control_height))
        .fill(Color32::TRANSPARENT)
        .stroke(egui::Stroke::new(1.0, t.border)),
    )
}

pub fn form_action_bar(
    ui: &mut egui::Ui,
    shell: &ShellLayoutContract,
    confirm_label: &str,
    confirm_enabled: bool,
    cancel_label: &str,
) -> FormActionBarResponse {
    let t = Theme::from_contract(&shell.colors);
    let style = FormActionBarStyle::new(
        t.text,
        t.accent,
        t.border,
        shell.theme_metrics.font_ui,
        shell.theme_metrics.chrome_control_height,
    );
    qnc_ui_kit::show_form_action_bar(ui, &style, confirm_label, confirm_enabled, cancel_label)
}

pub fn panel_title_row(
    ui: &mut egui::Ui,
    title: &str,
    shell: &ShellLayoutContract,
) -> egui::Response {
    chrome_row_fill(ui, title, shell, true)
}

fn chrome_row_fill(
    ui: &mut egui::Ui,
    title: &str,
    shell: &ShellLayoutContract,
    draw_bottom_rule: bool,
) -> egui::Response {
    let t = Theme::from_contract(&shell.colors);
    let width = ui.available_width();
    let output = ui.allocate_ui_with_layout(
        Vec2::new(width, shell.theme_metrics.chrome_row_height),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().button_padding = Vec2::new(8.0, 2.0);
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
            egui::Frame::NONE
                .fill(t.bg)
                .inner_margin(egui::Margin {
                    left: shell.theme_metrics.chrome_pad_x,
                    right: shell.theme_metrics.chrome_pad_x,
                    top: shell.theme_metrics.chrome_pad_y,
                    bottom: shell.theme_metrics.chrome_pad_y,
                })
                .show(ui, |ui| {
                    ui.set_min_size(Vec2::new(
                        ui.available_width(),
                        shell.theme_metrics.chrome_control_height,
                    ));
                    ui.set_max_height(shell.theme_metrics.chrome_control_height);
                    theme::strong_label(ui, title, shell.theme_metrics.font_ui, t.text);
                });
        },
    );
    if draw_bottom_rule {
        let rect = output.response.rect;
        ui.painter().hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            egui::Stroke::new(1.0, t.border),
        );
    }
    output.response
}

pub fn section(
    ui: &mut egui::Ui,
    width: f32,
    title: &str,
    settings: &SettingsPanelMetrics,
    shell: &ShellLayoutContract,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let t = Theme::from_contract(&shell.colors);
    ui.add_space(8.0);
    let y = ui.cursor().top();
    let x0 = ui.min_rect().left();
    ui.painter().hline(
        egui::Rangef::new(x0, x0 + width),
        y,
        egui::Stroke::new(1.0, t.border),
    );
    ui.add_space(10.0);
    ui.set_max_width(width);
    ui.label(
        RichText::new(title.to_uppercase())
            .size(settings.group_title_font_size)
            .strong()
            .color(t.muted),
    );
    ui.add_space(settings.section_gap);
    add_contents(ui);
}

pub fn inline_row(
    ui: &mut egui::Ui,
    content_w: f32,
    label: &str,
    settings: &SettingsPanelMetrics,
    shell: &ShellLayoutContract,
    content: impl FnOnce(&mut egui::Ui, f32),
    trailing: impl FnOnce(&mut egui::Ui, f32),
) {
    let t = Theme::from_contract(&shell.colors);
    let row_h = settings.row_height;
    let (label_w, middle_w, button_w) = inline_cols(content_w, settings);
    ui.horizontal(|ui| {
        ui.set_min_height(row_h);
        ui.set_max_width(content_w);
        ui.add_space(settings.inner_pad_x);
        ui.add_sized(
            Vec2::new(label_w, row_h),
            egui::Label::new(
                RichText::new(label)
                    .size(settings.label_font_size)
                    .strong()
                    .color(t.muted),
            ),
        );
        ui.add_space(settings.inline_column_gap);
        ui.allocate_ui_with_layout(
            Vec2::new(middle_w, row_h),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_max_width(middle_w);
                content(ui, middle_w);
            },
        );
        ui.add_space(settings.inline_column_gap);
        ui.allocate_ui_with_layout(
            Vec2::new(button_w, row_h),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.set_min_width(button_w);
                ui.set_max_width(button_w);
                trailing(ui, button_w);
            },
        );
    });
}

pub fn trailing_actions(
    ui: &mut egui::Ui,
    content_w: f32,
    settings: &SettingsPanelMetrics,
    trailing: impl FnOnce(&mut egui::Ui),
) {
    let row_h = settings.row_height;
    let (label_w, middle_w, button_w) = inline_cols(content_w, settings);
    ui.horizontal(|ui| {
        ui.set_min_height(row_h);
        ui.set_max_width(content_w);
        ui.add_space(
            settings.inner_pad_x
                + label_w
                + settings.inline_column_gap
                + middle_w
                + settings.inline_column_gap,
        );
        ui.allocate_ui_with_layout(
            Vec2::new(button_w, row_h),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.set_min_width(button_w);
                ui.set_max_width(button_w);
                ui.spacing_mut().item_spacing.x = 8.0;
                trailing(ui);
            },
        );
    });
}

fn inline_cols(content_w: f32, settings: &SettingsPanelMetrics) -> (f32, f32, f32) {
    let inner = (content_w - settings.inner_pad_x * 2.0).max(0.0);
    let mut label_w = settings.inline_label_width;
    let mut button_w = settings.inline_button_width;
    let gaps = settings.inline_column_gap * 2.0;
    if label_w + button_w + gaps > inner {
        let scale =
            (inner - gaps).max(80.0) / (settings.inline_label_width + settings.inline_button_width);
        label_w = (settings.inline_label_width * scale).max(88.0);
        button_w = (settings.inline_button_width * scale).max(96.0);
    }
    let middle_w = (inner - label_w - button_w - gaps).max(48.0);
    (label_w, middle_w, button_w)
}
