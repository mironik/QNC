use eframe::egui::{self, RichText, Sense};
use qnc_settings_path::{path_string, set_string_path};
use serde_json::Value;

use crate::{
    layout_contract::{AdvancedSection, AdvancedSettings, SettingsPanelMetrics, ShellLayoutContract},
    theme::Theme,
    widgets,
};

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
    /// The keyboard presets of the catalog (id, name) and the one used when the project
    /// chooses none: a choice of the project, not of its template (user 2026-10-09).
    pub keyboard_presets: &'a [(String, String)],
    pub keyboard_default: &'a str,
}

/// The Advanced settings: its sections come from the layout contract (fields drawn by the
/// public piece `qnc-settings-form`); this board only composes the three blocks that are
/// more than fields (keyboard preset, export preset, workflow applications).
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut egui::Ui,
    content_w: f32,
    settings: &SettingsPanelMetrics,
    advanced: &AdvancedSettings,
    shell: &ShellLayoutContract,
    open: &mut bool,
    draft: AdvancedDraft<'_>,
    application_action: &mut Option<ApplicationSelectionAction>,
) -> bool {
    let t = Theme::from_contract(&shell.colors);
    ui.set_max_width(content_w);
    let summary = egui::Frame::NONE
        .fill(t.raised)
        .stroke(egui::Stroke::new(1.0, t.border))
        .inner_margin(egui::Margin::symmetric(8, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&advanced.title).size(13.0).strong().color(t.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(if *open { "▲" } else { "▼" }).size(11.0).color(t.muted));
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
    let style = qnc_settings_form::GridStyle {
        label_color: t.muted,
        label_size: advanced.grid.label_size,
        min_cell_width: advanced.grid.min_cell_width,
        gap: advanced.grid.gap,
        cell_height: advanced.grid.cell_height,
    };
    let mut draft = draft;
    let mut changed = false;
    for section in &advanced.sections {
        widgets::section(ui, content_w, &section.title, settings, shell, |ui| {
            changed |= section_body(ui, section, advanced, &style, settings, shell, &mut draft, application_action);
        });
    }
    changed
}

#[allow(clippy::too_many_arguments)]
fn section_body(
    ui: &mut egui::Ui,
    section: &AdvancedSection,
    advanced: &AdvancedSettings,
    style: &qnc_settings_form::GridStyle,
    settings: &SettingsPanelMetrics,
    shell: &ShellLayoutContract,
    draft: &mut AdvancedDraft<'_>,
    application_action: &mut Option<ApplicationSelectionAction>,
) -> bool {
    let width = ui.available_width();
    let one = qnc_settings_form::cell_width(width, 1, style);
    let mut changed = false;
    match section.block.as_deref() {
        Some("keyboard_preset") => {
            let path = "keyboard_shortcuts.active_preset";
            let current = path_string(draft.draft_settings, path, draft.keyboard_default);
            if let Some(chosen) = qnc_settings_form::choice_cell(
                ui, one, &section.label, "pts_kbd_preset", &current, draft.keyboard_presets, style,
            ) {
                set_string_path(draft.draft_settings, path, chosen);
                changed = true;
            }
        }
        Some("export_preset") => {
            let mut presets: Vec<_> = qnc_export_preset::presets(draft.draft_settings)
                .into_iter()
                .map(|preset| (preset.id, preset.name))
                .collect();
            presets.push((qnc_export_preset::MANUAL_PRESET_ID.to_string(), advanced.texts.manual_preset.clone()));
            let current = path_string(draft.draft_settings, "export.preset", qnc_export_preset::MANUAL_PRESET_ID);
            let selected = qnc_settings_form::choice_cell(ui, one, &section.label, "pts_export_preset", &current, &presets, style)
                .unwrap_or(current);
            changed |= qnc_export_preset::apply_preset(draft.draft_settings, &selected);
        }
        Some("applications") => applications(ui, advanced, shell, draft, application_action),
        _ => {}
    }
    changed |= qnc_settings_form::field_grid(ui, width, draft.draft_settings, &section.fields, &advanced.options, style);
    if let Some(then) = section.then.as_ref().filter(|then| then.when.holds(draft.draft_settings)) {
        ui.add_space(settings.section_gap);
        changed |= qnc_settings_form::field_grid(ui, ui.available_width(), draft.draft_settings, &then.fields, &advanced.options, style);
        if then.save_preset {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(draft.export_preset_draft_name)
                        .desired_width((ui.available_width() - 160.0).max(120.0))
                        .hint_text(&advanced.texts.new_preset_hint),
                );
                let can_save = !draft.export_preset_draft_name.trim().is_empty();
                if widgets::accent_btn(ui, &advanced.texts.save_preset, can_save, shell).clicked()
                    && qnc_export_preset::save_custom_preset(draft.draft_settings, draft.export_preset_draft_name)
                {
                    draft.export_preset_draft_name.clear();
                    changed = true;
                }
            });
        }
    }
    changed
}

fn applications(
    ui: &mut egui::Ui,
    advanced: &AdvancedSettings,
    shell: &ShellLayoutContract,
    draft: &AdvancedDraft<'_>,
    application_action: &mut Option<ApplicationSelectionAction>,
) {
    let t = Theme::from_contract(&shell.colors);
    let applications = draft.applications;
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
                    label: advanced.texts.no_selection.clone(),
                    selected: group.no_selection,
                });
            }
            qnc_ui_kit::OptionColumn {
                id: group.priority_group.clone(),
                label: advanced.texts.group.replace("{group}", &group.priority_group),
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
    if let Some(selected) = qnc_ui_kit::show_option_columns(ui, "project_workflow_groups", &columns, &style) {
        *application_action = Some(ApplicationSelectionAction::Choose {
            priority_group: selected.column_id,
            application_id: selected.option_id,
        });
    }
    if applications.loading {
        ui.label(&advanced.texts.loading_catalog);
    }
    if let Some(error) = &applications.error {
        ui.label(error);
    }
}
