//! The story sheets: the ones QNC brings (`catalogs/story-sheets`, their names are text codes)
//! and the ones a newsroom made on this computer (`data/story-sheets`, names as typed), and
//! which one is chosen (`data/story-sheet.json`). A sheet is fields by code; its names are free.

use std::fs;
use std::path::{Path, PathBuf};

use qnc_settings_form::{FieldKind, FieldSpec, OptionSets};
use serde::{Deserialize, Serialize};

/// One sheet: its fields in order, their choices, and whether it may be changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetSpec {
    pub sheet_id: String,
    pub name: String,
    /// While locked only the values are typed; the sheet itself is not changed.
    #[serde(default)]
    pub locked: bool,
    pub fields: Vec<FieldSpec>,
    #[serde(default)]
    pub options: OptionSets,
}

/// Where the sheet comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Brought by QNC: it is copied, never changed.
    Prepared,
    /// Made on this computer.
    Own,
}

#[derive(Debug, Clone)]
pub struct Sheets {
    root: PathBuf,
    pub list: Vec<(SheetSpec, Origin)>,
    pub chosen: usize,
}

#[derive(Debug, Deserialize, Serialize)]
struct Chosen {
    selected: String,
}

impl Sheets {
    /// The sheets of the QNC root; one that cannot be read is left out and named in the error.
    pub fn load(root: &Path) -> (Self, Vec<String>) {
        let mut list = Vec::new();
        let mut errors = Vec::new();
        for (dir, origin) in [(prepared_dir(root), Origin::Prepared), (own_dir(root), Origin::Own)] {
            let mut files: Vec<PathBuf> = fs::read_dir(&dir).into_iter().flatten().flatten().map(|entry| entry.path()).collect();
            files.retain(|file| file.extension().is_some_and(|extension| extension == "json"));
            files.sort();
            for file in files {
                match read_sheet(&file) {
                    Ok(sheet) => list.push((sheet, origin)),
                    Err(error) => errors.push(format!("{}: {error}", file.display())),
                }
            }
        }
        let selected = fs::read_to_string(chosen_file(root)).ok().and_then(|text| serde_json::from_str::<Chosen>(&text).ok()).map(|chosen| chosen.selected);
        let chosen = selected.and_then(|id| list.iter().position(|(sheet, _)| sheet.sheet_id == id)).unwrap_or(0);
        (Self { root: root.to_path_buf(), list, chosen }, errors)
    }

    pub fn current(&self) -> Option<&SheetSpec> {
        self.list.get(self.chosen).map(|(sheet, _)| sheet)
    }

    /// Chooses sheet `index` for the next stories on this computer.
    pub fn choose(&mut self, index: usize) -> Result<(), String> {
        let Some((sheet, _)) = self.list.get(index) else { return Ok(()) };
        self.chosen = index;
        write_json(&chosen_file(&self.root), &Chosen { selected: sheet.sheet_id.clone() })
    }

    /// Keeps `sheet` as a sheet of this computer (a new one, or the own one of the same id)
    /// and chooses it. A prepared sheet is never written over: its copy gets its own id.
    pub fn keep(&mut self, mut sheet: SheetSpec) -> Result<(), String> {
        sheet.fields.retain(|field| valid_code(&field.path));
        let prepared = self.list.iter().any(|(other, origin)| *origin == Origin::Prepared && other.sheet_id == sheet.sheet_id);
        if prepared || !valid_code(&sheet.sheet_id) {
            sheet.sheet_id = free_id(&self.list, &code_of(&sheet.name));
        }
        write_json(&own_dir(&self.root).join(format!("{}.json", sheet.sheet_id)), &sheet)?;
        let index = match self.list.iter().position(|(other, _)| other.sheet_id == sheet.sheet_id) {
            Some(index) => {
                self.list[index] = (sheet, Origin::Own);
                index
            }
            None => {
                self.list.push((sheet, Origin::Own));
                self.list.len() - 1
            }
        };
        self.choose(index)
    }
}

/// A sheet file; its names may be text codes (`@...`) of the dictionary.
fn read_sheet(file: &Path) -> Result<SheetSpec, String> {
    let text = fs::read_to_string(file).map_err(|error| error.to_string())?;
    let value = qnc_ui_text::resolve_contract(&text)?;
    let sheet: SheetSpec = serde_json::from_value(value).map_err(|error| error.to_string())?;
    if let Some(field) = sheet.fields.iter().find(|field| !valid_code(&field.path)) {
        return Err(qnc_ui_text::fill("sheet.bad_code", &[("code", &field.path)]));
    }
    Ok(sheet)
}

fn write_json(file: &Path, value: &impl Serialize) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    fs::write(file, text).map_err(|error| error.to_string())
}

fn prepared_dir(root: &Path) -> PathBuf {
    root.join("catalogs").join("story-sheets")
}

/// Beside the other settings of this computer.
fn own_dir(root: &Path) -> PathBuf {
    qnc_dev_diagnostics::config_path(root).with_file_name("story-sheets")
}

fn chosen_file(root: &Path) -> PathBuf {
    qnc_dev_diagnostics::config_path(root).with_file_name("story-sheet.json")
}

/// A field or sheet code: lower-case letters, digits and `_`, starting with a letter.
pub fn valid_code(code: &str) -> bool {
    code.chars().next().is_some_and(|first| first.is_ascii_lowercase())
        && code.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// A code made of a typed name: letters and digits kept (accents dropped), the rest `_`.
pub fn code_of(name: &str) -> String {
    let mut code = String::new();
    for c in name.trim().to_lowercase().chars() {
        let plain = match c {
            'č' | 'ć' => 'c',
            'đ' => 'd',
            'š' => 's',
            'ž' => 'z',
            c => c,
        };
        if plain.is_ascii_alphanumeric() {
            code.push(plain);
        } else if !code.ends_with('_') && !code.is_empty() {
            code.push('_');
        }
    }
    let code = code.trim_end_matches('_').to_string();
    if code.chars().next().is_some_and(|first| first.is_ascii_lowercase()) {
        code
    } else {
        format!("f_{code}")
    }
}

fn free_id(list: &[(SheetSpec, Origin)], base: &str) -> String {
    let taken = |id: &str| list.iter().any(|(sheet, _)| sheet.sheet_id == id);
    if !taken(base) {
        return base.to_string();
    }
    (2..).map(|n| format!("{base}_{n}")).find(|id| !taken(id)).unwrap_or_default()
}

/// A new typed field of a sheet, its code made of its name and free in the sheet.
pub fn new_field(sheet: &SheetSpec, label: &str) -> FieldSpec {
    new_field_in(&sheet.fields, label)
}

/// A new text field named `label` among `fields`, its code free among them.
pub fn new_field_in(fields: &[FieldSpec], label: &str) -> FieldSpec {
    let base = code_of(label);
    let taken = |code: &str| fields.iter().any(|field| field.path == code);
    let code = if taken(&base) { (2..).map(|n| format!("{base}_{n}")).find(|code| !taken(code)).unwrap_or_default() } else { base };
    FieldSpec { kind: FieldKind::Text, label: label.trim().to_string(), id: code.clone(), path: code, options: None, default: String::new(), hint: String::new() }
}
