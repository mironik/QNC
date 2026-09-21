use std::path::PathBuf;

use qnc_content_read::ContentReader;
use qnc_transport_resolver::ResolvedEndpoint;
use qnc_work_settings::SettingsReader;
use rusqlite::{Connection, OpenFlags};

fn main() -> Result<(), String> {
    let registry = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data/project_store.db"));
    let settings_reader = SettingsReader::local(registry);
    let settings = settings_reader.read().map_err(|error| error.to_string())?;
    let workspace_dir = settings_reader
        .local_workspace_dir(&settings)
        .map_err(|error| error.to_string())?;

    println!("project_id={}", settings.project_id);
    println!("project_name={}", settings.project_name);
    println!("workspace_db_uri={}", settings.workspace_db_uri);
    println!("output_root_uri={}", settings.output_root_uri);
    println!("workspace_dir={}", workspace_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_default());

    let reader = ContentReader::for_project(&settings_reader, &settings)?;
    let binding = settings_reader
        .workspace_binding(&settings)
        .map_err(|error| error.to_string())?;
    let workspace_file = match binding
        .resolver
        .resolve(&settings.workspace_db_uri)
        .map_err(|error| error.to_string())?
        .endpoint
    {
        ResolvedEndpoint::LocalPath(file) => file,
        ResolvedEndpoint::NetworkEndpoint { .. } => {
            return Err("network workspace dump is not supported".into())
        }
    };
    let conn = Connection::open_with_flags(&workspace_file, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| error.to_string())?;
    let counts = |sql: &str| -> Result<i64, String> {
        conn.query_row(sql, [], |row| row.get(0))
            .map_err(|error| error.to_string())
    };
    println!("db_file={}", workspace_file.display());
    println!("db_clips_total={}", counts("SELECT count(*) FROM public_clips")?);
    println!(
        "db_clips_listed={}",
        counts("SELECT count(*) FROM public_clips WHERE selected != 0 OR import_status IN ('queued','processing','original_ready','generating_proxy','imported','done')")?
    );
    println!(
        "db_thumbnail_uri_count={}",
        counts("SELECT count(*) FROM public_clips WHERE thumbnail_uri IS NOT NULL AND thumbnail_uri <> ''")?
    );
    println!(
        "db_filmstrip_artifacts={}",
        counts("SELECT count(*) FROM public_filmstrip_artifacts")?
    );
    println!(
        "db_filmstrip_frames={}",
        counts("SELECT count(*) FROM public_filmstrip_frames")?
    );

    let clips = reader.summaries()?;
    println!("clips={}", clips.len());
    for clip in clips.iter().take(24) {
        let filmstrip = reader.filmstrip(&clip.clip_id)?;
        let filmstrip_frames = filmstrip.as_ref().map(|record| record.frame_count).unwrap_or(0);
        let first_filmstrip_uri = filmstrip
            .as_ref()
            .and_then(|record| record.frames.first())
            .map(|frame| frame.artifact_uri.as_str())
            .unwrap_or("");
        println!(
            "clip id={} name={} status={} imported={} thumb={} media={} filmstrip_frames={} first_filmstrip={}",
            clip.clip_id,
            clip.name,
            clip.import_status,
            clip.imported,
            clip.thumbnail_uri.as_deref().unwrap_or(""),
            clip.imported_media_uri.as_deref().unwrap_or(""),
            filmstrip_frames,
            first_filmstrip_uri
        );
    }
    let mut missing = conn
        .prepare(
            "SELECT c.name, c.clip_id, c.import_status, c.selected
             FROM public_clips c
             LEFT JOIN public_filmstrip_artifacts f ON f.clip_id = c.clip_id
             WHERE (c.selected != 0 OR c.import_status IN ('queued','processing','original_ready','generating_proxy','imported','done'))
               AND f.clip_id IS NULL
             ORDER BY c.name, c.clip_id
             LIMIT 32",
        )
        .map_err(|error| error.to_string())?;
    let rows = missing
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    for row in rows {
        let (name, clip_id, status, selected) = row.map_err(|error| error.to_string())?;
        println!(
            "missing_filmstrip name={} id={} status={} selected={}",
            name, clip_id, status, selected
        );
    }
    if let Some(dir) = workspace_dir {
        let filmstrip_dir = dir.join("filmstrip");
        let mut disk_dirs = std::collections::BTreeSet::new();
        if let Ok(entries) = std::fs::read_dir(&filmstrip_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let count = std::fs::read_dir(&path)
                    .map(|entries| {
                        entries
                            .flatten()
                            .filter(|entry| {
                                entry.path().extension().is_some_and(|ext| {
                                    ext.to_string_lossy().eq_ignore_ascii_case("jpg")
                                })
                            })
                            .count()
                    })
                    .unwrap_or(0);
                if count > 0 {
                    if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                        disk_dirs.insert((name.to_string(), count));
                    }
                }
            }
        }
        let mut db_dirs = std::collections::BTreeSet::new();
        let mut stmt = conn
            .prepare("SELECT artifact_uri FROM public_filmstrip_artifacts")
            .map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        for row in rows {
            let uri = row.map_err(|error| error.to_string())?;
            if let Some(name) = uri.rsplit('/').next() {
                db_dirs.insert(name.to_string());
            }
        }
        println!("disk_filmstrip_dirs={}", disk_dirs.len());
        println!("db_filmstrip_dirs={}", db_dirs.len());
        for (name, count) in disk_dirs.iter().take(128) {
            if !db_dirs.contains(name) {
                println!("disk_without_db_filmstrip dir={} jpg_count={}", name, count);
            }
        }
        for name in db_dirs.iter().take(128) {
            if !disk_dirs.iter().any(|(disk, _)| disk == name) {
                println!("db_without_disk_filmstrip dir={}", name);
            }
        }
    }
    Ok(())
}
