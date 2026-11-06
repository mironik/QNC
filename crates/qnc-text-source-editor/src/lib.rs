//! TextSourceEditor (user 2026-10-28): the small editor at the bottom of a panel where text is
//! written and corrected before it goes somewhere (the OFF text of the script goes into its
//! table with Add OFF). It paints the text it is given, a line of what the text is (its
//! length to read) and the buttons the caller names; it gives back the text when typing
//! changed it, or the action of a pressed button. It holds no text, reads nothing and knows no
//! form: the caller keeps the text and gives it again at the next paint.

use eframe::egui::{self, RichText};

pub const MODULE_ID: &str = "qnc.module.text-source-editor";

/// A button under the text: its label, the catalog action it stands for, what it names (empty:
/// nothing; for example the kind of block the text goes in as) and whether it can be
/// pressed now.
pub struct EditorButton<'a> {
    pub label: &'a str,
    pub action_id: &'a str,
    pub target: &'a str,
    pub enabled: bool,
    /// The group the button belongs to; buttons of one group stand together and a wider gap
    /// parts the groups (user 2026-11-01: buttons in logical groups, not scattered).
    pub group: u8,
}

/// The gap between two groups of buttons.
const GROUP_GAP: f32 = 18.0;

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

fn focus_id(id: &str) -> egui::Id {
    egui::Id::new(("qnc_text_source_editor_focus", id))
}

/// The editor `id` takes the keyboard the next time it is shown, so the cursor is in it at once
/// (user 2026-11-03: a click on NAJAVA opens the box, the cursor has to be in it). Once only;
/// a click elsewhere afterwards keeps its focus.
pub fn focus(ctx: &egui::Context, id: &str) {
    ctx.data_mut(|data| data.insert_temp(focus_id(id), true));
}

/// What the editor gave back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Typing changed the text; the whole new text.
    Text(String),
    /// A button was pressed; its action and what it names.
    Button(String, Option<String>),
}

/// Paints the editor over the whole of `ui` (the text takes what the title, info and buttons
/// leave).
pub fn show(ui: &mut egui::Ui, editor: &Editor<'_>, theme: &qnc_desktop_theme::Theme) -> Option<Answer> {
    let mut answer = None;
    let style = qnc_ui_kit::ChromeButtonStyle { text: theme.text, border: theme.border, focus: theme.focus, height: theme.chrome_control_height };
    ui.horizontal(|ui| {
        ui.label(RichText::new(editor.title).color(theme.text_muted).size(theme.font_ui));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut group = None;
            for button in editor.buttons.iter().rev() {
                if group.is_some_and(|group| group != button.group) {
                    ui.add_space(GROUP_GAP);
                }
                group = Some(button.group);
                if qnc_ui_kit::chrome_button(ui, button.label, button.enabled, false, style).clicked() {
                    answer = Some(Answer::Button(button.action_id.to_string(), (!button.target.is_empty()).then(|| button.target.to_string())));
                }
            }
        });
    });
    let info_height = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
    let height = (ui.available_height() - info_height).max(48.0);
    let mut text = editor.text.to_string();
    egui::ScrollArea::vertical().id_salt((editor.id, "scroll")).max_height(height).auto_shrink([false, false]).show(ui, |ui| {
        let edit = qnc_ui_kit::text_area(&mut text)
            .id_salt(editor.id)
            .hint_text(editor.hint)
            .font(egui::FontId::proportional(theme.font_ui + 1.0))
            .desired_width(f32::INFINITY)
            .desired_rows(4)
            .frame(true);
        let response = ui.add_sized([ui.available_width(), height.max(ui.available_height())], edit);
        if ui.ctx().data_mut(|data| data.remove_temp::<bool>(focus_id(editor.id))).is_some() {
            response.request_focus();
        }
        if response.changed() && answer.is_none() {
            answer = Some(Answer::Text(text.clone()));
        }
    });
    ui.label(RichText::new(editor.info).color(theme.text_muted).size(theme.font_ui));
    answer
}
