//! What a field is (audit 2026-10-28: the description of a field lived in the screen piece that
//! draws it, so the script database and the script profile pulled the whole screen library
//! with them). A field of a settings page, a story sheet or a kind of a script block: its kind,
//! name, code, named choices, default and hint, and the codes fields, sheets and profiles are
//! kept by. Data only: it draws nothing, reads and writes nothing.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const MODULE_ID: &str = "qnc.module.field-spec";

/// Named choices: `(stored value, shown label)`.
pub type OptionSets = BTreeMap<String, Vec<(String, String)>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    /// One of the named choices; stored as text.
    Choice,
    /// A decimal number, typed or one of the named choices.
    Number,
    /// A whole number, typed or one of the named choices.
    Integer,
    /// Typed text, stored as it is typed.
    Text,
    /// A length typed as `m:ss` (or whole seconds), stored as whole seconds; 0 is none.
    Duration,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldSpec {
    pub kind: FieldKind,
    pub label: String,
    /// The widget id (unique on the page).
    pub id: String,
    /// Dotted path in the settings, e.g. `input.mode` (the code of a sheet's or a kind's field).
    pub path: String,
    /// The named choices (none for a typed number).
    #[serde(default)]
    pub options: Option<String>,
    /// The value shown when the settings have none.
    #[serde(default)]
    pub default: String,
    /// Grey text in an empty typed field.
    #[serde(default)]
    pub hint: String,
}

/// A field, sheet or profile code: lower-case letters, digits and `_`, starting with a letter.
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

/// A new text field named `label` among `fields`, its code free among them.
pub fn new_field_in(fields: &[FieldSpec], label: &str) -> FieldSpec {
    let base = code_of(label);
    let taken = |code: &str| fields.iter().any(|field| field.path == code);
    let code = if taken(&base) { (2..).map(|n| format!("{base}_{n}")).find(|code| !taken(code)).unwrap_or_default() } else { base };
    FieldSpec { kind: FieldKind::Text, label: label.trim().to_string(), id: code.clone(), path: code, options: None, default: String::new(), hint: String::new() }
}
