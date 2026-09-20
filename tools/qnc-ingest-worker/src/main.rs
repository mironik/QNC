//! Background application started by Uvezi. It reads the settings of the active project
//! again in every turn, takes the queued clips of the selection and does what the
//! settings say (link copies nothing, proxy or original copies), then ends after it has
//! been idle for a while. There is only one of it. While a player works on this machine
//! it waits, and it writes what happened to a log next to the data.

use std::{
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

/// How long the worker stays when there is nothing to do.
const IDLE_EXIT: Duration = Duration::from_secs(60);

fn main() -> ExitCode {
    let Some(root) = root_argument() else {
        eprintln!("Upotreba: qnc-ingest-worker --root <QNC korijen>");
        return ExitCode::from(2);
    };
    if qnc_ingest_import_worker::is_running(&root) {
        return ExitCode::SUCCESS;
    }
    qnc_ingest_import_worker::touch_lock(&root);
    let pause = Arc::new(AtomicBool::new(false));
    let (watcher, keep) = (pause.clone(), root.clone());
    std::thread::spawn(move || loop {
        qnc_ingest_import_worker::touch_lock(&keep);
        watcher.store(qnc_playback_marker::is_active(), Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(500));
    });
    let result = qnc_ingest_import_worker::run_service(&root, pause, IDLE_EXIT);
    qnc_ingest_import_worker::remove_lock(&root);
    match result {
        Ok(summary) => {
            log(
                &root,
                &format!(
                    "uvoz gotov: {} uvezeno, {} neuspjelo",
                    summary.imported, summary.failed
                ),
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            log(&root, &format!("uvoz nije uspio: {error}"));
            ExitCode::from(1)
        }
    }
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

fn log(root: &Path, line: &str) {
    let directory = root.join("data").join("diagnostics");
    let _ = std::fs::create_dir_all(&directory);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("ingest-worker.log"))
    {
        let _ = writeln!(file, "{line}");
    }
}
