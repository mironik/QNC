use std::collections::HashMap;

use eframe::egui;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutEvent {
    pub code: Option<String>,
    pub key: Option<String>,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub text_input_reserved: bool,
}

impl ShortcutEvent {
    pub fn code(code: impl Into<String>) -> Self {
        Self {
            code: Some(code.into()),
            key: None,
            shift: false,
            ctrl: false,
            alt: false,
            text_input_reserved: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyChord {
    pub code: Option<String>,
    pub key: Option<String>,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl KeyChord {
    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }

        let key = self
            .code
            .as_deref()
            .map(code_to_label)
            .or(self.key.as_deref())
            .unwrap_or("?");
        parts.push(key);
        parts.join("+")
    }

    pub fn matches(&self, event: &ShortcutEvent) -> bool {
        if event.text_input_reserved {
            return false;
        }
        if self.shift != event.shift || self.ctrl != event.ctrl || self.alt != event.alt {
            return false;
        }
        if let (Some(chord_code), Some(event_code)) = (self.code.as_deref(), event.code.as_deref())
        {
            if chord_code.eq_ignore_ascii_case(event_code) {
                return true;
            }
        }
        if let (Some(chord_key), Some(event_key)) = (self.key.as_deref(), event.key.as_deref()) {
            if chord_key.eq_ignore_ascii_case(event_key) {
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutAction {
    pub action_id: String,
    pub label: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShortcutPreset {
    pub preset_id: String,
    pub name: String,
    pub description: String,
    pub scopes: HashMap<String, HashMap<String, Vec<KeyChord>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutCatalog {
    pub version: i64,
    pub active_preset: String,
    pub actions: HashMap<String, ShortcutAction>,
    pub presets: HashMap<String, ShortcutPreset>,
}

impl ShortcutCatalog {
    pub fn from_json_str(contents: &str) -> Result<Self, String> {
        let value = serde_json::from_str::<Value>(contents)
            .map_err(|err| format!("invalid keyboard shortcut JSON: {err}"))?;
        let object = value
            .as_object()
            .ok_or_else(|| "keyboard shortcut catalog must be a JSON object".to_string())?;

        let version = object
            .get("version")
            .and_then(Value::as_i64)
            .ok_or_else(|| "keyboard shortcut catalog requires numeric version".to_string())?;
        let active_preset = required_str(object, "active_preset")?.to_string();

        let mut actions = HashMap::new();
        let action_object = object
            .get("actions")
            .and_then(Value::as_object)
            .ok_or_else(|| "keyboard shortcut catalog requires actions object".to_string())?;
        for (action_id, meta) in action_object {
            let label = meta
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or(action_id)
                .to_string();
            actions.insert(
                action_id.clone(),
                ShortcutAction {
                    action_id: action_id.clone(),
                    label,
                },
            );
        }

        let mut presets = HashMap::new();
        let preset_object = object
            .get("presets")
            .and_then(Value::as_object)
            .ok_or_else(|| "keyboard shortcut catalog requires presets object".to_string())?;
        for (preset_id, preset_value) in preset_object {
            let preset = parse_preset(preset_id, preset_value)?;
            for scope in preset.scopes.values() {
                for action_id in scope.keys() {
                    if !actions.contains_key(action_id) {
                        return Err(format!(
                            "preset '{preset_id}' references unknown action '{action_id}'"
                        ));
                    }
                }
            }
            presets.insert(preset_id.clone(), preset);
        }

        if !presets.contains_key(&active_preset) {
            return Err(format!("active preset '{active_preset}' is not defined"));
        }

        Ok(Self {
            version,
            active_preset,
            actions,
            presets,
        })
    }

    pub fn apply_user_overrides_json(&mut self, contents: &str) -> Result<(), String> {
        let value = serde_json::from_str::<Value>(contents)
            .map_err(|err| format!("invalid keyboard shortcut user override JSON: {err}"))?;
        let root = value.get("user").unwrap_or(&value);

        let preset_id = root
            .get("active_preset")
            .and_then(Value::as_str)
            .unwrap_or(&self.active_preset)
            .to_string();

        if self.presets.contains_key(&preset_id) {
            self.active_preset = preset_id.clone();
        }

        let Some(bindings) = root.get("bindings").and_then(Value::as_object) else {
            return Ok(());
        };
        let Some(preset_overrides) = bindings.get(&preset_id).and_then(Value::as_object) else {
            return Ok(());
        };

        let Some(preset) = self.presets.get_mut(&preset_id) else {
            return Ok(());
        };

        for (scope_id, scope_value) in preset_overrides {
            let Some(action_overrides) = scope_value.as_object() else {
                return Err(format!("override scope '{scope_id}' must be an object"));
            };
            let scope = preset.scopes.entry(scope_id.clone()).or_default();
            for (action_id, bindings_value) in action_overrides {
                if !self.actions.contains_key(action_id) {
                    return Err(format!("override references unknown action '{action_id}'"));
                }
                let Some(bindings) = bindings_value.as_array() else {
                    return Err(format!("override for '{action_id}' must be an array"));
                };
                if bindings.is_empty() {
                    scope.remove(action_id);
                } else {
                    let mut parsed = Vec::new();
                    for binding in bindings {
                        parsed.push(parse_chord(binding)?);
                    }
                    scope.insert(action_id.clone(), parsed);
                }
            }
        }

        Ok(())
    }

    pub fn action_ids_for_event(&self, scope: &str, event: &ShortcutEvent) -> Vec<&str> {
        let Some(scope_bindings) = self.scope_bindings(scope) else {
            return Vec::new();
        };

        let mut matches = scope_bindings
            .iter()
            .filter_map(|(action_id, chords)| {
                chords
                    .iter()
                    .any(|chord| chord.matches(event))
                    .then_some(action_id.as_str())
            })
            .collect::<Vec<_>>();
        matches.sort_unstable();
        matches.dedup();
        matches
    }

    pub fn chord_hint(&self, scope: &str, action_id: &str) -> Option<String> {
        let scope_bindings = self.scope_bindings(scope)?;
        let chords = scope_bindings.get(action_id)?;
        if chords.is_empty() {
            return None;
        }
        Some(
            chords
                .iter()
                .map(KeyChord::display)
                .collect::<Vec<_>>()
                .join(" / "),
        )
    }

    fn scope_bindings(&self, scope: &str) -> Option<&HashMap<String, Vec<KeyChord>>> {
        self.presets
            .get(&self.active_preset)
            .and_then(|preset| preset.scopes.get(scope))
            .or_else(|| {
                self.presets
                    .get("default")
                    .and_then(|preset| preset.scopes.get(scope))
            })
    }
}

fn parse_preset(preset_id: &str, value: &Value) -> Result<ShortcutPreset, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("preset '{preset_id}' must be an object"))?;

    let name = object
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(preset_id)
        .to_string();
    let description = object
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let mut scopes = HashMap::new();
    for (scope_id, scope_value) in object {
        if scope_id == "name" || scope_id == "description" {
            continue;
        }
        let scope_object = scope_value
            .as_object()
            .ok_or_else(|| format!("preset '{preset_id}' scope '{scope_id}' must be an object"))?;
        let mut scope_bindings = HashMap::new();
        for (action_id, bindings_value) in scope_object {
            let bindings = bindings_value.as_array().ok_or_else(|| {
                format!("preset '{preset_id}' action '{action_id}' bindings must be an array")
            })?;
            let mut chords = Vec::new();
            for binding in bindings {
                chords.push(parse_chord(binding)?);
            }
            scope_bindings.insert(action_id.clone(), chords);
        }
        scopes.insert(scope_id.clone(), scope_bindings);
    }

    Ok(ShortcutPreset {
        preset_id: preset_id.to_string(),
        name,
        description,
        scopes,
    })
}

fn parse_chord(value: &Value) -> Result<KeyChord, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "shortcut binding must be an object".to_string())?;
    let code = object
        .get("code")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let key = object
        .get("key")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    if code.is_none() && key.is_none() {
        return Err("shortcut binding requires code or key".to_string());
    }
    Ok(KeyChord {
        code,
        key,
        shift: bool_field(object, "shift")?,
        ctrl: bool_field(object, "ctrl")? || bool_field(object, "ctrlKey")?,
        alt: bool_field(object, "alt")?,
    })
}

fn bool_field(object: &serde_json::Map<String, Value>, field: &str) -> Result<bool, String> {
    match object.get(field) {
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(format!("shortcut modifier '{field}' must be bool")),
        None => Ok(false),
    }
}

fn required_str<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<&'a str, String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("missing string field '{field}'"))
}

fn code_to_label(code: &str) -> &str {
    match code {
        "KeyA" => "A",
        "KeyB" => "B",
        "KeyC" => "C",
        "KeyD" => "D",
        "KeyE" => "E",
        "KeyF" => "F",
        "KeyG" => "G",
        "KeyH" => "H",
        "KeyI" => "I",
        "KeyJ" => "J",
        "KeyK" => "K",
        "KeyL" => "L",
        "KeyM" => "M",
        "KeyN" => "N",
        "KeyO" => "O",
        "KeyP" => "P",
        "KeyQ" => "Q",
        "KeyR" => "R",
        "KeyS" => "S",
        "KeyT" => "T",
        "KeyU" => "U",
        "KeyV" => "V",
        "KeyW" => "W",
        "KeyX" => "X",
        "KeyY" => "Y",
        "KeyZ" => "Z",
        "Space" => "Space",
        "BracketLeft" | "OpenBracket" => "[",
        "BracketRight" | "CloseBracket" => "]",
        "Slash" => "/",
        "Comma" => ",",
        "Period" => ".",
        other => other,
    }
}

pub fn egui_shortcut_events(ctx: &egui::Context) -> Vec<ShortcutEvent> {
    let text_input_reserved = ctx.memory(|memory| memory.focused().is_some());
    ctx.input(|input| {
        input
            .events
            .iter()
            .filter_map(|event| egui_event_to_shortcut(event, text_input_reserved))
            .collect()
    })
}

pub fn consume_egui_action_presses(
    ctx: &egui::Context,
    catalog: &ShortcutCatalog,
    scope: &str,
    action_id: &str,
) -> usize {
    let text_input_reserved = ctx.memory(|memory| memory.focused().is_some());
    ctx.input_mut(|input| {
        let mut consumed = 0usize;
        input.events.retain(|event| {
            let consume = egui_event_to_shortcut(event, text_input_reserved).is_some_and(|event| {
                catalog
                    .action_ids_for_event(scope, &event)
                    .into_iter()
                    .any(|candidate| candidate == action_id)
            });
            if consume {
                consumed += 1;
            }
            !consume
        });
        consumed
    })
}

fn egui_event_to_shortcut(event: &egui::Event, text_input_reserved: bool) -> Option<ShortcutEvent> {
    match event {
        egui::Event::Key {
            key,
            physical_key,
            pressed,
            repeat,
            modifiers,
            ..
        } if *pressed && !*repeat => Some(ShortcutEvent {
            code: physical_key.as_ref().and_then(egui_catalog_key_code),
            key: egui_catalog_key_name(key),
            shift: modifiers.shift,
            ctrl: modifiers.ctrl || modifiers.command,
            alt: modifiers.alt,
            text_input_reserved,
        }),
        _ => None,
    }
}

fn egui_catalog_key_name(key: &egui::Key) -> Option<String> {
    use egui::Key;

    let name = match key {
        Key::ArrowLeft => "ArrowLeft",
        Key::ArrowRight => "ArrowRight",
        Key::Space => " ",
        Key::I => "i",
        Key::O => "o",
        _ => return None,
    };
    Some(name.to_string())
}

fn egui_catalog_key_code(key: &egui::Key) -> Option<String> {
    use egui::Key;

    let code = match key {
        Key::ArrowLeft => "ArrowLeft",
        Key::ArrowRight => "ArrowRight",
        Key::Space => "Space",
        Key::I => "KeyI",
        Key::O => "KeyO",
        _ => return None,
    };
    Some(code.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> ShortcutCatalog {
        ShortcutCatalog::from_json_str(include_str!(
            "../../../contracts/qnc-keyboard-shortcuts.json"
        ))
        .expect("valid QNC keyboard catalog")
    }

    #[test]
    fn loads_existing_qnc_v4_catalog_copy() {
        let catalog = catalog();
        assert_eq!(catalog.version, 1);
        assert_eq!(catalog.active_preset, "default");
        assert!(catalog.actions.len() >= 40);
        assert!(catalog.presets.len() >= 6);
    }

    #[test]
    fn resolves_play_pause_from_catalog() {
        let catalog = catalog();
        let actions = catalog.action_ids_for_event("storyboard", &ShortcutEvent::code("Space"));
        assert!(actions.contains(&"play_pause"));
    }

    #[test]
    fn resolves_mark_in_from_catalog_code() {
        let catalog = catalog();
        let actions = catalog.action_ids_for_event("storyboard", &ShortcutEvent::code("KeyI"));
        assert!(actions.contains(&"mark_in"));
    }

    #[test]
    fn key_i_and_key_o_reach_the_ingest_mark_actions() {
        assert_eq!(egui_catalog_key_code(&egui::Key::I).as_deref(), Some("KeyI"));
        assert_eq!(egui_catalog_key_code(&egui::Key::O).as_deref(), Some("KeyO"));
        let catalog = catalog();
        assert!(catalog
            .action_ids_for_event("ingest", &ShortcutEvent::code("KeyI"))
            .contains(&"mark_in"));
        assert!(catalog
            .action_ids_for_event("ingest", &ShortcutEvent::code("KeyO"))
            .contains(&"mark_out"));
    }

    #[test]
    fn text_input_focus_blocks_shortcuts() {
        let catalog = catalog();
        let mut event = ShortcutEvent::code("Space");
        event.text_input_reserved = true;
        assert!(catalog
            .action_ids_for_event("storyboard", &event)
            .is_empty());
    }

    #[test]
    fn user_override_can_unbind_catalog_action() {
        let mut catalog = catalog();
        catalog
            .apply_user_overrides_json(
                r#"{
                    "user": {
                        "active_preset": "default",
                        "bindings": {
                            "default": {
                                "storyboard": {
                                    "play_pause": []
                                }
                            }
                        }
                    }
                }"#,
            )
            .expect("override");

        assert!(catalog
            .action_ids_for_event("storyboard", &ShortcutEvent::code("Space"))
            .is_empty());
    }

    #[test]
    fn egui_adapter_resolves_and_consumes_catalog_action() {
        let catalog = catalog();
        let ctx = egui::Context::default();
        ctx.input_mut(|input| {
            input.events.push(egui::Event::Key {
                key: egui::Key::Space,
                physical_key: Some(egui::Key::Space),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
        });

        assert_eq!(
            consume_egui_action_presses(&ctx, &catalog, "ingest", "play_pause"),
            1
        );
        assert!(egui_shortcut_events(&ctx).is_empty());
    }
}
