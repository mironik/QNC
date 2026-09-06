use std::path::PathBuf;

use eframe::egui;
use qnc_shell_desktop_api::{EmbeddedAppFactory, ShellDesktopApp};

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
}
