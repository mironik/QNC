//! TextSourceEditor (user 2026-10-28): the small editor at the bottom of a panel where text is
//! written and corrected before it goes somewhere (the OFF text of the script goes into its
//! table with Add OFF). It paints the text it is given, a line of what the text is (its
//! length to read) and the buttons the caller names; it gives back the text when typing
//! changed it, or the action of a pressed button. It holds no text, reads nothing and knows no
//! form: the caller keeps the text and gives it again at the next paint.

use eframe::egui::{self, RichText};

pub const MODULE_ID: &str = "qnc.module.text-source-editor";

/// A button under the text: its label, the catalog action it stands for and whether it can be
/// pressed now.
pub struct EditorButton<'a> {
    pub label: &'a str,
    pub action_id: &'a str,
    pub enabled: bool,
}

/// What the editor shows.
pub struct Editor<'a> {
    /// The id the editor keeps its cursor under (one per place it is shown).
    pub id: &'a str,
    pub title: &'a str,
    pub text: &'a str,
    /// A line under the text (for example how long it takes to read).
    pub info: &'a str,
    /// Shown greyed in an empty editor.
    pub hint: &'a str,
    pub buttons: &'a [EditorButton<'a>],
}

/// What the editor gave back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Typing changed the text; the whole new text.
    Text(String),
    /// A button was pressed; its action.
    Button(String),
}

/// Paints the editor over the whole of `ui` (the text takes what the title, info and buttons
/// leave).
pub fn show(ui: &mut egui::Ui, editor: &Editor<'_>, theme: &qnc_desktop_theme::Theme) -> Option<Answer> {
    let mut answer = None;
    let style = qnc_ui_kit::ChromeButtonStyle { text: theme.text, border: theme.border, focus: theme.focus, height: theme.chrome_control_height };
    ui.horizontal(|ui| {
        ui.label(RichText::new(editor.title).color(theme.text_muted).size(theme.font_ui));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for button in editor.buttons.iter().rev() {
                if qnc_ui_kit::chrome_button(ui, button.label, button.enabled, false, style).clicked() {
                    answer = Some(Answer::Button(button.action_id.to_string()));
                }
            }
        });
    });
    let info_height = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
    let height = (ui.available_height() - info_height).max(48.0);
    let mut text = editor.text.to_string();
    egui::ScrollArea::vertical().id_salt((editor.id, "scroll")).max_height(height).auto_shrink([false, false]).show(ui, |ui| {
        let edit = egui::TextEdit::multiline(&mut text)
            .id_salt(editor.id)
            .hint_text(editor.hint)
            .font(egui::FontId::proportional(theme.font_ui + 1.0))
            .desired_width(f32::INFINITY)
            .desired_rows(4)
            .frame(true);
        if ui.add_sized([ui.available_width(), height.max(ui.available_height())], edit).changed() && answer.is_none() {
            answer = Some(Answer::Text(text.clone()));
        }
    });
    ui.label(RichText::new(editor.info).color(theme.text_muted).size(theme.font_ui));
    answer
}
