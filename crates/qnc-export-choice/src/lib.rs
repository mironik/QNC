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

/// The picture and sound of the story itself: what Original keeps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StoryFormat {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub audio_channels: u16,
    pub audio_sample_rate: u32,
}

/// The delivery values a picked line renders in, as the project's `export` keeps them:
/// `original` is XDCAM HD422 50 Mbit (MPEG-2 4:2:2) in MXF at the story's own picture and
/// sound (user 2026-10-24: the originals' XAVC 4:2:2 10-bit has no encoder here, MXF is what
/// TV houses take); `project` the project's export as saved; a preset id that preset's values
/// over the project's export. XML and Express have none.
pub fn delivery_values(id: &str, export: &Value, story: StoryFormat) -> Result<Value, String> {
    let mut values = export.clone();
    let object = values.as_object_mut().ok_or("The project has no export settings.")?;
    match id {
        "xml" | "express" => return Err(format!("'{id}' renders no delivery")),
        PROJECT_SETTINGS => {}
        "original" => {
            let rate = if story.fps.fract().abs() < 1e-6 { format!("{}", story.fps as u32) } else { format!("{:.2}", story.fps) };
            for (key, value) in [
                ("format", Value::from(format!("{}p{rate} 4:2:2", story.height))),
                ("video_codec", Value::from("mpeg2_422_50mbit")),
                ("container", Value::from("mxf")),
                ("width", Value::from(story.width)),
                ("height", Value::from(story.height)),
                ("fps", Value::from(story.fps)),
                ("field_order", Value::from("progressive")),
                ("audio_channels", Value::from(story.audio_channels)),
                ("audio_sample_rate", Value::from(story.audio_sample_rate)),
            ] {
                object.insert(key.into(), value);
            }
        }
        preset => {
            let settings = serde_json::json!({ "export": export });
            let found = qnc_export_preset::presets(&settings).into_iter().find(|candidate| candidate.id == preset);
            let preset_values = found.ok_or_else(|| format!("unknown export setting '{preset}'"))?.values;
            for (key, value) in preset_values.as_object().into_iter().flatten() {
                object.insert(key.clone(), value.clone());
            }
        }
    }
    Ok(values)
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
    fn original_is_xdcam_hd422_at_the_storys_own_format_and_a_preset_lays_its_values_over_the_project() {
        let export = serde_json::json!({ "preset": "h264_1080p50", "video_codec": "h264", "width": 1280, "fps": 25, "audio_channels": 2, "audio_sample_rate": 48000, "directory": "export" });
        let story = StoryFormat { width: 1920, height: 1080, fps: 50.0, audio_channels: 4, audio_sample_rate: 48000 };
        let original = delivery_values("original", &export, story).unwrap();
        assert_eq!(original["video_codec"], "mpeg2_422_50mbit");
        assert_eq!((original["width"].as_u64(), original["fps"].as_f64(), original["audio_channels"].as_u64()), (Some(1920), Some(50.0), Some(4)));
        assert_eq!((original["field_order"].as_str(), original["directory"].as_str()), (Some("progressive"), Some("export")));
        assert_eq!(delivery_values(PROJECT_SETTINGS, &export, story).unwrap(), export);
        let preset = delivery_values("h264_1080p50", &serde_json::json!({ "video_codec": "mpeg2_422_50mbit" }), story).unwrap();
        assert_eq!(preset["video_codec"], "h264");
        assert!(delivery_values("express", &export, story).is_err());
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
