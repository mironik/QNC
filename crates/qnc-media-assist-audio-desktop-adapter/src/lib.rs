use std::path::PathBuf;

use eframe::egui;
use qnc_shell_desktop_api::{EmbeddedAppFactory, ShellDesktopApp};

const GROUP: &str = "g";

pub fn factory() -> EmbeddedAppFactory {
    EmbeddedAppFactory {
        desktop_entry: "qnc_media_assist_audio",
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
        self.app.show_desktop(ctx, ui, &mut qnc_board::Frame::bare());
    }

    fn show_in_frame(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, frame: &mut qnc_board::Frame<'_>) {
        self.app.show_desktop(ctx, ui, frame);
    }

    fn on_activated(&mut self) {
        self.app.set_active(true);
    }

    fn on_deactivated(&mut self) {
        self.app.set_active(false);
    }

    fn footer_status(&self) -> Option<&str> {
        Some(self.app.footer_status())
    }
}
