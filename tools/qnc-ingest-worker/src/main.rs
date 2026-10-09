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

/// Runs while it holds the lease; then, with the lease released, looks at the import
/// queue once more: an Uvezi whose start found the lease held (and ended at once) is
/// never lost, it is taken here.
fn run_background(root: &Path) -> Result<(), String> {
    while run_once(root)? {
        if !import_waiting(root)? {
            break;
        }
    }
    Ok(())
}

/// Whether clips wait in the import queue of the active project.
fn import_waiting(root: &Path) -> Result<bool, String> {
    let active_project = qnc_active_project_read::ActiveProjectReader::from_root(root)
        .map_err(|error| error.to_string())?;
    let snapshot = active_project.read().map_err(|error| error.to_string())?;
    let reader = qnc_content_read::ContentReader::for_project(
        active_project.settings_reader(),
        &snapshot.settings,
    )?;
    Ok(reader
        .summaries()?
        .iter()
        .any(|clip| clip.import_status == "queued"))
}

/// The import queue as the project database has it, looked at every second while the
/// other work of this run goes on (Uvezi may come at any time) and once more when it
/// has ended; an import runs only when clips wait, so its result stays the last real one.
fn import_while(
    root: &Path,
    running: &std::sync::atomic::AtomicBool,
    closed: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    loop {
        if closed.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }
        let last = !running.load(std::sync::atomic::Ordering::Relaxed);
        if import_waiting(root)? {
            qnc_ingest_import_worker::run_import(root)?;
        }
        if last {
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// One run under the lease; false when another worker holds it.
fn run_once(root: &Path) -> Result<bool, String> {
    let active_project = qnc_active_project_read::ActiveProjectReader::from_root(root)
        .map_err(|error| error.to_string())?;
    let snapshot = active_project.read().map_err(|error| error.to_string())?;
    let started = snapshot.settings.project_id.clone();
    let reader = active_project.settings_reader().clone();
    let target = qnc_content_store::ContentTarget::for_project(&reader, &snapshot.settings)?;
    // One worker per project: the lease covers the import and the artifacts, so a
    // second start while the artifacts are still built finds it and ends at once.
    if qnc_playback_activity::is_fresh(
        &target,
        qnc_playback_activity::WORKER,
        qnc_playback_activity::WORKER_FRESH_SECONDS,
    ) {
        return Ok(false);
    }
    let player_works = qnc_playback_activity::playback_pause(target.clone());
    let _lease = qnc_playback_activity::Beat::start(target, qnc_playback_activity::WORKER)?;
    let running = std::sync::atomic::AtomicBool::new(true);
    // v5 media probe job: a card record gets what playback lacks, once, in the
    // background, so a preview of a clip not yet imported plays. Beside the artifacts
    // only the clip a preview wants; the others after them, several at once.
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
    // The card sets the pace: while filmstrip and wave read it, the completion takes
    // only the clip a preview wants, the rest right after them.
    let artifacts_running = std::sync::atomic::AtomicBool::new(true);
    let card_busy = || artifacts_running.load(std::sync::atomic::Ordering::Relaxed);
    // The project database is the truth: once its active project is no longer the one
    // this run started for (closed, or another opened), the run stops and lets its
    // database go, so the closed project can be deleted.
    let closed = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                if !still_active(&active_project, &started) {
                    closed.store(true, std::sync::atomic::Ordering::Relaxed);
                    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                    return;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        });
        let import = scope.spawn(|| import_while(root, &running, &closed));
        let completion = scope.spawn(|| {
            let done = qnc_record_completion::complete_active_project(
                root,
                &make_backend,
                PROBE_WORKERS,
                &*player_works,
                &card_busy,
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
        let artifacts = run_artifacts(root, &closed);
        artifacts_running.store(false, std::sync::atomic::Ordering::Relaxed);
        let _ = completion.join();
        if closed.load(std::sync::atomic::Ordering::Relaxed) {
            running.store(false, std::sync::atomic::Ordering::Relaxed);
            let _ = import.join();
            eprintln!("projekt je zatvoren: pozadinski proces staje");
            return Ok(false);
        }
        // Posters for clips whose source has none (a single file): their records are
        // final now, so the start of the clip is known.
        match qnc_ingest_import_worker::make_missing_posters(root) {
            Ok(0) => {}
            Ok(made) => eprintln!("posteri napravljeni: {made}"),
            Err(error) => eprintln!("posteri nisu napravljeni: {error}"),
        }
        // Wave needs the audio facts the completion has just written; the filmstrips are
        // already made, so this pass only makes the waves the first one had to skip.
        let artifacts = artifacts.and_then(|()| run_artifacts(root, &closed));
        running.store(false, std::sync::atomic::Ordering::Relaxed);
        let import = import.join().map_err(|_| "Uvoz je pao.".to_string())?;
        artifacts.and(import).map(|()| true)
    })
}

/// Whether the active project in the database is still the one a run started for.
fn still_active(active_project: &qnc_active_project_read::ActiveProjectReader, started: &str) -> bool {
    active_project
        .read()
        .is_ok_and(|snapshot| snapshot.settings.project_id == started)
}

fn run_artifacts(root: &Path, closed: &std::sync::atomic::AtomicBool) -> Result<(), String> {
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
        if closed.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(()); // dropping the artifacts cancels their workers
        }
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
