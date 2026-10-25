use std::env;

fn main() -> eframe::Result<()> {
    if env::args().any(|arg| arg == "--check-contracts") {
        match qnc_ingest_desktop::check_contracts_message() {
            Ok(message) => {
                println!("{message}");
                return Ok(());
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(2);
            }
        }
    }

    // The one root rule (`qnc-dev-diagnostics`): never the working directory by default.
    let qnc_root = qnc_dev_diagnostics::resolve_qnc_root().unwrap_or_else(|| {
        eprintln!("{}", qnc_ui_text::text("shell.root_not_found"));
        std::process::exit(2);
    });
    let app = qnc_ingest_desktop::create_ingest_app(qnc_root)
        .unwrap_or_else(|error| panic!("failed to create Ingest app: {error}"));
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("QNC Ingest")
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([1100.0, 700.0])
            .with_maximized(true),
        ..Default::default()
    };

    eframe::run_native("QNC Ingest", options, Box::new(|_| Ok(Box::new(app))))
}
