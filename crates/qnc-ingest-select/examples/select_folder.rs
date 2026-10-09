//! Runs the same Select as the Ingest application on one folder of an isolated QNC
//! root (AGENTS 0.4): the active project is read from its database and must lie inside
//! that root; the folder is only read. Usage: select_folder ROOT FOLDER
use qnc_ingest_select::{selection_config::SelectionConfig, Event, SelectSession, SelectTarget};
use qnc_source_reader::SourceReference;
use std::{path::PathBuf, sync::Arc, time::Duration};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (Some(root), Some(folder)) = (args.next(), args.next()) else {
        return Err("usage: select_folder <isolated QNC root> <folder>".into());
    };
    let (root, folder) = (PathBuf::from(root), PathBuf::from(folder));
    std::env::set_var("QNC_ROOT", &root);
    let reader = qnc_active_project_read::ActiveProjectReader::from_root(&root).map_err(|e| e.to_string())?;
    let snapshot = reader.read().map_err(|e| e.to_string())?;
    let settings = reader.settings_reader();
    let project_dir = settings
        .local_workspace_dir(&snapshot.settings)
        .map_err(|e| e.to_string())?
        .ok_or("the project has no local directory")?;
    let canonical = |p: &PathBuf| p.canonicalize().map_err(|e| format!("{}: {e}", p.display()));
    if !canonical(&project_dir)?.starts_with(canonical(&root)?) {
        return Err("refused: the active project is not inside the given root".into());
    }
    let config = SelectionConfig::load(&root).map_err(|e| e.to_string())?;
    let folder = canonical(&folder)?;
    let (source, relative) = config
        .sources
        .iter()
        .find_map(|s| {
            let base = s.location.file.as_ref()?.canonicalize().ok()?;
            let rest = folder.strip_prefix(&base).ok()?;
            let parts: Vec<_> = rest.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
            Some((s.location.uri.clone(), if parts.is_empty() { ".".into() } else { parts.join("/") }))
        })
        .ok_or("the folder is in no registered source")?;
    let selected = SourceReference::new(&source, &relative).map_err(|e| e.to_string())?;
    println!("project {} folder {}", snapshot.settings.project_id, selected.uri());
    let target = SelectTarget::for_project(settings, &snapshot.settings)?;
    let registry = Arc::new(qnc_ingest_cameras::registry()?);
    let mut session = SelectSession::default();
    session.start(config, selected, target, registry)?;
    loop {
        for event in session.poll(64) {
            match event {
                Event::Clip(clip) => println!("clip {} {:.2}s", clip.name, clip.duration_seconds),
                Event::Saved { error: Some(error), .. } | Event::Warning(error) => println!("warning {error}"),
                Event::Finished(result) => {
                    println!("finished {result:?}");
                    return result.map(|_| ());
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
