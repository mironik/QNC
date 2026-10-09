//! Block: the Project settings panel and its sub-blocks (template picker, project name,
//! AI switches, location fields with their browsers, template actions, advanced settings,
//! own template), in the order of the layout contract (`pts_slots`).

use eframe::egui::{self, RichText, Sense, Vec2};
use qnc_project_session::{LocationSourceKind, ProjectIntent};

use crate::{
    layout_contract::SettingsPanelMetrics,
    location_browser::{self, LocationBrowserAction, LocationBrowserInput},
    project_advanced,
    theme::{self, Theme},
    widgets, Blocks,
};

impl Blocks<'_> {
    pub(crate) fn settings_panel(&mut self, ui: &mut egui::Ui, width: f32, height: f32) {
        let t = Theme::from_contract(&self.contracts.shell.colors);
        let settings = self.contracts.project.right_settings_panel.clone();
        let (panel_rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
        ui.painter().rect_filled(panel_rect, 0.0, t.bg);
        let inner = panel_rect.shrink(settings.panel_pad);
        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(inner)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.set_clip_rect(inner);
                ui.set_min_width(settings.field_min_width);
                self.pts_panel(
                    ui,
                    inner.width().max(40.0),
                    inner.height().max(40.0),
                    &settings,
                );
            },
        );
    }

    fn pts_panel(
        &mut self,
        ui: &mut egui::Ui,
        width: f32,
        height: f32,
        settings: &SettingsPanelMetrics,
    ) {
        let t = Theme::from_contract(&self.contracts.shell.colors);
        let (panel, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
        ui.painter().rect_filled(panel, 0.0, t.bg);
        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(panel)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.set_clip_rect(panel);
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                self.pts_head(ui, width, settings);
                self.pts_fixed(ui, width, settings);
                self.pts_scroll(ui, width, panel.bottom(), settings);
            },
        );
    }

    fn pts_head(&mut self, ui: &mut egui::Ui, width: f32, settings: &SettingsPanelMetrics) {
        let t = Theme::from_contract(&self.contracts.shell.colors);
        widgets::panel_title_row(ui, &settings.title, &self.contracts.shell);
        egui::Frame::NONE
            .inner_margin(egui::Margin {
                left: settings.inner_pad_x as i8,
                right: settings.inner_pad_x as i8,
                top: 0,
                bottom: settings.inner_pad_y as i8,
            })
            .show(ui, |ui| {
                ui.set_max_width(width);
                theme::label(ui, &settings.subtitle, 12.0, t.muted);
            });
    }

    fn pts_fixed(&mut self, ui: &mut egui::Ui, width: f32, settings: &SettingsPanelMetrics) {
        egui::Frame::NONE
            .inner_margin(egui::Margin {
                left: settings.inner_pad_x as i8,
                right: settings.inner_pad_x as i8,
                top: settings.inner_pad_y as i8,
                bottom: settings.inner_pad_y as i8,
            })
            .show(ui, |ui| {
                ui.set_max_width(width);
                ui.spacing_mut().item_spacing.y = settings.section_gap;
                for slot in self.contracts.project.pts_slots.fixed_order.clone() {
                    match slot.as_str() {
                        "TemplatePicker" => self.template_picker(ui, width, settings),
                        "ProjectCreate" => self.project_create(ui, width, settings),
                        "AiSettings" => self.ai_settings(ui, width, settings),
                        "ProjectsRoot" => {
                            self.path_row(ui, width, settings, "Lokacija projekata:", true)
                        }
                        "ExportDirectory" => {
                            self.path_row(ui, width, settings, "Export direktorij:", false)
                        }
                        "TemplateActions" => self.template_actions(ui, width, settings),
                        _ => {}
                    }
                }
            });
    }

    fn pts_scroll(
        &mut self,
        ui: &mut egui::Ui,
        width: f32,
        panel_bottom: f32,
        settings: &SettingsPanelMetrics,
    ) {
        let scroll_top = ui.cursor().top() + 1.0;
        let scroll_h = (panel_bottom - scroll_top).max(48.0);
        let scroll_rect = egui::Rect::from_min_size(
            egui::pos2(ui.min_rect().left(), scroll_top),
            Vec2::new(width, scroll_h),
        );
        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(scroll_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.set_clip_rect(scroll_rect);
                egui::ScrollArea::vertical()
                    .id_salt("pts_scroll")
                    .max_height(scroll_h)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        egui::Frame::NONE
                            .inner_margin(egui::Margin {
                                left: settings.inner_pad_x as i8,
                                right: settings.inner_pad_x as i8,
                                top: settings.inner_pad_y as i8,
                                bottom: settings.inner_pad_y as i8,
                            })
                            .show(ui, |ui| {
                                ui.set_max_width(width);
                                ui.spacing_mut().item_spacing.y = settings.section_gap;
                                for slot in self.contracts.project.pts_slots.scroll_order.clone() {
                                    match slot.as_str() {
                                        "Advanced" => self.advanced(ui, width, settings),
                                        "CustomTemplate" if self.session.template_create_open => {
                                            self.custom_template(ui, width, settings);
                                        }
                                        _ => {}
                                    }
                                }
                            });
                    });
            },
        );
    }

    fn template_picker(
        &mut self,
        ui: &mut egui::Ui,
        content_w: f32,
        settings: &SettingsPanelMetrics,
    ) {
        let t = Theme::from_contract(&self.contracts.shell.colors);
        let selected_name = self
            .session
            .templates
            .iter()
            .find(|template| template.selected)
            .map(|template| template.name.clone())
            .unwrap_or_else(|| "—".to_string());
        ui.set_max_width(content_w);
        ui.label(
            RichText::new("RADNI TOK")
                .size(settings.label_font_size)
                .strong()
                .color(t.muted),
        );
        ui.add_space(6.0);
        let head = egui::Frame::NONE
            .fill(t.raised)
            .stroke(egui::Stroke::new(1.0, t.border))
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.set_max_width(content_w);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(selected_name)
                            .size(self.contracts.shell.theme_metrics.font_ui)
                            .strong()
                            .color(t.text),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(if self.session.picker_open { "▲" } else { "▼" })
                                .size(11.0)
                                .color(t.muted),
                        );
                    });
                });
            });
        if head.response.interact(Sense::click()).clicked() {
            self.session.picker_open = !self.session.picker_open;
        }
        if self.session.picker_open {
            ui.add_space(settings.section_gap);
            if self.session.templates.is_empty() {
                theme::label(
                    ui,
                    "Nema templatea.",
                    self.contracts.shell.theme_metrics.font_ui,
                    t.muted,
                );
            } else {
                for template in self.session.templates.clone() {
                    let mut delete_requested = false;
                    let row = egui::Frame::NONE
                        .fill(if template.selected { t.surface } else { t.bg })
                        .inner_margin(egui::Margin::symmetric(8, 5))
                        .show(ui, |ui| {
                            ui.set_max_width(content_w);
                            ui.horizontal(|ui| {
                                let mut label = RichText::new(&template.name)
                                    .size(self.contracts.shell.theme_metrics.font_ui)
                                    .color(t.text);
                                if template.selected {
                                    label = label.strong();
                                }
                                ui.label(label);
                                if !template.system {
                                    theme::label(ui, "custom", 11.0, t.muted);
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if !template.system
                                            && ui
                                                .add_sized(
                                                    Vec2::new(32.0, 24.0),
                                                    egui::Button::new(
                                                        RichText::new("X")
                                                            .size(
                                                                self.contracts
                                                                    .shell
                                                                    .theme_metrics
                                                                    .font_ui,
                                                            )
                                                            .color(t.muted),
                                                    )
                                                    .frame(false),
                                                )
                                                .on_hover_text("Obriši template")
                                                .clicked()
                                        {
                                            delete_requested = true;
                                        }
                                    },
                                );
                            });
                            if !template.description.trim().is_empty() {
                                theme::label(ui, &template.description, 11.0, t.muted);
                            }
                        });
                    if delete_requested {
                        self.intents.push(ProjectIntent::RequestDeleteTemplate {
                            template_id: template.template_id,
                        });
                    } else if row.response.interact(Sense::click()).clicked() {
                        self.intents.push(ProjectIntent::SelectTemplate {
                            template_id: template.template_id,
                        });
                    }
                }
            }
        }
    }

    fn project_create(
        &mut self,
        ui: &mut egui::Ui,
        content_w: f32,
        settings: &SettingsPanelMetrics,
    ) {
        let can_create =
            !self.session.project_name.trim().is_empty() && !self.session.selected_template_id.trim().is_empty();
        let shell = self.contracts.shell.clone();
        let mut should_create = false;
        widgets::inline_row(
            ui,
            content_w,
            "Naziv projekta:",
            settings,
            &shell,
            |ui, width| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.session.project_name)
                        .desired_width(width.max(40.0))
                        .hint_text("Naziv novog projekta"),
                );
            },
            |ui, _| {
                if widgets::primary_btn(ui, "Novi projekt", can_create, &shell).clicked() {
                    should_create = true;
                }
            },
        );
        if should_create {
            self.intents.push(ProjectIntent::CreateProject);
        }
    }

    fn ai_settings(&mut self, ui: &mut egui::Ui, content_w: f32, settings: &SettingsPanelMetrics) {
        widgets::section(ui, content_w, "AI", settings, &self.contracts.shell, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            let switches = &self.contracts.project.ai_switches;
            if qnc_settings_switches::show(ui, &mut self.session.draft_settings, switches) {
                self.session.status = "Postavke promijenjene.".to_string();
            }
        });
    }

    fn path_row(
        &mut self,
        ui: &mut egui::Ui,
        content_w: f32,
        settings: &SettingsPanelMetrics,
        label: &str,
        project_root: bool,
    ) {
        let shell = self.contracts.shell.clone();
        let mut should_pick = false;
        let mut text_changed = false;
        widgets::inline_row(
            ui,
            content_w,
            label,
            settings,
            &shell,
            |ui, width| {
                let draft = if project_root {
                    &mut self.session.projects_root
                } else {
                    &mut self.session.export_dir
                };
                let response = ui.add(
                    egui::TextEdit::singleline(draft)
                        .desired_width(width.max(40.0))
                        .hint_text("Putanja"),
                );
                if response.changed() {
                    text_changed = true;
                }
            },
            |ui, _| {
                if widgets::primary_btn(ui, "Odaberi…", true, &shell).clicked() {
                    should_pick = true;
                }
            },
        );
        if text_changed {
            if project_root {
                self.session.projects_root_dirty = true;
                qnc_settings_path::set_string_path(
                    &mut self.session.draft_settings,
                    "storage.projects_root",
                    self.session.projects_root.clone(),
                );
            } else {
                self.session.export_dir_dirty = true;
                qnc_settings_path::set_string_path(
                    &mut self.session.draft_settings,
                    "export.directory",
                    self.session.export_dir.clone(),
                );
            }
            self.session.status = "Postavke promijenjene.".to_string();
        }
        if should_pick {
            let action = if project_root {
                ProjectIntent::PickProjectsRoot
            } else {
                ProjectIntent::PickExportDir
            };
            self.intents.push(action);
        }
        if project_root && self.session.projects_root_browser_open {
            self.location_browser(ui, true);
        }
        if !project_root && self.session.export_dir_browser_open {
            self.location_browser(ui, false);
        }
    }

    fn location_browser(&mut self, ui: &mut egui::Ui, project_root: bool) {
        let shell = self.contracts.shell.clone();
        let browser_action = {
            let browser = if project_root {
                &self.session.projects_root_browser
            } else {
                &self.session.export_dir_browser
            };
            ui.add_space(6.0);
            location_browser::show(
                ui,
                LocationBrowserInput {
                    id_salt: if project_root {
                        "project_root"
                    } else {
                        "export_dir"
                    },
                    kind: browser.kind,
                    browser: &browser.browser,
                    error: browser.error.as_deref(),
                    max_tree_height: Some(if project_root { 170.0 } else { 150.0 }),
                    shell: &shell,
                },
            )
        };

        ui.add_space(8.0);
        let can_confirm = {
            let browser = if project_root {
                &self.session.projects_root_browser
            } else {
                &self.session.export_dir_browser
            };
            browser.kind == LocationSourceKind::Local
                && !browser.browser.roots
                && browser.browser.current_uri.is_some()
                && !browser.busy
        };
        let form_actions = widgets::form_action_bar(ui, &shell, "U redu", can_confirm, "Odustani");

        let mut action = match browser_action {
            LocationBrowserAction::None => None,
            LocationBrowserAction::SelectKind(kind) => Some(if project_root {
                ProjectIntent::SelectProjectsRootKind { kind }
            } else {
                ProjectIntent::SelectExportDirKind { kind }
            }),
            LocationBrowserAction::OpenUri(uri) => Some(if project_root {
                ProjectIntent::OpenProjectsRootUri { uri }
            } else {
                ProjectIntent::OpenExportDirUri { uri }
            }),
            LocationBrowserAction::OpenParent => Some(if project_root {
                ProjectIntent::OpenProjectsRootParent
            } else {
                ProjectIntent::OpenExportDirParent
            }),
            LocationBrowserAction::OpenRoots => Some(if project_root {
                ProjectIntent::OpenProjectsRootRoots
            } else {
                ProjectIntent::OpenExportDirRoots
            }),
        };

        if action.is_none() {
            if form_actions.cancel_clicked {
                action = Some(if project_root {
                    ProjectIntent::CancelProjectsRootBrowser
                } else {
                    ProjectIntent::CancelExportDirBrowser
                });
            } else if form_actions.confirm_clicked {
                action = Some(if project_root {
                    ProjectIntent::ConfirmProjectsRootBrowser
                } else {
                    ProjectIntent::ConfirmExportDirBrowser
                });
            }
        }

        if let Some(action) = action {
            self.intents.push(action);
        }
    }

    fn template_actions(
        &mut self,
        ui: &mut egui::Ui,
        content_w: f32,
        settings: &SettingsPanelMetrics,
    ) {
        widgets::trailing_actions(ui, content_w, settings, |ui| {
            if self.session.template_create_open
                && widgets::action_btn(ui, "Odustani", &self.contracts.shell).clicked()
            {
                self.intents.push(ProjectIntent::CancelTemplateCreate);
            }
            if widgets::action_btn(ui, "Novi template", &self.contracts.shell).clicked() {
                self.intents.push(ProjectIntent::BeginTemplateCreate);
            }
        });
    }

    fn advanced(&mut self, ui: &mut egui::Ui, content_w: f32, settings: &SettingsPanelMetrics) {
        let applications = self.session.component.applications.view();
        let keyboard_presets = self.keyboard_preset_options();
        let mut application_action = None;
        if project_advanced::show(
            ui,
            content_w,
            settings,
            &self.contracts.shell,
            &mut self.session.advanced_open,
            project_advanced::AdvancedDraft {
                draft_settings: &mut self.session.draft_settings,
                export_preset_draft_name: &mut self.session.export_preset_draft_name,
                applications: &applications,
                keyboard_presets: &keyboard_presets,
                keyboard_default: &self.contracts.shortcuts.active_preset,
            },
            &mut application_action,
        ) {
            self.session.status = "Postavke promijenjene.".to_string();
        }
        if let Some(action) = application_action {
            use project_advanced::ApplicationSelectionAction;
            self.intents.push(match action {
                ApplicationSelectionAction::Choose {
                    priority_group,
                    application_id,
                } => ProjectIntent::SelectWorkflowGroup {
                    priority_group,
                    application_id,
                },
            });
        }
    }

    fn custom_template(
        &mut self,
        ui: &mut egui::Ui,
        content_w: f32,
        settings: &SettingsPanelMetrics,
    ) {
        let can_save =
            !self.session.template_draft_name.trim().is_empty() && !self.session.selected_template_id.is_empty();
        let mut should_save = false;
        let shell = self.contracts.shell.clone();
        let base_name = self.session.selected_template_name();
        widgets::section(ui, content_w, "Novi template", settings, &shell, |ui| {
            let t = Theme::from_contract(&shell.colors);
            ui.label(
                RichText::new(format!("Baza: {base_name}"))
                    .size(12.0)
                    .color(t.muted),
            );
            ui.add_space(8.0);
            widgets::inline_row(
                ui,
                content_w,
                "Naziv templatea:",
                settings,
                &shell,
                |ui, width| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.session.template_draft_name)
                            .desired_width(width.max(40.0))
                            .hint_text("Naziv novog templatea"),
                    );
                },
                |ui, _| {
                    if widgets::primary_btn(ui, "Spremi", can_save, &shell).clicked() {
                        should_save = true;
                    }
                },
            );
            ui.add_space(8.0);
            ui.add(
                egui::TextEdit::multiline(&mut self.session.template_draft_description)
                    .desired_width((content_w - settings.inner_pad_x * 2.0).max(80.0))
                    .desired_rows(2)
                    .hint_text("Kratki opis"),
            );
        });
        if should_save {
            self.intents.push(ProjectIntent::SaveUserTemplate);
        }
    }

    fn keyboard_preset_options(&self) -> Vec<(String, String)> {
        let preferred = [
            "default", "resolve", "premiere", "finalcut", "edius", "avid",
        ];
        let mut options = Vec::new();
        for id in preferred {
            if let Some(preset) = self.contracts.shortcuts.presets.get(id) {
                options.push((
                    id.to_string(),
                    if preset.name.trim().is_empty() {
                        id.to_string()
                    } else {
                        preset.name.clone()
                    },
                ));
            }
        }
        let mut extra = self
            .contracts
            .shortcuts
            .presets
            .iter()
            .filter(|(id, _)| !preferred.iter().any(|preferred| preferred == &id.as_str()))
            .map(|(id, preset)| {
                (
                    id.clone(),
                    if preset.name.trim().is_empty() {
                        id.clone()
                    } else {
                        preset.name.clone()
                    },
                )
            })
            .collect::<Vec<_>>();
        extra.sort_by(|a, b| a.0.cmp(&b.0));
        options.extend(extra);
        options
    }
}
