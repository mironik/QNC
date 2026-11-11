//! The examples of the AI B-roll groups of this computer (user 2026-11-10: "the projects are
//! deleted, then the examples are deleted too; that is no good"). A shot whose group the user
//! corrected is kept here with its vector, outside every project: in the examples database of
//! this computer (`data/qnc-shot-examples.db`, of which this block is the only writer), so the
//! newsroom's corrections outlive the projects. A shot like a corrected one follows its group.
//! It reads and writes only that database; it loads no model and reads no media.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags};

pub const MODULE_ID: &str = "qnc.module.shot-examples";

/// The examples database of a QNC root.
pub const DATABASE: &str = "data/qnc-shot-examples.db";

/// The group of a shot that is no B-roll at all.
pub const NOT_BROLL: &str = "none";

/// A shot whose vector is at least this like an example's (cosine of unit vectors) follows its
/// group (a first estimate: a shot of the same scene is above it, another scene below).
pub const LIKE: f32 = 0.85;

/// The longest vector an example may have (a model's width).
const MOST_DIMENSIONS: usize = 4096;

/// One corrected shot: the model of its vector, what it is (the project, clip and frames it was
/// corrected in: replacing its own correction only), the group the user gave it and its vector.
#[derive(Debug, Clone, PartialEq)]
pub struct Example {
    pub model: String,
    pub key: String,
    pub label: String,
    pub vector: Vec<f32>,
}

/// How a shot got its group from the examples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Taught {
    /// The user corrected this very shot.
    Corrected,
    /// It looks like a corrected shot.
    LikeCorrected,
    /// A caption a model wrote of this very shot names its group (user 2026-11-11: Florence-2 as
    /// the corrector of SigLIP).
    Described,
    /// It looks like a shot whose caption names its group.
    LikeDescribed,
}

/// The group a shot gets from the user's corrections and from the groups captions give
/// (`described`, examples made from the captions of this project's shots): a correction comes
/// before a caption (user 2026-11-11: "lower than your correction"), so the user's own and
/// like-corrected groups first, then the caption's of this shot, then of a described shot it
/// looks like.
pub fn group_with<'a>(corrected: &'a [Example], described: &'a [Example], key: &str, vector: &[f32]) -> Option<(&'a str, Taught)> {
    group_of(corrected, key, vector).or_else(|| {
        group_of(described, key, vector).map(|(label, taught)| (label, if taught == Taught::Corrected { Taught::Described } else { Taught::LikeDescribed }))
    })
}

/// What a shot is: its project (its public URI), clip and frames.
pub fn key(project_uri: &str, clip_id: &str, from: u64, to: u64) -> String {
    format!("{project_uri}|{clip_id}|{from}|{to}")
}

/// The group the examples give a shot: its own correction, else the group of the example most
/// like it from `LIKE` on; `None`: the examples say nothing of it.
pub fn group_of<'a>(examples: &'a [Example], key: &str, vector: &[f32]) -> Option<(&'a str, Taught)> {
    if let Some(own) = examples.iter().rev().find(|example| example.key == key) {
        return Some((&own.label, Taught::Corrected));
    }
    examples
        .iter()
        .filter(|example| example.vector.len() == vector.len())
        .map(|example| (example, example.vector.iter().zip(vector).map(|(a, b)| a * b).sum::<f32>()))
        .filter(|(_, like)| *like >= LIKE)
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(example, _)| (example.label.as_str(), Taught::LikeCorrected))
}

/// The examples database of `root`.
pub fn path(root: &Path) -> PathBuf {
    root.join(DATABASE)
}

/// The examples of `model` in the examples database of `root`, the oldest first; none while the
/// database does not exist.
pub fn read(root: &Path, model: &str) -> Result<Vec<Example>, String> {
    let file = path(root);
    if !file.is_file() {
        return Ok(Vec::new());
    }
    let conn = Connection::open_with_flags(&file, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(err)?;
    conn.busy_timeout(std::time::Duration::from_secs(5)).map_err(err)?;
    let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='public_shot_examples')", [], |r| r.get(0)).map_err(err)?;
    if !exists {
        return Ok(Vec::new());
    }
    let mut statement = conn.prepare("SELECT key, label, vector_json FROM public_shot_examples WHERE model = ?1 ORDER BY made_at_unix_ms, key").map_err(err)?;
    let rows = statement
        .query_map(params![model], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    rows.into_iter()
        .map(|(key, label, vector)| Ok(Example { model: model.into(), key, label, vector: serde_json::from_str(&vector).map_err(err)? }))
        .collect()
}

/// Keeps `example` in the examples database of `root`, replacing the correction of the same
/// shot.
pub fn save(root: &Path, example: &Example) -> Result<(), String> {
    let valid = !example.model.is_empty()
        && !example.key.is_empty()
        && example.key.len() <= 1000
        && !example.key.chars().any(char::is_control)
        && !example.label.is_empty()
        && example.label.len() <= 64
        && (1..=MOST_DIMENSIONS).contains(&example.vector.len())
        && example.vector.iter().all(|v| v.is_finite());
    if !valid {
        return Err(qnc_ui_text::text("db.shot_example_invalid").into());
    }
    let file = path(root);
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    let mut conn = Connection::open(&file).map_err(err)?;
    conn.busy_timeout(std::time::Duration::from_secs(5)).map_err(err)?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS shot_examples (
            model TEXT NOT NULL,
            key TEXT NOT NULL,
            label TEXT NOT NULL,
            vector_json TEXT NOT NULL,
            made_at_unix_ms INTEGER NOT NULL,
            PRIMARY KEY (model, key)
         );
         CREATE VIEW IF NOT EXISTS public_shot_examples AS
            SELECT model, key, label, vector_json, made_at_unix_ms FROM shot_examples;",
    )
    .map_err(err)?;
    let vector = serde_json::to_string(&example.vector).map_err(err)?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
    let tx = conn.transaction().map_err(err)?;
    tx.execute(
        "INSERT INTO shot_examples (model, key, label, vector_json, made_at_unix_ms) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(model, key) DO UPDATE SET label=excluded.label, vector_json=excluded.vector_json, made_at_unix_ms=excluded.made_at_unix_ms",
        params![example.model, example.key, example.label, vector, now],
    )
    .map_err(err)?;
    tx.commit().map_err(err)
}

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example(key: &str, label: &str, vector: Vec<f32>) -> Example {
        Example { model: "m".into(), key: key.into(), label: label.into(), vector }
    }

    #[test]
    fn a_corrected_shot_is_kept_outside_the_projects_and_replaced_by_its_next_correction() {
        let root = tempfile::tempdir().unwrap();
        assert!(read(root.path(), "m").unwrap().is_empty(), "no database yet: no examples");
        save(root.path(), &example("p|c|0|10", "sea", vec![1.0, 0.0])).unwrap();
        save(root.path(), &example("p|c|20|30", "people", vec![0.0, 1.0])).unwrap();
        save(root.path(), &example("p|c|0|10", "none", vec![1.0, 0.0])).unwrap();
        let kept = read(root.path(), "m").unwrap();
        assert_eq!(kept.iter().map(|e| (e.key.as_str(), e.label.as_str())).collect::<Vec<_>>(), [("p|c|20|30", "people"), ("p|c|0|10", "none")]);
        assert!(read(root.path(), "other").unwrap().is_empty(), "examples of another model are not compared");
        assert!(save(root.path(), &example("p|c|0|10", "", vec![1.0])).is_err());
        assert!(save(root.path(), &example("p|c|0|10", "sea", vec![f32::NAN])).is_err());
    }

    #[test]
    fn a_shot_gets_its_own_correction_else_the_group_of_the_example_most_like_it() {
        let examples = [example("a", "sea", vec![1.0, 0.0]), example("b", "people", vec![0.6, 0.8])];
        assert_eq!(group_of(&examples, "b", &[1.0, 0.0]), Some(("people", Taught::Corrected)));
        assert_eq!(group_of(&examples, "x", &[0.99, 0.141]), Some(("sea", Taught::LikeCorrected)));
        assert_eq!(group_of(&examples, "x", &[0.0, 1.0]), None, "like no example enough");
        assert_eq!(group_of(&examples, "x", &[1.0, 0.0, 0.0]), None, "another width is another model");
    }

    #[test]
    fn a_correction_comes_before_a_caption_and_a_caption_teaches_the_shots_like_it() {
        let corrected = [example("a", "sea", vec![1.0, 0.0])];
        let described = [example("a", "people", vec![1.0, 0.0]), example("b", "pool", vec![0.0, 1.0])];
        assert_eq!(group_with(&corrected, &described, "a", &[1.0, 0.0]), Some(("sea", Taught::Corrected)), "the user's own wins over the caption");
        assert_eq!(group_with(&corrected, &described, "x", &[0.99, 0.141]), Some(("sea", Taught::LikeCorrected)), "like a correction before a caption");
        assert_eq!(group_with(&corrected, &described, "b", &[0.0, 1.0]), Some(("pool", Taught::Described)));
        assert_eq!(group_with(&corrected, &described, "y", &[0.141, 0.99]), Some(("pool", Taught::LikeDescribed)));
        assert_eq!(group_with(&corrected, &described, "z", &[0.7, -0.7]), None);
    }
}
