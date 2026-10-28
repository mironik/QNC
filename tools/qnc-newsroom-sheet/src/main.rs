//! The story sheet on its own (user 2026-10-27: an administrator makes the sheet of the
//! newsroom when none of the prepared ones fits). Without an active project it only chooses,
//! changes or makes the sheet of this computer; with one it fills that project's sheet (its
//! name as the project has it) and keeps it in the project database.

use std::path::{Path, PathBuf};

use eframe::egui;
use qnc_db_broker::ProjectDbTarget;
use qnc_desktop_theme::{ShellLayoutContract, Theme};
use qnc_keyboard_shortcut::ShortcutCatalog;
use qnc_newsroom_sheet::{Answer, SheetOverlay, SheetSpec};
use qnc_newsroom_sheet_db::StorySheet;
use serde_json::Value;

/// The project the sheet belongs to, when one is active, and the script profile it is written with.
struct Project {
    target: ProjectDbTarget,
    profile: Option<String>,
}

/// What the window shows.
#[derive(PartialEq)]
enum Tab {
    Sheet,
    Profile,
}

struct App {
    overlay: SheetOverlay,
    profiles: qnc_script_profile_editor::ProfileEditor,
    tab: Tab,
    project: Option<Project>,
    theme: Theme,
    shortcuts: ShortcutCatalog,
}

fn main() -> eframe::Result<()> {
    let root = root_from_args().or_else(qnc_dev_diagnostics::resolve_qnc_root).unwrap_or_else(|| fail("QNC root not found (QNC_ROOT or --root)"));
    let theme = read_theme(&root).unwrap_or_else(|error| fail(&error));
    let shortcuts = std::fs::read_to_string(root.join("contracts").join("qnc-keyboard-shortcuts.json"))
        .map_err(|error| error.to_string())
        .and_then(|text| ShortcutCatalog::from_json_str(&text))
        .unwrap_or_else(|error| fail(&error));
    let (overlay, project) = open(&root);
    let profiles = qnc_script_profile_editor::ProfileEditor::new(&root);
    let app = App { overlay, profiles, tab: Tab::Sheet, project, theme, shortcuts };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title(qnc_ui_text::text("sheet.title")).with_inner_size([820.0, 820.0]),
        ..Default::default()
    };
    eframe::run_native("QNC Story sheet", options, Box::new(|_| Ok(Box::new(app))))
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2)
}

fn root_from_args() -> Option<PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|arg| arg == "--root").and_then(|at| args.get(at + 1)).map(PathBuf::from)
}

fn read_theme(root: &Path) -> Result<Theme, String> {
    let text = std::fs::read_to_string(root.join("contracts").join("ui").join("shell.layout.json")).map_err(|error| error.to_string())?;
    let shell: ShellLayoutContract = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    Ok(Theme::from_contract(&shell))
}

/// The overlay of the active project's sheet, or of this computer's sheets when there is none
/// (or it cannot be read: the reason is shown).
fn open(root: &Path) -> (SheetOverlay, Option<Project>) {
    let save = qnc_ui_text::text("sheet.save_values").to_string();
    let active = qnc_active_project_read::ActiveProjectReader::from_root(root).map_err(|error| error.message).and_then(|active| {
        let snapshot = active.read().map_err(|error| error.message)?;
        let target = ProjectDbTarget::for_project(active.settings_reader(), &snapshot.settings)?;
        let kept = qnc_newsroom_sheet_db::read(&target)?;
        Ok((snapshot.settings.project_name.clone(), target, kept))
    });
    match active {
        Ok((name, target, kept)) => {
            let sheet = serde_json::from_value::<SheetSpec>(kept.sheet).ok();
            let values = Value::Object(kept.values.into_iter().collect());
            let profile = qnc_script_db::read_profile(&target).ok().flatten().map(|profile| profile.name);
            (SheetOverlay::new(root, name, false, values, sheet, save), Some(Project { target, profile }))
        }
        Err(error) => {
            let mut overlay = SheetOverlay::new(root, String::new(), false, Value::Null, None, save);
            overlay.message = qnc_ui_text::fill("sheet.no_project", &[("reason", &error)]);
            (overlay, None)
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        qnc_desktop_theme::apply_settings_look(ctx, &self.theme);
        egui::TopBottomPanel::top("qnc_sheet_tabs").frame(egui::Frame::NONE.fill(self.theme.surface_alt).inner_margin(8)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Sheet, qnc_ui_text::text("sheet.tab_sheet"));
                ui.selectable_value(&mut self.tab, Tab::Profile, qnc_ui_text::text("sheet.tab_profile"));
            });
        });
        let mut asked = None;
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(self.theme.surface).inner_margin(24)).show(ctx, |ui| {
            if self.tab == Tab::Profile {
                let project = self.project.as_ref().map(|project| project.profile.clone().unwrap_or_else(|| "—".into()));
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    asked = qnc_script_profile_editor::show(ui, &mut self.profiles, &self.theme, project.as_deref());
                });
            }
        });
        if let Some(qnc_script_profile_editor::Answer::UseForProject(profile)) = asked {
            self.profiles.message = match self.project.as_mut() {
                Some(project) => match qnc_script_db::set_profile(&project.target, profile.clone()) {
                    Ok(()) => {
                        project.profile = Some(profile.name);
                        qnc_ui_text::text("sp.used").to_string()
                    }
                    Err(error) => error,
                },
                None => qnc_ui_text::text("sp.no_project").to_string(),
            };
        }
        if self.tab == Tab::Profile {
            return;
        }
        let next_field = ctx.wants_keyboard_input()
            && qnc_keyboard_shortcut::egui_shortcut_events(ctx)
                .iter()
                .any(|event| self.shortcuts.action_ids_for_event("project", event).contains(&"form_next_field"));
        match qnc_newsroom_sheet::show(ctx, &mut self.overlay, &self.theme, next_field) {
            Some(Answer::Cancel) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Some(Answer::Confirm) => match self.keep() {
                Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Err(error) => self.overlay.message = error,
            },
            None => {}
        }
    }
}

impl App {
    /// The filled sheet and its values into the active project.
    fn keep(&self) -> Result<(), String> {
        let project = self.project.as_ref().ok_or_else(|| qnc_ui_text::text("sheet.no_project_to_fill").to_string())?;
        let (sheet, values) = self.overlay.filled().ok_or_else(|| qnc_ui_text::text("sheet.none").to_string())?;
        let sheet = serde_json::to_value(sheet).map_err(|error| error.to_string())?;
        let values = values.as_object().cloned().unwrap_or_default().into_iter().collect();
        qnc_newsroom_sheet_db::save(&project.target, StorySheet { sheet, values })
    }
}
