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

/// Runs while it holds the lease; then, with the lease released, looks at the project
/// database once more: an Uvezi or a Select whose start found the lease held (and ended at
/// once) is never lost, its clips are taken here (user 2026-10-18: a second card's records
/// were never completed). Records that wait again unchanged after a run (their completion
/// failed) do not start another one.
fn run_background(root: &Path) -> Result<(), String> {
    let mut last_records: Vec<String> = Vec::new();
    while run_once(root)? {
        if import_waiting(root)? {
            continue;
        }
        let records = records_waiting(root)?;
        if records.is_empty() || records == last_records {
            break;
        }
        last_records = records;
    }
    Ok(())
}

/// The clips of the active project whose record still waits for its completion.
fn records_waiting(root: &Path) -> Result<Vec<String>, String> {
    let active_project = qnc_active_project_read::ActiveProjectReader::from_root(root)
        .map_err(|error| error.to_string())?;
    let snapshot = active_project.read().map_err(|error| error.to_string())?;
    qnc_content_read::ContentReader::for_project(active_project.settings_reader(), &snapshot.settings)?.records_waiting()
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
    run: &qnc_ingest_import_worker::ImportRun<'_>,
) -> Result<(), String> {
    loop {
        if closed.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }
        let last = !running.load(std::sync::atomic::Ordering::Relaxed);
        if import_waiting(root)? {
            qnc_ingest_import_worker::run_import(root, run)?;
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
    let player_works = qnc_playback_activity::playback_pause(target.clone());
    // Taken in one statement with its owner (external audit 2026-10-17): two starts never
    // both run; a run whose lease another took stops.
    let Some(lease) =
        qnc_playback_activity::Lease::claim(target, qnc_playback_activity::WORKER, qnc_playback_activity::WORKER_FRESH_SECONDS)?
    else {
        return Ok(false);
    };
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
            .ok_or_else(|| qnc_ui_text::text("worker.source_not_connected").to_string())?;
        source.backend(media).map_err(|error| error.to_string())
    };
    let cancel = std::sync::atomic::AtomicBool::new(false);
    // The card sets the pace (user 2026-10-15: waves as soon as possible): first the
    // records (a wave needs its clip's final record), each clip's wave as soon as its record
    // is final, the filmstrip after them. The completion never gives way to the filmstrip.
    let completing = std::sync::atomic::AtomicBool::new(true);
    // The project database is the truth: once its active project is no longer the one
    // this run started for (closed, or another opened), the run stops and lets its
    // database go, so the closed project can be deleted.
    let closed = std::sync::atomic::AtomicBool::new(false);
    // The import belongs to this run: its project, its lease, its cancel.
    let run = qnc_ingest_import_worker::ImportRun { project_id: &started, lease: &lease, cancel: &cancel };
    let artifacts_done = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                if !still_active(&active_project, &started) || !lease.held() {
                    closed.store(true, std::sync::atomic::Ordering::Relaxed);
                    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                    return;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        });
        let import = scope.spawn(|| import_while(root, &running, &closed, &run));
        // The records are completed for as long as this run works: a Select made while it
        // runs (or right after the worker was started for an opened project) adds clips
        // whose records are completed at once (user 2026-10-18: they waited until the
        // filmstrips were done, Play said NotFinal).
        let completion = scope.spawn(|| {
          // The records a round left waiting (its probe failed or was uncertain): only another
          // clip starts a new round (audit 2026-10-25: they started one every second).
          let mut stuck: std::collections::BTreeSet<String> = Default::default();
          loop {
            let done = qnc_record_completion::complete_active_project(
                root,
                &make_backend,
                PROBE_WORKERS,
                &*player_works,
                &cancel,
                Some(lease.write_lease()),
            );
            match done {
                Ok(done) => {
                    for (clip, error) in done.failed {
                        eprintln!("zapis klipa {clip} nije dovrsen: {error}");
                        // Also where the logs are read (live 2026-10-22: an FX3A record failed unseen).
                        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("record-completion failed clip={clip} error={error}"));
                    }
                }
                Err(error) => {
                    eprintln!("dovrsetak zapisa nije uspio: {error}");
                    qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("record-completion failed error={error}"));
                }
            }
            // Once more now the records are final: a clip whose record had no timing at
            // Select gets its poster from the start of the clip.
            if !closed.load(std::sync::atomic::Ordering::Relaxed) {
                make_posters(root, &config, &lease, &cancel);
            }
            completing.store(false, std::sync::atomic::Ordering::Relaxed);
            stuck = records_waiting(root).unwrap_or_default().into_iter().collect();
            // Until the artifacts are done, new records are looked for once a second.
            loop {
                let stop = closed.load(std::sync::atomic::Ordering::Relaxed)
                    || artifacts_done.load(std::sync::atomic::Ordering::Relaxed);
                if stop {
                    return;
                }
                std::thread::sleep(Duration::from_secs(1));
                if records_waiting(root).is_ok_and(|waiting| waiting.iter().any(|clip| !stuck.contains(clip))) {
                    completing.store(true, std::sync::atomic::Ordering::Relaxed);
                    break;
                }
            }
          }
        });
        // The records first: every clip can play (and editing start) before the card is read
        // for waves and filmstrips (user 2026-10-18: speed for editing comes first).
        while completing.load(std::sync::atomic::Ordering::Relaxed) && !closed.load(std::sync::atomic::Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(100));
        }
        let artifacts = run_artifacts(root, &lease, &closed, &completing);
        artifacts_done.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = completion.join();
        if closed.load(std::sync::atomic::Ordering::Relaxed) {
            running.store(false, std::sync::atomic::Ordering::Relaxed);
            let _ = import.join();
            eprintln!("projekt je zatvoren: pozadinski proces staje");
            return Ok(false);
        }
        // Wave needs the audio facts the completion has just written; the filmstrips are
        // already made, so this pass only makes the waves the first one had to skip.
        let artifacts =
            artifacts.and_then(|()| run_artifacts(root, &lease, &closed, &std::sync::atomic::AtomicBool::new(false)));
        running.store(false, std::sync::atomic::Ordering::Relaxed);
        let import = import.join().map_err(|_| qnc_ui_text::text("worker.import_crashed").to_string())?;
        // Once more after the import: a clip imported before its record was final could not
        // get its poster at Uvezi (user 2026-10-18).
        if !closed.load(std::sync::atomic::Ordering::Relaxed) {
            make_posters(root, &config, &lease, &cancel);
        }
        artifacts.and(import).map(|()| true)
    })
}

/// Posters of the clips without one, by their block (`qnc-missing-posters`); where each
/// medium is on this computer comes from the source bindings Select uses.
fn make_posters(
    root: &Path,
    config: &qnc_ingest_select::selection_config::SelectionConfig,
    lease: &qnc_playback_activity::Lease,
    cancel: &std::sync::atomic::AtomicBool,
) {
    let local_media = |uri: &str| {
        let reference = qnc_source_reader::SourceReference::from_uri(uri).ok()?;
        let source = config.sources.iter().find(|source| source.location.uri == reference.source_uri())?;
        source.local_media_path(uri).ok().flatten()
    };
    let project = match qnc_missing_posters::PosterProject::active(root) {
        Ok(project) => project,
        Err(error) => return eprintln!("posteri nisu napravljeni: {error}"),
    };
    match qnc_missing_posters::make_missing_posters(&project, &local_media, Some(lease.write_lease()), cancel) {
        Ok(0) => {}
        Ok(made) => eprintln!("posteri napravljeni: {made}"),
        Err(error) => eprintln!("posteri nisu napravljeni: {error}"),
    }
}

/// Whether the active project in the database is still the one a run started for.
fn still_active(active_project: &qnc_active_project_read::ActiveProjectReader, started: &str) -> bool {
    active_project
        .read()
        .is_ok_and(|snapshot| snapshot.settings.project_id == started)
}

/// Filmstrips and waves of the active project. While `completing`, records still become
/// final: the waves of newly final clips are taken every few seconds and the filmstrip waits.
fn run_artifacts(
    root: &Path,
    lease: &qnc_playback_activity::Lease,
    closed: &std::sync::atomic::AtomicBool,
    completing: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    let active_project = qnc_active_project_read::ActiveProjectReader::from_root(root)
        .map_err(|error| error.to_string())?;
    let snapshot = active_project.read().map_err(|error| error.to_string())?;
    let reader = active_project.settings_reader().clone();
    let settings = snapshot.settings;
    let target = qnc_content_store::ContentTarget::for_project(&reader, &settings)?;
    // Any player of any form announces itself in the project database; the
    // generators give way while it prepares or plays.
    // While the user edits two workers, while the player plays only the open clip's (user
    // 2026-10-20: measured, one core beside a playing player costs nothing).
    let level = qnc_playback_activity::playback_level(target.clone());
    let player_works = || level() == qnc_playback_activity::PlaybackLevel::Playing;
    let mut artifacts = qnc_content_artifacts::ProjectArtifacts::new();
    artifacts.set_host_root(root);
    // Published under the lease, checked by the database owner in the same transaction.
    artifacts.set_write_lease(lease.write_lease());
    artifacts.set_playback_priority(player_works());
    artifacts.set_cap(level().background_cap());
    artifacts.sync(&reader, &settings, target.clone(), false)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(60 * 60);
    let mut shown_checked = std::time::Instant::now() - Duration::from_secs(1);
    let mut synced = std::time::Instant::now();
    loop {
        let still_completing = completing.load(std::sync::atomic::Ordering::Relaxed);
        if !still_completing && !artifacts.has_pending_work() {
            break;
        }
        if closed.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(()); // dropping the artifacts cancels their workers
        }
        // The lists are read again every 3 s while records are completed: a clip whose
        // record became final joins the waves.
        if still_completing && synced.elapsed() >= Duration::from_secs(3) {
            synced = std::time::Instant::now();
            artifacts.sync(&reader, &settings, target.clone(), false)?;
        }
        // The clip a preview has open (written to the project database) goes first.
        if shown_checked.elapsed() >= Duration::from_millis(500) {
            shown_checked = std::time::Instant::now();
            if let Some(clip_id) = qnc_playback_activity::read(&target, qnc_playback_activity::PLAYBACK_CLIP) {
                artifacts.prefer(&clip_id);
            }
        }
        artifacts.set_playback_priority(player_works());
        artifacts.set_cap(level().background_cap());
        let polled = artifacts.poll(None);
        if let Some(error) = polled.error {
            return Err(error);
        }
        if std::time::Instant::now() > deadline {
            return Err(qnc_ui_text::text("worker.artifacts_timeout").into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

/// Probes at once. The working media is USB 3 or faster (user 2026-10-18; USB 2.0 is no
/// working medium): there a probe is bound by the processor, and measured on a stick 8 at
/// once took 0.22 s a clip, 2 at once 0.37 s. The records are done before the artifacts
/// read the card, so the probes have it to themselves.
const PROBE_WORKERS: usize = 6;

fn root_argument() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--root" {
            return args.next().map(PathBuf::from);
        }
    }
    None
}
