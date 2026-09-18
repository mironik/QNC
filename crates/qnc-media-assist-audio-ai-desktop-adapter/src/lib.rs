use std::path::PathBuf;

use eframe::egui;
use qnc_shell_desktop_api::{EmbeddedAppFactory, ShellDesktopApp};

const GROUP: &str = "e";

pub fn factory() -> EmbeddedAppFactory {
    EmbeddedAppFactory {
        desktop_entry: "qnc_media_assist_audio_ai",
        create,
    }
}

fn create(_root: PathBuf) -> Result<Box<dyn ShellDesktopApp>, String> {
    Ok(Box::new(Adapter {
        app: qnc_editorial_desktop::EditorialApp::new(GROUP)?,
    }))
}

struct Adapter {
    app: qnc_editorial_desktop::EditorialApp,
}

impl ShellDesktopApp for Adapter {
    fn show_desktop(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        self.app.show_desktop(ctx, ui);
    }

    fn footer_status(&self) -> Option<&str> {
        Some(self.app.footer_status())
    }
}
