//! Settings fields drawn from a schema (QNC public piece, audit 2026-10-10: the fields of
//! Project Advanced lived as code in the form). A layout contract names each field (label,
//! widget id, settings path, its choices or a number) and the condition it shows under;
//! this piece draws them in a grid and writes what changed into the settings draft. It
//! knows no application and no meaning of a field; the owner writes the draft.

use std::collections::BTreeMap;

use eframe::egui::{self, Color32, RichText, Vec2};
use qnc_settings_path::{number_string, path_string, set_path, set_string_path};
use serde::Deserialize;
use serde_json::{Number, Value};

/// Named choices of the schema: `(stored value, shown label)`.
pub type OptionSets = BTreeMap<String, Vec<(String, String)>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    /// One of the named choices; stored as text.
    Choice,
    /// A decimal number, typed or one of the named choices.
    Number,
    /// A whole number, typed or one of the named choices.
    Integer,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldSpec {
    pub kind: FieldKind,
    pub label: String,
    /// The widget id (unique on the page).
    pub id: String,
    /// Dotted path in the settings, e.g. `input.mode`.
    pub path: String,
    /// The named choices (none for a typed number).
    #[serde(default)]
    pub options: Option<String>,
    /// The value shown when the settings have none.
    #[serde(default)]
    pub default: String,
}

/// Fields shown only while a setting has a value, e.g. manual input while `input.mode`
/// is `manual`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    pub path: String,
    pub equals: String,
    #[serde(default)]
    pub default: String,
}

impl Condition {
    pub fn holds(&self, settings: &Value) -> bool {
        path_string(settings, &self.path, &self.default) == self.equals
    }
}

/// Look of the grid, from the form's contract.
#[derive(Debug, Clone, Copy)]
pub struct GridStyle {
    pub label_color: Color32,
    pub label_size: f32,
    pub min_cell_width: f32,
    pub gap: f32,
    pub cell_height: f32,
}

/// Width of one cell of a grid `grid_w` wide with `cols` columns.
pub fn cell_width(grid_w: f32, cols: usize, style: &GridStyle) -> f32 {
    let cols = cols.max(1) as f32;
    ((grid_w - style.gap * (cols - 1.0)) / cols).max(style.min_cell_width.min(grid_w))
}

fn grid_cols(grid_w: f32, style: &GridStyle) -> usize {
    (((grid_w + style.gap) / (style.min_cell_width + style.gap)).floor() as usize).max(1)
}

/// The fields in rows as wide as the grid; true when a value changed.
pub fn field_grid(
    ui: &mut egui::Ui,
    grid_w: f32,
    settings: &mut Value,
    fields: &[FieldSpec],
    options: &OptionSets,
    style: &GridStyle,
) -> bool {
    let cols = grid_cols(grid_w, style);
    let width = cell_width(grid_w, cols, style);
    let mut changed = false;
    for row in fields.chunks(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = style.gap;
            for field in row {
                changed |= field_cell(ui, width, settings, field, options, style);
            }
        });
        ui.add_space(style.gap);
    }
    changed
}

fn field_cell(
    ui: &mut egui::Ui,
    width: f32,
    settings: &mut Value,
    field: &FieldSpec,
    options: &OptionSets,
    style: &GridStyle,
) -> bool {
    let choices = field
        .options
        .as_ref()
        .and_then(|name| options.get(name))
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    match field.kind {
        FieldKind::Choice => {
            let current = path_string(settings, &field.path, &field.default);
            match choice_cell(ui, width, &field.label, &field.id, &current, choices, style) {
                Some(value) => {
                    set_string_path(settings, &field.path, value);
                    true
                }
                None => false,
            }
        }
        FieldKind::Number | FieldKind::Integer => {
            let before = number_string(settings, &field.path);
            let text = number_cell(ui, width, field, &before, choices, style);
            text != before && set_number(settings, &field.path, &text, field.kind == FieldKind::Integer)
        }
    }
}

/// A labelled choice: the new value when the user picked another one.
pub fn choice_cell(
    ui: &mut egui::Ui,
    width: f32,
    label: &str,
    id: &str,
    current: &str,
    choices: &[(String, String)],
    style: &GridStyle,
) -> Option<String> {
    let mut value = current.to_string();
    let display = choices
        .iter()
        .find(|(option, _)| option == current)
        .map(|(_, label)| label.clone())
        .unwrap_or_else(|| current.to_string());
    labelled(ui, width, label, style, |ui| {
        egui::ComboBox::from_id_salt(id)
            .selected_text(display)
            .width(width)
            .show_ui(ui, |ui| {
                for (option, label) in choices {
                    ui.selectable_value(&mut value, option.clone(), label);
                }
            });
    });
    (value != current).then_some(value)
}

fn number_cell(
    ui: &mut egui::Ui,
    width: f32,
    field: &FieldSpec,
    before: &str,
    choices: &[(String, String)],
    style: &GridStyle,
) -> String {
    let mut text = before.to_string();
    labelled(ui, width, &field.label, style, |ui| {
        if choices.is_empty() {
            ui.add(egui::TextEdit::singleline(&mut text).desired_width(width).hint_text("0"));
        } else {
            egui::ComboBox::from_id_salt(&field.id)
                .selected_text(if text.is_empty() { "—" } else { text.as_str() })
                .width(width)
                .show_ui(ui, |ui| {
                    for (option, label) in choices {
                        ui.selectable_value(&mut text, option.clone(), label);
                    }
                });
        }
    });
    text
}

fn labelled(ui: &mut egui::Ui, width: f32, label: &str, style: &GridStyle, body: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(
        Vec2::new(width, style.cell_height),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_width(width);
            ui.label(RichText::new(label).size(style.label_size).color(style.label_color));
            ui.add_space(6.0);
            body(ui);
        },
    );
}

/// Writes a typed number (a comma is a decimal point); false when it is no number.
pub fn set_number(settings: &mut Value, path: &str, text: &str, integer: bool) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let Ok(value) = trimmed.replace(',', ".").parse::<f64>() else {
        return false;
    };
    if integer {
        set_path(settings, path, Value::Number(Number::from(value.round() as i64)));
    } else if let Some(number) = Number::from_f64(value) {
        set_path(settings, path, Value::Number(number));
    } else {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typed_number_is_written_and_a_word_is_not() {
        let mut settings = serde_json::json!({});
        assert!(set_number(&mut settings, "input.fps", "29,97", false));
        assert_eq!(settings["input"]["fps"], serde_json::json!(29.97));
        assert!(set_number(&mut settings, "input.width", "1920.4", true));
        assert_eq!(settings["input"]["width"], serde_json::json!(1920));
        assert!(!set_number(&mut settings, "input.width", "auto", true));
        let condition = Condition { path: "input.mode".into(), equals: "manual".into(), default: "auto".into() };
        assert!(!condition.holds(&settings));
    }

    #[test]
    fn a_schema_reads_from_its_contract() {
        let field: FieldSpec = serde_json::from_value(serde_json::json!({
            "kind": "integer", "label": "Width", "id": "w", "path": "input.width"
        }))
        .unwrap();
        assert_eq!((field.kind, field.options), (FieldKind::Integer, None));
    }
}
