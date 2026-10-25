use std::{path::PathBuf, process};

use eframe::egui;

fn main() -> eframe::Result<()> {
    if std::env::args().any(|arg| arg == "--check-contracts") {
        let message = qnc_project_desktop::check_contracts_message().unwrap_or_else(|error| {
            eprintln!("qnc-project contract error: {error}");
            process::exit(1);
        });
        println!("{message}");
        return Ok(());
    }

    let project_root = resolve_qnc_root().unwrap_or_else(|error| {
        eprintln!("qnc-project root error: {error}");
        process::exit(1);
    });
    let app = match qnc_project_desktop::create_project_app(project_root) {
        Ok(app) => app,
        Err(error) => {
            eprintln!("qnc-project app error: {error}");
            process::exit(1);
        }
    };

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_min_inner_size([1100.0, 700.0])
            .with_maximized(true)
            .with_title("QNC Project"),
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "QNC Project",
        options,
        Box::new(move |cc| {
            if let Err(error) = qnc_project_desktop::apply_project_style(&cc.egui_ctx) {
                eprintln!("qnc-project style error: {error}");
                process::exit(1);
            }
            Ok(Box::new(app))
        }),
    )
}

/// The QNC root (`qnc-dev-diagnostics` holds the one rule).
fn resolve_qnc_root() -> Result<PathBuf, String> {
    qnc_dev_diagnostics::resolve_qnc_root().ok_or_else(|| qnc_ui_text::text("project.root_not_found").to_string())
}
