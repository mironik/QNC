use super::*;

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
