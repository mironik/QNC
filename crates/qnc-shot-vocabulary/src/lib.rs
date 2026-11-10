//! The vocabulary of what a shot may show (user 2026-11-10: "the model compares, it names
//! nothing"; the new card was sorted badly because the list of labels had no word for what it
//! showed). A list of about 400 places and things (`catalogs/ai/shot-vocabulary.json`: the 365
//! scenes of Places365 and what a newsroom films) is asked of the model once; the text vectors
//! are kept in the vocabulary database of this computer (`data/qnc-shot-vocabulary.db`, of which
//! this block is the only writer), not in a project: the AI agents are an add-on and the
//! vocabulary is the same for every project. A shot's best terms are worked out from its vector
//! when it is read. It loads no model and reads no media.

use std::path::{Path, PathBuf};

use qnc_shot_content_db::LabelSet;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};

pub const MODULE_ID: &str = "qnc.module.shot-vocabulary";

/// The vocabulary database of a QNC root.
pub const DATABASE: &str = "data/qnc-shot-vocabulary.db";

/// The vocabulary beside the application.
const LIST: &str = "catalogs/ai/shot-vocabulary.json";

/// The vocabulary list beside the application (a list of the labels' form).
pub fn installed_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    exe.parent()
        .into_iter()
        .flat_map(Path::ancestors)
        .map(|folder| folder.join(LIST))
        .find(|path| path.is_file())
        .ok_or_else(|| format!("{LIST} is not beside the application"))
}

/// The vocabulary list.
pub fn installed() -> Result<qnc_shot_labels::Labels, String> {
    qnc_shot_labels::Labels::read(&installed_path()?)
}

/// Whether `set` holds the vectors of `list` (the same terms and prompts, in order).
pub fn same(set: &LabelSet, list: &qnc_shot_labels::Labels) -> bool {
    set.labels.len() == list.labels.len() && set.labels.iter().zip(&list.labels).all(|(kept, term)| kept.code == term.code && kept.prompt == term.prompt)
}

/// The vectors of the vocabulary of `model` kept on this computer; none while not made.
pub fn read(root: &Path, model: &str) -> Result<Option<LabelSet>, String> {
    let file = root.join(DATABASE);
    if !file.is_file() {
        return Ok(None);
    }
    let conn = Connection::open_with_flags(&file, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(err)?;
    conn.busy_timeout(std::time::Duration::from_secs(5)).map_err(err)?;
    let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='public_shot_vocabulary')", [], |r| r.get(0)).map_err(err)?;
    if !exists {
        return Ok(None);
    }
    let json: Option<String> = conn.query_row("SELECT set_json FROM public_shot_vocabulary WHERE model = ?1", params![model], |r| r.get(0)).optional().map_err(err)?;
    json.map(|json| serde_json::from_str(&json).map_err(err)).transpose()
}

/// Keeps the vectors of the vocabulary of `set.model`, replacing what was kept for it.
pub fn save(root: &Path, set: &LabelSet) -> Result<(), String> {
    let width = set.labels.first().map_or(0, |term| term.vector.len());
    let valid = !set.model.is_empty() && !set.labels.is_empty() && set.scale.is_finite() && set.bias.is_finite() && set.labels.iter().all(|term| !term.code.is_empty() && term.vector.len() == width && width > 0 && term.vector.iter().all(|v| v.is_finite()));
    if !valid {
        return Err(qnc_ui_text::text("db.shot_vocabulary_invalid").into());
    }
    let file = root.join(DATABASE);
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    let mut conn = Connection::open(&file).map_err(err)?;
    conn.busy_timeout(std::time::Duration::from_secs(5)).map_err(err)?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS shot_vocabulary (model TEXT PRIMARY KEY, set_json TEXT NOT NULL, made_at_unix_ms INTEGER NOT NULL);
         CREATE VIEW IF NOT EXISTS public_shot_vocabulary AS SELECT model, set_json, made_at_unix_ms FROM shot_vocabulary;",
    )
    .map_err(err)?;
    let json = serde_json::to_string(set).map_err(err)?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
    let tx = conn.transaction().map_err(err)?;
    tx.execute(
        "INSERT INTO shot_vocabulary (model, set_json, made_at_unix_ms) VALUES (?1, ?2, ?3)
         ON CONFLICT(model) DO UPDATE SET set_json=excluded.set_json, made_at_unix_ms=excluded.made_at_unix_ms",
        params![set.model, json, now],
    )
    .map_err(err)?;
    tx.commit().map_err(err)
}

/// The `n` terms of the vocabulary a picture's vector fits best, best first: (code, share among
/// all terms).
pub fn best_terms(set: &LabelSet, list: &qnc_shot_labels::Labels, vector: &[f32], n: usize) -> Vec<(String, f32)> {
    let texts = qnc_shot_content::Texts { vectors: set.labels.iter().map(|term| term.vector.clone()).collect(), scale: set.scale, bias: set.bias };
    if !same(set, list) || set.labels.first().is_none_or(|term| term.vector.len() != vector.len()) {
        return Vec::new();
    }
    list.shares(&texts.logits(vector)).into_iter().take(n).collect()
}

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_shot_content_db::LabelVector;

    fn list() -> qnc_shot_labels::Labels {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vocabulary.json");
        std::fs::write(&path, r#"{"version":1,"least":0.0,"labels":[
            {"code":"pool","text":"vocab.pool","prompt":"a photo of a pool"},
            {"code":"harbor","text":"vocab.harbor","prompt":"a photo of a harbor"},
            {"code":"book","text":"vocab.book","prompt":"a photo of a book"}]}"#)
        .unwrap();
        qnc_shot_labels::Labels::read(&path).unwrap()
    }

    fn set() -> LabelSet {
        let term = |code: &str, prompt: &str, vector: Vec<f32>| LabelVector { code: code.into(), prompt: prompt.into(), vector };
        LabelSet {
            model: "m".into(),
            scale: 10.0,
            bias: 0.0,
            labels: vec![term("pool", "a photo of a pool", vec![1.0, 0.0]), term("harbor", "a photo of a harbor", vec![0.6, 0.8]), term("book", "a photo of a book", vec![0.0, 1.0])],
        }
    }

    #[test]
    fn the_vocabulary_is_kept_on_this_computer_and_gives_a_shots_best_terms() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(read(root.path(), "m").unwrap(), None, "nothing kept yet");
        save(root.path(), &set()).unwrap();
        let kept = read(root.path(), "m").unwrap().unwrap();
        assert!(same(&kept, &list()));
        assert_eq!(read(root.path(), "other").unwrap(), None);
        let best: Vec<String> = best_terms(&kept, &list(), &[0.9, 0.436], 2).into_iter().map(|(code, _)| code).collect();
        assert_eq!(best, ["pool", "harbor"]);
        assert!(best_terms(&kept, &list(), &[1.0], 2).is_empty(), "a vector of another model");
    }
}
