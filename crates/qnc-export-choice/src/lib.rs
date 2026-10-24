//! What the Export button offers (user 2026-10-24): Export to XML; right under it Express, the
//! prerendered story as it is (its format named, e.g. HD 1080p50 4:2:0); then the settings,
//! rendered from the camera originals: Original first and stressed, then the project's export
//! setting marked as default and chosen, then every other one (the built-in presets and the
//! project's own). When the project's export values match no preset they are offered as the
//! project's settings, as default. It reads no database (it gets the project's `export` as
//! saved), exports and writes nothing.

use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.export-choice";

/// The id of the project's own export values when they are no preset.
pub const PROJECT_SETTINGS: &str = "project";

/// What a line exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Xml,
    /// The story's prerender put together, in the project's delivery format.
    Express,
    /// Rendered from the originals in the format of the originals.
    Original,
    /// Rendered from the originals in an export setting.
    Setting,
}

/// One line of the choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportOption {
    pub kind: Kind,
    /// `xml`, `express`, `original`, a preset id, or `project`.
    pub id: String,
    pub label: String,
    /// The project's export setting.
    pub default: bool,
}

/// What was picked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picked {
    pub kind: Kind,
    pub id: String,
    pub label: String,
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
        let line = |kind, id: &str, label: String, default| ExportOption { kind, id: id.into(), label, default };
        let mut options = vec![
            line(Kind::Xml, "xml", qnc_ui_text::text("export.to_xml").into(), false),
            line(Kind::Express, "express", qnc_ui_text::fill("export.express", &[("format", &prerender_format(export))]), false),
            line(Kind::Original, "original", qnc_ui_text::text("export.original").into(), false),
        ];
        let mut settings_lines: Vec<ExportOption> = presets
            .into_iter()
            .map(|preset| line(Kind::Setting, &preset.id, preset.name, preset.id == current))
            .collect();
        if !settings_lines.iter().any(|option| option.default) {
            settings_lines.insert(0, line(Kind::Setting, PROJECT_SETTINGS, qnc_ui_text::text("export.project_settings").into(), true));
        }
        // The default right under Original, then the others.
        settings_lines.sort_by_key(|option| !option.default);
        options.extend(settings_lines);
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
        Some(Picked { kind: option.kind, id: option.id.clone(), label: option.label.clone() })
    }
}

/// The format the prerender makes, from the project's delivery settings: e.g. `HD 1080p50
/// 4:2:0` (the H.264 delivery is 4:2:0, XDCAM HD422 4:2:2).
fn prerender_format(export: &Value) -> String {
    let format = export.get("format").and_then(Value::as_str).unwrap_or_default();
    let chroma = match export.get("video_codec").and_then(Value::as_str).unwrap_or_default() {
        "h264" => "4:2:0",
        codec if codec.contains("422") => "4:2:2",
        _ => "",
    };
    format!("{format} {chroma}").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_express_original_then_the_default_chosen_then_the_others() {
        let choice = ExportChoice::from_settings(&serde_json::json!({ "preset": "xdcam_hd422_50i", "format": "HD 1080p50", "video_codec": "h264" }));
        let kinds: Vec<Kind> = choice.options.iter().take(4).map(|option| option.kind).collect();
        assert_eq!(kinds, [Kind::Xml, Kind::Express, Kind::Original, Kind::Setting]);
        assert!(choice.options[1].label.contains("HD 1080p50 4:2:0"), "{}", choice.options[1].label);
        assert_eq!((choice.options[3].id.as_str(), choice.options[3].default), ("xdcam_hd422_50i", true));
        assert_eq!(choice.chosen, 3);
        assert_eq!(choice.options.iter().filter(|option| option.default).count(), 1);
        assert!(choice.options.iter().any(|option| option.id == "h264_1080p50"), "every preset is offered");
    }

    #[test]
    fn values_of_no_preset_are_offered_as_the_project_settings() {
        let mut choice = ExportChoice::from_settings(&serde_json::json!({ "preset": "manual", "format": "HD 1080i50", "video_codec": "mpeg2_422_50mbit" }));
        assert!(choice.options[1].label.contains("4:2:2"));
        assert_eq!((choice.options[3].id.as_str(), choice.options[3].default), (PROJECT_SETTINGS, true));
        for _ in 0..5 {
            choice.step(true);
        }
        assert_eq!(choice.picked().unwrap().kind, Kind::Xml, "it stays in the list");
    }
}
