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
    let style = qnc_ui_kit::ChromeButtonStyle {
        text: theme.text,
        border: theme.border,
        focus: theme.focus,
        height: theme.chrome_control_height,
    };
    qnc_ui_kit::chrome_button(ui, text, enabled, false, style)
}

pub(super) fn primary_button(
    ui: &mut Ui,
    text: &str,
    enabled: bool,
    theme: &Theme,
) -> egui::Response {
    qnc_ui_kit::strong_button(ui, text, enabled, theme.accent, theme.chrome_control_height)
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
