//! Export profile presets over a JSON settings value. Pure: no I/O, no UI.
//! Built-in presets come from `contracts/export_profiles.json`; custom presets
//! live in the settings value under `export.custom_presets`.

use qnc_settings_path::{path_string, path_value, set_path, set_string_path};
use serde::Deserialize;
use serde_json::{json, Map, Value};

const EXPORT_PROFILE_CATALOG_JSON: &str = include_str!("../../../contracts/export_profiles.json");

/// Id of the pseudo preset that means "values were edited by hand".
pub const MANUAL_PRESET_ID: &str = "manual";

#[derive(Debug, Clone, Deserialize)]
struct ExportProfileCatalog {
    #[serde(default)]
    presets: Vec<ExportPreset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExportPreset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub values: Value,
}

/// Built-in presets followed by the custom presets saved in `settings`.
pub fn presets(settings: &Value) -> Vec<ExportPreset> {
    let mut presets = serde_json::from_str::<ExportProfileCatalog>(EXPORT_PROFILE_CATALOG_JSON)
        .map(|catalog| catalog.presets)
        .unwrap_or_default();
    presets.extend(custom_presets(settings));
    presets
}

/// Selects `id`: records it as `export.preset` and, unless it is the manual
/// pseudo preset, copies the preset values into `export.*`. Returns whether
/// `settings` was changed.
pub fn apply_preset(settings: &mut Value, id: &str) -> bool {
    if path_string(settings, "export.preset", MANUAL_PRESET_ID) == id {
        return false;
    }
    if id == MANUAL_PRESET_ID {
        set_string_path(settings, "export.preset", id.to_string());
        return true;
    }
    if let Some(preset) = presets(settings).into_iter().find(|preset| preset.id == id) {
        set_string_path(settings, "export.preset", preset.id);
        if let Some(values) = preset.values.as_object() {
            for (key, value) in values {
                set_path(settings, &format!("export.{key}"), value.clone());
            }
        }
    }
    true
}

/// Saves the current export values as a custom preset named `name` and selects
/// it. Returns `false` for an empty name.
pub fn save_custom_preset(settings: &mut Value, name: &str) -> bool {
    let name = name.trim().to_string();
    if name.is_empty() {
        return false;
    }

    let id = slug_preset_id(&name);
    let values = current_export_values(settings);
    let export = export_object_mut(settings);
    let custom_presets = export
        .entry("custom_presets".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    if !custom_presets.is_array() {
        *custom_presets = Value::Array(Vec::new());
    }
    let presets = custom_presets.as_array_mut().expect("custom_presets array");
    presets.retain(|preset| preset.get("id").and_then(Value::as_str) != Some(id.as_str()));
    presets.push(json!({
        "id": id.clone(),
        "name": name,
        "values": values
    }));
    export.insert("preset".to_string(), Value::String(id));
    true
}

fn custom_presets(settings: &Value) -> Vec<ExportPreset> {
    settings
        .get("export")
        .and_then(|export| export.get("custom_presets"))
        .and_then(Value::as_array)
        .map(|presets| {
            presets
                .iter()
                .filter_map(|preset| {
                    let id = preset.get("id")?.as_str()?.trim();
                    if id.is_empty() {
                        return None;
                    }
                    let name = preset
                        .get("name")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|name| !name.is_empty())
                        .unwrap_or(id);
                    Some(ExportPreset {
                        id: id.to_string(),
                        name: name.to_string(),
                        values: preset.get("values").cloned().unwrap_or_else(|| json!({})),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn current_export_values(settings: &Value) -> Value {
    let mut values = Map::new();
    for key in [
        "format",
        "fps",
        "width",
        "height",
        "field_order",
        "color_space",
        "container",
        "video_codec",
        "audio_sample_rate",
        "audio_channels",
    ] {
        if let Some(value) = path_value(settings, &format!("export.{key}")) {
            values.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(values)
}

fn export_object_mut(settings: &mut Value) -> &mut Map<String, Value> {
    if !settings.is_object() {
        *settings = Value::Object(Map::new());
    }
    let root = settings.as_object_mut().expect("settings object");
    let export = root
        .entry("export".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !export.is_object() {
        *export = Value::Object(Map::new());
    }
    export.as_object_mut().expect("export object")
}

fn slug_preset_id(name: &str) -> String {
    let slug = name
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let slug = slug.trim_matches('_').chars().take(40).collect::<String>();
    if slug.is_empty() {
        format!("custom_{}", unix_seconds())
    } else {
        format!("custom_{slug}")
    }
}

fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_export_preset_values_to_settings() {
        let mut settings = json!({});
        assert!(apply_preset(&mut settings, "h264_1080p50"));
        assert_eq!(settings["export"]["preset"], "h264_1080p50");
        assert_eq!(settings["export"]["container"], "mp4");
        assert_eq!(settings["export"]["video_codec"], "h264");
    }

    #[test]
    fn saved_custom_preset_is_listed_and_selected() {
        let mut settings = json!({});
        apply_preset(&mut settings, "h264_1080p50");
        assert!(!save_custom_preset(&mut settings, "   "));
        assert!(save_custom_preset(&mut settings, "Moj preset"));
        assert_eq!(settings["export"]["preset"], "custom_moj_preset");
        assert!(presets(&settings)
            .iter()
            .any(|preset| preset.id == "custom_moj_preset" && preset.name == "Moj preset"));
    }
}
