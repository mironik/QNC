//! The text of a recorded block (user 2026-11-08): as it was read (a word read green, read wrong
//! red, missed red and struck through) and with its M markers. A right click between two words
//! sets a marker there, on one that is there takes it.

use eframe::egui::{self, text::LayoutJob, FontId, Sense, Stroke, TextFormat};
use qnc_script::{ReadCheck, Verdict};

/// What the text is shown with.
pub(crate) struct Shown<'a> {
    pub check: Option<&'a ReadCheck>,
    pub marks: &'a [u32],
    pub taken: Option<u32>,
    pub lengths: &'a [f64],
}

/// The word a character of the job is nearest the start of, of the words at `spans` (their
/// characters in the job).
fn gap_at(spans: &[(usize, usize)], char_index: usize) -> usize {
    let Some(word) = spans.iter().position(|(_, end)| char_index < *end) else { return spans.len() };
    let (start, end) = spans[word];
    if char_index < start + (end - start) / 2 {
        word
    } else {
        word + 1
    }
}

fn job(text: &str, shown: &Shown<'_>, theme: &qnc_desktop_theme::Theme, width: f32) -> (LayoutJob, Vec<(usize, usize)>) {
    let font = FontId::proportional(theme.font_ui + 1.0);
    let plain = TextFormat { font_id: font, color: theme.text, ..Default::default() };
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    let (mut from, mut chars, mut spans) = (0, 0, Vec::new());
    for (index, word) in text.split_whitespace().enumerate() {
        let at = word.as_ptr() as usize - text.as_ptr() as usize;
        let between = &text[from..at];
        job.append(between, 0.0, plain.clone());
        chars += between.chars().count();
        if shown.marks.contains(&(index as u32)) {
            let color = if shown.taken == Some(index as u32) { theme.danger } else { theme.focus };
            job.append("| ", 0.0, TextFormat { color, ..plain.clone() });
            chars += 2;
        }
        let format = match shown.check.and_then(|check| check.words.iter().find(|checked| checked.at.start == at)).map(|checked| checked.verdict) {
            Some(Verdict::Read) => TextFormat { color: qnc_desktop_theme::done(), ..plain.clone() },
            Some(Verdict::Wrong) => TextFormat { color: theme.danger, ..plain.clone() },
            Some(Verdict::Missed) => TextFormat { color: theme.danger, strikethrough: Stroke::new(1.0, theme.danger), ..plain.clone() },
            None => plain.clone(),
        };
        job.append(word, 0.0, format);
        let length = word.chars().count();
        spans.push((chars, chars + length));
        chars += length;
        from = at + word.len();
    }
    job.append(&text[from..], 0.0, plain);
    (job, spans)
}

/// The text marked: whether it was clicked and the word a right click is before.
pub(crate) fn show(ui: &mut egui::Ui, text: &str, shown: &Shown<'_>, theme: &qnc_desktop_theme::Theme) -> (bool, Option<u32>) {
    let (layout, spans) = job(text, shown, theme, ui.available_width());
    let galley = ui.fonts(|fonts| fonts.layout_job(layout));
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::click());
    ui.painter().galley(rect.min, galley.clone(), theme.text);
    let right = response.secondary_clicked().then(|| response.interact_pointer_pos()).flatten().map(|pos| gap_at(&spans, galley.cursor_from_pos(pos - rect.min).ccursor.index)).filter(|gap| (1..spans.len()).contains(gap)).map(|gap| gap as u32);
    let hint = match shown.check {
        Some(check) => format!("{}\n{}", qnc_ui_text::fill("script.read_heard", &[("text", &check.heard)]), qnc_ui_text::text("script.mark_hint")),
        None => qnc_ui_text::text("script.mark_hint").to_string(),
    };
    let clicked = response.on_hover_text(hint).clicked();
    if let Some(extra) = shown.check.filter(|check| !check.extra.is_empty()).map(|check| check.extra.join(" ")) {
        ui.label(egui::RichText::new(qnc_ui_text::fill("script.read_extra", &[("words", &extra)])).color(theme.text_muted).size(theme.font_ui));
    }
    if !shown.lengths.is_empty() {
        let lengths = shown.lengths.iter().map(|seconds| format!("{seconds:.1} s")).collect::<Vec<_>>().join(" · ");
        ui.label(egui::RichText::new(qnc_ui_text::fill("script.mark_lengths", &[("lengths", &lengths)])).color(theme.text_muted).size(theme.font_ui));
    }
    (clicked, right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_is_before_the_word_it_is_nearer_the_start_of() {
        let spans = [(0, 4), (5, 9), (10, 14)];
        assert_eq!(gap_at(&spans, 1), 0);
        assert_eq!(gap_at(&spans, 3), 1);
        assert_eq!(gap_at(&spans, 6), 1);
        assert_eq!(gap_at(&spans, 8), 2);
        assert_eq!(gap_at(&spans, 20), 3);
    }
}
