//! Does an application let go of the project database when the project is closed (user
//! 2026-10-09: a closed project could not be deleted while QNC ran)? On an isolated QNC
//! root only (AGENTS 0.4). The editorial application (Story, Media Assist) opens a clip in
//! its preview, the project is closed through the public close component, the application
//! is left as it is (a hidden surface gets no call), and after a few seconds the project
//! database is renamed and back: Windows refuses while any handle is open. The pieces that
//! hold the database (write transports, the database intermediary, shared readers) are the
//! same in Ingest. Usage: qnc-release-check ROOT
use qnc_editorial_application::{EditorialApplication, EditorialIntent};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn main() -> Result<(), String> {
    let root = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("usage: qnc-release-check <isolated QNC root>")?,
    );
    std::env::set_var("QNC_ROOT", &root);
    let dir = isolated_project(&root)?;
    let mut app = EditorialApplication::new(&root);
    let clip = wait(&mut app, Duration::from_secs(20), |app| {
        app.view().clips.first().map(|clip| clip.clip_id.clone())
    })
    .ok_or("no clip shown")?;
    app.dispatch(EditorialIntent::PreviewClip(clip.clone()));
    wait(&mut app, Duration::from_secs(5), |_| None::<()>);
    println!("opened {clip}: {}", app.footer_status());
    qnc_project_close::CloseProjectComponent::from_root(&root).close_active_project()?;
    wait(&mut app, Duration::from_secs(3), |_| None::<()>);
    let db = dir.join("project.db");
    let moved = dir.join("project.db.release-check");
    match std::fs::rename(&db, &moved) {
        Ok(()) => {
            std::fs::rename(&moved, &db).map_err(|e| e.to_string())?;
            println!("released: the project database is not held");
            Ok(())
        }
        Err(error) => Err(format!("HELD: {error}")),
    }
}

/// The directory of the active project, refused when it is not inside `root`.
fn isolated_project(root: &Path) -> Result<PathBuf, String> {
    let reader = qnc_active_project_read::ActiveProjectReader::from_root(root).map_err(|e| e.to_string())?;
    let snapshot = reader.read().map_err(|e| e.to_string())?;
    let dir = reader
        .settings_reader()
        .local_workspace_dir(&snapshot.settings)
        .map_err(|e| e.to_string())?
        .ok_or("the project has no local directory")?;
    let canonical = |p: &Path| p.canonicalize().map_err(|e| format!("{}: {e}", p.display()));
    if !canonical(&dir)?.starts_with(canonical(root)?) {
        return Err("refused: the active project is not inside the given root (use an isolated copy)".into());
    }
    Ok(dir)
}

/// Polls the application until `found` gives a value or the time is up.
fn wait<T>(
    app: &mut EditorialApplication,
    time: Duration,
    found: impl Fn(&EditorialApplication) -> Option<T>,
) -> Option<T> {
    let until = Instant::now() + time;
    while Instant::now() < until {
        app.poll();
        if let Some(value) = found(app) {
            return Some(value);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}
