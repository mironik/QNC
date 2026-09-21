const GROUP: &str = "g";

fn main() -> eframe::Result<()> {
    if std::env::args().any(|arg| arg == "--check-contracts") {
        match qnc_editorial_desktop::check_contracts_message(GROUP) {
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

    let root = qnc_editorial_desktop::locate_qnc_root()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let app = qnc_editorial_desktop::EditorialApp::new(GROUP, root)
        .unwrap_or_else(|error| panic!("failed to create QNC Media Assist Audio: {error}"));
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("QNC Media Assist Audio")
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([1100.0, 700.0])
            .with_maximized(true),
        ..Default::default()
    };

    eframe::run_native(
        "QNC Media Assist Audio",
        options,
        Box::new(|_| Ok(Box::new(app))),
    )
}
