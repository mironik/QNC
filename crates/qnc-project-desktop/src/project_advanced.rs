use eframe::egui::{self, Color32, RichText, Sense, Vec2};
use serde::Deserialize;
use serde_json::{json, Map, Number, Value};

use crate::{
    layout_contract::{SettingsPanelMetrics, ShellLayoutContract},
    theme::Theme,
    widgets,
};

const EXPORT_PROFILE_CATALOG_JSON: &str = include_str!("../../../contracts/export_profiles.json");

const INPUT_FORMATS: &[(&str, &str)] = &[
    ("HD 1080p50", "HD 1080p50 (PAL)"),
    ("HD 1080i50", "HD 1080i50 (PAL)"),
    ("HD 1080p30", "HD 1080p29.97 (NTSC)"),
    ("HD 1080p60", "HD 1080p59.94 (NTSC)"),
    ("HD 1080i60", "HD 1080i59.94 (NTSC)"),
    ("UHD 2160p", "UHD 2160p"),
];
const FPS_OPTIONS: &[&str] = &["25", "50", "29.97", "30", "59.94", "60"];
const FIELD_ORDER: &[(&str, &str)] = &[
    ("progressive", "Progressive"),
    ("upper_first", "Upper first (i)"),
];
const COLOR_SPACE: &[(&str, &str)] = &[("rec709", "rec709"), ("rec2020", "rec2020")];
const CONTAINERS: &[(&str, &str)] = &[("mxf_op1a", "MXF OP1a"), ("mp4", "MP4"), ("mov", "MOV")];
const VIDEO_CODECS: &[(&str, &str)] = &[
    ("mpeg2_422_50mbit", "MPEG-2 422 50 Mbit"),
    ("h264", "H.264"),
    ("prores_422", "ProRes 422"),
    ("dnxhd_hq", "DNxHD HQ"),
];
const INGEST_PROFILES: &[(&str, &str)] = &[("field", "Teren"), ("house", "TV kuca")];
const INGEST_MEDIA_MODES: &[(&str, &str)] = &[
    ("link", "Samo link"),
    ("proxy", "Proxy"),
    ("original", "Original"),
];
const PLAYBACK_INPUTS: &[(&str, &str)] = &[
    ("proxy_if_available", "Proxy ako postoji"),
    ("original", "Original"),
    ("proxy", "Proxy"),
];
const EXPORT_MODES: &[(&str, &str)] = &[
    ("xml_master", "XML master"),
    ("xdcam", "XDCAM"),
    ("original", "Original"),
    ("avid", "Avid"),
];
const ORIGINAL_POLICIES: &[(&str, &str)] = &[
    ("link_when_available", "Link"),
    ("copy_background", "Kopiraj u pozadini"),
    ("ignore_for_fast_news", "Ignoriraj (brze vijesti)"),
];
const AUDIO_RATES: &[&str] = &["48000", "44100"];
const AUDIO_CHANNELS: &[&str] = &["2", "4", "6", "8"];

#[derive(Debug, Clone, Deserialize)]
struct ExportProfileCatalog {
    #[serde(default)]
    presets: Vec<ExportProfilePreset>,
}

#[derive(Debug, Clone, Deserialize)]
struct ExportProfilePreset {
    id: String,
    name: String,
    #[serde(default)]
    values: Value,
}

pub enum ApplicationSelectionAction {
    Choose {
        priority_group: String,
        application_id: Option<String>,
    },
}

pub struct AdvancedDraft<'a> {
    pub draft_settings: &'a mut Value,
    pub export_preset_draft_name: &'a mut String,
    pub applications: &'a crate::application_selection::ApplicationSelectionView,
}

pub fn show(
    ui: &mut egui::Ui,
    content_w: f32,
    settings: &SettingsPanelMetrics,
    shell: &ShellLayoutContract,
    open: &mut bool,
    draft: AdvancedDraft<'_>,
    application_action: &mut Option<ApplicationSelectionAction>,
) -> bool {
    let AdvancedDraft {
        draft_settings,
        export_preset_draft_name,
        applications,
    } = draft;
    let t = Theme::from_contract(&shell.colors);
    ui.set_max_width(content_w);
    let summary = egui::Frame::NONE
        .fill(t.raised)
        .stroke(egui::Stroke::new(1.0, t.border))
        .inner_margin(egui::Margin::symmetric(8, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Advanced postavke")
                        .size(13.0)
                        .strong()
                        .color(t.text),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(if *open { "▲" } else { "▼" })
                            .size(11.0)
                            .color(t.muted),
                    );
                });
            });
        });
    if summary.response.interact(Sense::click()).clicked() {
        *open = !*open;
    }
    if !*open {
        return false;
    }

    ui.spacing_mut().item_spacing.y = settings.section_gap;
    let mut changed = false;

    widgets::section(ui, content_w, "Input source", settings, shell, |ui| {
        let grid_w = ui.available_width();
        changed |= field_grid(
            ui,
            grid_w,
            draft_settings,
            t.muted,
            &[
                Field::combo("Mode", "pts_input_mode", "input.mode", INPUT_MODES, "auto"),
                Field::combo(
                    "Ingest profil",
                    "pts_ingest_profile",
                    "storage.ingest_profile",
                    INGEST_PROFILES,
                    "field",
                ),
                Field::combo(
                    "Ingest media",
                    "pts_ingest_media",
                    "storage.ingest_media",
                    INGEST_MEDIA_MODES,
                    "",
                ),
                Field::combo(
                    "Playback input",
                    "pts_playback_input",
                    "playback.input",
                    PLAYBACK_INPUTS,
                    "",
                ),
            ],
        );

        if path_string(draft_settings, "input.mode", "auto") == "manual" {
            ui.add_space(settings.section_gap);
            changed |= field_grid(
                ui,
                grid_w,
                draft_settings,
                t.muted,
                &[
                    Field::combo(
                        "Format",
                        "pts_input_format",
                        "input.format",
                        INPUT_FORMATS,
                        "",
                    ),
                    Field::number(
                        "Frame rate",
                        "pts_input_fps",
                        "input.fps",
                        FPS_OPTIONS,
                        false,
                    ),
                    Field::number("Width", "pts_input_width", "input.width", &[], true),
                    Field::number("Height", "pts_input_height", "input.height", &[], true),
                    Field::combo(
                        "Field order",
                        "pts_input_field",
                        "input.field_order",
                        FIELD_ORDER,
                        "",
                    ),
                    Field::combo(
                        "Color space",
                        "pts_input_cs",
                        "input.color_space",
                        COLOR_SPACE,
                        "",
                    ),
                ],
            );
        }
    });

    widgets::section(ui, content_w, "Export", settings, shell, |ui| {
        changed |= field_grid(
            ui,
            ui.available_width(),
            draft_settings,
            t.muted,
            &[
                Field::combo(
                    "Mode",
                    "pts_export_mode",
                    "export.default_mode",
                    EXPORT_MODES,
                    "xml_master",
                ),
                Field::combo(
                    "Original policy",
                    "pts_orig_policy",
                    "storage.original_policy",
                    ORIGINAL_POLICIES,
                    "link_when_available",
                ),
            ],
        );
    });

    widgets::section(ui, content_w, "Export format", settings, shell, |ui| {
        changed |= export_preset_cell(ui, ui.available_width(), draft_settings, t.muted);
        if path_string(draft_settings, "export.preset", "manual") == "manual" {
            ui.add_space(settings.section_gap);
            changed |= field_grid(
                ui,
                ui.available_width(),
                draft_settings,
                t.muted,
                &[
                    Field::combo(
                        "Format",
                        "pts_export_format",
                        "export.format",
                        INPUT_FORMATS,
                        "",
                    ),
                    Field::number(
                        "Frame rate",
                        "pts_export_fps",
                        "export.fps",
                        FPS_OPTIONS,
                        false,
                    ),
                    Field::number("Width", "pts_export_width", "export.width", &[], true),
                    Field::number("Height", "pts_export_height", "export.height", &[], true),
                    Field::combo(
                        "Field order",
                        "pts_export_field",
                        "export.field_order",
                        FIELD_ORDER,
                        "",
                    ),
                    Field::combo(
                        "Color space",
                        "pts_export_cs",
                        "export.color_space",
                        COLOR_SPACE,
                        "",
                    ),
                    Field::combo(
                        "Container",
                        "pts_export_container",
                        "export.container",
                        CONTAINERS,
                        "",
                    ),
                    Field::combo(
                        "Video codec",
                        "pts_export_codec",
                        "export.video_codec",
                        VIDEO_CODECS,
                        "",
                    ),
                    Field::number(
                        "Audio",
                        "pts_export_arate",
                        "export.audio_sample_rate",
                        AUDIO_RATES,
                        true,
                    ),
                    Field::number(
                        "Channels",
                        "pts_export_ach",
                        "export.audio_channels",
                        AUDIO_CHANNELS,
                        true,
                    ),
                ],
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(export_preset_draft_name)
                        .desired_width((ui.available_width() - 160.0).max(120.0))
                        .hint_text("Naziv novog preseta"),
                );
                let can_save = !export_preset_draft_name.trim().is_empty();
                if ui
                    .add_enabled(
                        can_save,
                        egui::Button::new(
                            RichText::new("Spremi u template")
                                .size(shell.theme_metrics.font_ui)
                                .color(Color32::WHITE),
                        )
                        .fill(t.accent),
                    )
                    .clicked()
                {
                    changed |= save_export_preset(draft_settings, export_preset_draft_name);
                }
            });
        }
    });

    widgets::section(
        ui,
        content_w,
        "Plugin tabovi u workflowu",
        settings,
        shell,
        |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            let columns: Vec<_> = applications
                .groups
                .iter()
                .map(|group| {
                    let mut options: Vec<_> = group
                        .choices
                        .iter()
                        .map(|choice| qnc_ui_kit::OptionItem {
                            id: Some(choice.application_id.clone()),
                            label: choice.label.clone(),
                            selected: choice.selected,
                        })
                        .collect();
                    if !applications.required_groups.contains(&group.priority_group) {
                        options.push(qnc_ui_kit::OptionItem {
                            id: None,
                            label: "Bez odabira".into(),
                            selected: group.no_selection,
                        });
                    }
                    qnc_ui_kit::OptionColumn {
                        id: group.priority_group.clone(),
                        label: format!("Grupa {}", group.priority_group),
                        options,
                    }
                })
                .collect();
            let style = qnc_ui_kit::OptionColumnsStyle {
                font_size: shell.theme_metrics.font_ui,
                text: t.text,
                min_column_width: 160.0,
                column_gap: 12.0,
                row_gap: 6.0,
            };
            if let Some(selected) =
                qnc_ui_kit::show_option_columns(ui, "project_workflow_groups", &columns, &style)
            {
                *application_action = Some(ApplicationSelectionAction::Choose {
                    priority_group: selected.column_id,
                    application_id: selected.option_id,
                });
            }
            if applications.loading {
                ui.label("Ucitavanje kataloga...");
            }
            if let Some(error) = &applications.error {
                ui.label(error);
            }
        },
    );

    changed
}

pub fn set_string_path(settings: &mut Value, path: &str, value: String) {
    set_path(settings, path, Value::String(value));
}

pub fn set_bool_path(settings: &mut Value, path: &str, value: bool) {
    set_path(settings, path, Value::Bool(value));
}

pub fn bool_path(settings: &Value, path: &str, default: bool) -> bool {
    path_value(settings, path)
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

#[derive(Clone, Copy)]
struct CellLayout {
    width: f32,
    label_color: Color32,
}

fn field_grid(
    ui: &mut egui::Ui,
    grid_w: f32,
    settings: &mut Value,
    label_color: Color32,
    fields: &[Field<'_>],
) -> bool {
    let cols = field_grid_cols(grid_w);
    let layout = CellLayout {
        width: field_cell_width(grid_w, cols),
        label_color,
    };
    let mut changed = false;
    let mut index = 0;
    while index < fields.len() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            for _ in 0..cols {
                if index >= fields.len() {
                    break;
                }
                changed |= render_field(ui, settings, layout, &fields[index]);
                index += 1;
            }
        });
        ui.add_space(8.0);
    }
    changed
}

fn render_field(
    ui: &mut egui::Ui,
    settings: &mut Value,
    layout: CellLayout,
    field: &Field<'_>,
) -> bool {
    match field {
        Field::Combo { .. } => combo_cell(ui, settings, layout, field),
        Field::Number { .. } => number_cell(ui, settings, layout, field),
    }
}

fn combo_cell(
    ui: &mut egui::Ui,
    settings: &mut Value,
    layout: CellLayout,
    field: &Field<'_>,
) -> bool {
    let Field::Combo {
        label,
        id,
        path,
        options,
        default,
    } = field
    else {
        return false;
    };
    let before = path_string(settings, path, default);
    let mut value = before.clone();
    let display = options
        .iter()
        .find(|(option, _)| *option == value.as_str())
        .map(|(_, label)| (*label).to_string())
        .unwrap_or_else(|| value.clone());
    ui.allocate_ui_with_layout(
        Vec2::new(layout.width, 52.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_width(layout.width);
            ui.label(RichText::new(*label).size(12.0).color(layout.label_color));
            ui.add_space(6.0);
            egui::ComboBox::from_id_salt(id)
                .selected_text(display)
                .width(layout.width)
                .show_ui(ui, |ui| {
                    for &(option, label) in (*options).iter() {
                        ui.selectable_value(&mut value, option.to_string(), label);
                    }
                });
        },
    );
    if value != before {
        set_string_path(settings, path, value);
        true
    } else {
        false
    }
}

fn number_cell(
    ui: &mut egui::Ui,
    settings: &mut Value,
    layout: CellLayout,
    field: &Field<'_>,
) -> bool {
    let Field::Number {
        label,
        id,
        path,
        options,
        integer,
    } = field
    else {
        return false;
    };
    let before = number_string(settings, path);
    let mut text = before.clone();
    ui.allocate_ui_with_layout(
        Vec2::new(layout.width, 52.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_width(layout.width);
            ui.label(RichText::new(*label).size(12.0).color(layout.label_color));
            ui.add_space(6.0);
            if options.is_empty() {
                ui.add(
                    egui::TextEdit::singleline(&mut text)
                        .desired_width(layout.width)
                        .hint_text("0"),
                );
            } else {
                egui::ComboBox::from_id_salt(id)
                    .selected_text(if text.is_empty() {
                        "—"
                    } else {
                        text.as_str()
                    })
                    .width(layout.width)
                    .show_ui(ui, |ui| {
                        for &option in (*options).iter() {
                            ui.selectable_value(&mut text, option.to_string(), option);
                        }
                    });
            }
        },
    );

    if text != before {
        if let Some(value) = parse_decimal(&text) {
            if *integer {
                set_path(
                    settings,
                    path,
                    Value::Number(Number::from(value.round() as i64)),
                );
            } else if let Some(number) = Number::from_f64(value) {
                set_path(settings, path, Value::Number(number));
            }
            true
        } else {
            false
        }
    } else {
        false
    }
}

fn export_preset_cell(
    ui: &mut egui::Ui,
    grid_w: f32,
    settings: &mut Value,
    label_color: Color32,
) -> bool {
    let mut presets = export_presets(settings)
        .into_iter()
        .map(|preset| (preset.id, preset.name, preset.values))
        .collect::<Vec<_>>();
    presets.push(("manual".to_string(), "Ručno".to_string(), json!({})));
    let before = path_string(settings, "export.preset", "manual");
    let mut selected = before.clone();
    let display = presets
        .iter()
        .find(|(id, _, _)| id == &selected)
        .map(|(_, name, _)| name.clone())
        .unwrap_or_else(|| selected.clone());

    let cell_w = field_cell_width(grid_w, 1);
    ui.allocate_ui_with_layout(
        Vec2::new(cell_w, 52.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_width(cell_w);
            ui.label(RichText::new("Preset").size(12.0).color(label_color));
            ui.add_space(6.0);
            egui::ComboBox::from_id_salt("pts_export_preset")
                .selected_text(display)
                .width(cell_w)
                .show_ui(ui, |ui| {
                    for (id, name, _) in &presets {
                        ui.selectable_value(&mut selected, id.clone(), name);
                    }
                });
        },
    );

    if selected == before {
        return false;
    }
    if selected == "manual" {
        set_string_path(settings, "export.preset", selected);
        return true;
    }
    if let Some((_, _, values)) = presets.into_iter().find(|(id, _, _)| id == &selected) {
        set_string_path(settings, "export.preset", selected);
        if let Some(values) = values.as_object() {
            for (key, value) in values {
                set_path(settings, &format!("export.{key}"), value.clone());
            }
        }
    }
    true
}

fn export_presets(settings: &Value) -> Vec<ExportProfilePreset> {
    let mut presets = serde_json::from_str::<ExportProfileCatalog>(EXPORT_PROFILE_CATALOG_JSON)
        .map(|catalog| catalog.presets)
        .unwrap_or_default();
    presets.extend(custom_export_presets(settings));
    presets
}

fn custom_export_presets(settings: &Value) -> Vec<ExportProfilePreset> {
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
                    Some(ExportProfilePreset {
                        id: id.to_string(),
                        name: name.to_string(),
                        values: preset.get("values").cloned().unwrap_or_else(|| json!({})),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn save_export_preset(settings: &mut Value, draft_name: &mut String) -> bool {
    let name = draft_name.trim().to_string();
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
    draft_name.clear();
    true
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

pub fn string_path(settings: &Value, path: &str, default: &str) -> String {
    path_string(settings, path, default)
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

fn path_string(settings: &Value, path: &str, default: &str) -> String {
    path_value(settings, path)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn number_string(settings: &Value, path: &str) -> String {
    match path_value(settings, path) {
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::String(value)) => value.clone(),
        _ => String::new(),
    }
}

fn path_value<'a>(settings: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = settings;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

fn set_path(settings: &mut Value, path: &str, value: Value) {
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

fn parse_decimal(raw: &str) -> Option<f64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.replace(',', ".").parse::<f64>().ok()
}

const INPUT_MODES: &[(&str, &str)] = &[("auto", "AUTO"), ("manual", "Ručno")];

enum Field<'a> {
    Combo {
        label: &'a str,
        id: &'a str,
        path: &'a str,
        options: &'a [(&'a str, &'a str)],
        default: &'a str,
    },
    Number {
        label: &'a str,
        id: &'a str,
        path: &'a str,
        options: &'a [&'a str],
        integer: bool,
    },
}

impl<'a> Field<'a> {
    const fn combo(
        label: &'a str,
        id: &'a str,
        path: &'a str,
        options: &'a [(&'a str, &'a str)],
        default: &'a str,
    ) -> Self {
        Self::Combo {
            label,
            id,
            path,
            options,
            default,
        }
    }

    const fn number(
        label: &'a str,
        id: &'a str,
        path: &'a str,
        options: &'a [&'a str],
        integer: bool,
    ) -> Self {
        Self::Number {
            label,
            id,
            path,
            options,
            integer,
        }
    }
}

fn field_cell_width(grid_w: f32, cols: usize) -> f32 {
    let cols = cols.max(1) as f32;
    ((grid_w - 8.0 * (cols - 1.0)) / cols).max(160.0_f32.min(grid_w))
}

fn field_grid_cols(grid_w: f32) -> usize {
    let cols = ((grid_w + 8.0) / (160.0 + 8.0)).floor() as usize;
    cols.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_export_preset_values_to_settings() {
        let mut settings = json!({});
        let preset = export_presets(&settings)
            .into_iter()
            .find(|preset| preset.id == "h264_1080p50")
            .expect("preset");
        set_string_path(&mut settings, "export.preset", preset.id.clone());
        if let Some(values) = preset.values.as_object() {
            for (key, value) in values {
                set_path(&mut settings, &format!("export.{key}"), value.clone());
            }
        }
        assert_eq!(settings["export"]["preset"], "h264_1080p50");
        assert_eq!(settings["export"]["container"], "mp4");
        assert_eq!(settings["export"]["video_codec"], "h264");
    }

    #[test]
    fn nested_paths_create_objects() {
        let mut settings = json!({});
        set_bool_path(&mut settings, "ai.coverage_suggestions", true);
        set_string_path(&mut settings, "storage.ingest_profile", "house".to_string());
        assert!(bool_path(&settings, "ai.coverage_suggestions", false));
        assert_eq!(settings["storage"]["ingest_profile"], "house");
    }
}
