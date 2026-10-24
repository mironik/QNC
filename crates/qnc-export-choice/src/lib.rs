//! What the Export button offers (user 2026-10-24): first, apart, Export to XML; then every
//! export setting the project has (the built-in presets and the project's own), the one the
//! project is set to marked as default and chosen; when the project's export values match no
//! preset, they are offered first as the project's settings. The user may pick any. It reads
//! no database (it gets the project's `export` as saved), exports and writes nothing.

use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.export-choice";

/// The id of the project's own export values when they are no preset.
pub const PROJECT_SETTINGS: &str = "project";

/// One line of the choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportOption {
    /// `xml`, a preset id, or `project`.
    pub id: String,
    pub label: String,
    /// The project's export setting.
    pub default: bool,
    /// Export to XML (shown apart, at the top).
    pub xml: bool,
}

/// What was picked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Picked {
    Xml,
    /// An export setting: its id and what it is called.
    Setting { id: String, label: String },
}

/// The open choice.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExportChoice {
    pub options: Vec<ExportOption>,
    pub chosen: usize,
}

impl ExportChoice {
    /// The choice for a project whose export settings are `export` (as saved).
    pub fn from_settings(export: &Value) -> Self {
        let settings = serde_json::json!({ "export": export });
        let current = export.get("preset").and_then(Value::as_str).unwrap_or_default();
        let presets = qnc_export_preset::presets(&settings);
        let mut options = vec![ExportOption {
            id: "xml".into(),
            label: qnc_ui_text::text("export.to_xml").into(),
            default: false,
            xml: true,
        }];
        if !presets.iter().any(|preset| preset.id == current) {
            options.push(ExportOption {
                id: PROJECT_SETTINGS.into(),
                label: qnc_ui_text::text("export.project_settings").into(),
                default: true,
                xml: false,
            });
        }
        options.extend(presets.into_iter().map(|preset| ExportOption {
            default: preset.id == current,
            id: preset.id,
            label: preset.name,
            xml: false,
        }));
        let chosen = options.iter().position(|option| option.default).unwrap_or(0);
        Self { options, chosen }
    }

    /// Up (`up`) or down the list, staying in it.
    pub fn step(&mut self, up: bool) {
        self.chosen = match up {
            true => self.chosen.saturating_sub(1),
            false => (self.chosen + 1).min(self.options.len().saturating_sub(1)),
        };
    }

    pub fn choose(&mut self, index: usize) {
        if index < self.options.len() {
            self.chosen = index;
        }
    }

    pub fn picked(&self) -> Option<Picked> {
        let option = self.options.get(self.chosen)?;
        Some(match option.xml {
            true => Picked::Xml,
            false => Picked::Setting { id: option.id.clone(), label: option.label.clone() },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_comes_first_and_the_project_setting_is_the_chosen_default() {
        let choice = ExportChoice::from_settings(&serde_json::json!({ "preset": "h264_1080p50" }));
        assert!(choice.options[0].xml);
        let default: Vec<_> = choice.options.iter().filter(|option| option.default).collect();
        assert_eq!(default.len(), 1);
        assert_eq!(default[0].id, "h264_1080p50");
        assert_eq!(choice.picked(), Some(Picked::Setting { id: "h264_1080p50".into(), label: default[0].label.clone() }));
        assert!(choice.options.iter().any(|option| option.id == "xdcam_hd422_50i"), "every preset is offered");
    }

    #[test]
    fn values_of_no_preset_are_offered_as_the_project_settings() {
        let mut choice = ExportChoice::from_settings(&serde_json::json!({ "preset": "manual", "format": "HD 1080i50" }));
        assert_eq!((choice.options[1].id.as_str(), choice.options[1].default), (PROJECT_SETTINGS, true));
        assert_eq!(choice.chosen, 1);
        choice.step(true);
        assert_eq!(choice.picked(), Some(Picked::Xml));
        choice.step(true);
        assert_eq!(choice.chosen, 0, "it stays in the list");
    }
}
