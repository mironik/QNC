use std::path::PathBuf;

use eframe::egui;
use qnc_shell_desktop_api::{
    DesktopApplicationRef, DesktopNavigation, EmbeddedAppFactory, ShellDesktopApp,
};

pub fn factory() -> EmbeddedAppFactory {
    EmbeddedAppFactory {
        desktop_entry: "qnc_ingest",
        create,
    }
}

fn create(root: PathBuf) -> Result<Box<dyn ShellDesktopApp>, String> {
    Ok(Box::new(IngestDesktopAdapter {
        app: qnc_ingest_desktop::create_ingest_app(root)?,
    }))
}

struct IngestDesktopAdapter {
    app: qnc_ingest_desktop::IngestApp,
}

impl ShellDesktopApp for IngestDesktopAdapter {
    fn show_desktop(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        self.app.show_desktop(ctx, ui);
    }

    fn footer_status(&self) -> Option<&str> {
        Some(self.app.footer_status())
    }

    fn on_activated(&mut self) {
        self.app.on_activated();
    }

    fn take_navigation_request(&mut self) -> Option<DesktopNavigation> {
        self.app
            .take_navigation_request()
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
