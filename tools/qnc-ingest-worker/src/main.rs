//! Background application started by Uvezi. It reads the settings of the active project
//! once, takes the queued clips of the selection and does what the settings say (link
//! copies nothing, proxy or original copies), then ends. Its lease, the pause while a
//! player works and its result are kept in the project database, nowhere else.

use std::{path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let Some(root) = root_argument() else {
        eprintln!("Upotreba: qnc-ingest-worker --root <QNC korijen>");
        return ExitCode::from(2);
    };
    match qnc_ingest_import_worker::run_service(&root) {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("uvoz nije uspio: {error}");
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
