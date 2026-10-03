//! The QNC desktop (moved unchanged out of `qnc-app`, user rule 2026-09-30: everything is
//! a block): the shell layout model, the surfaces of the registered applications, their
//! activation and navigation, and one layout with the surface of the active application
//! over the footer block.

use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
    time::Duration,
};

use eframe::egui::{self, Color32, FontFamily, FontId, RichText, Sense, TextStyle, Vec2};
use qnc_contracts::validate_ui_layout_contract_json;
use qnc_project_close::CloseProjectComponent;
use qnc_shell_desktop_api::{
    next_group_tab, DesktopApplicationRef, DesktopNavigation, EmbeddedAppFactory, ShellDesktopApp,
};
use qnc_shell_footer::{FooterInput, FooterIntent, FooterStyle, Palette, ThemeId};
use qnc_app_registry::{AppManifest, AppRegistry};
use serde::Deserialize;

const SHELL_LAYOUT_JSON: &str = include_str!("../../../contracts/ui/shell.layout.json");

#[derive(Debug, Clone, Deserialize)]
pub struct ShellLayoutContract {
    pub layout_id: String,
    pub application_tabs: Vec<String>,
    shell_metrics: ShellMetrics,
    theme_metrics: ThemeMetrics,
    colors: ThemeColors,
}

impl ShellLayoutContract {
    pub fn load_embedded() -> Result<Self, String> {
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

/// The desktop: the surface of the active application over the footer.
pub struct QncShell {
    layout: ShellLayoutContract,
    qnc_root: PathBuf,
    executable_dir: PathBuf,
    app_registry: AppRegistry,
    embedded_factories: HashMap<String, EmbeddedAppFactory>,
    embedded_apps: HashMap<String, Box<dyn ShellDesktopApp>>,
    active_tab: String,
    theme_id: ThemeId,
    status: String,
    /// The launch bar: the applications of the active project, re-read from its database.
    tabs: qnc_desktop_tabs::Tabs,
    /// Reads the launch bar on its own thread (the desktop never waits on the database).
    tabs_watch: Option<qnc_desktop_tabs::Watcher>,
}

/// How often the launch bar reads the active project again (the database is the truth).
const TABS_REREAD: Duration = Duration::from_secs(1);

impl QncShell {
    /// `factories` maps a registered `desktop_entry` to the public surface of its application.
    pub fn new(
        layout: ShellLayoutContract,
        qnc_root: PathBuf,
        app_registry: AppRegistry,
        factories: HashMap<String, EmbeddedAppFactory>,
    ) -> Self {
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
            embedded_factories: factories,
            embedded_apps: HashMap::new(),
            active_tab,
            theme_id: ThemeId::Dark,
            status: "Spreman.".to_string(),
            tabs: qnc_desktop_tabs::Tabs { tab_ids: Vec::new(), project_open: false, error: None },
            tabs_watch: None,
        }
    }

    fn activate_tab(&mut self, tab_id: &str) {
        let Some(app) = self.app_registry.find(tab_id).cloned() else {
            self.status = format!("Aplikacija nije registrirana: {tab_id}");
            return;
        };

        if app.host_mode == "embedded_public_api" {
            if self.ensure_embedded_component(&app) {
                if self.active_tab != tab_id {
                    // Only one surface is shown; a hidden one must not keep a player
                    // reading the media while another plays.
                    if let Some(previous) = self.embedded_apps.get_mut(&self.active_tab) {
                        previous.on_deactivated();
                    }
                }
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
        self.refresh_tabs(true);
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

    /// The surface of the active application in the desktop frame.
    fn body(&mut self, ui: &mut egui::Ui, frame: &mut qnc_board::Frame<'_>) {
        let app = self.app_registry.find(&self.active_tab).cloned();
        if let Some(app) = app {
            if app.host_mode == "embedded_public_api" {
                self.ensure_embedded_component(&app);
                if let Some(component) = self.embedded_apps.get_mut(&app.tab_id) {
                    let ctx = ui.ctx().clone();
                    component.show_in_frame(&ctx, ui, frame);
                    if self.consume_navigation(&app) {
                        ui.ctx().request_repaint();
                    }
                    return;
                }
            }

            qnc_board::show_surface_in_frame(ui, frame, |ui| self.placeholder(ui, &app.label));
            return;
        }

        qnc_board::show_surface_in_frame(ui, frame, |ui| self.placeholder(ui, "Nema registrirane aplikacije"));
    }

    fn placeholder(&self, ui: &mut egui::Ui, label: &str) {
        let theme = self.palette();
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
                // Closing the project returns to the project application (user rule
                // 2026-10-01): the bar shows only the first group again.
                self.refresh_tabs(true);
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

    /// The applications of the active project: `now` (startup, close, next group) reads
    /// them at once, otherwise the last read of the background watcher (once a second);
    /// when the shown application is not one of them, the first one is shown.
    fn refresh_tabs(&mut self, now: bool) {
        if self.tabs_watch.is_none() {
            let watch = qnc_desktop_tabs::Watcher::start(self.qnc_root.clone(), self.shell_available_apps(), TABS_REREAD);
            self.tabs_watch = Some(watch);
        }
        let watch = self.tabs_watch.as_ref().expect("started above");
        if now {
            watch.reread();
            self.tabs = qnc_desktop_tabs::read(&self.qnc_root, &self.shell_available_apps());
        } else if let Some(tabs) = watch.take() {
            self.tabs = tabs;
        } else {
            return;
        }
        if let Some(error) = &self.tabs.error {
            self.status = error.clone();
        }
        if !self.tabs.tab_ids.contains(&self.active_tab) {
            if let Some(first) = self.tabs.tab_ids.first().cloned() {
                self.activate_tab(&first);
            }
        }
    }

    /// The shell palette of the chosen theme.
    fn palette(&self) -> Palette {
        self.theme_id.palette(contract_palette(&self.layout.colors))
    }

    /// What the footer place of the desktop frame shows this frame.
    fn footer(&self) -> FooterBlock {
        FooterBlock {
            style: FooterStyle {
                font_ui: self.layout.theme_metrics.font_ui,
                pad_x: self.layout.theme_metrics.chrome_pad_x,
                columns: self.layout.shell_metrics.workspace_footer_columns,
            },
            palette: self.palette(),
            tabs: self
                .tabs
                .tab_ids
                .iter()
                .filter_map(|tab| self.app_registry.find(tab))
                .map(|app| (app.tab_id.clone(), app.label.clone()))
                .collect(),
            active_tab: self.active_tab.clone(),
            theme: self.theme_id,
            status: self.footer_status().to_string(),
            project_open: self.tabs.project_open,
            intent: None,
        }
    }

    fn apply_footer(&mut self, ctx: &egui::Context, intent: FooterIntent) {
        match intent {
            FooterIntent::Activate(tab_id) => self.activate_tab(&tab_id),
            FooterIntent::CloseProject => self.close_active_project(),
            FooterIntent::Theme(theme) => {
                self.theme_id = theme;
                apply_visuals(ctx, &self.layout);
                self.status = format!("Tema: {}", theme.label());
            }
        }
    }
}

impl eframe::App for QncShell {
    /// The desktop frame: the board of the active application over the footer, drawn as
    /// one layout tree (user rules 2026-09-30 and 2026-10-01: the footer is a place of
    /// every board; the layout is the frame).
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let started = std::time::Instant::now();
        self.draw(ctx);
        // A frame of the desktop longer than 25 ms is a moment the hand waits: logged with
        // the application on screen, so a slow piece can be found (diagnostics only).
        let spent = started.elapsed();
        if spent > Duration::from_millis(25) && qnc_dev_diagnostics::player_diagnostics_enabled() {
            qnc_dev_diagnostics::log_line(
                qnc_dev_diagnostics::DiagnosticsStream::Player,
                format!("ui-frame-slow ms={} tab={}", spent.as_millis(), self.active_tab),
            );
        }
    }
}

impl QncShell {
    fn draw(&mut self, ctx: &egui::Context) {
        self.refresh_tabs(false);
        ctx.request_repaint_after(TABS_REREAD);
        let mut footer = self.footer();
        let mut frame = qnc_board::Frame::desktop(self.layout.shell_metrics.footer_height, &mut footer);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(self.palette().bg))
            .show(ctx, |ui| self.body(ui, &mut frame));
        drop(frame);
        if let Some(intent) = footer.intent {
            self.apply_footer(ctx, intent);
        }
    }
}

/// The footer place of the desktop frame.
struct FooterBlock {
    style: FooterStyle,
    palette: Palette,
    tabs: Vec<(String, String)>,
    active_tab: String,
    theme: ThemeId,
    status: String,
    project_open: bool,
    intent: Option<FooterIntent>,
}

impl qnc_board::FrameBlocks for FooterBlock {
    fn block(&mut self, ui: &mut egui::Ui, name: &str, rect: egui::Rect) {
        if name != "footer" {
            return;
        }
        let tabs: Vec<(&str, &str)> = self.tabs.iter().map(|(id, label)| (id.as_str(), label.as_str())).collect();
        let input = FooterInput { tabs: &tabs, active_tab: &self.active_tab, theme: self.theme, status: &self.status, project_open: self.project_open };
        self.intent = qnc_shell_footer::show(ui, rect, &self.style, &self.palette, input);
    }
}

pub fn apply_app_fonts(ctx: &egui::Context, shell: &ShellLayoutContract) {
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

pub fn apply_visuals(ctx: &egui::Context, shell: &ShellLayoutContract) {
    let theme = contract_palette(&shell.colors);
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

/// The palette of the shell layout contract (the Dark theme).
fn contract_palette(colors: &ThemeColors) -> Palette {
    Palette {
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

fn rgb(value: [u8; 3]) -> Color32 {
    Color32::from_rgb(value[0], value[1], value[2])
}

pub fn resolve_qnc_root() -> Result<PathBuf, String> {
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
    use std::{fs, process, time::SystemTime};

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
        let shell = QncShell::new(layout, root.clone(), test_registry(), test_factories());
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
        let mut shell = QncShell::new(layout, root.clone(), test_registry(), test_factories());

        shell.activate_tab("project");

        assert!(shell.embedded_apps.contains_key("project"));
        assert!(!shell.status.contains("nema registriran embedded adapter"));
        let _ = fs::remove_dir_all(root);
    }

    fn test_factories() -> HashMap<String, EmbeddedAppFactory> {
        let project = qnc_project_desktop_adapter::factory();
        HashMap::from([(project.desktop_entry.to_string(), project)])
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
            test_factories(),
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
        drop(shell.tabs_watch.take()); // its reading thread holds the database
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
        drop(shell.tabs_watch.take()); // its reading thread holds the database
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
        drop(shell.tabs_watch.take()); // its reading thread holds the database
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

        fn on_deactivated(&mut self) {
            self.status = format!("Deactivated after {}", self.activations);
        }
    }

    #[test]
    fn leaving_a_surface_tells_it_to_release_its_player() {
        let mut shell = navigation_shell();
        shell.embedded_apps.insert(
            "variant".into(),
            Box::new(StatusSurface {
                status: String::new(),
                activations: 0,
            }),
        );
        shell.activate_tab("variant");
        shell.activate_tab("variant");
        let status = |shell: &QncShell| {
            shell.embedded_apps["variant"]
                .footer_status()
                .unwrap_or_default()
                .to_string()
        };
        assert_eq!(
            status(&shell),
            "Activation 2",
            "re-selecting the same tab is no switch"
        );
        shell.activate_tab("project");
        assert_eq!(status(&shell), "Deactivated after 2");
        drop(shell.tabs_watch.take()); // its reading thread holds the database
        fs::remove_dir_all(shell.qnc_root).unwrap();
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
        drop(shell.tabs_watch.take()); // its reading thread holds the database
        fs::remove_dir_all(shell.qnc_root).unwrap();
    }

    #[test]
    fn close_active_project_calls_the_public_module_and_returns_to_the_first_group() {
        let mut shell = navigation_shell();
        let data = shell.qnc_root.join("data");
        fs::create_dir_all(&data).unwrap();
        let db = data.join("qnc-projects.db");
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
        fs::write(project_dir.join("project.db"), []).unwrap();
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
        assert!(project_dir.join("project.db").is_file());
        assert_eq!(shell.active_tab, "project", "back to the project application");
        assert!(shell.embedded_apps.contains_key("variant"), "no surface is destroyed");
        assert_eq!(shell.tabs.tab_ids, ["project"], "the bar shows only the first group");
        assert_eq!(shell.status, "Aktivni projekt zatvoren.");
        drop(conn);
        drop(shell.tabs_watch.take()); // its reading thread holds the database
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
}
