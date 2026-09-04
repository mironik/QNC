use std::{env, path::PathBuf, process};

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

fn resolve_qnc_root() -> Result<PathBuf, String> {
    let mut starts = Vec::new();
    if let Ok(path) = env::current_exe() {
        if let Some(parent) = path.parent() {
            starts.push(parent.to_path_buf());
        }
    }
    if let Ok(path) = env::current_dir() {
        starts.push(path);
    }

    for start in starts {
        for candidate in start.ancestors() {
            if is_qnc_root(candidate) {
                return Ok(candidate.to_path_buf());
            }
        }
    }

    Err("Ne mogu pronaći QNC root s AGENTS.md, seed/system_seed.json i contracts/ui/project.layout.json.".to_string())
}

fn is_qnc_root(path: &std::path::Path) -> bool {
    path.join("AGENTS.md").is_file()
        && path.join("seed").join("system_seed.json").is_file()
        && path
            .join("contracts")
            .join("ui")
            .join("project.layout.json")
            .is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, time::SystemTime};

    #[test]
    fn qnc_root_requires_agents_seed_and_project_layout() {
        let root = env::temp_dir().join(format!(
            "qnc_root_resolver_{}_{}",
            process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(root.join("seed")).expect("seed dir");
        fs::create_dir_all(root.join("contracts").join("ui")).expect("ui dir");
        fs::write(root.join("AGENTS.md"), "").expect("agents");
        fs::write(root.join("seed").join("system_seed.json"), "{}").expect("seed");
        fs::write(
            root.join("contracts")
                .join("ui")
                .join("project.layout.json"),
            "{}",
        )
        .expect("layout");

        assert!(is_qnc_root(&root));

        fs::remove_file(root.join("AGENTS.md")).expect("remove agents");
        assert!(!is_qnc_root(&root));
        let _ = fs::remove_dir_all(root);
    }
}
