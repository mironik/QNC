use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, RichText, Sense, Vec2};
use qnc_keyboard_shortcut::ShortcutEvent;
use serde_json::Value;

use crate::{
    layout_contract::{AppContracts, ProjectListMetrics, SettingsPanelMetrics},
    location_browser::{self, LocationBrowserAction, LocationBrowserInput, LocationSourceKind},
    project_advanced,
    project_component::{
        DirectoryBrowserEntry, DirectoryBrowserListing, ProjectComponent, ProjectsState,
        TemplatesState,
    },
    theme::{self, Theme},
    widgets,
};
use qnc_project_store::{ProjectRow, ProjectTemplateRow};

pub struct ProjectApp {
    contracts: AppContracts,
    component: ProjectComponent,
    project_name: String,
    projects_root: String,
    export_dir: String,
    projects_root_dirty: bool,
    export_dir_dirty: bool,
    projects_root_browser_open: bool,
    projects_root_browser: LocationBrowserState,
    export_dir_browser_open: bool,
    export_dir_browser: LocationBrowserState,
    picker_open: bool,
    template_create_open: bool,
    advanced_open: bool,
    selected_project: Option<usize>,
    delete_candidate: Option<ProjectRow>,
    delete_template_candidate: Option<ProjectTemplateRow>,
    projects: Vec<ProjectRow>,
    selected_template_id: String,
    templates: Vec<ProjectTemplateRow>,
    draft_settings: Value,
    template_draft_name: String,
    template_draft_description: String,
    export_preset_draft_name: String,
    status: String,
}

#[derive(Debug, Clone)]
enum ProjectAction {
    OpenSelectedProject,
    OpenProjectRow { index: usize },
    CreateProject,
    RequestDeleteProject { index: usize },
    CancelDeleteProject,
    ConfirmDeleteProject,
    SelectTemplate { template_id: String },
    BeginTemplateCreate,
    CancelTemplateCreate,
    SaveUserTemplate,
    RequestDeleteTemplate { template_id: String },
    CancelDeleteTemplate,
    ConfirmDeleteTemplate,
    PickProjectsRoot,
    SelectProjectsRootKind { kind: LocationSourceKind },
    OpenProjectsRootPath { path: String },
    ConfirmProjectsRootBrowser,
    CancelProjectsRootBrowser,
    PickExportDir,
    SelectExportDirKind { kind: LocationSourceKind },
    OpenExportDirPath { path: String },
    ConfirmExportDirBrowser,
    CancelExportDirBrowser,
}

impl ProjectAction {
    fn action_id(&self) -> &'static str {
        match self {
            Self::OpenSelectedProject => "project_open_selected",
            Self::OpenProjectRow { .. } => "project_open_row",
            Self::CreateProject => "project_create",
            Self::RequestDeleteProject { .. } => "project_delete_request",
            Self::CancelDeleteProject => "project_delete_cancel",
            Self::ConfirmDeleteProject => "project_delete_confirm",
            Self::SelectTemplate { .. } => "project_template_select",
            Self::BeginTemplateCreate => "project_template_create_begin",
            Self::CancelTemplateCreate => "project_template_create_cancel",
            Self::SaveUserTemplate => "project_template_save",
            Self::RequestDeleteTemplate { .. } => "project_template_delete_request",
            Self::CancelDeleteTemplate => "project_template_delete_cancel",
            Self::ConfirmDeleteTemplate => "project_template_delete_confirm",
            Self::PickProjectsRoot => "project_pick_projects_root",
            Self::SelectProjectsRootKind { .. } => "project_projects_root_browser_select_source",
            Self::OpenProjectsRootPath { .. } => "project_projects_root_browser_open_path",
            Self::ConfirmProjectsRootBrowser => "project_projects_root_browser_confirm",
            Self::CancelProjectsRootBrowser => "project_projects_root_browser_cancel",
            Self::PickExportDir => "project_pick_export_dir",
            Self::SelectExportDirKind { .. } => "project_export_dir_browser_select_source",
            Self::OpenExportDirPath { .. } => "project_export_dir_browser_open_path",
            Self::ConfirmExportDirBrowser => "project_export_dir_browser_confirm",
            Self::CancelExportDirBrowser => "project_export_dir_browser_cancel",
        }
    }
}

#[derive(Debug, Clone)]
struct LocationBrowserState {
    kind: LocationSourceKind,
    roots: bool,
    path: String,
    parent: Option<String>,
    entries: Vec<DirectoryBrowserEntry>,
    error: Option<String>,
    busy: bool,
}

impl Default for LocationBrowserState {
    fn default() -> Self {
        Self {
            kind: LocationSourceKind::Local,
            roots: true,
            path: String::new(),
            parent: None,
            entries: Vec::new(),
            error: None,
            busy: false,
        }
    }
}

impl ProjectApp {
    pub(crate) fn new(contracts: AppContracts, component: ProjectComponent) -> Self {
        let status = contracts.project.left_project_list.ready_text.clone();
        let projects_root = component.projects_root_display();
        let mut app = Self {
            contracts,
            component,
            project_name: String::new(),
            projects_root,
            export_dir: "exports/projekti".to_string(),
            projects_root_dirty: false,
            export_dir_dirty: false,
            projects_root_browser_open: false,
            projects_root_browser: LocationBrowserState::default(),
            export_dir_browser_open: false,
            export_dir_browser: LocationBrowserState::default(),
            picker_open: false,
            template_create_open: false,
            advanced_open: false,
            selected_project: None,
            delete_candidate: None,
            delete_template_candidate: None,
            projects: Vec::new(),
            selected_template_id: String::new(),
            templates: Vec::new(),
            draft_settings: Value::Object(Default::default()),
            template_draft_name: String::new(),
            template_draft_description: String::new(),
            export_preset_draft_name: String::new(),
            status,
        };
        app.refresh_projects();
        app.refresh_templates();
        app
    }
}

impl eframe::App for ProjectApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let t = Theme::from_contract(&self.contracts.shell.colors);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(t.bg))
            .show(ctx, |ui| {
                self.show_desktop(ctx, ui);
            });
    }
}

impl ProjectApp {
    pub fn show_desktop(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        self.dispatch_keyboard_shortcuts(ctx);
        self.project_board(ui);
        self.delete_project_overlay(ctx);
        self.delete_template_overlay(ctx);
    }

    fn dispatch_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        if self.delete_candidate.is_some()
            || self.delete_template_candidate.is_some()
            || self.picker_open
            || self.projects_root_browser_open
            || self.export_dir_browser_open
            || self.template_create_open
        {
            return;
        }

        for event in Self::shortcut_events(ctx) {
            let actions = self
                .contracts
                .shortcuts
                .action_ids_for_event("project", &event);
            if actions.contains(&"project_open_selected") {
                self.dispatch_project_action(ProjectAction::OpenSelectedProject);
            }
        }
    }

    fn dispatch_project_action(&mut self, action: ProjectAction) {
        let action_id = action.action_id();
        if !self.contracts.shortcuts.actions.contains_key(action_id) {
            self.status = format!("Nepoznata Project akcija: {action_id}");
            return;
        }

        match action {
            ProjectAction::OpenSelectedProject => {
                if let Some(index) = self.selected_project {
                    self.perform_open_project(index);
                }
            }
            ProjectAction::OpenProjectRow { index } => {
                self.perform_open_project(index);
            }
            ProjectAction::CreateProject => {
                self.perform_create_project();
            }
            ProjectAction::RequestDeleteProject { index } => {
                self.request_delete_project(index);
            }
            ProjectAction::CancelDeleteProject => {
                self.delete_candidate = None;
                self.status = "Brisanje otkazano.".to_string();
            }
            ProjectAction::ConfirmDeleteProject => {
                if let Some(project) = self.delete_candidate.clone() {
                    self.perform_confirm_delete_project(&project);
                }
            }
            ProjectAction::SelectTemplate { template_id } => {
                self.perform_select_template(&template_id);
            }
            ProjectAction::BeginTemplateCreate => {
                let selected_name = self.selected_template_name();
                self.template_create_open = true;
                if self.template_draft_name.trim().is_empty() && selected_name != "—" {
                    self.template_draft_name = format!("{selected_name} custom");
                }
            }
            ProjectAction::CancelTemplateCreate => {
                self.template_create_open = false;
                self.template_draft_name.clear();
                self.template_draft_description.clear();
                self.export_preset_draft_name.clear();
            }
            ProjectAction::SaveUserTemplate => {
                self.perform_save_custom_template();
            }
            ProjectAction::RequestDeleteTemplate { template_id } => {
                self.request_delete_template(&template_id);
            }
            ProjectAction::CancelDeleteTemplate => {
                self.delete_template_candidate = None;
                self.status = "Brisanje templatea otkazano.".to_string();
            }
            ProjectAction::ConfirmDeleteTemplate => {
                if let Some(template) = self.delete_template_candidate.clone() {
                    self.perform_confirm_delete_template(&template);
                }
            }
            ProjectAction::PickProjectsRoot => {
                self.toggle_projects_root_browser();
            }
            ProjectAction::SelectProjectsRootKind { kind } => {
                self.select_projects_root_kind(kind);
            }
            ProjectAction::OpenProjectsRootPath { path } => {
                self.open_projects_root_path(&path);
            }
            ProjectAction::ConfirmProjectsRootBrowser => {
                self.confirm_projects_root_browser();
            }
            ProjectAction::CancelProjectsRootBrowser => {
                self.projects_root_browser_open = false;
            }
            ProjectAction::PickExportDir => {
                self.toggle_export_dir_browser();
            }
            ProjectAction::SelectExportDirKind { kind } => {
                self.select_export_dir_kind(kind);
            }
            ProjectAction::OpenExportDirPath { path } => {
                self.open_export_dir_path(&path);
            }
            ProjectAction::ConfirmExportDirBrowser => {
                self.confirm_export_dir_browser();
            }
            ProjectAction::CancelExportDirBrowser => {
                self.export_dir_browser_open = false;
            }
        }
    }

    fn project_board(&mut self, ui: &mut egui::Ui) {
        let t = Theme::from_contract(&self.contracts.shell.colors);
        let rect = ui.available_rect_before_wrap();
        ui.allocate_exact_size(rect.size(), Sense::hover());
        ui.set_clip_rect(rect);
        ui.painter().rect_filled(rect, 0.0, t.bg);

        let board = &self.contracts.project.board;
        let mut left_w = (rect.width() * board.left_ratio).max(board.left_min_width);
        let divider_w = board.divider_width;
        if left_w + divider_w + board.right_min_width > rect.width() {
            left_w = (rect.width() - divider_w - board.right_min_width).max(180.0);
        }
        let right_w = (rect.width() - left_w - divider_w).max(0.0);

        let left_rect = egui::Rect::from_min_size(rect.min, Vec2::new(left_w, rect.height()));
        let divider_rect = egui::Rect::from_min_size(
            egui::pos2(left_rect.right(), rect.top()),
            Vec2::new(divider_w, rect.height()),
        );
        let right_rect =
            egui::Rect::from_min_max(egui::pos2(divider_rect.right(), rect.top()), rect.max);
        ui.painter().rect_filled(divider_rect, 0.0, t.border);

        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(left_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.set_clip_rect(left_rect);
                self.project_list(ui, left_w, rect.height());
            },
        );
        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(right_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.set_clip_rect(right_rect);
                self.settings_panel(ui, right_w, rect.height());
            },
        );
    }

    fn shortcut_events(ctx: &egui::Context) -> Vec<ShortcutEvent> {
        let text_input_reserved = ctx.memory(|memory| memory.focused().is_some());
        ctx.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Key {
                        key,
                        physical_key,
                        pressed,
                        repeat,
                        modifiers,
                        ..
                    } if *pressed && !*repeat => Some(ShortcutEvent {
                        code: physical_key.as_ref().and_then(Self::catalog_key_code),
                        key: Self::catalog_key_name(key),
                        shift: modifiers.shift,
                        ctrl: modifiers.ctrl || modifiers.command,
                        alt: modifiers.alt,
                        text_input_reserved,
                    }),
                    _ => None,
                })
                .collect()
        })
    }

    fn catalog_key_name(key: &egui::Key) -> Option<String> {
        use egui::Key;

        let name = match key {
            Key::ArrowDown => "ArrowDown",
            Key::ArrowLeft => "ArrowLeft",
            Key::ArrowRight => "ArrowRight",
            Key::ArrowUp => "ArrowUp",
            Key::Escape => "Escape",
            Key::Tab => "Tab",
            Key::Backspace => "Backspace",
            Key::Enter => "Enter",
            Key::Space => " ",
            Key::Delete => "Delete",
            Key::Home => "Home",
            Key::Comma => ",",
            Key::Slash => "/",
            Key::OpenBracket => "[",
            Key::CloseBracket => "]",
            Key::Period => ".",
            Key::Quote => "'",
            Key::A => "A",
            Key::B => "B",
            Key::C => "C",
            Key::D => "D",
            Key::E => "E",
            Key::F => "F",
            Key::G => "G",
            Key::H => "H",
            Key::I => "I",
            Key::J => "J",
            Key::K => "K",
            Key::L => "L",
            Key::M => "M",
            Key::N => "N",
            Key::O => "O",
            Key::P => "P",
            Key::Q => "Q",
            Key::R => "R",
            Key::S => "S",
            Key::T => "T",
            Key::U => "U",
            Key::V => "V",
            Key::W => "W",
            Key::X => "X",
            Key::Y => "Y",
            Key::Z => "Z",
            Key::F1 => "F1",
            _ => return None,
        };
        Some(name.to_string())
    }

    fn catalog_key_code(key: &egui::Key) -> Option<String> {
        use egui::Key;

        let code = match key {
            Key::ArrowDown => "ArrowDown",
            Key::ArrowLeft => "ArrowLeft",
            Key::ArrowRight => "ArrowRight",
            Key::ArrowUp => "ArrowUp",
            Key::Escape => "Escape",
            Key::Tab => "Tab",
            Key::Backspace => "Backspace",
            Key::Enter => "Enter",
            Key::Space => "Space",
            Key::Delete => "Delete",
            Key::Home => "Home",
            Key::Comma => "Comma",
            Key::Slash => "Slash",
            Key::OpenBracket => "BracketLeft",
            Key::CloseBracket => "BracketRight",
            Key::Period => "Period",
            Key::Quote => "Quote",
            Key::A => "KeyA",
            Key::B => "KeyB",
            Key::C => "KeyC",
            Key::D => "KeyD",
            Key::E => "KeyE",
            Key::F => "KeyF",
            Key::G => "KeyG",
            Key::H => "KeyH",
            Key::I => "KeyI",
            Key::J => "KeyJ",
            Key::K => "KeyK",
            Key::L => "KeyL",
            Key::M => "KeyM",
            Key::N => "KeyN",
            Key::O => "KeyO",
            Key::P => "KeyP",
            Key::Q => "KeyQ",
            Key::R => "KeyR",
            Key::S => "KeyS",
            Key::T => "KeyT",
            Key::U => "KeyU",
            Key::V => "KeyV",
            Key::W => "KeyW",
            Key::X => "KeyX",
            Key::Y => "KeyY",
            Key::Z => "KeyZ",
            Key::F1 => "F1",
            _ => return None,
        };
        Some(code.to_string())
    }

    fn project_list(&mut self, ui: &mut egui::Ui, width: f32, height: f32) {
        let t = Theme::from_contract(&self.contracts.shell.colors);
        let metrics = self.contracts.project.left_project_list.clone();
        let (panel_rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
        ui.painter().rect_filled(panel_rect, 0.0, t.bg);
        let content = egui::Rect::from_min_max(
            egui::pos2(
                panel_rect.left() + metrics.panel_pad,
                panel_rect.top() + metrics.panel_pad,
            ),
            egui::pos2(
                panel_rect.right() - metrics.panel_pad,
                panel_rect.bottom() - metrics.panel_pad,
            ),
        );
        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(content)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.set_clip_rect(content);
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                self.project_list_contents(ui, content, &metrics);
            },
        );
    }

    fn project_list_contents(
        &mut self,
        ui: &mut egui::Ui,
        content: egui::Rect,
        metrics: &ProjectListMetrics,
    ) {
        let t = Theme::from_contract(&self.contracts.shell.colors);
        let content_w = content.width().max(40.0);
        let (title_rect, _) = ui.allocate_exact_size(
            Vec2::new(content_w, metrics.title_row_height),
            Sense::hover(),
        );
        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(title_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.label(
                    RichText::new(&metrics.title)
                        .size(self.contracts.shell.theme_metrics.font_ui)
                        .strong()
                        .color(t.text),
                );
            },
        );
        ui.painter().hline(
            title_rect.x_range(),
            title_rect.bottom() - 0.5,
            egui::Stroke::new(1.0, t.border),
        );
        ui.add_space(metrics.below_title_pad);

        let foot_h = self.contracts.shell.theme_metrics.chrome_row_height;
        let table_top = ui.cursor().top();
        let table_bottom = (content.bottom() - foot_h).max(table_top + 40.0);
        let table_rect = egui::Rect::from_min_max(
            egui::pos2(content.left(), table_top),
            egui::pos2(content.right(), table_bottom),
        );
        let foot_rect = egui::Rect::from_min_size(
            egui::pos2(content.left(), table_bottom),
            Vec2::new(content_w, foot_h),
        );

        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(table_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.set_clip_rect(table_rect);
                ui.set_max_width(table_rect.width().max(40.0));
                egui::ScrollArea::vertical()
                    .id_salt("project_list")
                    .max_height(table_rect.height())
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.set_max_width(table_rect.width().max(40.0));
                        if self.projects.is_empty() {
                            ui.label(
                                RichText::new(&metrics.empty_text)
                                    .size(self.contracts.shell.theme_metrics.font_ui)
                                    .color(t.muted),
                            );
                            return;
                        }
                        for index in 0..self.projects.len() {
                            let project = self.projects[index].clone();
                            self.project_row(ui, index, &project, metrics);
                            if index + 1 < self.projects.len() {
                                ui.add_space(metrics.row_gap);
                            }
                        }
                    });
            },
        );

        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(foot_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.set_clip_rect(foot_rect);
                ui.label(
                    RichText::new(&self.status)
                        .size(self.contracts.shell.theme_metrics.font_ui)
                        .color(t.muted),
                );
            },
        );
    }

    fn project_row(
        &mut self,
        ui: &mut egui::Ui,
        index: usize,
        project: &ProjectRow,
        metrics: &ProjectListMetrics,
    ) {
        let t = Theme::from_contract(&self.contracts.shell.colors);
        let table_w = ui.available_width().max(40.0);
        let (row_rect, _) =
            ui.allocate_exact_size(Vec2::new(table_w, metrics.row_height), Sense::hover());
        let delete_hit_w = metrics.delete_column_width.max(48.0);
        let delete_rect = egui::Rect::from_min_size(
            egui::pos2(row_rect.right() - delete_hit_w, row_rect.top()),
            Vec2::new(delete_hit_w, metrics.row_height),
        );
        let select_rect = egui::Rect::from_min_max(
            row_rect.min,
            egui::pos2(delete_rect.left() - metrics.column_gap, row_rect.bottom()),
        );
        let response = ui.interact(
            select_rect,
            ui.id().with(("project_list_row_select", index)),
            Sense::click(),
        );
        let selected = self.selected_project == Some(index);
        if selected || response.hovered() {
            let fill = if selected {
                t.surface
            } else {
                t.surface.linear_multiply(0.55)
            };
            ui.painter().rect_filled(row_rect, 0.0, fill);
        }
        if response.clicked() {
            self.dispatch_project_action(ProjectAction::OpenProjectRow { index });
        }

        let label_rect = egui::Rect::from_min_max(
            egui::pos2(row_rect.left() + 20.0, row_rect.top()),
            select_rect.right_bottom(),
        );
        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(label_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                let mut label = RichText::new(&project.name)
                    .size(self.contracts.shell.theme_metrics.font_ui)
                    .color(t.text);
                if project.active {
                    label = label.strong();
                }
                ui.add(egui::Label::new(label).truncate().selectable(false));
            },
        );

        let delete_response = ui
            .interact(
                delete_rect,
                ui.id().with(("project_list_row_delete", index)),
                Sense::click(),
            )
            .on_hover_text("Ukloni projekt");
        let delete_color = if delete_response.hovered() {
            t.text
        } else {
            t.muted
        };
        ui.painter().text(
            delete_rect.center(),
            egui::Align2::CENTER_CENTER,
            "×",
            egui::FontId::proportional(self.contracts.shell.theme_metrics.font_ui + 5.0),
            delete_color,
        );
        if delete_response.clicked() {
            self.dispatch_project_action(ProjectAction::RequestDeleteProject { index });
        }
    }

    fn delete_project_overlay(&mut self, ctx: &egui::Context) {
        let Some(project) = self.delete_candidate.clone() else {
            return;
        };
        let t = Theme::from_contract(&self.contracts.shell.colors);
        let font_ui = self.contracts.shell.theme_metrics.font_ui;
        let screen_rect = ctx.screen_rect();
        egui::Area::new(egui::Id::new("delete_project_backdrop"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen_rect.min)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(screen_rect.size(), Sense::click());
                ui.painter()
                    .rect_filled(rect, 0.0, Color32::from_black_alpha(130));
            });

        let dialog_size = Vec2::new(418.0, 162.0);
        let dialog_pos = screen_rect.center() - (dialog_size * 0.5);
        egui::Area::new(egui::Id::new("delete_project_confirm"))
            .order(egui::Order::Foreground)
            .fixed_pos(dialog_pos)
            .show(ctx, |ui| {
                let (dialog_rect, _) = ui.allocate_exact_size(dialog_size, Sense::hover());
                ui.painter().rect_filled(dialog_rect, 0.0, t.surface);
                ui.painter().rect_stroke(
                    dialog_rect,
                    0.0,
                    egui::Stroke::new(1.0, t.border),
                    egui::StrokeKind::Inside,
                );

                let title_rect = egui::Rect::from_min_size(
                    egui::pos2(dialog_rect.left() + 20.0, dialog_rect.top() + 20.0),
                    Vec2::new(dialog_rect.width() - 40.0, 24.0),
                );
                ui.painter().text(
                    title_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Želite ukloniti projekt?",
                    egui::FontId::proportional(font_ui),
                    t.text,
                );

                let name_rect = egui::Rect::from_min_size(
                    egui::pos2(dialog_rect.left() + 20.0, dialog_rect.top() + 58.0),
                    Vec2::new(dialog_rect.width() - 40.0, 24.0),
                );
                ui.painter().text(
                    name_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    project.name.as_str(),
                    egui::FontId::proportional(font_ui),
                    t.muted,
                );

                let button_h = self.contracts.shell.theme_metrics.chrome_control_height;
                let button_w = 48.0;
                let gap = 8.0;
                let buttons_w = (button_w * 2.0) + gap;
                let buttons_x = dialog_rect.center().x - (buttons_w * 0.5);
                let no_rect = egui::Rect::from_min_size(
                    egui::pos2(buttons_x, dialog_rect.bottom() - 22.0 - button_h),
                    Vec2::new(button_w, button_h),
                );
                let yes_rect = egui::Rect::from_min_size(
                    egui::pos2(no_rect.right() + gap, no_rect.top()),
                    Vec2::new(button_w, button_h),
                );

                let no_clicked = ui
                    .put(
                        no_rect,
                        egui::Button::new(RichText::new("NE").color(t.text).size(font_ui))
                            .fill(Color32::TRANSPARENT)
                            .stroke(egui::Stroke::new(1.0, t.border)),
                    )
                    .clicked();
                let yes_clicked = ui
                    .put(
                        yes_rect,
                        egui::Button::new(
                            RichText::new("DA")
                                .color(Color32::WHITE)
                                .strong()
                                .size(font_ui),
                        )
                        .fill(t.accent),
                    )
                    .clicked();
                if no_clicked {
                    self.dispatch_project_action(ProjectAction::CancelDeleteProject);
                }
                if yes_clicked {
                    self.dispatch_project_action(ProjectAction::ConfirmDeleteProject);
                }
            });
    }

    fn settings_panel(&mut self, ui: &mut egui::Ui, width: f32, height: f32) {
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
                                        "CustomTemplate" if self.template_create_open => {
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
                            RichText::new(if self.picker_open { "▲" } else { "▼" })
                                .size(11.0)
                                .color(t.muted),
                        );
                    });
                });
            });
        if head.response.interact(Sense::click()).clicked() {
            self.picker_open = !self.picker_open;
        }
        if self.picker_open {
            ui.add_space(settings.section_gap);
            if self.templates.is_empty() {
                theme::label(
                    ui,
                    "Nema templatea.",
                    self.contracts.shell.theme_metrics.font_ui,
                    t.muted,
                );
            } else {
                for template in self.templates.clone() {
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
                        self.dispatch_project_action(ProjectAction::RequestDeleteTemplate {
                            template_id: template.template_id,
                        });
                    } else if row.response.interact(Sense::click()).clicked() {
                        self.dispatch_project_action(ProjectAction::SelectTemplate {
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
            !self.project_name.trim().is_empty() && !self.selected_template_id.trim().is_empty();
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
                    egui::TextEdit::singleline(&mut self.project_name)
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
            self.dispatch_project_action(ProjectAction::CreateProject);
        }
    }

    fn ai_settings(&mut self, ui: &mut egui::Ui, content_w: f32, settings: &SettingsPanelMetrics) {
        widgets::section(ui, content_w, "AI", settings, &self.contracts.shell, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            let mut enabled =
                project_advanced::bool_path(&self.draft_settings, "ai.enabled", false);
            if ui
                .checkbox(&mut enabled, "AI analiza kadrova i virtualni kadrovi")
                .changed()
            {
                project_advanced::set_bool_path(&mut self.draft_settings, "ai.enabled", enabled);
                self.status = "Postavke promijenjene.".to_string();
            }
            let mut coverage =
                project_advanced::bool_path(&self.draft_settings, "ai.coverage_suggestions", true);
            if ui.checkbox(&mut coverage, "Coverage suggestions").changed() {
                project_advanced::set_bool_path(
                    &mut self.draft_settings,
                    "ai.coverage_suggestions",
                    coverage,
                );
                self.status = "Postavke promijenjene.".to_string();
            }
            let mut transcription =
                project_advanced::bool_path(&self.draft_settings, "ai.transcription", false);
            if ui
                .checkbox(&mut transcription, "Transkripcija u Media tabu")
                .changed()
            {
                project_advanced::set_bool_path(
                    &mut self.draft_settings,
                    "ai.transcription",
                    transcription,
                );
                self.status = "Postavke promijenjene.".to_string();
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
                    &mut self.projects_root
                } else {
                    &mut self.export_dir
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
                self.projects_root_dirty = true;
                project_advanced::set_string_path(
                    &mut self.draft_settings,
                    "storage.projects_root",
                    self.projects_root.clone(),
                );
            } else {
                self.export_dir_dirty = true;
                project_advanced::set_string_path(
                    &mut self.draft_settings,
                    "export.directory",
                    self.export_dir.clone(),
                );
            }
            self.status = "Postavke promijenjene.".to_string();
        }
        if should_pick {
            let action = if project_root {
                ProjectAction::PickProjectsRoot
            } else {
                ProjectAction::PickExportDir
            };
            self.dispatch_project_action(action);
        }
        if project_root && self.projects_root_browser_open {
            self.location_browser(ui, true);
        }
        if !project_root && self.export_dir_browser_open {
            self.location_browser(ui, false);
        }
    }

    fn location_browser(&mut self, ui: &mut egui::Ui, project_root: bool) {
        let shell = self.contracts.shell.clone();
        let action = {
            let browser = if project_root {
                &self.projects_root_browser
            } else {
                &self.export_dir_browser
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
                    roots: browser.roots,
                    path: &browser.path,
                    parent: browser.parent.as_deref(),
                    entries: &browser.entries,
                    error: browser.error.as_deref(),
                    busy: browser.busy,
                    confirm_label: "U redu",
                    max_tree_height: Some(if project_root { 170.0 } else { 150.0 }),
                    shell: &shell,
                },
            )
        };

        match action {
            LocationBrowserAction::None => {}
            LocationBrowserAction::SelectKind(kind) => {
                let action = if project_root {
                    ProjectAction::SelectProjectsRootKind { kind }
                } else {
                    ProjectAction::SelectExportDirKind { kind }
                };
                self.dispatch_project_action(action);
            }
            LocationBrowserAction::OpenPath(path) => {
                let action = if project_root {
                    ProjectAction::OpenProjectsRootPath { path }
                } else {
                    ProjectAction::OpenExportDirPath { path }
                };
                self.dispatch_project_action(action);
            }
            LocationBrowserAction::Confirm => {
                let action = if project_root {
                    ProjectAction::ConfirmProjectsRootBrowser
                } else {
                    ProjectAction::ConfirmExportDirBrowser
                };
                self.dispatch_project_action(action);
            }
            LocationBrowserAction::Cancel => {
                let action = if project_root {
                    ProjectAction::CancelProjectsRootBrowser
                } else {
                    ProjectAction::CancelExportDirBrowser
                };
                self.dispatch_project_action(action);
            }
        }
    }

    fn template_actions(
        &mut self,
        ui: &mut egui::Ui,
        content_w: f32,
        settings: &SettingsPanelMetrics,
    ) {
        widgets::trailing_actions(ui, content_w, settings, |ui| {
            if self.template_create_open
                && widgets::action_btn(ui, "Odustani", &self.contracts.shell).clicked()
            {
                self.dispatch_project_action(ProjectAction::CancelTemplateCreate);
            }
            if widgets::action_btn(ui, "Novi template", &self.contracts.shell).clicked() {
                self.dispatch_project_action(ProjectAction::BeginTemplateCreate);
            }
        });
    }

    fn advanced(&mut self, ui: &mut egui::Ui, content_w: f32, settings: &SettingsPanelMetrics) {
        if project_advanced::show(
            ui,
            content_w,
            settings,
            &self.contracts.shell,
            &mut self.advanced_open,
            &mut self.draft_settings,
            &mut self.export_preset_draft_name,
        ) {
            self.status = "Postavke promijenjene.".to_string();
        }
    }

    fn custom_template(
        &mut self,
        ui: &mut egui::Ui,
        content_w: f32,
        settings: &SettingsPanelMetrics,
    ) {
        let can_save =
            !self.template_draft_name.trim().is_empty() && !self.selected_template_id.is_empty();
        let mut should_save = false;
        let mut keyboard_changed = None;
        let shell = self.contracts.shell.clone();
        let base_name = self.selected_template_name();
        let mut shortcut_preset = project_advanced::string_path(
            &self.draft_settings,
            "keyboard_shortcuts.active_preset",
            &self.contracts.shortcuts.active_preset,
        );
        let keyboard_options = self.keyboard_preset_options();
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
                        egui::TextEdit::singleline(&mut self.template_draft_name)
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
                egui::TextEdit::multiline(&mut self.template_draft_description)
                    .desired_width((content_w - settings.inner_pad_x * 2.0).max(80.0))
                    .desired_rows(2)
                    .hint_text("Kratki opis"),
            );
            ui.add_space(6.0);
            ui.allocate_ui_with_layout(
                Vec2::new((content_w - settings.inner_pad_x * 2.0).max(160.0), 52.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.label(
                        RichText::new("Tipkovnica")
                            .size(settings.label_font_size)
                            .color(t.muted),
                    );
                    ui.add_space(6.0);
                    let display = keyboard_options
                        .iter()
                        .find(|(id, _)| id == &shortcut_preset)
                        .map(|(_, label)| label.clone())
                        .unwrap_or_else(|| shortcut_preset.clone());
                    let before = shortcut_preset.clone();
                    egui::ComboBox::from_id_salt("pts_kbd_preset")
                        .selected_text(display)
                        .width(ui.available_width().max(160.0))
                        .show_ui(ui, |ui| {
                            for (id, label) in &keyboard_options {
                                ui.selectable_value(&mut shortcut_preset, id.clone(), label);
                            }
                        });
                    if shortcut_preset != before {
                        keyboard_changed = Some(shortcut_preset.clone());
                    }
                },
            );
        });
        if let Some(preset) = keyboard_changed {
            project_advanced::set_string_path(
                &mut self.draft_settings,
                "keyboard_shortcuts.active_preset",
                preset,
            );
            self.status = "Postavke promijenjene.".to_string();
        }
        if should_save {
            self.dispatch_project_action(ProjectAction::SaveUserTemplate);
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

    fn refresh_projects(&mut self) {
        match self.component.load_projects() {
            Ok(projects) => self.apply_projects_state(projects),
            Err(error) => {
                self.status = format!("DB: {error}");
            }
        }
    }

    fn perform_create_project(&mut self) {
        let settings = self.settings_draft_for_save();
        match self.component.create_project(
            &self.project_name,
            &self.selected_template_id,
            &settings,
        ) {
            Ok(created) => {
                self.project_name.clear();
                self.status = format!("Otvoren projekt: {}", created.project.name);
                self.apply_projects_state(created.projects);
            }
            Err(error) => {
                self.status = format!("DB: {error}");
            }
        }
    }

    fn refresh_templates(&mut self) {
        match self.component.load_templates() {
            Ok(templates) => self.apply_templates_state(templates),
            Err(error) => {
                self.status = format!("DB: {error}");
            }
        }
    }

    fn perform_select_template(&mut self, template_id: &str) {
        match self.component.select_template(template_id) {
            Ok(templates) => {
                self.apply_templates_state(templates);
                self.picker_open = false;
                self.status = format!("Template: {}", self.selected_template_name());
            }
            Err(error) => {
                self.status = format!("DB: {error}");
            }
        }
    }

    fn selected_template_name(&self) -> String {
        self.templates
            .iter()
            .find(|template| template.template_id == self.selected_template_id)
            .map(|template| template.name.clone())
            .unwrap_or_else(|| "—".to_string())
    }

    fn perform_save_custom_template(&mut self) {
        let settings = self.settings_draft_for_save();
        match self.component.create_user_template(
            &self.template_draft_name,
            &self.template_draft_description,
            &self.selected_template_id,
            &settings,
        ) {
            Ok(created) => {
                self.selected_template_id = created.template.template_id;
                self.template_create_open = false;
                self.template_draft_name.clear();
                self.template_draft_description.clear();
                self.export_preset_draft_name.clear();
                self.apply_templates_state(created.templates);
                self.status = format!("Template: {}", self.selected_template_name());
            }
            Err(error) => {
                self.status = format!("DB: {error}");
            }
        }
    }

    fn browser_start_path(path: &str) -> String {
        let clean = location_browser::clean_location_path(path);
        if Path::new(&clean).is_absolute() {
            clean
        } else {
            String::new()
        }
    }

    fn toggle_projects_root_browser(&mut self) {
        self.projects_root_browser_open = !self.projects_root_browser_open;
        if self.projects_root_browser_open
            && self.projects_root_browser.kind == LocationSourceKind::Local
        {
            let start = Self::browser_start_path(&self.projects_root);
            self.load_projects_root_browser(&start);
        }
    }

    fn select_projects_root_kind(&mut self, kind: LocationSourceKind) {
        self.projects_root_browser.kind = kind;
        if kind == LocationSourceKind::Local
            && self.projects_root_browser.entries.is_empty()
            && self.projects_root_browser.path.trim().is_empty()
        {
            self.load_projects_root_browser("");
        }
    }

    fn open_projects_root_path(&mut self, path: &str) {
        if self.projects_root_browser.kind == LocationSourceKind::Local {
            self.load_projects_root_browser(path);
        }
    }

    fn confirm_projects_root_browser(&mut self) {
        let path = location_browser::clean_location_path(&self.projects_root_browser.path);
        if path.trim().is_empty() {
            return;
        }
        match self.component.set_projects_root(PathBuf::from(&path)) {
            Ok(projects_root) => {
                self.projects_root = location_browser::clean_location_path(&projects_root);
                self.projects_root_dirty = true;
                self.projects_root_browser_open = false;
                project_advanced::set_string_path(
                    &mut self.draft_settings,
                    "storage.projects_root",
                    self.projects_root.clone(),
                );
                self.status = "Lokacija projekata postavljena.".to_string();
            }
            Err(error) => {
                self.status = format!("DB: {error}");
            }
        }
    }

    fn toggle_export_dir_browser(&mut self) {
        self.export_dir_browser_open = !self.export_dir_browser_open;
        if self.export_dir_browser_open && self.export_dir_browser.kind == LocationSourceKind::Local
        {
            let start = Self::browser_start_path(&self.export_dir);
            self.load_export_dir_browser(&start);
        }
    }

    fn select_export_dir_kind(&mut self, kind: LocationSourceKind) {
        self.export_dir_browser.kind = kind;
        if kind == LocationSourceKind::Local
            && self.export_dir_browser.entries.is_empty()
            && self.export_dir_browser.path.trim().is_empty()
        {
            self.load_export_dir_browser("");
        }
    }

    fn open_export_dir_path(&mut self, path: &str) {
        if self.export_dir_browser.kind == LocationSourceKind::Local {
            self.load_export_dir_browser(path);
        }
    }

    fn confirm_export_dir_browser(&mut self) {
        let path = location_browser::clean_location_path(&self.export_dir_browser.path);
        if path.trim().is_empty() {
            return;
        }
        self.export_dir = path;
        self.export_dir_dirty = true;
        self.export_dir_browser_open = false;
        project_advanced::set_string_path(
            &mut self.draft_settings,
            "export.directory",
            self.export_dir.clone(),
        );
        self.status = "Export direktorij postavljen.".to_string();
    }

    fn load_projects_root_browser(&mut self, path: &str) {
        self.projects_root_browser.busy = true;
        self.projects_root_browser.error = None;
        match self.component.list_directory(path) {
            Ok(listing) => self.apply_projects_root_listing(listing),
            Err(error) => self.set_projects_root_browser_error(error),
        }
    }

    fn load_export_dir_browser(&mut self, path: &str) {
        self.export_dir_browser.busy = true;
        self.export_dir_browser.error = None;
        match self.component.list_directory(path) {
            Ok(listing) => self.apply_export_dir_listing(listing),
            Err(error) => self.set_export_dir_browser_error(error),
        }
    }

    fn apply_projects_root_listing(&mut self, listing: DirectoryBrowserListing) {
        Self::apply_location_listing(&mut self.projects_root_browser, listing);
    }

    fn apply_export_dir_listing(&mut self, listing: DirectoryBrowserListing) {
        Self::apply_location_listing(&mut self.export_dir_browser, listing);
    }

    fn apply_location_listing(
        browser: &mut LocationBrowserState,
        listing: DirectoryBrowserListing,
    ) {
        browser.roots = listing.roots;
        browser.path = location_browser::clean_location_path(&listing.path);
        browser.parent = listing
            .parent
            .map(|parent| location_browser::clean_location_path(&parent));
        browser.entries = listing
            .entries
            .into_iter()
            .map(|entry| DirectoryBrowserEntry {
                name: location_browser::clean_location_path(&entry.name),
                path: location_browser::clean_location_path(&entry.path),
            })
            .collect();
        browser.error = None;
        browser.busy = false;
    }

    fn set_projects_root_browser_error(&mut self, error: String) {
        self.projects_root_browser.error = Some(error);
        self.projects_root_browser.entries.clear();
        self.projects_root_browser.busy = false;
    }

    fn set_export_dir_browser_error(&mut self, error: String) {
        self.export_dir_browser.error = Some(error);
        self.export_dir_browser.entries.clear();
        self.export_dir_browser.busy = false;
    }

    fn perform_open_project(&mut self, index: usize) {
        let Some(project) = self.projects.get(index).cloned() else {
            return;
        };
        let metrics = self.contracts.project.left_project_list.clone();
        match self.component.open_project(&project.project_id) {
            Ok(projects) => {
                self.selected_project = Some(index);
                self.status = metrics.ready_text.clone();
                self.apply_projects_state(projects);
            }
            Err(error) => {
                self.status = format!("DB: {error}");
            }
        }
    }

    fn request_delete_project(&mut self, index: usize) {
        let Some(project) = self.projects.get(index).cloned() else {
            return;
        };
        self.selected_project = Some(index);
        self.delete_candidate = Some(project);
        self.status = "Potvrdi uklanjanje projekta.".to_string();
    }

    fn perform_confirm_delete_project(&mut self, project: &ProjectRow) {
        match self.component.delete_project(&project.project_id) {
            Ok(deleted) => {
                self.delete_candidate = None;
                self.status = format!("Uklonjen projekt: {}", deleted.project.name);
                self.apply_projects_state(deleted.projects);
            }
            Err(error) => {
                self.status = format!("DB: {error}");
            }
        }
    }

    fn delete_template_overlay(&mut self, ctx: &egui::Context) {
        let Some(template) = self.delete_template_candidate.clone() else {
            return;
        };
        let t = Theme::from_contract(&self.contracts.shell.colors);
        let font_ui = self.contracts.shell.theme_metrics.font_ui;
        let screen_rect = ctx.screen_rect();
        egui::Area::new(egui::Id::new("delete_template_backdrop"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen_rect.min)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(screen_rect.size(), Sense::click());
                ui.painter()
                    .rect_filled(rect, 0.0, Color32::from_black_alpha(130));
            });

        let dialog_size = Vec2::new(418.0, 162.0);
        let dialog_pos = screen_rect.center() - (dialog_size * 0.5);
        egui::Area::new(egui::Id::new("delete_template_confirm"))
            .order(egui::Order::Foreground)
            .fixed_pos(dialog_pos)
            .show(ctx, |ui| {
                let (dialog_rect, _) = ui.allocate_exact_size(dialog_size, Sense::hover());
                ui.painter().rect_filled(dialog_rect, 0.0, t.surface);
                ui.painter().rect_stroke(
                    dialog_rect,
                    0.0,
                    egui::Stroke::new(1.0, t.border),
                    egui::StrokeKind::Inside,
                );

                let title_rect = egui::Rect::from_min_size(
                    egui::pos2(dialog_rect.left() + 20.0, dialog_rect.top() + 20.0),
                    Vec2::new(dialog_rect.width() - 40.0, 24.0),
                );
                ui.painter().text(
                    title_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Želite ukloniti template?",
                    egui::FontId::proportional(font_ui),
                    t.text,
                );

                let name_rect = egui::Rect::from_min_size(
                    egui::pos2(dialog_rect.left() + 20.0, dialog_rect.top() + 58.0),
                    Vec2::new(dialog_rect.width() - 40.0, 24.0),
                );
                ui.painter().text(
                    name_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    template.name.as_str(),
                    egui::FontId::proportional(font_ui),
                    t.muted,
                );

                let button_h = self.contracts.shell.theme_metrics.chrome_control_height;
                let button_w = 48.0;
                let gap = 8.0;
                let buttons_w = (button_w * 2.0) + gap;
                let buttons_x = dialog_rect.center().x - (buttons_w * 0.5);
                let no_rect = egui::Rect::from_min_size(
                    egui::pos2(buttons_x, dialog_rect.bottom() - 22.0 - button_h),
                    Vec2::new(button_w, button_h),
                );
                let yes_rect = egui::Rect::from_min_size(
                    egui::pos2(no_rect.right() + gap, no_rect.top()),
                    Vec2::new(button_w, button_h),
                );

                let no_clicked = ui
                    .put(
                        no_rect,
                        egui::Button::new(RichText::new("NE").color(t.text).size(font_ui))
                            .fill(Color32::TRANSPARENT)
                            .stroke(egui::Stroke::new(1.0, t.border)),
                    )
                    .clicked();
                let yes_clicked = ui
                    .put(
                        yes_rect,
                        egui::Button::new(
                            RichText::new("DA")
                                .color(Color32::WHITE)
                                .strong()
                                .size(font_ui),
                        )
                        .fill(t.accent),
                    )
                    .clicked();
                if no_clicked {
                    self.dispatch_project_action(ProjectAction::CancelDeleteTemplate);
                }
                if yes_clicked {
                    self.dispatch_project_action(ProjectAction::ConfirmDeleteTemplate);
                }
            });
    }

    fn request_delete_template(&mut self, template_id: &str) {
        if let Some(template) = self
            .templates
            .iter()
            .find(|template| template.template_id == template_id)
            .cloned()
        {
            self.delete_template_candidate = Some(template);
        }
    }

    fn perform_confirm_delete_template(&mut self, template: &ProjectTemplateRow) {
        match self.component.delete_user_template(&template.template_id) {
            Ok(templates) => {
                self.delete_template_candidate = None;
                self.apply_templates_state(templates);
                self.status = format!("Template obrisan: {}", template.name);
            }
            Err(error) => {
                self.delete_template_candidate = None;
                self.status = format!("DB: {error}");
            }
        }
    }

    fn apply_projects_state(&mut self, state: ProjectsState) {
        self.selected_project = state.selected_project;
        self.projects = state.projects;
    }

    fn apply_templates_state(&mut self, state: TemplatesState) {
        self.selected_template_id = state.selected_template_id;
        self.templates = state.templates;
        self.draft_settings = state.draft_settings;
        self.projects_root_dirty = false;
        self.export_dir_dirty = false;
    }

    fn settings_draft_for_save(&self) -> Value {
        let mut settings = self.draft_settings.clone();
        if self.projects_root_dirty && !self.projects_root.trim().is_empty() {
            project_advanced::set_string_path(
                &mut settings,
                "storage.projects_root",
                self.projects_root.clone(),
            );
        }
        if self.export_dir_dirty && !self.export_dir.trim().is_empty() {
            project_advanced::set_string_path(
                &mut settings,
                "export.directory",
                self.export_dir.clone(),
            );
        }
        settings
    }
}
