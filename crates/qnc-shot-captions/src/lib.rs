//! The captions of the shots (user 2026-11-10: Florence-2 as the second AI engine; the AI agents
//! are an add-on, their growing data is not put into the project database). A sentence a model
//! wrote of what a shot shows is kept in the captions database of this computer
//! (`data/qnc-shot-captions.db`, of which this block is the only writer), under what the shot is
//! (its project's public URI, clip and frames: the key of `qnc-shot-examples`) and the model.
//! It loads no model and reads no media.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags};

pub const MODULE_ID: &str = "qnc.module.shot-captions";

/// The captions database of a QNC root.
pub const DATABASE: &str = "data/qnc-shot-captions.db";

/// The longest caption kept.
const LONGEST: usize = 2000;

/// What a shot is: its project (its public URI), clip and frames.
pub fn key(project_uri: &str, clip_id: &str, from: u64, to: u64) -> String {
    format!("{project_uri}|{clip_id}|{from}|{to}")
}

/// A shot's caption: its frames in its clip and the sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caption {
    pub from: u64,
    pub to: u64,
    pub text: String,
}

/// The captions database of `root`.
pub fn path(root: &Path) -> PathBuf {
    root.join(DATABASE)
}

/// The keys of the shots of `project_uri` that have a caption of `model`.
pub fn captioned_here(root: &Path, model: &str, project_uri: &str) -> Result<Vec<String>, String> {
    let Some(conn) = open_read(root)? else { return Ok(Vec::new()) };
    let mut statement = conn.prepare("SELECT shot_key FROM public_shot_captions WHERE model = ?1 AND shot_key LIKE ?2 ESCAPE '\\'").map_err(err)?;
    let rows = statement.query_map(params![model, format!("{}|%", like(project_uri))], |r| r.get::<_, String>(0)).map_err(err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(err)
}

/// The captions of `model` of a clip of `project_uri`, by its frames.
pub fn read_clip_here(root: &Path, model: &str, project_uri: &str, clip_id: &str) -> Result<Vec<Caption>, String> {
    let Some(conn) = open_read(root)? else { return Ok(Vec::new()) };
    let prefix = format!("{project_uri}|{clip_id}|");
    let mut statement = conn.prepare("SELECT shot_key, caption FROM public_shot_captions WHERE model = ?1 AND shot_key LIKE ?2 ESCAPE '\\'").map_err(err)?;
    let rows = statement.query_map(params![model, format!("{}%", like(&prefix))], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).map_err(err)?;
    let mut captions = Vec::new();
    for row in rows {
        let (shot, text) = row.map_err(err)?;
        // Only "from|to" after the clip (a clip whose id goes on is another clip).
        let Some((from, to)) = shot.strip_prefix(&prefix).and_then(|frames| frames.split_once('|')) else { continue };
        if let (Ok(from), Ok(to)) = (from.parse(), to.parse()) {
            captions.push(Caption { from, to, text });
        }
    }
    captions.sort_by_key(|caption| (caption.from, caption.to));
    Ok(captions)
}

/// Keeps the caption `text` of `model` of the shot `shot_key`, replacing what was kept for it.
pub fn save_here(root: &Path, model: &str, shot_key: &str, text: &str) -> Result<(), String> {
    let text = text.trim();
    if model.is_empty() || shot_key.matches('|').count() < 3 || text.is_empty() || text.len() > LONGEST {
        return Err(qnc_ui_text::text("db.shot_caption_invalid").into());
    }
    let file = path(root);
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    let conn = Connection::open(&file).map_err(err)?;
    conn.busy_timeout(std::time::Duration::from_secs(5)).map_err(err)?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS shot_captions (model TEXT NOT NULL, shot_key TEXT NOT NULL, caption TEXT NOT NULL, made_at_unix_ms INTEGER NOT NULL, PRIMARY KEY (model, shot_key));
         CREATE VIEW IF NOT EXISTS public_shot_captions AS SELECT model, shot_key, caption, made_at_unix_ms FROM shot_captions;",
    )
    .map_err(err)?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
    conn.execute(
        "INSERT INTO shot_captions (model, shot_key, caption, made_at_unix_ms) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(model, shot_key) DO UPDATE SET caption=excluded.caption, made_at_unix_ms=excluded.made_at_unix_ms",
        params![model, shot_key, text, now],
    )
    .map_err(err)?;
    Ok(())
}

fn open_read(root: &Path) -> Result<Option<Connection>, String> {
    let file = path(root);
    if !file.is_file() {
        return Ok(None);
    }
    let conn = Connection::open_with_flags(&file, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(err)?;
    conn.busy_timeout(std::time::Duration::from_secs(5)).map_err(err)?;
    let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='public_shot_captions')", [], |r| r.get(0)).map_err(err)?;
    Ok(exists.then_some(conn))
}

/// `text` as a literal of a LIKE pattern.
fn like(text: &str) -> String {
    text.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_caption_is_kept_on_this_computer_by_its_shot_and_model() {
        let root = tempfile::tempdir().unwrap();
        assert!(read_clip(root.path(), "m", "qnc://p_1", "a").unwrap().is_empty(), "nothing kept yet");
        save(root.path(), "m", &key("qnc://p_1", "a", 10, 50), "A swimming pool.").unwrap();
        save(root.path(), "m", &key("qnc://p_1", "a", 0, 10), "A fountain.").unwrap();
        save(root.path(), "m", &key("qnc://p_1", "ab", 0, 10), "Another clip.").unwrap();
        save(root.path(), "m", &key("qnc://p_12", "a", 0, 10), "Another project.").unwrap();
        save(root.path(), "m", &key("qnc://p_1", "a", 10, 50), "A pool and trees.").unwrap();
        let clip = read_clip(root.path(), "m", "qnc://p_1", "a").unwrap();
        assert_eq!(clip, vec![Caption { from: 0, to: 10, text: "A fountain.".into() }, Caption { from: 10, to: 50, text: "A pool and trees.".into() }]);
        assert!(read_clip(root.path(), "other", "qnc://p_1", "a").unwrap().is_empty());
        assert_eq!(captioned(root.path(), "m", "qnc://p_1").unwrap().len(), 3);
        assert!(save(root.path(), "m", "not a key", "x").is_err());
        assert!(save(root.path(), "m", &key("qnc://p_1", "a", 0, 1), "  ").is_err());
    }
}

/// The keys of the shots of `project_uri` that have a caption of `model`: on the AI server of
/// this computer when it names one (docs/102 step 7b: common to every QNC that uses it), else in
/// the captions database of `root`.
pub fn captioned(root: &Path, model: &str, project_uri: &str) -> Result<Vec<String>, String> {
    match qnc_ai_place::server_of(root)? {
        Some(server) => qnc_ai_store::captioned(&qnc_ai_store::on(&server), model, project_uri),
        None => captioned_here(root, model, project_uri),
    }
}

/// The captions of `model` of a clip of `project_uri`: on the AI server, else here.
pub fn read_clip(root: &Path, model: &str, project_uri: &str, clip_id: &str) -> Result<Vec<Caption>, String> {
    match qnc_ai_place::server_of(root)? {
        Some(server) => Ok(qnc_ai_store::read_captions(&qnc_ai_store::on(&server), model, project_uri, clip_id)?
            .into_iter()
            .map(|caption| Caption { from: caption.from, to: caption.to, text: caption.text })
            .collect()),
        None => read_clip_here(root, model, project_uri, clip_id),
    }
}

/// Keeps the caption of a shot: on the AI server, else here.
pub fn save(root: &Path, model: &str, shot_key: &str, text: &str) -> Result<(), String> {
    match qnc_ai_place::server_of(root)? {
        Some(server) => qnc_ai_store::save_caption(&qnc_ai_store::on(&server), model, shot_key, text),
        None => save_here(root, model, shot_key, text),
    }
}
