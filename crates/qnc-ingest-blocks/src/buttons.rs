use super::*;

pub(super) fn text_link(ui: &mut Ui, text: &str, enabled: bool, theme: &Theme) -> egui::Response {
    ui.add_enabled(
        enabled,
        Label::new(RichText::new(text).color(if enabled {
            theme.text
        } else {
            theme.text_muted
        }))
        .sense(Sense::click()),
    )
}

pub(super) fn fixed_text_link(
    ui: &mut Ui,
    text: &str,
    enabled: bool,
    width: f32,
    theme: &Theme,
) -> egui::Response {
    ui.allocate_ui_with_layout(
        Vec2::new(width, theme.chrome_control_height),
        Layout::left_to_right(Align::Center),
        |ui| text_link(ui, text, enabled, theme),
    )
    .inner
}

pub(super) fn action_button(
    ui: &mut Ui,
    text: &str,
    enabled: bool,
    theme: &Theme,
) -> egui::Response {
    ui.add_enabled(
        enabled,
        Button::new(RichText::new(text).color(theme.text))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, theme.border))
            .corner_radius(CornerRadius::same(0))
            .min_size(Vec2::new(0.0, theme.chrome_control_height)),
    )
}

pub(super) fn primary_button(
    ui: &mut Ui,
    text: &str,
    enabled: bool,
    theme: &Theme,
) -> egui::Response {
    ui.add_enabled(
        enabled,
        Button::new(RichText::new(text).color(Color32::WHITE).strong())
            .fill(theme.accent)
            .stroke(Stroke::new(0.0, Color32::TRANSPARENT))
            .corner_radius(CornerRadius::same(0))
            .min_size(Vec2::new(0.0, theme.chrome_control_height)),
    )
}

pub(super) fn form_action_bar_style(theme: &Theme) -> FormActionBarStyle {
    FormActionBarStyle::new(
        theme.text,
        theme.accent,
        theme.border,
        theme.font_ui,
        theme.chrome_control_height,
    )
}
