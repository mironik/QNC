//! The look of every piece from the one desktop theme (user 2026-10-20, rule 11: the Ingest,
//! Story and Project forms and their layout models each turned the theme into the same
//! styles). One function per piece; it knows no form.

use eframe::egui::Color32;
use qnc_desktop_theme::Theme;

pub const MODULE_ID: &str = "qnc.module.theme-styles";

/// The colours of every timeline (and of its trim zoom, the timeline enlarged); the gui-kit holds
/// them (C15), the timeline only draws with them.
pub fn timeline(theme: &Theme) -> qnc_timeline::TimelineTheme {
    let rgb = |r, g, b| Color32::from_rgb(r, g, b);
    qnc_timeline::TimelineTheme {
        background: theme.bg,
        label_background: theme.surface_alt,
        video_background: theme.surface,
        // Audio lanes in the colours of a story's video segment: its dark green band with
        // the mint of its body as the wave (user 2026-10-14).
        audio_primary_background: rgb(17, 103, 88),
        audio_secondary_background: rgb(17, 103, 88),
        border: theme.border_soft,
        text: theme.text,
        muted: theme.text_muted,
        playhead: theme.accent,
        focus: rgb(255, 180, 60),
        shot_range: rgb(96, 165, 250),
        in_out_dim: Color32::from_black_alpha(133),
        wave_a1: rgb(132, 214, 158),
        wave_a2: rgb(132, 214, 158),
        wave_a3: rgb(132, 214, 158),
        wave_a4: rgb(132, 214, 158),
        audio_wave: rgb(80, 190, 148),
        lane_taken: rgb(240, 200, 40),
        lane_silent: rgb(200, 45, 45),
        marker_hover: rgb(230, 40, 40),
        level_marks: [
            Color32::from_rgba_unmultiplied(220, 230, 225, 70),
            Color32::from_rgba_unmultiplied(240, 200, 40, 150),
            Color32::from_rgba_unmultiplied(220, 60, 50, 170),
        ],
        level_curve: rgb(235, 235, 235),
        cut: rgb(230, 30, 30),
        cut_side: rgb(255, 64, 64),
        unheard: Color32::from_rgba_premultiplied(70, 10, 10, 70),
    }
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
        dot_ready: Color32::from_rgb(0x30, 0xd1, 0x58),
        dot_original_ready: Color32::from_rgb(0x0a, 0x84, 0xff),
        dot_pending: Color32::from_rgb(0xff, 0xd6, 0x0a),
        dot_error: Color32::from_rgb(0xff, 0x45, 0x3a),
        check_on: Color32::from_rgb(0xff, 0x95, 0x00),
        check_mark: Color32::from_rgb(0x1a, 0x1a, 0x1a),
        check_off: Color32::from_rgba_unmultiplied(0, 0, 0, 90),
        check_off_rim: Color32::from_rgba_unmultiplied(255, 255, 255, 140),
    }
}
