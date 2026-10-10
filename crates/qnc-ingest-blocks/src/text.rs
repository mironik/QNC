use super::*;

pub(super) fn muted(text: &str, theme: &Theme) -> RichText {
    RichText::new(text).color(theme.text_muted)
}
