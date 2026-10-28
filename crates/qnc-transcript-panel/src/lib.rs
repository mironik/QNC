//! The transcript panel (user 2026-10-26: Media Assist Audio AI, what was said in the clip on
//! the preview; a click on a word takes the playhead there). It paints the transcript view and
//! gives the click as the catalog action `transcript_cue_word` with the word's first frame; it
//! holds no state, reads nothing and knows no form.

use eframe::egui::{self, RichText, Sense};
use qnc_action_event::ActionEvent;
use qnc_source_timecode::SourceTimecode;
use qnc_transcript::{Line, QueueQuality, QueueState, TranscriptView, CUE_WORD, CUT_WORD, OPEN_CLIP, PANEL};

mod quotes;

pub const MODULE_ID: &str = "qnc.module.transcript-panel";

/// Paints the transcript of the shown clip on the whole of `ui`; `timecode` is the shown clip's
/// source timecode (frames are shown without it).
pub fn show(ui: &mut egui::Ui, view: &TranscriptView, theme: &qnc_desktop_theme::Theme, timecode: Option<&SourceTimecode>) -> Option<ActionEvent> {
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(rect, 0.0, theme.bg);
    let inner = rect.shrink(8.0);
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
    ui.set_clip_rect(inner.intersect(ui.clip_rect()));
    ui.label(RichText::new(qnc_ui_text::text("transcript.title")).color(theme.text).strong());
    if !view.message.is_empty() {
        ui.label(RichText::new(&view.message).color(theme.text_muted).size(theme.font_ui));
    }
    let opened = paint_queue(&mut ui, view, theme);
    ui.add_space(6.0);
    let quoted = quotes::show(&mut ui, view, theme);
    ui.add_space(6.0);
    let early = opened.or(quoted);
    if view.lines.is_empty() && view.silences.is_empty() {
        let empty = if view.clip_id.is_some() { "transcript.empty" } else { "transcript.no_clip" };
        ui.label(RichText::new(qnc_ui_text::text(empty)).color(theme.text_muted).size(theme.font_ui));
        return early;
    }
    let mut asked = None;
    let at = |frame: u64| timecode.map_or_else(|| frame.to_string(), |tc| tc.label(frame));
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(&mut ui, |ui| {
        // Lines and silences in the order of their frames.
        let mut silences = view.silences.iter().peekable();
        for line in view.lines.iter() {
            while let Some((from, to)) = silences.next_if(|(from, _)| *from < line.from) {
                paint_silence(ui, &at(*from), &at(*to), theme);
            }
            if let Some(word) = paint_line(ui, line, view, theme) {
                asked = Some(word);
            }
            ui.add_space(4.0);
        }
        for (from, to) in silences {
            paint_silence(ui, &at(*from), &at(*to), theme);
        }
    });
    asked.map(|(from, to, double)| match double {
        // A double click on a word is a cut there (user 2026-10-26); its first click cued it.
        true => ActionEvent::key(CUT_WORD).with_value(from).with_target(to.to_string()).at(PANEL),
        false => ActionEvent::key(CUE_WORD).with_value(from).at(PANEL),
    }).or(early)
}

/// Sound that was transcribed and said nothing, in red (user 2026-10-26).
fn paint_silence(ui: &mut egui::Ui, from: &str, to: &str, theme: &qnc_desktop_theme::Theme) {
    let text = qnc_ui_text::fill("transcript.silence", &[("from", from), ("to", to)]);
    ui.label(RichText::new(text).color(theme.danger).size(theme.font_ui));
    ui.add_space(4.0);
}

/// The queue of the project, one row a clip: its name, the length of its IN/OUT, Fast or
/// Quality and where it is; the running one with a progress bar and how long it runs, a done
/// one with how long it took and how many times faster than real time (user 2026-10-26).
fn paint_queue(ui: &mut egui::Ui, view: &TranscriptView, theme: &qnc_desktop_theme::Theme) -> Option<ActionEvent> {
    let mut opened = None;
    if view.queue.is_empty() {
        return None;
    }
    ui.add_space(4.0);
    ui.label(RichText::new(qnc_ui_text::text("transcript.queue")).color(theme.text_muted).size(theme.font_ui));
    for row in &view.queue {
        let (state, color) = match row.state {
            QueueState::Waiting => (qnc_ui_text::text("transcript.waiting").to_string(), theme.text_muted),
            QueueState::Running => (
                row.elapsed_seconds.map_or_else(|| qnc_ui_text::text("transcript.running").to_string(), |elapsed| qnc_ui_text::fill("transcript.running_for", &[("elapsed", &clock(elapsed))])),
                theme.focus,
            ),
            QueueState::Failed => (qnc_ui_text::text("transcript.failed").to_string(), theme.danger),
            QueueState::Done => (
                row.speed().map_or_else(
                    || qnc_ui_text::text("transcript.done").to_string(),
                    |speed| qnc_ui_text::fill("transcript.done_in", &[("took", &clock(row.took_seconds)), ("speed", &format!("{speed:.1}"))]),
                ),
                theme.text_muted,
            ),
        };
        let quality = if row.quality == QueueQuality::Fast { "transcript.fast" } else { "transcript.best" };
        let text = format!("{}  ·  {}  ·  {}  ·  {}", row.name, clock(row.length_seconds), qnc_ui_text::text(quality), state);
        // A done clip is a link: its text opens without looking for it in the pool (user 2026-10-26).
        let done = row.state == QueueState::Done;
        let mut rich = RichText::new(text).color(color).size(theme.font_ui);
        if done {
            rich = rich.underline();
        }
        if view.clip_id.as_deref() == Some(row.clip_id.as_str()) {
            rich = rich.strong().color(theme.text);
        }
        let label = ui.add(egui::Label::new(rich).sense(if done { Sense::click() } else { Sense::hover() }));
        if done && label.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if done && label.clicked() {
            opened = Some(ActionEvent::key(OPEN_CLIP).with_target(row.clip_id.clone()).at(PANEL));
        }
        if !row.error.is_empty() {
            label.on_hover_text(&row.error);
        }
        if row.state == QueueState::Running {
            let bar = match row.progress {
                Some(done) => egui::ProgressBar::new(done as f32).text(format!("{:.0} %", done * 100.0)),
                None => egui::ProgressBar::new(0.0).text(qnc_ui_text::text("transcript.reading")).animate(true),
            };
            ui.add(bar.desired_height(theme.font_ui + 4.0).fill(theme.focus));
        }
    }
    opened
}

/// Seconds as m:ss (h:mm:ss from an hour on).
fn clock(seconds: f64) -> String {
    let whole = seconds.max(0.0).round() as u64;
    let (hours, minutes, seconds) = (whole / 3600, whole / 60 % 60, whole % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// One line as one text wrapped at the panel's width (user 2026-10-26: "sentences go out of the
/// panel, they should wrap"), the word said at the playhead lit; returns the first frame of a
/// clicked word.
fn paint_line(ui: &mut egui::Ui, line: &Line, view: &TranscriptView, theme: &qnc_desktop_theme::Theme) -> Option<(u64, u64, bool)> {
    let running = view.running.is_some_and(|(from, to)| line.from >= from && line.to <= to);
    let base = if running { theme.text_muted } else { theme.text };
    let font = egui::FontId::proportional(theme.font_ui);
    let words: Vec<(&str, u64, u64)> = if line.words.is_empty() {
        vec![(line.text.as_str(), line.from, line.to)]
    } else {
        line.words.iter().map(|w| (w.text.as_str(), w.from, w.to.max(w.from + 1))).collect()
    };
    let mut job = egui::text::LayoutJob::default();
    let mut spans = Vec::with_capacity(words.len());
    for (at, (text, from, to)) in words.iter().enumerate() {
        if at > 0 {
            job.append(" ", 0.0, egui::TextFormat::simple(font.clone(), base));
        }
        let lit = said(view, *from, *to);
        let marked = view.marks.is_some_and(|(a, b)| (a..b).contains(&((from + to) / 2)));
        let mut format = egui::TextFormat::simple(font.clone(), if lit { theme.black } else { base });
        if lit {
            format.background = theme.focus;
        } else if marked {
            // Between IN and OUT: what a statement takes (user 2026-10-26).
            format.background = theme.surface_alt;
            format.underline = egui::Stroke::new(1.0, theme.focus);
        }
        let start = job.text.chars().count();
        job.append(text, 0.0, format);
        spans.push((start, start + text.chars().count(), (*from, *to)));
    }
    job.wrap.max_width = ui.available_width().max(1.0);
    let galley = ui.fonts(|fonts| fonts.layout_job(job));
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::click());
    let word_at = |pos: egui::Pos2| {
        let char_index = galley.cursor_from_pos(pos - rect.min).ccursor.index;
        spans.iter().find(|(start, end, _)| (*start..=*end).contains(&char_index)).map(|(_, _, word)| *word)
    };
    if response.hovered() && response.hover_pos().and_then(word_at).is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let double = response.double_clicked();
    let clicked = if response.clicked() || double { response.interact_pointer_pos().and_then(word_at).map(|(from, to)| (from, to, double)) } else { None };
    ui.painter().galley(rect.min, galley, base);
    clicked
}

fn said(view: &TranscriptView, from: u64, to: u64) -> bool {
    view.playhead.is_some_and(|playhead| (from..to).contains(&playhead))
}
