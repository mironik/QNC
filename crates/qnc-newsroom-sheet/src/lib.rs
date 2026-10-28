//! The story sheet (user 2026-10-27: the data of a news story is a sheet the newsroom shapes;
//! an administrator makes their own when none of the prepared ones fits; the project name is
//! written in it and the form takes it). The overlay over the desktop fills a sheet: the
//! project name first, then its fields one under the other, the catalog key for the next
//! field moving on. Its editor changes the sheet (names, kinds, choices, order, new fields)
//! unless it is locked. It writes the sheets of this computer; the values go back to its
//! owner, which keeps them in the project database (`qnc-newsroom-sheet-db`).

mod sheets;

use std::path::Path;

use eframe::egui::{self, Align2, RichText, Stroke, Vec2};
use qnc_desktop_theme::Theme;
use qnc_settings_form::{FieldKind, GridStyle};
use serde_json::Value;

pub use sheets::{code_of, new_field, new_field_in, valid_code, Origin, SheetSpec, Sheets};

pub const MODULE_ID: &str = "qnc.module.newsroom-sheet";

const WIDTH: f32 = 760.0;
pub const LABEL_WIDTH: f32 = 200.0;
/// As tall as the template picker row of the Project panel (user 2026-10-28).
pub const ROW_HEIGHT: f32 = 32.0;
pub const ROW_GAP: f32 = 12.0;

/// What the user did with the overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Create (a new project) or Save (an open one): the owner takes name and values.
    Confirm,
    Cancel,
}

/// The overlay of one story: its sheet, project name and values.
pub struct SheetOverlay {
    pub sheets: Sheets,
    pub name: String,
    /// The name is typed only for a new project; an open one shows its name.
    pub name_editable: bool,
    /// The value of each field, by code.
    pub values: Value,
    pub confirm_label: String,
    /// The sheet being changed, while the editor is open.
    editing: Option<SheetSpec>,
    new_label: String,
    pub message: String,
}

impl SheetOverlay {
    /// A new overlay over the sheets of the QNC root; `kept` is the sheet a project already
    /// has (it is shown as it was kept, before the sheets of this computer).
    pub fn new(root: &Path, name: String, name_editable: bool, values: Value, kept: Option<SheetSpec>, confirm_label: String) -> Self {
        let (mut sheets, errors) = Sheets::load(root);
        if let Some(kept) = kept {
            match sheets.list.iter().position(|(sheet, _)| sheet.sheet_id == kept.sheet_id) {
                Some(index) => {
                    sheets.list[index].0 = kept;
                    sheets.chosen = index;
                }
                None => {
                    sheets.list.insert(0, (kept, Origin::Own));
                    sheets.chosen = 0;
                }
            }
        }
        let values = if values.is_object() { values } else { Value::Object(Default::default()) };
        let message = errors.join("\n");
        // A new project starts with today's date (user 2026-10-28: y/m/d is easy to read).
        let name = if name_editable && name.trim().is_empty() { today() } else { name };
        Self { sheets, name, name_editable, values, confirm_label, editing: None, new_label: String::new(), message }
    }

    /// The sheet filled and its values, for the owner to keep.
    pub fn filled(&self) -> Option<(SheetSpec, Value)> {
        let sheet = self.sheets.current()?.clone();
        let mut values = serde_json::Map::new();
        for field in &sheet.fields {
            if let Some(value) = self.values.get(&field.path) {
                values.insert(field.path.clone(), value.clone());
            }
        }
        Some((sheet, Value::Object(values)))
    }
}

const MARGIN: f32 = 24.0;

/// Today on this computer as `yyyy/mm/dd `, the start of a new project name.
pub fn today() -> String {
    chrono::Local::now().format("%Y/%m/%d ").to_string()
}

/// Draws the overlay in the middle of the screen, whole on it: a title bar as a window has,
/// the sheet (it scrolls when it is taller than the screen), the buttons on the right at the
/// bottom. `next_field` is the catalog key for the next field, pressed while a field is typed in.
pub fn show(ctx: &egui::Context, overlay: &mut SheetOverlay, theme: &Theme, next_field: bool) -> Option<Answer> {
    let screen = ctx.screen_rect();
    let width = WIDTH.min(screen.width() - 2.0 * MARGIN);
    let bar = theme.chrome_row_height.max(ROW_HEIGHT + 4.0);
    let buttons = theme.chrome_control_height + 2.0 * MARGIN;
    let body_height = (screen.height() - 2.0 * MARGIN - bar - buttons - 2.0 * MARGIN).max(80.0);
    let editing = overlay.editing.is_some();
    let title = qnc_ui_text::text(if editing { "sheet.edit_title" } else { "sheet.title" });
    let mut answer = None;
    egui::Area::new(egui::Id::new("qnc_story_sheet"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::NONE.fill(theme.panel).stroke(Stroke::new(1.0, theme.border)).show(ui, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 0.0;
                title_bar(ui, theme, title, width, bar);
                egui::Frame::NONE.inner_margin(egui::Margin { left: MARGIN as i8, right: MARGIN as i8, top: MARGIN as i8, bottom: 0 }).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    egui::ScrollArea::vertical().max_height(body_height).auto_shrink([false, true]).show(ui, |ui| {
                        if editing {
                            editor(ui, overlay, theme);
                        } else {
                            fill(ui, overlay, theme, next_field);
                        }
                    });
                });
                egui::Frame::NONE.inner_margin(MARGIN).show(ui, |ui| {
                    answer = if editing { editor_buttons(ui, overlay, theme) } else { fill_buttons(ui, overlay, theme) };
                });
            });
        });
    answer
}

/// The title of the overlay in a bar along its top, as a window has.
fn title_bar(ui: &mut egui::Ui, theme: &Theme, title: &str, width: f32, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, theme.surface_alt);
    ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, theme.border));
    ui.painter().text(
        rect.left_center() + Vec2::new(MARGIN, 0.0),
        Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(theme.font_ui),
        theme.text,
    );
}

/// Buttons on the right of their row, the proposed one rightmost.
fn right_row(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add);
}

fn style(theme: &Theme) -> GridStyle {
    GridStyle {
        label_color: theme.text_muted,
        label_size: theme.font_ui,
        min_cell_width: 160.0,
        gap: ROW_GAP,
        cell_height: ROW_HEIGHT + 4.0,
        label_beside: Some((LABEL_WIDTH, ROW_HEIGHT)),
    }
}

/// A text box as tall as a row, its text in the middle.
pub fn row_box(text: &mut String, width: f32) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(text).desired_width(width).min_size(Vec2::new(0.0, ROW_HEIGHT)).vertical_align(egui::Align::Center)
}

/// A row's label on the left, as wide as the labels of the fields and as tall as the row.
pub fn label(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    let text = RichText::new(text).size(theme.font_ui).color(theme.text_muted);
    ui.add_sized(Vec2::new(LABEL_WIDTH, ROW_HEIGHT), egui::Label::new(text));
    ui.add_space(ROW_GAP);
}

pub fn button(ui: &mut egui::Ui, label: &str, enabled: bool, theme: &Theme) -> bool {
    qnc_ui_kit::chrome_button(ui, label, enabled, false, qnc_theme_styles::button(theme)).clicked()
}

/// The sheet filled: its choice, the project name, its fields.
fn fill(ui: &mut egui::Ui, overlay: &mut SheetOverlay, theme: &Theme, next_field: bool) {
    let width = ui.available_width();
    let field_w = width - LABEL_WIDTH - ROW_GAP;
    ui.spacing_mut().interact_size.y = ROW_HEIGHT;
    let mut chosen = None;
    ui.horizontal(|ui| {
        label(ui, theme, qnc_ui_text::text("sheet.sheet"));
        let shown = overlay.sheets.current().map_or("—".to_string(), |sheet| sheet.name.clone());
        egui::ComboBox::from_id_salt("qnc_story_sheet_choice").selected_text(shown).width(field_w * 0.6).show_ui(ui, |ui| {
            for (index, (sheet, _)) in overlay.sheets.list.iter().enumerate() {
                if ui.selectable_label(index == overlay.sheets.chosen, &sheet.name).clicked() {
                    chosen = Some(index);
                }
            }
        });
        if button(ui, qnc_ui_text::text("sheet.edit"), overlay.sheets.current().is_some(), theme) {
            overlay.editing = overlay.sheets.current().cloned();
        }
    });
    if let Some(index) = chosen {
        if let Err(error) = overlay.sheets.choose(index) {
            overlay.message = error;
        }
    }
    ui.add_space(ROW_GAP);
    let name_id = egui::Id::new("qnc_story_sheet_name");
    let from_name = next_field && ui.memory(|memory| memory.has_focus(name_id));
    ui.horizontal(|ui| {
        label(ui, theme, qnc_ui_text::text("project.project_name"));
        let edit = egui::TextEdit::singleline(&mut overlay.name)
            .id(name_id)
            .desired_width(field_w)
            .min_size(Vec2::new(0.0, ROW_HEIGHT))
            .vertical_align(egui::Align::Center);
        ui.add_enabled(overlay.name_editable, edit.hint_text(qnc_ui_text::text("project.name_new_project")));
    });
    ui.add_space(ROW_GAP / 2.0);
    if let Some(sheet) = overlay.sheets.current().cloned() {
        qnc_settings_form::field_column(ui, width, &mut overlay.values, &sheet.fields, &sheet.options, &style(theme), next_field, from_name);
    }
    if !overlay.message.is_empty() {
        ui.label(RichText::new(&overlay.message).color(theme.danger));
    }
}

/// Confirm (rightmost) and Cancel of the filled sheet.
fn fill_buttons(ui: &mut egui::Ui, overlay: &SheetOverlay, theme: &Theme) -> Option<Answer> {
    let mut answer = None;
    right_row(ui, |ui| {
        let ready = !overlay.name.trim().is_empty() && overlay.sheets.current().is_some();
        if qnc_ui_kit::strong_button(ui, &overlay.confirm_label, ready, theme.accent, theme.chrome_control_height).clicked() {
            answer = Some(Answer::Confirm);
        }
        if button(ui, qnc_ui_text::text("common.cancel"), true, theme) {
            answer = Some(Answer::Cancel);
        }
    });
    answer
}

const KINDS: [(FieldKind, &str); 4] = [
    (FieldKind::Text, "sheet.kind_text"),
    (FieldKind::Choice, "sheet.kind_choice"),
    (FieldKind::Duration, "sheet.kind_duration"),
    (FieldKind::Integer, "sheet.kind_number"),
];

/// The editor of the sheet: its name, the lock, each field (name, kind, choices, order,
/// removal) and a new field; Save sheet keeps it on this computer, Back leaves it as it was.
fn editor(ui: &mut egui::Ui, overlay: &mut SheetOverlay, theme: &Theme) {
    let width = ui.available_width();
    ui.spacing_mut().interact_size.y = ROW_HEIGHT;
    ui.spacing_mut().item_spacing.y = ROW_GAP / 2.0;
    let Some(sheet) = overlay.editing.as_mut() else { return };
    ui.horizontal(|ui| {
        label(ui, theme, qnc_ui_text::text("sheet.name"));
        ui.add_enabled(!sheet.locked, row_box(&mut sheet.name, width - LABEL_WIDTH - ROW_GAP));
    });
    ui.checkbox(&mut sheet.locked, qnc_ui_text::text("sheet.locked"));
    ui.add_space(ROW_GAP);
    fields_editor(ui, "qnc_story_sheet", &mut sheet.fields, &mut sheet.options, &mut overlay.new_label, sheet.locked, theme);
    if !overlay.message.is_empty() {
        ui.label(RichText::new(&overlay.message).color(theme.danger));
    }
}

/// The editor of a list of fields (user 2026-10-28: the same for a story sheet and for the fields
/// of a kind of script block): each field's name, kind, choices, order and removal, and a new
/// field whose code comes from its name. Nothing changes while `locked`.
pub fn fields_editor(
    ui: &mut egui::Ui,
    salt: &str,
    fields: &mut Vec<qnc_settings_form::FieldSpec>,
    options: &mut qnc_settings_form::OptionSets,
    new_label: &mut String,
    locked: bool,
    theme: &Theme,
) {
    let width = ui.available_width();
    let field_label_w = width * 0.4;
    let (mut up, mut down, mut gone) = (None, None, None);
    let count = fields.len();
    for (index, field) in fields.iter_mut().enumerate() {
        ui.add_enabled_ui(!locked, |ui| {
            ui.horizontal(|ui| {
                ui.add(row_box(&mut field.label, field_label_w));
                let kind = KINDS.iter().find(|(kind, _)| *kind == field.kind).map_or("", |(_, code)| qnc_ui_text::text(code));
                egui::ComboBox::from_id_salt((salt, "kind", index)).selected_text(kind).width(110.0).show_ui(ui, |ui| {
                    for (kind, code) in KINDS {
                        ui.selectable_value(&mut field.kind, kind, qnc_ui_text::text(code));
                    }
                });
                up = up.or((button(ui, "▲", index > 0, theme)).then_some(index));
                down = down.or((button(ui, "▼", index + 1 < count, theme)).then_some(index));
                gone = gone.or((button(ui, qnc_ui_text::text("sheet.remove"), true, theme)).then_some(index));
            });
            if field.kind == FieldKind::Choice {
                choices_row(ui, field, options, width);
            }
        });
    }
    if let Some(index) = up {
        fields.swap(index, index - 1);
    }
    if let Some(index) = down {
        fields.swap(index, index + 1);
    }
    if let Some(index) = gone {
        fields.remove(index);
    }
    ui.add_enabled_ui(!locked, |ui| {
        ui.horizontal(|ui| {
            ui.add(row_box(new_label, field_label_w).hint_text(qnc_ui_text::text("sheet.new_field_hint")));
            if button(ui, qnc_ui_text::text("sheet.add_field"), !new_label.trim().is_empty(), theme) {
                let field = new_field_in(fields, new_label);
                fields.push(field);
                new_label.clear();
            }
        });
    });
}

/// Save sheet (rightmost) and Back of the editor; the overlay stays open either way.
fn editor_buttons(ui: &mut egui::Ui, overlay: &mut SheetOverlay, theme: &Theme) -> Option<Answer> {
    let (mut keep, mut back) = (false, false);
    right_row(ui, |ui| {
        keep = qnc_ui_kit::strong_button(ui, qnc_ui_text::text("sheet.save"), true, theme.accent, theme.chrome_control_height).clicked();
        back = button(ui, qnc_ui_text::text("sheet.back"), true, theme);
    });
    if keep {
        if let Some(Err(error)) = overlay.editing.take().map(|sheet| overlay.sheets.keep(sheet)) {
            overlay.message = error;
        }
    } else if back {
        overlay.editing = None;
    }
    None
}

/// The choices of a choice field, typed as names separated by commas.
fn choices_row(ui: &mut egui::Ui, field: &mut qnc_settings_form::FieldSpec, options: &mut qnc_settings_form::OptionSets, width: f32) {
    let key = field.options.clone().unwrap_or_else(|| field.path.clone());
    let mut typed = options.get(&key).map(|choices| choices.iter().map(|(_, label)| label.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.add_space(24.0);
        let edited = ui.add(row_box(&mut typed, width - 24.0).hint_text(qnc_ui_text::text("sheet.choices_hint"))).changed();
        if edited || field.options.is_none() {
            let choices = typed.split(',').map(str::trim).filter(|label| !label.is_empty()).map(|label| (code_of(label), label.to_string())).collect();
            options.insert(key.clone(), choices);
            field.options = Some(key);
        }
    });
}

#[cfg(test)]
mod tests;
