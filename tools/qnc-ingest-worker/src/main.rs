//! Background application started by Select and by Uvezi. It reads the settings of the
//! active project once, takes the queued clips of the selection and does what the
//! settings say (link copies nothing, proxy or original copies), completes the card
//! records that playback still lacks something of (v5 media probe), builds the
//! artifacts, then ends. Its lease, the pause while a
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
    let player_works = qnc_playback_activity::playback_pause(target.clone());
    let _lease = qnc_playback_activity::Beat::start(target, qnc_playback_activity::WORKER)?;
    qnc_ingest_import_worker::run_import(root)?;
    // v5 media probe job: a card record gets what playback lacks, once, in the
    // background, so a preview of a clip not yet imported plays. It runs beside the
    // artifacts, several clips at once, the clip a preview wants first.
    let config = qnc_ingest_select::selection_config::SelectionConfig::load(root)
        .map_err(|error| error.to_string())?;
    let make_backend = |source_uri: &str, media: &[qnc_source_reader::SourceReference]| {
        let source = config
            .sources
            .iter()
            .find(|source| source.location.uri == source_uri)
            .ok_or("Izvor klipa nije spojen na ovo racunalo.")?;
        source.backend(media).map_err(|error| error.to_string())
    };
    let cancel = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let done = qnc_record_completion::complete_active_project(
                root,
                &make_backend,
                PROBE_WORKERS,
                &*player_works,
                &cancel,
            );
            match done {
                Ok(done) => {
                    for (clip, error) in done.failed {
                        eprintln!("zapis klipa {clip} nije dovrsen: {error}");
                    }
                }
                Err(error) => eprintln!("dovrsetak zapisa nije uspio: {error}"),
            }
        });
        run_artifacts(root)
    })
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

/// Probes read the card; the card, not the processor, sets the pace (about 0.5 s per
/// MXF). Two at once keep it busy without crowding the filmstrip reads into timeouts.
const PROBE_WORKERS: usize = 2;

fn root_argument() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--root" {
            return args.next().map(PathBuf::from);
        }
    }
    None
}
