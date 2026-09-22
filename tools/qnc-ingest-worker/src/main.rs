//! Background application started by Uvezi. It reads the settings of the active project
//! once, takes the queued clips of the selection and does what the settings say (link
//! copies nothing, proxy or original copies), then ends. Its lease, the pause while a
//! player works and its result are kept in the project database, nowhere else.

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

fn main() -> ExitCode {
    let Some(root) = root_argument() else {
        eprintln!("Upotreba: qnc-ingest-worker --root <QNC korijen>");
        return ExitCode::from(2);
    };
    match run_background(&root) {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("pozadinski proces nije uspio: {error}");
            ExitCode::from(1)
        }
    }
}

fn run_background(root: &Path) -> Result<(), String> {
    qnc_ingest_import_worker::run_service(root)?;
    run_artifacts(root)
}

fn run_artifacts(root: &Path) -> Result<(), String> {
    let reader =
        qnc_work_settings::SettingsReader::from_root(root).map_err(|error| error.to_string())?;
    let settings = reader.read().map_err(|error| error.to_string())?;
    let target = qnc_content_store::ContentTarget::for_project(&reader, &settings)?;
    let mut artifacts = qnc_content_artifacts::ProjectArtifacts::new();
    artifacts.set_host_root(root);
    artifacts.sync(&reader, &settings, target, false)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(60 * 60);
    while artifacts.has_pending_work() {
        let polled = artifacts.poll(None);
        if let Some(error) = polled.error {
            return Err(error);
        }
        if std::time::Instant::now() > deadline {
            return Err("Filmstrip/wave pozadinski proces nije zavrsio u roku.".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

fn root_argument() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--root" {
            return args.next().map(PathBuf::from);
        }
    }
    None
}
