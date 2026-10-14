//! The words of the QNC user interface (user decision 2026-10-14: everything in English,
//! every text with a code, so the language can be chosen). The layout contracts write a
//! text as `@code`; this block replaces it with the text of the chosen language from the
//! dictionaries in `contracts/text`. The language is a setting of this computer
//! (`ui-language.json` beside the other settings of this computer, `{"language": "en"}`); without it the base
//! dictionary, English, is used. A code a dictionary does not have is an error, never a
//! made-up text. It reads no project database and knows no application.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.ui-text";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The language every other dictionary follows.
pub const BASE_LANGUAGE: &str = "en";

/// The dictionaries QNC is built with: language code and its file.
pub const DICTIONARIES: &[(&str, &str)] = &[("en", include_str!("../../../contracts/text/en.json"))];

/// The texts of one language.
#[derive(Debug, Clone)]
pub struct Texts {
    pub language: String,
    texts: BTreeMap<String, String>,
}

impl Texts {
    /// The dictionary of `language`.
    pub fn for_language(language: &str) -> Result<Self, String> {
        let (_, json) = DICTIONARIES
            .iter()
            .find(|(code, _)| *code == language)
            .ok_or_else(|| format!("QNC nema rjecnik za jezik '{language}'."))?;
        let file: Dictionary = serde_json::from_str(json).map_err(|error| format!("rjecnik '{language}': {error}"))?;
        if file.language != language {
            return Err(format!("rjecnik '{language}' je oznacen kao '{}'", file.language));
        }
        Ok(Self { language: file.language, texts: file.texts })
    }

    /// The language this computer chose (`ui-language.json` beside its other settings), else the base one.
    pub fn for_host(root: Option<&Path>) -> Result<Self, String> {
        let setting = root.map(|root| qnc_dev_diagnostics::config_path(root).with_file_name("ui-language.json")).filter(|path| path.is_file());
        let language = match setting {
            Some(path) => {
                let text = std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
                let setting: Setting = serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
                setting.language
            }
            None => BASE_LANGUAGE.to_string(),
        };
        Self::for_language(&language)
    }

    /// The text of a code.
    pub fn get(&self, code: &str) -> Result<&str, String> {
        self.texts
            .get(code)
            .map(String::as_str)
            .ok_or_else(|| format!("rjecnik '{}' nema tekst '{code}'", self.language))
    }

    /// Every `"@code"` string of a contract replaced by its text; the first missing code is an error.
    pub fn resolve(&self, value: &mut Value) -> Result<(), String> {
        match value {
            Value::String(text) if text.starts_with('@') => {
                *text = self.get(&text[1..])?.to_string();
                Ok(())
            }
            Value::Array(items) => items.iter_mut().try_for_each(|item| self.resolve(item)),
            Value::Object(fields) => fields.values_mut().try_for_each(|field| self.resolve(field)),
            _ => Ok(()),
        }
    }

    pub fn codes(&self) -> impl Iterator<Item = &str> {
        self.texts.keys().map(String::as_str)
    }
}

/// The texts of this computer's language, read once per process (the QNC root of this
/// process); a missing setting is the base language.
pub fn host() -> Result<&'static Texts, String> {
    static HOST: OnceLock<Result<Texts, String>> = OnceLock::new();
    HOST.get_or_init(|| Texts::for_host(qnc_dev_diagnostics::locate_qnc_root().as_deref()))
        .as_ref()
        .map_err(Clone::clone)
}

/// The text of a code in this computer's language, for words the code of a block shows.
/// A code the dictionary lacks shows as the code itself (a test keeps every code used in
/// the source in every dictionary), never as a made-up text.
pub fn text(code: &'static str) -> &'static str {
    host().ok().and_then(|texts| texts.get(code).ok()).unwrap_or(code)
}

/// The text of a code with its `{name}` places filled.
pub fn fill(code: &'static str, values: &[(&str, &str)]) -> String {
    values.iter().fold(text(code).to_string(), |text, (name, value)| text.replace(&format!("{{{name}}}"), value))
}

/// A contract JSON with its `@code` texts in this computer's language.
pub fn resolve_contract(json: &str) -> Result<Value, String> {
    let mut value: Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
    host()?.resolve(&mut value)?;
    Ok(value)
}

/// Every `@code` a contract JSON names.
pub fn codes_in(value: &Value) -> Vec<String> {
    match value {
        Value::String(text) if text.starts_with('@') => vec![text[1..].to_string()],
        Value::Array(items) => items.iter().flat_map(codes_in).collect(),
        Value::Object(fields) => fields.values().flat_map(codes_in).collect(),
        _ => Vec::new(),
    }
}

#[derive(serde::Deserialize)]
struct Dictionary {
    language: String,
    texts: BTreeMap<String, String>,
}

#[derive(serde::Deserialize)]
struct Setting {
    language: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_become_texts_and_a_missing_code_is_an_error() {
        let texts = Texts::for_language(BASE_LANGUAGE).unwrap();
        let mut contract = serde_json::json!({ "label": "@ingest.import", "symbol": ">", "list": ["@common.cancel"] });
        texts.resolve(&mut contract).unwrap();
        assert_eq!(contract["label"], "Import");
        assert_eq!(contract["symbol"], ">", "a symbol is not a word");
        assert_eq!(contract["list"][0], "Cancel");
        assert!(texts.resolve(&mut serde_json::json!("@no.such.code")).is_err());
        assert!(Texts::for_language("xx").is_err(), "no made-up language");
    }

    #[test]
    fn every_code_of_the_layout_contracts_is_in_every_dictionary() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/ui");
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|ext| ext == "json") {
                let contract: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
                for (language, _) in DICTIONARIES {
                    let texts = Texts::for_language(language).unwrap();
                    for code in codes_in(&contract) {
                        assert!(texts.get(&code).is_ok(), "{}: '{language}' lacks '{code}'", path.display());
                    }
                }
            }
        }
    }

    #[test]
    fn every_code_the_source_uses_is_in_every_dictionary() {
        let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut files = Vec::new();
        let mut dirs = vec![crates];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() && !path.ends_with("target") {
                    dirs.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    files.push(path);
                }
            }
        }
        for file in files {
            let source = std::fs::read_to_string(&file).unwrap();
            for marker in ["qnc_ui_text::text(\"", "qnc_ui_text::fill(\""] {
                for part in source.split(marker).skip(1) {
                    let code = part.split('"').next().unwrap();
                    for (language, _) in DICTIONARIES {
                        assert!(Texts::for_language(language).unwrap().get(code).is_ok(), "{}: '{language}' lacks '{code}'", file.display());
                    }
                }
            }
        }
    }

    #[test]
    fn every_dictionary_has_the_codes_of_the_base_one() {
        let base = Texts::for_language(BASE_LANGUAGE).unwrap();
        for (language, _) in DICTIONARIES {
            let other = Texts::for_language(language).unwrap();
            for code in base.codes() {
                assert!(other.get(code).is_ok(), "'{language}' lacks '{code}'");
            }
        }
    }
}
