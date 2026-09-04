use std::path::PathBuf;

use eframe::egui;

pub trait ShellDesktopApp {
    fn show_desktop(&mut self, ctx: &egui::Context, ui: &mut egui::Ui);
}

#[derive(Clone, Copy)]
pub struct EmbeddedAppFactory {
    pub desktop_entry: &'static str,
    pub create: fn(PathBuf) -> Result<Box<dyn ShellDesktopApp>, String>,
}
