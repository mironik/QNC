//! What the Export button offers (user 2026-10-24, 2026-10-25): apart at the top Export to
//! XML and Express, the prerendered story as it is (its format named, e.g. HD 1080p50 4:2:0);
//! under them two tabs, PAL and NTSC, each with the settings rendered from the camera
//! originals in groups (Original, XDCAM HD422, H.264, DNxHD): Original first, the project's
//! export setting marked as default. A setting this story cannot be exported in yet (its
//! codec or its rate) is greyed and cannot be chosen. When the project's export values match
//! no preset they are offered as the project's settings, as default. It reads no database (it
//! gets the project's `export` as saved and the story's rate), exports and writes nothing.

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

/// The tab of a setting, by its rate: 25 and 50 are PAL, the others NTSC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Standard {
    #[default]
    Pal,
    Ntsc,
}

impl Standard {
    pub fn of_rate(fps: f64) -> Self {
        if (fps - 25.0).abs() < 0.01 || (fps - 50.0).abs() < 0.01 {
            Self::Pal
        } else {
            Self::Ntsc
        }
    }

    pub fn label(self) -> &'static str {
        qnc_ui_text::text(match self {
            Self::Pal => "export.tab.pal",
            Self::Ntsc => "export.tab.ntsc",
        })
    }
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
    /// The tab it is in; `None` above the tabs (XML, Express).
    pub standard: Option<Standard>,
    /// The group heading it is under in its tab.
    pub group: String,
    /// Whether this story can be exported in it; greyed when not.
    pub available: bool,
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
    /// The tab shown.
    pub tab: Standard,
}

/// The group heading of a codec.
fn group_of(codec: &str) -> String {
    qnc_ui_text::text(match codec {
        "mpeg2_422_50mbit" => "export.group.xdcam",
        "h264" => "export.group.h264",
        codec if codec.starts_with("dnxhd") => "export.group.dnxhd",
        _ => "export.group.other",
    })
    .into()
}

/// Whether a story at `story_fps` is exported in these values: the codecs the render makes
/// (H.264, XDCAM HD422), progressive at the story's rate, interlaced (XDCAM only) at the
/// story's rate or half of it, with at least the story's sound outputs (the render refuses a
/// delivery with fewer; audit 2026-10-25). An unknown story rate or output count leaves them
/// to the render.
fn renders(values: &Value, story_fps: Option<f64>, outputs: Option<u16>) -> bool {
    let codec = values.get("video_codec").and_then(Value::as_str).unwrap_or_default();
    let fps = values.get("fps").and_then(Value::as_f64).unwrap_or_default();
    let interlaced = values.get("field_order").and_then(Value::as_str).is_some_and(|order| order != "progressive");
    let codec_made = match codec {
        "h264" => !interlaced,
        "mpeg2_422_50mbit" => true,
        _ => false,
    };
    let near = |a: f64, b: f64| (a - b).abs() < 0.01;
    let rate_made = story_fps.is_none_or(|story| near(story, fps) || (interlaced && near(story, 2.0 * fps)));
    let channels = values.get("audio_channels").and_then(Value::as_u64).unwrap_or_default();
    let sound_made = outputs.is_none_or(|outputs| channels >= u64::from(outputs));
    codec_made && rate_made && sound_made
}

/// Whether Express is made: the prerender renders progressive units at the story's own rate
/// in the project's delivery, so it is the same rule for progressive values only.
fn express_made(export: &Value, story_fps: Option<f64>, outputs: Option<u16>) -> bool {
    let progressive = export.get("field_order").and_then(Value::as_str) == Some("progressive");
    progressive && renders(export, story_fps, outputs)
}

impl ExportChoice {
    /// The choice for a project whose export settings are `export` (as saved), for a story
    /// at `story_fps` (unknown while it has no segment) with `outputs` sound outputs (the
    /// project's `audio.channels`).
    pub fn from_settings(export: &Value, story_fps: Option<f64>, outputs: Option<u16>) -> Self {
        let settings = serde_json::json!({ "export": export });
        let current = export.get("preset").and_then(Value::as_str).unwrap_or_default();
        let presets = qnc_export_preset::presets(&settings);
        let line = |kind, id: &str, label: String, default, values: &Value| ExportOption {
            kind,
            id: id.into(),
            label,
            default,
            standard: Some(Standard::of_rate(values.get("fps").and_then(Value::as_f64).unwrap_or_default())),
            group: group_of(values.get("video_codec").and_then(Value::as_str).unwrap_or_default()),
            available: renders(values, story_fps, outputs),
        };
        let top = |kind, id: &str, label: String, available| ExportOption { kind, id: id.into(), label, default: false, standard: None, group: String::new(), available };
        let export_fps = export.get("fps").and_then(Value::as_f64).unwrap_or_default();
        let mut options = vec![
            top(Kind::Xml, "xml", qnc_ui_text::text("export.to_xml").into(), true),
            top(Kind::Express, "express", qnc_ui_text::fill("export.express", &[("format", &prerender_format(export))]), express_made(export, story_fps, outputs)),
            ExportOption {
                kind: Kind::Original,
                id: "original".into(),
                label: qnc_ui_text::text("export.original").into(),
                default: false,
                // The tab of the story's rate; before the story has one, of the project's delivery.
                standard: Some(Standard::of_rate(story_fps.unwrap_or(export_fps))),
                group: qnc_ui_text::text("export.group.original").into(),
                available: true,
            },
        ];
        let mut settings_lines: Vec<ExportOption> = presets
            .into_iter()
            .map(|preset| {
                let mut values = export.clone();
                if let (Some(object), Some(over)) = (values.as_object_mut(), preset.values.as_object()) {
                    object.extend(over.clone());
                }
                line(Kind::Setting, &preset.id, preset.name, preset.id == current, &values)
            })
            .collect();
        if !settings_lines.iter().any(|option| option.default) {
            settings_lines.insert(0, line(Kind::Setting, PROJECT_SETTINGS, qnc_ui_text::text("export.project_settings").into(), true, export));
        }
        // By group in a fixed order, the default first in its group.
        let order = ["export.group.xdcam", "export.group.h264", "export.group.dnxhd", "export.group.other"].map(qnc_ui_text::text);
        settings_lines.sort_by_key(|option| (order.iter().position(|group| *group == option.group), !option.default));
        options.extend(settings_lines);
        let chosen = options.iter().position(|option| option.default && option.available).unwrap_or(0);
        let tab = options[chosen].standard.unwrap_or(options[2].standard.unwrap_or_default());
        Self { options, chosen, tab }
    }

    /// The lines shown, in order: above the tabs, then the tab's.
    pub fn shown(&self) -> Vec<usize> {
        (0..self.options.len()).filter(|&index| self.options[index].standard.is_none_or(|standard| standard == self.tab)).collect()
    }

    /// Up (`up`) or down the lines shown that can be chosen, staying in them.
    pub fn step(&mut self, up: bool) {
        let open: Vec<usize> = self.shown().into_iter().filter(|&index| self.options[index].available).collect();
        let Some(at) = open.iter().position(|&index| index == self.chosen) else {
            self.chosen = open.first().copied().unwrap_or(0);
            return;
        };
        self.chosen = match up {
            true => open[at.saturating_sub(1)],
            false => open[(at + 1).min(open.len() - 1)],
        };
    }

    /// A line shown that can be chosen.
    pub fn choose(&mut self, index: usize) {
        if self.shown().contains(&index) && self.options[index].available {
            self.chosen = index;
        }
    }

    /// Another tab; a line of the tab left goes to the default of the new one, else its
    /// first that can be chosen, else the top.
    pub fn set_tab(&mut self, tab: Standard) {
        self.tab = tab;
        if self.options.get(self.chosen).is_some_and(|option| option.standard.is_none_or(|standard| standard == tab)) {
            return;
        }
        let open = |option: &ExportOption| option.standard == Some(tab) && option.available;
        self.chosen = (self.options.iter().position(|option| open(option) && option.default))
            .or_else(|| self.options.iter().position(open))
            .unwrap_or(0);
    }

    pub fn picked(&self) -> Option<Picked> {
        let option = self.options.get(self.chosen).filter(|option| option.available)?;
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
    let codec = export.get("video_codec").and_then(Value::as_str).unwrap_or_default();
    let chroma = match codec {
        "h264" => "4:2:0",
        codec if codec.contains("422") => "4:2:2",
        _ => "",
    };
    // An H.264 prerender is encoded at the project's constant bitrate (user 2026-10-24).
    let bitrate = match (codec, export.get("video_bitrate_mbps").and_then(Value::as_u64)) {
        ("h264", Some(mbps)) => format!("{mbps} Mbit/s CBR"),
        _ => String::new(),
    };
    format!("{format} {chroma} {bitrate}").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_express_original_then_the_default_chosen_then_the_others() {
        let choice = ExportChoice::from_settings(&serde_json::json!({ "preset": "xdcam_hd422_50i", "format": "HD 1080p50", "video_codec": "h264" }), Some(50.0), None);
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
        let mut choice = ExportChoice::from_settings(&serde_json::json!({ "preset": "manual", "format": "HD 1080i50", "video_codec": "mpeg2_422_50mbit", "fps": 25.0, "field_order": "upper_first" }), Some(50.0), None);
        assert!(choice.options[1].label.contains("4:2:2"));
        assert_eq!((choice.options[3].id.as_str(), choice.options[3].default), (PROJECT_SETTINGS, true));
        for _ in 0..5 {
            choice.step(true);
        }
        assert_eq!(choice.picked().unwrap().kind, Kind::Xml, "it stays in the list");
    }

    #[test]
    fn pal_and_ntsc_tabs_in_groups_and_what_this_story_cannot_be_exported_in_is_greyed() {
        // User 2026-10-25: XML and Express above the tabs, the settings by group, not made greyed.
        let mut choice = ExportChoice::from_settings(&serde_json::json!({ "preset": "h264_1080p50", "video_codec": "h264", "fps": 50.0, "field_order": "progressive" }), Some(50.0), None);
        assert_eq!(choice.tab, Standard::Pal);
        let shown: Vec<&str> = choice.shown().into_iter().map(|index| choice.options[index].id.as_str()).collect();
        assert_eq!(&shown[..3], ["xml", "express", "original"]);
        assert!(!shown.contains(&"h264_1080p30"), "NTSC is in the other tab");
        let option = |choice: &ExportChoice, id: &str| choice.options.iter().find(|option| option.id == id).unwrap().clone();
        assert!(option(&choice, "xdcam_hd422_50i").available, "50p gives 50i");
        assert!(option(&choice, "h264_1080p50").available);
        assert!(!option(&choice, "dnxhd_hq").available, "DNxHD is not made");
        assert!(!option(&choice, "h264_1080p30").available, "no rate conversion");
        let groups: Vec<String> = choice.shown().into_iter().skip(2).map(|index| choice.options[index].group.clone()).collect();
        let mut seen: Vec<String> = Vec::new();
        for group in groups {
            if seen.last() != Some(&group) {
                assert!(!seen.contains(&group), "a group is together: {seen:?} {group}");
                seen.push(group);
            }
        }
        let dnxhd = choice.options.iter().position(|option| option.id == "dnxhd_hq").unwrap();
        let before = choice.chosen;
        choice.choose(dnxhd);
        assert_eq!(choice.chosen, before, "a greyed line is not chosen");
        choice.set_tab(Standard::Ntsc);
        assert!(choice.shown().iter().all(|&index| choice.options[index].standard != Some(Standard::Pal)));
        assert_eq!(choice.chosen, 0, "nothing of NTSC can be made from 50p: the top");
        for _ in 0..10 {
            choice.step(false);
        }
        assert!(choice.options[choice.chosen].available);
    }

    /// Audit 2026-10-25: the box offered what the render then refused.
    #[test]
    fn express_and_settings_with_fewer_outputs_than_the_story_are_greyed() {
        let export = serde_json::json!({ "preset": "h264_1080p50", "video_codec": "h264", "fps": 50.0, "field_order": "progressive", "audio_channels": 2 });
        let option = |choice: &ExportChoice, id: &str| choice.options.iter().find(|option| option.id == id).unwrap().available;
        let stereo = ExportChoice::from_settings(&export, Some(50.0), Some(2));
        assert!(option(&stereo, "express") && option(&stereo, "h264_1080p50"));
        let four = ExportChoice::from_settings(&export, Some(50.0), Some(4));
        assert!(!option(&four, "express"), "the delivery has 2 outputs, the story 4");
        assert!(!option(&four, "h264_1080p50") && option(&four, "original"));
        let interlaced = serde_json::json!({ "preset": "xdcam_hd422_50i", "video_codec": "mpeg2_422_50mbit", "fps": 25.0, "field_order": "upper_first", "audio_channels": 2 });
        let interlaced = ExportChoice::from_settings(&interlaced, Some(50.0), Some(2));
        assert!(!option(&interlaced, "express"), "the prerender makes no interlaced delivery");
        assert!(option(&interlaced, "xdcam_hd422_50i"), "the render from the originals does");
        let dnxhd = ExportChoice::from_settings(&serde_json::json!({ "preset": "dnxhd_hq", "video_codec": "dnxhd_hq", "fps": 25.0, "field_order": "upper_first" }), Some(25.0), None);
        assert!(!option(&dnxhd, "express"));
    }
}
