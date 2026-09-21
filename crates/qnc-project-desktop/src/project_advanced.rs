use eframe::egui::{self, Color32, RichText, Sense, Vec2};
use qnc_settings_path::{number_string, path_string, set_path, set_string_path};
use serde_json::{Number, Value};

use crate::{
    layout_contract::{SettingsPanelMetrics, ShellLayoutContract},
    theme::Theme,
    widgets,
};

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

pub enum ApplicationSelectionAction {
    Choose {
        priority_group: String,
        application_id: Option<String>,
    },
}

pub struct AdvancedDraft<'a> {
    pub draft_settings: &'a mut Value,
    pub export_preset_draft_name: &'a mut String,
    pub applications: &'a qnc_application_selection::ApplicationSelectionView,
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
                    if qnc_export_preset::save_custom_preset(
                        draft_settings,
                        export_preset_draft_name,
                    ) {
                        export_preset_draft_name.clear();
                        changed = true;
                    }
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
    let mut presets = qnc_export_preset::presets(settings)
        .into_iter()
        .map(|preset| (preset.id, preset.name))
        .collect::<Vec<_>>();
    presets.push((
        qnc_export_preset::MANUAL_PRESET_ID.to_string(),
        "Ručno".to_string(),
    ));
    let before = path_string(
        settings,
        "export.preset",
        qnc_export_preset::MANUAL_PRESET_ID,
    );
    let mut selected = before.clone();
    let display = presets
        .iter()
        .find(|(id, _)| id == &selected)
        .map(|(_, name)| name.clone())
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
                    for (id, name) in &presets {
                        ui.selectable_value(&mut selected, id.clone(), name);
                    }
                });
        },
    );

    qnc_export_preset::apply_preset(settings, &selected)
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
