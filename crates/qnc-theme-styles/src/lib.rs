//! The look of every piece from the one desktop theme (user 2026-10-20, rule 11: the Ingest,
//! Story and Project forms and their layout models each turned the theme into the same
//! styles). One function per piece; it knows no form.

use qnc_desktop_theme::Theme;

pub const MODULE_ID: &str = "qnc.module.theme-styles";

/// The colours of every timeline.
pub fn timeline(theme: &Theme) -> qnc_timeline::TimelineTheme {
    qnc_timeline::TimelineTheme::from_qnc_theme(theme.bg, theme.surface, theme.surface_alt, theme.border_soft, theme.text, theme.text_muted, theme.accent)
}

/// The chrome of a dock and its header; `header_timeline_gap` is the form's measure.
pub fn dock(theme: &Theme, header_timeline_gap: f32) -> qnc_source_dock::TimelineDockStyle {
    qnc_source_dock::TimelineDockStyle {
        fill: theme.panel_alt,
        border: theme.border,
        text: theme.text,
        font_ui: theme.font_ui,
        chrome_row_height: theme.chrome_row_height,
        chrome_control_height: theme.chrome_control_height,
        chrome_pad_x: theme.chrome_pad_x,
        chrome_pad_y: theme.chrome_pad_y,
        header_timeline_gap,
    }
}

/// A row of tabs and commands; `tab_gap` is the form's measure.
pub fn row(theme: &Theme, tab_gap: f32) -> qnc_media_pool_head::RowStyle {
    qnc_media_pool_head::RowStyle {
        text: theme.text,
        muted: theme.text_muted,
        accent: theme.accent,
        border: theme.border,
        font_ui: theme.font_ui,
        control_height: theme.chrome_control_height,
        tab_gap,
    }
}

/// A chrome button.
pub fn button(theme: &Theme) -> qnc_ui_kit::ChromeButtonStyle {
    qnc_ui_kit::ChromeButtonStyle { text: theme.text, border: theme.border, focus: theme.focus, height: theme.chrome_control_height }
}

/// A bar of confirm and cancel buttons.
pub fn action_bar(theme: &Theme) -> qnc_ui_kit::FormActionBarStyle {
    qnc_ui_kit::FormActionBarStyle::new(theme.text, theme.accent, theme.border, theme.font_ui, theme.chrome_control_height)
}

/// The preview monitor.
pub fn monitor(theme: &Theme) -> qnc_monitor::MonitorChrome {
    qnc_monitor::MonitorChrome { fill: theme.black, border: theme.border, muted: theme.text_muted, font_size: theme.font_ui }
}

/// The media pool cards.
pub fn cards(theme: &Theme) -> qnc_media_card::CardStyle {
    qnc_media_card::CardStyle {
        raised: theme.surface_alt,
        surface: theme.surface,
        border: theme.border,
        text: theme.text,
        muted: theme.text_muted,
        select_red: theme.danger,
    }
}
