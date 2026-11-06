//! The story sheet as data (docs/99 step 3): the sheets of this computer and the prepared ones,
//! the overlay of one story (its sheet, project name and values) and what the user answered.
//! Core: no egui; `qnc-newsroom-sheet` draws it.

mod sheets;

use std::path::Path;

use serde_json::Value;

pub use sheets::{code_of, new_field, new_field_in, valid_code, Origin, SheetSpec, Sheets};

pub const MODULE_ID: &str = "qnc.module.newsroom-sheet-model";

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
    pub editing: Option<SheetSpec>,
    pub new_label: String,
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

/// Today on this computer as `yyyy/mm/dd `, the start of a new project name.
pub fn today() -> String {
    chrono::Local::now().format("%Y/%m/%d ").to_string()
}

#[cfg(test)]
mod tests;
