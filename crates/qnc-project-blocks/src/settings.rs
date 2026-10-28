//! Block: the Project settings panel and its sub-blocks (template picker, project name,
//! AI switches, location fields with their browsers, template actions, advanced settings,
//! own template), in the order of the layout contract (`pts_slots`).

use eframe::egui::{self, RichText, Sense, Vec2};
use qnc_project_session::{LocationTarget, ProjectIntent};

use crate::{
    layout_contract::SettingsPanelMetrics,
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
                self.pts_scroll(ui, width, panel.bottom(), settings);
            },
        );
    }

    fn pts_head(&mut self, ui: &mut egui::Ui, width: f32, settings: &SettingsPanelMetrics) {
        let t = Theme::from_contract(&self.contracts.shell.colors);
        widgets::panel_title_row(ui, &settings.title, &self.contracts.theme);
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

    /// The slots shown always, first in the scroll (user 2026-10-27: the template list
    /// scrolls with the panel).
    fn pts_fixed(&mut self, ui: &mut egui::Ui, width: f32, settings: &SettingsPanelMetrics) {
        for slot in self.contracts.project.pts_slots.fixed_order.clone() {
            match slot.as_str() {
                "TemplatePicker" => self.template_picker(ui, width, settings),
                "ProjectCreate" => self.project_create(ui, width, settings),
                _ => {}
            }
        }
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
                                self.pts_fixed(ui, width, settings);
                                // The Advanced bar; the slots of the scroll show only while it is open.
                                project_advanced::header(ui, width, &self.contracts.project.advanced_settings, &self.contracts.theme, &mut self.session.advanced_open);
                                let slots = if self.session.advanced_open { self.contracts.project.pts_slots.scroll_order.clone() } else { Vec::new() };
                                for slot in slots {
                                    match slot.as_str() {
                                        "AiSettings" => self.ai_settings(ui, width, settings),
                                        "Locations" => {
                                            self.path_row(ui, width, settings, LocationTarget::ProjectsRoot);
                                            self.path_row(ui, width, settings, LocationTarget::ExportDir);
                                        }
                                        "TemplateActions" => self.template_actions(ui, width, settings),
                                        "AdvancedSections" => self.advanced(ui, width, settings),
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
            RichText::new(qnc_ui_text::text("project.workflow_title"))
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
                    &self.contracts.project.texts.templates_empty,
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
                                                .on_hover_text(&self.contracts.project.texts.delete_template_hint)
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

    /// After the template: New project opens the story sheet, where its name is typed (user
    /// 2026-10-27); the open project's sheet opens with Story sheet.
    fn project_create(&mut self, ui: &mut egui::Ui, content_w: f32, settings: &SettingsPanelMetrics) {
        let (texts, shell) = (&self.contracts.project.texts, &self.contracts.theme);
        let can_create = !self.session.selected_template_id.trim().is_empty();
        let open = self.session.story_project.is_some();
        let mut intent = None;
        widgets::trailing_actions(ui, content_w, settings, |ui| {
            if open && widgets::action_btn(ui, &texts.story_sheet, shell).clicked() {
                intent = Some(ProjectIntent::OpenStorySheet);
            }
            if widgets::primary_btn(ui, &texts.new_project, can_create, shell).clicked() {
                intent = Some(ProjectIntent::CreateProject);
            }
        });
        self.intents.extend(intent);
    }

    fn ai_settings(&mut self, ui: &mut egui::Ui, content_w: f32, settings: &SettingsPanelMetrics) {
        widgets::section(ui, content_w, "AI", settings, &self.contracts.theme, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            let switches = &self.contracts.project.ai_switches;
            if qnc_settings_switches::show(ui, &mut self.session.draft_settings, switches) {
                self.session.status = self.contracts.project.texts.settings_changed.clone();
            }
        });
    }

    /// A location field (piece `qnc-location-field`): the path, Choose, and while open its
    /// browser with Select/Cancel.
    fn path_row(&mut self, ui: &mut egui::Ui, content_w: f32, settings: &SettingsPanelMetrics, target: LocationTarget) {
        let (shell, texts) = (&self.contracts.theme, &self.contracts.project.texts);
        let projects = target == LocationTarget::ProjectsRoot;
        let label = if projects { &texts.projects_root_label } else { &texts.export_directory_label };
        let field = if projects { &mut self.session.projects_root } else { &mut self.session.export_dir };
        let (mut typed, mut pick) = (false, false);
        widgets::inline_row(ui, content_w, label, settings, shell, |ui, width| {
            typed = ui.add(egui::TextEdit::singleline(&mut field.text).desired_width(width.max(40.0)).hint_text(&texts.path_hint)).changed();
        }, |ui, _| pick = widgets::primary_btn(ui, &texts.choose, true, shell).clicked());
        let labels = qnc_location_field::FieldLabels {
            sources: &texts.sources, up: &texts.up, disks: &texts.disks, internet: &texts.internet,
            empty_folder: &texts.empty_folder, empty_disks: &texts.empty_disks, empty_lan: &texts.empty_lan,
            empty_internet: &texts.empty_internet, confirm: &texts.confirm, cancel: &texts.cancel,
        };
        let (id, height) = if projects { ("project_root", 170.0) } else { ("export_dir", 150.0) };
        let answer = field.open.then(|| qnc_location_field::show_browser(ui, field, id, height, &labels, &self.contracts.theme)).flatten();
        if typed {
            self.session.location_typed(target);
            self.session.status = texts.settings_changed.clone();
        }
        self.intents.extend(pick.then_some(ProjectIntent::PickLocation { target }));
        self.intents.extend(answer.map(|answer| ProjectIntent::AnswerLocation { target, answer }));
    }

    fn template_actions(
        &mut self,
        ui: &mut egui::Ui,
        content_w: f32,
        settings: &SettingsPanelMetrics,
    ) {
        widgets::trailing_actions(ui, content_w, settings, |ui| {
            if self.session.template_create_open
                && widgets::action_btn(ui, &self.contracts.project.texts.cancel, &self.contracts.theme).clicked()
            {
                self.intents.push(ProjectIntent::CancelTemplateCreate);
            }
            if widgets::action_btn(ui, &self.contracts.project.texts.new_template, &self.contracts.theme).clicked() {
                self.intents.push(ProjectIntent::BeginTemplateCreate);
            }
        });
    }

    fn advanced(&mut self, ui: &mut egui::Ui, content_w: f32, settings: &SettingsPanelMetrics) {
        let applications = self.session.component.applications.view();
        let keyboard_presets = self.contracts.shortcuts.preset_choices(&self.contracts.project.advanced_settings.keyboard_preset_order);
        let mut application_action = None;
        if project_advanced::show(
            ui,
            content_w,
            settings,
            &self.contracts.project.advanced_settings,
            &self.contracts.theme,
            project_advanced::AdvancedDraft {
                draft_settings: &mut self.session.draft_settings,
                export_preset_draft_name: &mut self.session.export_preset_draft_name,
                applications: &applications,
                keyboard_presets: &keyboard_presets,
                keyboard_default: &self.contracts.shortcuts.active_preset,
            },
            &mut application_action,
        ) {
            self.session.status = self.contracts.project.texts.settings_changed.clone();
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
        let shell = self.contracts.theme.clone();
        let base_name = self.session.selected_template_name();
        widgets::section(ui, content_w, &self.contracts.project.texts.new_template, settings, &shell, |ui| {
            let base = self.contracts.project.texts.template_base.replace("{base}", &base_name);
            ui.label(RichText::new(base).size(12.0).color(shell.text_muted));
            ui.add_space(8.0);
            widgets::inline_row(
                ui,
                content_w,
                &self.contracts.project.texts.template_name_label,
                settings,
                &shell,
                |ui, width| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.session.template_draft_name)
                            .desired_width(width.max(40.0))
                            .hint_text(&self.contracts.project.texts.template_name_hint),
                    );
                },
                |ui, _| {
                    if widgets::primary_btn(ui, &self.contracts.project.texts.save, can_save, &shell).clicked() {
                        should_save = true;
                    }
                },
            );
            ui.add_space(8.0);
            ui.add(
                egui::TextEdit::multiline(&mut self.session.template_draft_description)
                    .desired_width((content_w - settings.inner_pad_x * 2.0).max(80.0))
                    .desired_rows(2)
                    .hint_text(&self.contracts.project.texts.short_description_hint),
            );
        });
        if should_save {
            self.intents.push(ProjectIntent::SaveUserTemplate);
        }
    }
}
