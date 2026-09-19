use std::path::PathBuf;

use eframe::egui;
use qnc_shell_desktop_api::{EmbeddedAppFactory, ShellDesktopApp};

const GROUP: &str = "l";

pub fn factory() -> EmbeddedAppFactory {
    EmbeddedAppFactory {
        desktop_entry: "qnc_media_assist_video",
        create,
    }
}

fn create(root: PathBuf) -> Result<Box<dyn ShellDesktopApp>, String> {
    Ok(Box::new(Adapter {
        app: qnc_editorial_desktop::EditorialApp::new(GROUP, root)?,
    }))
}

struct Adapter {
    app: qnc_editorial_desktop::EditorialApp,
}

impl ShellDesktopApp for Adapter {
    fn show_desktop(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        self.app.show_desktop(ctx, ui);
    }

    fn on_activated(&mut self) {
        self.app.on_activated();
    }

    fn footer_status(&self) -> Option<&str> {
        Some(self.app.footer_status())
    }
}
