//! Two-state settings (checkboxes) that a layout contract describes: the setting path,
//! the values for on and off, what an absent setting shows, and the label. A form only
//! places them; this piece reads and writes the settings draft it is given. It knows no
//! application, saves nothing and reads no database.

use serde::Deserialize;
use serde_json::Value;

/// One checkbox of a layout contract.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Switch {
    /// Dotted path in the settings, e.g. `artifacts.filmstrip`.
    pub path: String,
    pub on: Value,
    pub off: Value,
    /// What the checkbox shows while the setting is absent.
    pub default_on: bool,
    pub label: String,
}

impl Switch {
    pub fn is_on(&self, settings: &Value) -> bool {
        qnc_settings_path::switch_on(settings, &self.path, &self.on, self.default_on)
    }

    /// Writes the value of `checked` into the draft.
    pub fn set(&self, settings: &mut Value, checked: bool) {
        let value = if checked { &self.on } else { &self.off };
        qnc_settings_path::set_path(settings, &self.path, value.clone());
    }
}

/// Paints the switches in order; true when one of them changed the draft.
pub fn show(ui: &mut eframe::egui::Ui, settings: &mut Value, switches: &[Switch]) -> bool {
    let mut changed = false;
    for switch in switches {
        let mut checked = switch.is_on(settings);
        if ui.checkbox(&mut checked, switch.label.as_str()).changed() {
            switch.set(settings, checked);
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn filmstrip() -> Switch {
        serde_json::from_value(json!({
            "path": "artifacts.filmstrip", "on": "auto", "off": "off",
            "default_on": true, "label": "Filmstrip"
        }))
        .unwrap()
    }

    #[test]
    fn a_switch_shows_its_default_until_set_and_writes_its_own_values() {
        let switch = filmstrip();
        let mut settings = json!({"ai": {"enabled": false}});
        assert!(switch.is_on(&settings), "absent: the contract default");
        switch.set(&mut settings, false);
        assert_eq!(settings["artifacts"]["filmstrip"], json!("off"));
        assert!(!switch.is_on(&settings));
        switch.set(&mut settings, true);
        assert_eq!(settings["artifacts"]["filmstrip"], json!("auto"));
        assert_eq!(settings["ai"]["enabled"], json!(false), "nothing else changes");
    }
}
