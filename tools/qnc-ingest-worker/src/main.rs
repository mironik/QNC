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
    let active_project = qnc_active_project_read::ActiveProjectReader::from_root(root)
        .map_err(|error| error.to_string())?;
    let snapshot = active_project.read().map_err(|error| error.to_string())?;
    let reader = active_project.settings_reader().clone();
    let target = qnc_content_store::ContentTarget::for_project(&reader, &snapshot.settings)?;
    // One worker per project: the lease covers the import and the artifacts, so a
    // second start while the artifacts are still built finds it and ends at once.
    if qnc_playback_activity::is_fresh(
        &target,
        qnc_playback_activity::WORKER,
        qnc_playback_activity::WORKER_FRESH_SECONDS,
    ) {
        return Ok(());
    }
    let _lease = qnc_playback_activity::Beat::start(target, qnc_playback_activity::WORKER)?;
    qnc_ingest_import_worker::run_import(root)?;
    run_artifacts(root)
}

fn run_artifacts(root: &Path) -> Result<(), String> {
    let active_project = qnc_active_project_read::ActiveProjectReader::from_root(root)
        .map_err(|error| error.to_string())?;
    let snapshot = active_project.read().map_err(|error| error.to_string())?;
    let reader = active_project.settings_reader().clone();
    let settings = snapshot.settings;
    let target = qnc_content_store::ContentTarget::for_project(&reader, &settings)?;
    // Any player of any form announces itself in the project database; the
    // generators give way while it prepares or plays.
    let player_works = qnc_playback_activity::playback_pause(target.clone());
    let mut artifacts = qnc_content_artifacts::ProjectArtifacts::new();
    artifacts.set_host_root(root);
    artifacts.set_playback_priority(player_works());
    artifacts.sync(&reader, &settings, target, false)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(60 * 60);
    while artifacts.has_pending_work() {
        artifacts.set_playback_priority(player_works());
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
