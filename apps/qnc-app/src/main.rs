use std::{
    collections::{BTreeSet, HashMap},
    env, fs,
    path::{Path, PathBuf},
    process,
};

use eframe::egui::{self, Color32, FontFamily, FontId, RichText, Sense, TextStyle, Vec2};
use qnc_contracts::validate_ui_layout_contract_json;
use qnc_project_close::CloseProjectComponent;
use qnc_shell_desktop_api::{
    next_group_tab, DesktopApplicationRef, DesktopNavigation, EmbeddedAppFactory, ShellDesktopApp,
};
use serde::Deserialize;

const SHELL_LAYOUT_JSON: &str = include_str!("../../../contracts/ui/shell.layout.json");

fn main() -> eframe::Result<()> {
    let layout = ShellLayoutContract::load_embedded().unwrap_or_else(|error| {
        eprintln!("qnc-app shell contract error: {error}");
        process::exit(1);
    });

    let qnc_root = resolve_qnc_root().unwrap_or_else(|error| {
        eprintln!("qnc-app root error: {error}");
        process::exit(1);
    });

    let app_registry = AppRegistry::load(&qnc_root).unwrap_or_else(|error| {
        eprintln!("qnc-app registry error: {error}");
        process::exit(1);
    });

    if env::args().any(|arg| arg == "--check-contracts") {
        println!(
            "qnc-app contracts OK: shell={} layout_tabs={} registered_apps={}",
            layout.layout_id,
            layout.application_tabs.join(","),
            app_registry.summary()
        );
        return Ok(());
    }

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_min_inner_size([1100.0, 700.0])
            .with_maximized(true)
            .with_title("QNC"),
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "QNC",
        options,
        Box::new(move |cc| {
            apply_app_fonts(&cc.egui_ctx, &layout);
            apply_visuals(&cc.egui_ctx, &layout);
            Ok(Box::new(QncShell::new(layout, qnc_root, app_registry)))
        }),
    )
}

#[derive(Debug, Clone, Deserialize)]
struct ShellLayoutContract {
    layout_id: String,
    application_tabs: Vec<String>,
    shell_metrics: ShellMetrics,
    theme_metrics: ThemeMetrics,
    colors: ThemeColors,
}

impl ShellLayoutContract {
    fn load_embedded() -> Result<Self, String> {
        let report =
            validate_ui_layout_contract_json("contracts/ui/shell.layout.json", SHELL_LAYOUT_JSON);
        if !report.is_ok() {
            return Err(report.errors.join("; "));
        }

        let layout = serde_json::from_str::<Self>(SHELL_LAYOUT_JSON)
            .map_err(|error| format!("shell layout parse failed: {error}"))?;
        if layout.layout_id != "qnc.ui.shell" {
            return Err(format!("unexpected shell layout id '{}'", layout.layout_id));
        }
        if layout.application_tabs.is_empty() {
            return Err("shell layout has no application_tabs".to_string());
        }
        if layout.shell_metrics.footer_height <= 0.0
            || layout.shell_metrics.workspace_footer_columns == 0
            || layout.theme_metrics.font_ui <= 0.0
            || layout.theme_metrics.chrome_control_height <= 0.0
        {
            return Err("shell layout metrics must be positive".to_string());
        }
        Ok(layout)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ShellMetrics {
    footer_height: f32,
    workspace_footer_columns: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct ThemeMetrics {
    font_ui: f32,
    chrome_pad_x: i8,
    chrome_control_height: f32,
}

#[derive(Debug, Clone, Deserialize)]
struct ThemeColors {
    bg: [u8; 3],
    surface: [u8; 3],
    raised: [u8; 3],
    border: [u8; 3],
    text: [u8; 3],
    muted: [u8; 3],
    accent: [u8; 3],
    focus: [u8; 3],
}

#[derive(Debug, Clone, Deserialize)]
struct AppManifest {
    application_id: String,
    tab_id: String,
    label: String,
    enabled: bool,
    system: bool,
    removable: bool,
    priority_group: String,
    host_mode: String,
    desktop_entry: String,
    standalone_executable: Option<String>,
}

impl AppManifest {
    fn load(path: &Path) -> Result<Self, String> {
        let contents = fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let manifest = serde_json::from_str::<Self>(&contents)
            .map_err(|error| format!("invalid {}: {error}", path.display()))?;
        manifest.validate(path)?;
        Ok(manifest)
    }

    fn validate(&self, path: &Path) -> Result<(), String> {
        let mut errors = Vec::new();
        for (field, value) in [
            ("application_id", &self.application_id),
            ("tab_id", &self.tab_id),
            ("label", &self.label),
            ("host_mode", &self.host_mode),
            ("desktop_entry", &self.desktop_entry),
        ] {
            if value.trim().is_empty() {
                errors.push(format!("{field} is empty"));
            }
        }
        if !self.application_id.starts_with("qnc.")
            || self.application_id.starts_with("qnc.module.")
        {
            errors.push(format!(
                "application_id '{}' is not a QNC application id",
                self.application_id
            ));
        }
        if !matches!(
            self.host_mode.as_str(),
            "embedded_public_api" | "external_component"
        ) {
            errors.push(format!("unsupported host_mode '{}'", self.host_mode));
        }
        if self.system && self.removable {
            errors.push("system app registry entries cannot be removable".to_string());
        }
        match &self.standalone_executable {
            Some(executable) if !executable.trim().is_empty() => {}
            _ => errors.push(
                "standalone_executable is required because every QNC application must run outside the shell desktop".to_string(),
            ),
        }
        if self.priority_group.len() != 1 || !self.priority_group.as_bytes()[0].is_ascii_lowercase()
        {
            errors.push("priority_group must be a single letter a-z".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(format!("{}: {}", path.display(), errors.join("; ")))
        }
    }
}

#[derive(Debug, Clone)]
struct AppRegistry {
    entries: Vec<AppManifest>,
}

impl AppRegistry {
    fn load(root: &Path) -> Result<Self, String> {
        let apps_dir = root.join("apps");
        let entries = fs::read_dir(&apps_dir)
            .map_err(|error| format!("cannot read {}: {error}", apps_dir.display()))?;

        let mut manifests = Vec::new();
        for entry in entries {
            let entry = entry
                .map_err(|error| format!("cannot read {} entry: {error}", apps_dir.display()))?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("qnc-app.json");
            if !manifest_path.is_file() {
                continue;
            }

            let manifest = AppManifest::load(&manifest_path)?;
            if manifest.enabled {
                manifests.push(manifest);
            }
        }

        let mut seen_tabs = BTreeSet::new();
        let mut seen_apps = BTreeSet::new();
        for manifest in &manifests {
            if !seen_tabs.insert(manifest.tab_id.clone()) {
                return Err(format!("duplicate app tab_id '{}'", manifest.tab_id));
            }
            if !seen_apps.insert(manifest.application_id.clone()) {
                return Err(format!(
                    "duplicate app application_id '{}'",
                    manifest.application_id
                ));
            }
        }

        manifests.sort_by(|a, b| {
            a.priority_group
                .cmp(&b.priority_group)
                .then_with(|| a.label.cmp(&b.label))
                .then_with(|| a.application_id.cmp(&b.application_id))
        });

        if manifests.is_empty() {
            return Err(format!(
                "no enabled QNC app manifests found in {}",
                apps_dir.display()
            ));
        }

        Ok(Self { entries: manifests })
    }

    fn first_tab_id(&self) -> Option<String> {
        self.entries.first().map(|entry| entry.tab_id.clone())
    }

    fn find(&self, tab_id: &str) -> Option<&AppManifest> {
        self.entries.iter().find(|entry| entry.tab_id == tab_id)
    }

    fn entries(&self) -> &[AppManifest] {
        &self.entries
    }

    fn summary(&self) -> String {
        self.entries
            .iter()
            .map(|entry| {
                format!(
                    "{}:{}:{}",
                    entry.application_id, entry.tab_id, entry.desktop_entry
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn embedded_app_factories() -> HashMap<String, EmbeddedAppFactory> {
    let project = qnc_project_desktop_adapter::factory();
    let ingest = qnc_ingest_desktop_adapter::factory();
    let media_assist_audio_ai = qnc_media_assist_audio_ai_desktop_adapter::factory();
    let media_assist_audio = qnc_media_assist_audio_desktop_adapter::factory();
    let media_assist_video = qnc_media_assist_video_desktop_adapter::factory();
    let story = qnc_story_desktop_adapter::factory();
    HashMap::from([
        (project.desktop_entry.to_string(), project),
        (ingest.desktop_entry.to_string(), ingest),
        (
            media_assist_audio_ai.desktop_entry.to_string(),
            media_assist_audio_ai,
        ),
        (
            media_assist_audio.desktop_entry.to_string(),
            media_assist_audio,
        ),
        (
            media_assist_video.desktop_entry.to_string(),
            media_assist_video,
        ),
        (story.desktop_entry.to_string(), story),
    ])
}

struct QncShell {
    layout: ShellLayoutContract,
    qnc_root: PathBuf,
    executable_dir: PathBuf,
    app_registry: AppRegistry,
    embedded_factories: HashMap<String, EmbeddedAppFactory>,
    embedded_apps: HashMap<String, Box<dyn ShellDesktopApp>>,
    active_tab: String,
    theme_id: ThemeId,
    status: String,
}

impl QncShell {
    fn new(layout: ShellLayoutContract, qnc_root: PathBuf, app_registry: AppRegistry) -> Self {
        let active_tab = app_registry
            .first_tab_id()
            .unwrap_or_else(|| "project".to_string());
        Self {
            layout,
            executable_dir: env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(Path::to_path_buf))
                .unwrap_or_default(),
            qnc_root,
            app_registry,
            embedded_factories: embedded_app_factories(),
            embedded_apps: HashMap::new(),
            active_tab,
            theme_id: ThemeId::Dark,
            status: "Spreman.".to_string(),
        }
    }

    fn activate_tab(&mut self, tab_id: &str) {
        let Some(app) = self.app_registry.find(tab_id).cloned() else {
            self.status = format!("Aplikacija nije registrirana: {tab_id}");
            return;
        };

        if app.host_mode == "embedded_public_api" {
            if self.ensure_embedded_component(&app) {
                self.active_tab = tab_id.to_string();
                if let Some(component) = self.embedded_apps.get_mut(tab_id) {
                    component.on_activated();
                }
                self.status = format!("{} aktivan.", app.label);
            }
        } else {
            self.status = format!(
                "{} nema aktivan shell desktop ulaz ({})",
                app.label, app.application_id
            );
        }
    }

    fn ensure_embedded_component(&mut self, app: &AppManifest) -> bool {
        if self.embedded_apps.contains_key(&app.tab_id) {
            return true;
        }

        let Some(factory) = self.embedded_factories.get(&app.desktop_entry).copied() else {
            self.status = format!(
                "{} nema registriran embedded adapter ({})",
                app.label, app.desktop_entry
            );
            return false;
        };

        match (factory.create)(self.qnc_root.clone()) {
            Ok(component) => {
                self.embedded_apps.insert(app.tab_id.clone(), component);
                self.status = format!("{} otvoren u QNC desktopu.", app.label);
                true
            }
            Err(error) => {
                self.status = format!("{} nije otvoren: {error}", app.label);
                false
            }
        }
    }

    fn consume_navigation(&mut self, source: &AppManifest) -> bool {
        let Some(component) = self.embedded_apps.get_mut(&source.tab_id) else {
            return false;
        };
        let Some(request) = component.take_navigation_request() else {
            return false;
        };
        let sequence = component.navigation_sequence();
        let available = self.shell_available_apps();
        let result = match request {
            DesktopNavigation::NextGroup => sequence
                .and_then(|sequence| next_group_tab(&source.application_id, &sequence, &available)),
        };
        match result {
            Ok(Some(tab)) => self.activate_tab(&tab),
            Ok(None) => self.status = "Nema sljedece odabrane grupe.".into(),
            Err(error) => self.status = format!("{}: {error}", request.action_id()),
        }
        true
    }

    fn shell_available_apps(&self) -> Vec<DesktopApplicationRef> {
        self.app_registry
            .entries()
            .iter()
            .filter(|app| self.can_activate_in_shell(app))
            .map(|app| DesktopApplicationRef {
                application_id: app.application_id.clone(),
                tab_id: app.tab_id.clone(),
                priority_group: app.priority_group.clone(),
            })
            .collect()
    }

    fn can_activate_in_shell(&self, app: &AppManifest) -> bool {
        match app.host_mode.as_str() {
            "embedded_public_api" => {
                self.embedded_factories.contains_key(&app.desktop_entry)
                    || self.embedded_apps.contains_key(&app.tab_id)
            }
            "external_component" => self.standalone_executable_exists(app),
            _ => false,
        }
    }

    fn standalone_executable_exists(&self, app: &AppManifest) -> bool {
        app.standalone_executable.as_ref().is_some_and(|name| {
            !name.contains(['/', '\\'])
                && self
                    .executable_dir
                    .join(format!("{name}{}", env::consts::EXE_SUFFIX))
                    .is_file()
        })
    }

    fn body(&mut self, ui: &mut egui::Ui) {
        let app = self.app_registry.find(&self.active_tab).cloned();
        if let Some(app) = app {
            if app.host_mode == "embedded_public_api" {
                self.ensure_embedded_component(&app);
                if let Some(component) = self.embedded_apps.get_mut(&app.tab_id) {
                    let ctx = ui.ctx().clone();
                    component.show_desktop(&ctx, ui);
                    if self.consume_navigation(&app) {
                        ui.ctx().request_repaint();
                    }
                    return;
                }
            }

            self.placeholder(ui, &app.label);
            return;
        }

        self.placeholder(ui, "Nema registrirane aplikacije");
    }

    fn placeholder(&self, ui: &mut egui::Ui, label: &str) {
        let theme = self.theme();
        let rect = ui.available_rect_before_wrap();
        ui.allocate_exact_size(rect.size(), Sense::hover());
        ui.painter().rect_filled(rect, 0.0, theme.bg);
        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Center)),
            |ui| {
                ui.add_space((rect.height() * 0.42).max(0.0));
                ui.label(
                    RichText::new(label)
                        .size(self.layout.theme_metrics.font_ui)
                        .strong()
                        .color(theme.muted),
                );
            },
        );
    }

    fn footer_status(&self) -> &str {
        self.embedded_apps
            .get(&self.active_tab)
            .and_then(|component| component.footer_status())
            .unwrap_or(&self.status)
    }

    fn close_active_project(&mut self) {
        let result = CloseProjectComponent::from_root(&self.qnc_root).close_active_project();
        match result {
            Ok(outcome) => {
                self.status = if outcome.closed {
                    "Aktivni projekt zatvoren.".into()
                } else {
                    "Nema aktivnog projekta.".into()
                };
            }
            Err(error) => {
                self.status = format!("Close project: {error}");
            }
        }
    }

    fn footer(&mut self, ctx: &egui::Context) {
        let theme = self.theme();
        egui::TopBottomPanel::bottom("footer")
            .exact_height(self.layout.shell_metrics.footer_height)
            .frame(
                egui::Frame::NONE
                    .fill(theme.bg)
                    .inner_margin(egui::Margin::symmetric(
                        self.layout.theme_metrics.chrome_pad_x,
                        0,
                    )),
            )
            .show(ctx, |ui| {
                let h = ui.available_height();
                let columns = self.layout.shell_metrics.workspace_footer_columns.max(3);
                let apps = self.app_registry.entries().to_vec();
                let mut tab_to_activate = None;
                let mut close_project = false;
                let status = self.footer_status().to_string();
                ui.columns(columns, |cols| {
                    cols[0].with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_height(h);
                        self.theme_picker(ui);
                    });

                    cols[1].with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                        ui.set_min_height(h);
                        ui.horizontal_centered(|ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;
                            for app in &apps {
                                let selected = self.active_tab == app.tab_id;
                                if self.link_tab(ui, &app.label, selected).clicked() {
                                    tab_to_activate = Some(app.tab_id.clone());
                                }
                            }
                        });
                    });

                    cols[2].with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.set_min_height(h);
                        let close_button = egui::Button::new(
                            RichText::new("Close project")
                                .size(self.layout.theme_metrics.font_ui)
                                .color(theme.text),
                        )
                        .min_size(Vec2::new(118.0, 24.0));
                        if ui
                            .add(close_button)
                            .on_hover_text("Zatvori aktivni projekt")
                            .clicked()
                        {
                            close_project = true;
                        }
                        ui.add(
                            egui::Label::new(
                                RichText::new(&status)
                                    .size(self.layout.theme_metrics.font_ui)
                                    .color(theme.muted),
                            )
                            .truncate(),
                        )
                        .on_hover_text(status.clone());
                    });
                });

                if let Some(tab_id) = tab_to_activate {
                    self.activate_tab(&tab_id);
                }
                if close_project {
                    self.close_active_project();
                }
            });
    }

    fn theme_picker(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme();
        ui.label(
            RichText::new("Tema")
                .size(self.layout.theme_metrics.font_ui)
                .color(theme.muted),
        );
        let mut selected = self.theme_id;
        egui::ComboBox::from_id_salt("qnc_shell_theme")
            .selected_text(selected.label())
            .width(110.0)
            .show_ui(ui, |ui| {
                for id in ThemeId::ALL {
                    ui.selectable_value(&mut selected, id, id.label());
                }
            });
        if selected != self.theme_id {
            self.theme_id = selected;
            apply_visuals(ui.ctx(), &self.layout);
            self.status = format!("Tema: {}", selected.label());
        }
    }

    fn link_tab(&self, ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
        let theme = self.theme();
        let text = if selected {
            RichText::new(label)
                .size(self.layout.theme_metrics.font_ui)
                .strong()
                .color(theme.text)
        } else {
            RichText::new(label)
                .size(self.layout.theme_metrics.font_ui)
                .color(theme.muted)
        };
        let response = ui.add(
            egui::Label::new(text)
                .sense(Sense::click())
                .selectable(false),
        );
        if selected {
            ui.painter().hline(
                response.rect.left()..=response.rect.right(),
                response.rect.bottom() + 1.0,
                egui::Stroke::new(2.0, theme.accent),
            );
        }
        response
    }

    fn theme(&self) -> Theme {
        match self.theme_id {
            ThemeId::Dark => Theme::from_contract(&self.layout.colors),
            ThemeId::Soft => Theme {
                bg: Color32::from_rgb(22, 27, 38),
                surface: Color32::from_rgb(32, 40, 56),
                raised: Color32::from_rgb(45, 55, 74),
                border: Color32::from_rgb(75, 88, 110),
                text: Color32::from_rgb(236, 239, 244),
                muted: Color32::from_rgb(168, 178, 194),
                accent: Color32::from_rgb(52, 199, 148),
                focus: Color32::from_rgb(255, 196, 90),
            },
            ThemeId::HighContrast => Theme {
                bg: Color32::BLACK,
                surface: Color32::from_rgb(18, 18, 18),
                raised: Color32::from_rgb(36, 36, 36),
                border: Color32::from_rgb(180, 180, 180),
                text: Color32::WHITE,
                muted: Color32::from_rgb(200, 200, 200),
                accent: Color32::from_rgb(0, 255, 170),
                focus: Color32::from_rgb(255, 200, 0),
            },
        }
    }
}

impl eframe::App for QncShell {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.footer(ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(self.theme().bg))
            .show(ctx, |ui| self.body(ui));
    }
}

#[derive(Debug, Clone, Copy)]
struct Theme {
    bg: Color32,
    surface: Color32,
    raised: Color32,
    border: Color32,
    text: Color32,
    muted: Color32,
    accent: Color32,
    focus: Color32,
}

impl Theme {
    fn from_contract(colors: &ThemeColors) -> Self {
        Self {
            bg: rgb(colors.bg),
            surface: rgb(colors.surface),
            raised: rgb(colors.raised),
            border: rgb(colors.border),
            text: rgb(colors.text),
            muted: rgb(colors.muted),
            accent: rgb(colors.accent),
            focus: rgb(colors.focus),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeId {
    Dark,
    Soft,
    HighContrast,
}

impl ThemeId {
    const ALL: [ThemeId; 3] = [ThemeId::Dark, ThemeId::Soft, ThemeId::HighContrast];

    fn label(self) -> &'static str {
        match self {
            ThemeId::Dark => "Dark",
            ThemeId::Soft => "Soft",
            ThemeId::HighContrast => "High contrast",
        }
    }
}

fn apply_app_fonts(ctx: &egui::Context, shell: &ShellLayoutContract) {
    let mut style = (*ctx.style()).clone();
    let font_ui = shell.theme_metrics.font_ui;
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(font_ui - 1.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(font_ui, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(font_ui, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(20.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(font_ui, FontFamily::Proportional),
    );
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    ctx.set_style(style);
}

fn apply_visuals(ctx: &egui::Context, shell: &ShellLayoutContract) {
    let theme = Theme::from_contract(&shell.colors);
    let mut style = (*ctx.style()).clone();
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.visuals.panel_fill = theme.bg;
    style.visuals.window_fill = theme.surface;
    style.visuals.extreme_bg_color = theme.bg;
    style.visuals.faint_bg_color = theme.surface;
    style.visuals.code_bg_color = theme.raised;
    style.visuals.override_text_color = Some(theme.text);
    style.visuals.widgets.noninteractive.bg_fill = theme.surface;
    style.visuals.widgets.noninteractive.weak_bg_fill = theme.raised;
    style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, theme.muted);
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, theme.border);
    style.visuals.widgets.inactive.bg_fill = theme.raised;
    style.visuals.widgets.inactive.weak_bg_fill = theme.surface;
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, theme.text);
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, theme.border);
    style.visuals.widgets.hovered.bg_fill = theme.raised;
    style.visuals.widgets.hovered.weak_bg_fill = theme.raised;
    style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, theme.text);
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, theme.accent);
    style.visuals.widgets.active.bg_fill = theme.accent;
    style.visuals.widgets.active.weak_bg_fill = theme.accent;
    style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, theme.accent);
    style.visuals.selection.bg_fill = theme.accent.linear_multiply(0.35);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, theme.focus);
    style.visuals.hyperlink_color = theme.accent;
    ctx.set_style(style);
}

fn rgb(value: [u8; 3]) -> Color32 {
    Color32::from_rgb(value[0], value[1], value[2])
}

fn resolve_qnc_root() -> Result<PathBuf, String> {
    let mut starts = Vec::new();
    if let Ok(path) = env::current_exe() {
        if let Some(parent) = path.parent() {
            starts.push(parent.to_path_buf());
        }
    }
    if let Ok(path) = env::current_dir() {
        starts.push(path);
    }

    for start in starts {
        for candidate in start.ancestors() {
            if is_qnc_root(candidate) {
                return Ok(candidate.to_path_buf());
            }
        }
    }

    Err("Ne mogu pronaci QNC root s AGENTS.md, seed/system_seed.json i contracts/ui/shell.layout.json.".to_string())
}

fn is_qnc_root(path: &Path) -> bool {
    path.join("AGENTS.md").is_file()
        && path.join("seed").join("system_seed.json").is_file()
        && path
            .join("contracts")
            .join("ui")
            .join("shell.layout.json")
            .is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, time::SystemTime};

    #[test]
    fn loads_embedded_shell_contract() {
        let layout = ShellLayoutContract::load_embedded().expect("shell layout");
        assert_eq!(layout.layout_id, "qnc.ui.shell");
        assert_eq!(
            layout.application_tabs,
            ["project", "ingest", "media_assist", "storyboard"]
        );
    }

    #[test]
    fn qnc_root_requires_agents_seed_and_shell_layout() {
        let root = env::temp_dir().join(format!(
            "qnc_shell_root_{}_{}",
            process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(root.join("seed")).expect("seed dir");
        fs::create_dir_all(root.join("contracts").join("ui")).expect("ui dir");
        fs::write(root.join("AGENTS.md"), "").expect("agents");
        fs::write(root.join("seed").join("system_seed.json"), "{}").expect("seed");
        fs::write(
            root.join("contracts").join("ui").join("shell.layout.json"),
            "{}",
        )
        .expect("shell");

        assert!(is_qnc_root(&root));

        fs::remove_file(root.join("AGENTS.md")).expect("remove agents");
        assert!(!is_qnc_root(&root));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_tab_is_hosted_component() {
        let root = env::temp_dir().join(format!(
            "qnc_shell_component_{}_{}",
            process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(root.join("contracts").join("ui")).expect("ui dir");
        fs::write(root.join("AGENTS.md"), "").expect("agents");
        fs::write(
            root.join("contracts").join("ui").join("shell.layout.json"),
            "{}",
        )
        .expect("shell");
        let layout = ShellLayoutContract::load_embedded().expect("shell layout");
        let shell = QncShell::new(layout, root.clone(), test_registry());
        assert_eq!(shell.active_tab, "project");
        assert!(shell.embedded_apps.is_empty());
        assert!(shell.embedded_factories.contains_key("qnc_project"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn embedded_component_dispatch_uses_desktop_entry_registry() {
        let root = env::temp_dir().join(format!(
            "qnc_shell_dispatch_{}_{}",
            process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(root.join("contracts").join("ui")).expect("ui dir");
        fs::write(root.join("AGENTS.md"), "").expect("agents");
        fs::write(
            root.join("contracts").join("ui").join("shell.layout.json"),
            "{}",
        )
        .expect("shell");
        let layout = ShellLayoutContract::load_embedded().expect("shell layout");
        let mut shell = QncShell::new(layout, root.clone(), test_registry());

        shell.activate_tab("project");

        assert!(shell.embedded_apps.contains_key("project"));
        assert!(!shell.status.contains("nema registriran embedded adapter"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn app_registry_requires_standalone_executable() {
        let root = temp_root("qnc_shell_registry_standalone");
        write_app_manifest(
            &root,
            "qnc-project",
            r#"{
                "application_id": "qnc.project",
                "tab_id": "project",
                "label": "Project",
                "enabled": true,
                "system": true,
                "removable": false,
                "priority_group": "a",
                "host_mode": "embedded_public_api",
                "desktop_entry": "qnc_project"
            }"#,
        );

        let error = AppRegistry::load(&root).expect_err("standalone executable is required");
        assert!(error.contains("standalone_executable is required"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn app_registry_loads_enabled_manifests_in_order() {
        let root = temp_root("qnc_shell_registry_order");
        write_app_manifest(
            &root,
            "qnc-story",
            r#"{
                "application_id": "qnc.story",
                "tab_id": "storyboard",
                "label": "Story",
                "enabled": true,
                "system": true,
                "removable": false,
                "priority_group": "c",
                "host_mode": "external_component",
                "desktop_entry": "qnc_story",
                "standalone_executable": "qnc-story"
            }"#,
        );
        write_app_manifest(
            &root,
            "qnc-project",
            r#"{
                "application_id": "qnc.project",
                "tab_id": "project",
                "label": "Project",
                "enabled": true,
                "system": true,
                "removable": false,
                "priority_group": "a",
                "host_mode": "embedded_public_api",
                "desktop_entry": "qnc_project",
                "standalone_executable": "qnc-project"
            }"#,
        );

        let registry = AppRegistry::load(&root).expect("registry");
        assert_eq!(
            registry
                .entries()
                .iter()
                .map(|entry| entry.tab_id.as_str())
                .collect::<Vec<_>>(),
            ["project", "storyboard"]
        );
        assert_eq!(registry.first_tab_id().as_deref(), Some("project"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn app_registry_ignores_disabled_manifests() {
        let root = temp_root("qnc_shell_registry_disabled");
        write_app_manifest(
            &root,
            "qnc-project",
            r#"{
                "application_id": "qnc.project",
                "tab_id": "project",
                "label": "Project",
                "enabled": true,
                "system": true,
                "removable": false,
                "priority_group": "a",
                "host_mode": "embedded_public_api",
                "desktop_entry": "qnc_project",
                "standalone_executable": "qnc-project"
            }"#,
        );
        write_app_manifest(
            &root,
            "qnc-ingest",
            r#"{
                "application_id": "qnc.ingest",
                "tab_id": "ingest",
                "label": "Ingest",
                "enabled": false,
                "system": true,
                "removable": false,
                "priority_group": "b",
                "host_mode": "external_component",
                "desktop_entry": "qnc_ingest",
                "standalone_executable": "qnc-ingest"
            }"#,
        );

        let registry = AppRegistry::load(&root).expect("registry");
        assert_eq!(registry.entries().len(), 1);
        assert_eq!(registry.entries()[0].application_id, "qnc.project");
        let _ = fs::remove_dir_all(root);
    }

    fn test_registry() -> AppRegistry {
        AppRegistry {
            entries: vec![AppManifest {
                application_id: "qnc.project".to_string(),
                tab_id: "project".to_string(),
                label: "Project".to_string(),
                enabled: true,
                system: true,
                removable: false,
                priority_group: "a".into(),
                host_mode: "embedded_public_api".to_string(),
                desktop_entry: "qnc_project".to_string(),
                standalone_executable: Some("qnc-project".to_string()),
            }],
        }
    }

    struct NavigationSurface {
        pending: bool,
        sequence: Result<Vec<DesktopApplicationRef>, String>,
    }

    impl ShellDesktopApp for NavigationSurface {
        fn show_desktop(&mut self, _: &egui::Context, _: &mut egui::Ui) {}
        fn take_navigation_request(&mut self) -> Option<DesktopNavigation> {
            std::mem::take(&mut self.pending).then_some(DesktopNavigation::NextGroup)
        }
        fn navigation_sequence(&self) -> Result<Vec<DesktopApplicationRef>, String> {
            self.sequence.clone()
        }
    }

    fn navigation_shell() -> QncShell {
        let root = temp_root("qnc_shell_navigation");
        fs::create_dir_all(&root).unwrap();
        let mut registry = test_registry();
        let mut target = registry.entries[0].clone();
        target.application_id = "qnc.variant".into();
        target.tab_id = "variant".into();
        target.priority_group = "c".into();
        target.desktop_entry = "variant_adapter".into();
        target.standalone_executable = Some("qnc-variant".into());
        registry.entries.push(target);
        let sequence = registry
            .entries
            .iter()
            .map(|app| {
                fs::write(
                    root.join(format!(
                        "{}{}",
                        app.standalone_executable.as_ref().unwrap(),
                        env::consts::EXE_SUFFIX
                    )),
                    [],
                )
                .unwrap();
                DesktopApplicationRef {
                    application_id: app.application_id.clone(),
                    tab_id: app.tab_id.clone(),
                    priority_group: app.priority_group.clone(),
                }
            })
            .collect();
        let mut shell = QncShell::new(
            ShellLayoutContract::load_embedded().unwrap(),
            root.clone(),
            registry,
        );
        shell.executable_dir = root;
        shell.embedded_apps.insert(
            "project".into(),
            Box::new(NavigationSurface {
                pending: true,
                sequence: Ok(sequence),
            }),
        );
        shell.embedded_factories.insert(
            "variant_adapter".into(),
            EmbeddedAppFactory {
                desktop_entry: "variant_adapter",
                create: |_| {
                    Ok(Box::new(NavigationSurface {
                        pending: false,
                        sequence: Ok(vec![]),
                    }))
                },
            },
        );
        shell
    }

    #[test]
    fn navigation_trigger_activates_adapter_once_without_business_payload() {
        let mut shell = navigation_shell();
        let source = shell.app_registry.find("project").unwrap().clone();
        assert!(shell.consume_navigation(&source));
        assert_eq!(shell.active_tab, "variant");
        assert!(shell.embedded_apps.contains_key("variant"));
        shell.activate_tab("project");
        assert!(!shell.consume_navigation(&source));
        assert_eq!(shell.active_tab, "project");
        fs::remove_dir_all(shell.qnc_root).unwrap();
    }

    #[test]
    fn unavailable_target_preserves_source_surface_and_error() {
        let mut shell = navigation_shell();
        shell.embedded_factories.clear();
        let source = shell.app_registry.find("project").unwrap().clone();
        assert!(shell.consume_navigation(&source));
        assert_eq!(shell.active_tab, "project");
        let error = shell.status.clone();
        assert!(error.contains("nije dostupna"));
        assert!(shell.ensure_embedded_component(&source));
        assert_eq!(shell.status, error);
        fs::remove_dir_all(shell.qnc_root).unwrap();
    }

    #[test]
    fn missing_current_standalone_executable_does_not_block_embedded_navigation() {
        let mut shell = navigation_shell();
        fs::remove_file(
            shell
                .executable_dir
                .join(format!("qnc-project{}", env::consts::EXE_SUFFIX)),
        )
        .unwrap();
        let source = shell.app_registry.find("project").unwrap().clone();
        shell.consume_navigation(&source);
        assert_eq!(shell.active_tab, "variant");
        assert!(shell.embedded_apps.contains_key("variant"));
        fs::remove_dir_all(shell.qnc_root).unwrap();
    }

    struct StatusSurface {
        status: String,
        activations: usize,
    }

    impl ShellDesktopApp for StatusSurface {
        fn show_desktop(&mut self, _: &egui::Context, _: &mut egui::Ui) {}

        fn footer_status(&self) -> Option<&str> {
            Some(&self.status)
        }

        fn on_activated(&mut self) {
            self.activations += 1;
            self.status = format!("Activation {}", self.activations);
        }
    }

    #[test]
    fn footer_uses_only_active_surface_status_without_application_name_switch() {
        let mut shell = navigation_shell();
        shell.embedded_apps.insert(
            "variant".into(),
            Box::new(StatusSurface {
                status: "DB project name".into(),
                activations: 0,
            }),
        );
        shell.status = "Host status".into();
        assert_eq!(shell.footer_status(), "Host status");
        shell.active_tab = "variant".into();
        assert_eq!(shell.footer_status(), "DB project name");
        shell.status = "Theme changed".into();
        assert_eq!(shell.footer_status(), "DB project name");
        shell.activate_tab("project");
        assert_ne!(shell.footer_status(), "DB project name");
        shell.activate_tab("variant");
        assert_eq!(shell.footer_status(), "Activation 1");
        shell.activate_tab("project");
        shell.activate_tab("variant");
        assert_eq!(shell.footer_status(), "Activation 2");
        fs::remove_dir_all(shell.qnc_root).unwrap();
    }

    #[test]
    fn close_active_project_only_calls_public_module_without_changing_shell_surfaces() {
        let mut shell = navigation_shell();
        let data = shell.qnc_root.join("data");
        fs::create_dir_all(&data).unwrap();
        let db = data.join("project_store.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE app_settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE VIEW public_app_settings AS SELECT key, value FROM app_settings;
            INSERT INTO app_settings VALUES('active_project_id', 'p1');
            ",
        )
        .unwrap();
        let project_dir = shell.qnc_root.join("projects").join("p1");
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(project_dir.join("qnc_project.db"), []).unwrap();
        shell.embedded_apps.insert(
            "variant".into(),
            Box::new(StatusSurface {
                status: "Ingest runtime".into(),
                activations: 0,
            }),
        );
        shell.active_tab = "variant".into();

        shell.close_active_project();

        let active: String = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='active_project_id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(active.is_empty());
        assert!(project_dir.join("qnc_project.db").is_file());
        assert_eq!(shell.active_tab, "variant");
        assert!(shell.embedded_apps.contains_key("variant"));
        assert_eq!(shell.status, "Aktivni projekt zatvoren.");
        drop(conn);
        fs::remove_dir_all(shell.qnc_root).unwrap();
    }

    fn temp_root(prefix: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "{}_{}_{}",
            prefix,
            process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ))
    }

    fn write_app_manifest(root: &Path, app_dir: &str, contents: &str) {
        let app_root = root.join("apps").join(app_dir);
        fs::create_dir_all(&app_root).expect("app dir");
        fs::write(app_root.join("qnc-app.json"), contents).expect("app manifest");
    }
}
