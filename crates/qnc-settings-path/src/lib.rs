//! Dotted-path helpers over a JSON settings value. Pure: no I/O, no UI, no
//! knowledge of which application owns the settings.

use serde_json::{Map, Value};

pub fn path_value<'a>(settings: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = settings;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

pub fn path_string(settings: &Value, path: &str, default: &str) -> String {
    path_value(settings, path)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

pub fn number_string(settings: &Value, path: &str) -> String {
    match path_value(settings, path) {
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::String(value)) => value.clone(),
        _ => String::new(),
    }
}

pub fn bool_path(settings: &Value, path: &str, default: bool) -> bool {
    path_value(settings, path)
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

pub fn set_path(settings: &mut Value, path: &str, value: Value) {
    if !settings.is_object() {
        *settings = Value::Object(Map::new());
    }
    let mut current = settings;
    let parts = path.split('.').collect::<Vec<_>>();
    for part in &parts[..parts.len().saturating_sub(1)] {
        if !current.get(part).is_some_and(Value::is_object) {
            current
                .as_object_mut()
                .expect("settings object")
                .insert((*part).to_string(), Value::Object(Map::new()));
        }
        current = current
            .get_mut(part)
            .expect("path segment inserted as object");
    }
    if let Some(last) = parts.last() {
        current
            .as_object_mut()
            .expect("settings object")
            .insert((*last).to_string(), value);
    }
}

pub fn set_string_path(settings: &mut Value, path: &str, value: String) {
    set_path(settings, path, Value::String(value));
}

pub fn set_bool_path(settings: &mut Value, path: &str, value: bool) {
    set_path(settings, path, Value::Bool(value));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn nested_paths_create_objects() {
        let mut settings = json!({});
        set_bool_path(&mut settings, "ai.coverage_suggestions", true);
        set_string_path(&mut settings, "storage.ingest_profile", "house".to_string());
        assert!(bool_path(&settings, "ai.coverage_suggestions", false));
        assert_eq!(settings["storage"]["ingest_profile"], "house");
    }
}
