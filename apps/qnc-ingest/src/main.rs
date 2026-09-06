use std::{env, path::PathBuf};

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

    let qnc_root = locate_qnc_root()
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let app = qnc_ingest_desktop::create_ingest_app(qnc_root)
        .unwrap_or_else(|error| panic!("failed to create Ingest app: {error}"));
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("QNC Ingest")
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([1100.0, 700.0])
            .with_maximized(true),
        ..Default::default()
    };

    eframe::run_native("QNC Ingest", options, Box::new(|_| Ok(Box::new(app))))
}

fn locate_qnc_root() -> Option<PathBuf> {
    let mut starts = Vec::new();
    if let Ok(exe) = env::current_exe() {
        starts.push(exe);
    }
    if let Ok(cwd) = env::current_dir() {
        starts.push(cwd);
    }

    for start in starts {
        let mut current = if start.is_file() {
            start.parent().map(PathBuf::from)
        } else {
            Some(start)
        };
        while let Some(dir) = current {
            if is_qnc_root(&dir) {
                return Some(dir);
            }
            current = dir.parent().map(PathBuf::from);
        }
    }

    None
}

fn is_qnc_root(dir: &std::path::Path) -> bool {
    dir.join("AGENTS.md").is_file()
        && dir
            .join("contracts")
            .join("ui")
            .join("ingest.layout.json")
            .is_file()
        && dir
            .join("contracts")
            .join("applications")
            .join("ingest.application.json")
            .is_file()
        && dir
            .join("contracts")
            .join("qnc-keyboard-shortcuts.json")
            .is_file()
}
