use std::path::PathBuf;

use eframe::egui;
use qnc_shell_desktop_api::{
    DesktopApplicationRef, DesktopNavigation, EmbeddedAppFactory, ShellDesktopApp,
};

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

    fn take_navigation_request(&mut self) -> Option<DesktopNavigation> {
        self.app
            .take_navigation_trigger()
            .then_some(DesktopNavigation::NextGroup)
    }

    fn navigation_sequence(&self) -> Result<Vec<DesktopApplicationRef>, String> {
        self.app.navigation_sequence().map(|steps| {
            steps
                .into_iter()
                .map(|step| DesktopApplicationRef {
                    application_id: step.application_id,
                    tab_id: step.tab_id,
                    priority_group: step.priority_group,
                })
                .collect()
        })
    }
}
