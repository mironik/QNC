use std::path::PathBuf;

use eframe::egui;
use qnc_shell_desktop_api::{EmbeddedAppFactory, ShellDesktopApp};

pub fn factory() -> EmbeddedAppFactory {
    EmbeddedAppFactory {
        desktop_entry: "qnc_project",
        create,
    }
}

fn create(root: PathBuf) -> Result<Box<dyn ShellDesktopApp>, String> {
    Ok(Box::new(ProjectDesktopAdapter {
        app: qnc_project_desktop::create_project_app(root)?,
    }))
}

struct ProjectDesktopAdapter {
    app: qnc_project_desktop::ProjectApp,
}

impl ShellDesktopApp for ProjectDesktopAdapter {
    fn show_desktop(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        self.app.show_desktop(ctx, ui);
    }
}
