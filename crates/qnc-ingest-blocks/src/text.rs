use super::*;

pub(super) fn format_duration(seconds: f64) -> String {
    if !seconds.is_finite() || seconds <= 0.0 {
        return "00:00".to_string();
    }
    let total = seconds.round() as i64;
    let minutes = total / 60;
    let secs = total % 60;
    format!("{minutes:02}:{secs:02}")
}

pub(super) fn muted(text: &str, theme: &Theme) -> RichText {
    RichText::new(text).color(theme.text_muted)
}
