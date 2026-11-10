//! The content table of a project database and its requests.

use crate::*;
use rusqlite::{params, Connection, OptionalExtension};

/// The tables this module owns; it writes no other.
const OWNED: [&str; 2] = ["clip_content", "shot_label_vectors"];

/// The longest vector a part may have (a model's width).
const MOST_DIMENSIONS: usize = 4096;

pub(crate) struct Store {
    conn: Connection,
    ready: bool,
}

impl Store {
    /// Creates the table if missing (read-write) and serves it; a read-only connection of a
    /// project without it reads no content.
    pub(crate) fn attach(conn: Connection, access: Access) -> Result<Self> {
        if access == Access::ReadWrite {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS clip_content (
                    clip_id TEXT PRIMARY KEY,
                    model TEXT NOT NULL,
                    version INTEGER NOT NULL,
                    parts_json TEXT NOT NULL,
                    error TEXT NOT NULL,
                    took_ms INTEGER NOT NULL,
                    made_at_utc TEXT NOT NULL
                 );
                 CREATE VIEW IF NOT EXISTS public_clip_content AS
                    SELECT clip_id, model, version, parts_json, error, took_ms, made_at_utc FROM clip_content;
                 CREATE TABLE IF NOT EXISTS shot_label_vectors (
                    model TEXT PRIMARY KEY,
                    labels_json TEXT NOT NULL,
                    made_at_utc TEXT NOT NULL
                 );
                 CREATE VIEW IF NOT EXISTS public_shot_label_vectors AS
                    SELECT model, labels_json, made_at_utc FROM shot_label_vectors;",
            )
            .map_err(err)?;
        }
        let ready = conn
            .query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='public_shot_label_vectors')", [], |r| r.get(0))
            .map_err(err)?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        qnc_db_broker::write_only_own_tables(&conn, access, &OWNED);
        Ok(Self { conn, ready })
    }

    pub(crate) fn execute(&mut self, operation: &Operation) -> Result<Data> {
        match operation {
            Operation::ReadClip { .. } if !self.ready => Ok(Data::Clip(None)),
            Operation::ReadAll { .. } if !self.ready => Ok(Data::All(Vec::new())),
            Operation::ReadLabels { .. } if !self.ready => Ok(Data::Labels(None)),
            Operation::ReadClip { clip_id } => self.read_clip(clip_id),
            Operation::ReadAll { model } => self.read_all(model),
            Operation::Save(content) => self.save(content),
            Operation::SaveLabels(labels) => self.save_labels(labels),
            Operation::ReadLabels { model } => self.read_labels(model),
        }
    }

    fn save_labels(&mut self, labels: &LabelSet) -> Result<Data> {
        let width = labels.labels.first().map_or(0, |label| label.vector.len());
        let valid = !labels.model.is_empty()
            && labels.scale.is_finite()
            && labels.bias.is_finite()
            && !labels.labels.is_empty()
            && labels.labels.iter().all(|label| {
                !label.code.is_empty() && label.vector.len() == width && (1..=MOST_DIMENSIONS).contains(&width) && label.vector.iter().all(|v| v.is_finite())
            });
        if !valid {
            return Err(qnc_ui_text::text("db.shot_content_invalid").into());
        }
        let json = serde_json::to_string(labels).map_err(err)?;
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        tx.execute(
            "INSERT INTO shot_label_vectors (model, labels_json, made_at_utc) VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
             ON CONFLICT(model) DO UPDATE SET labels_json=excluded.labels_json, made_at_utc=excluded.made_at_utc",
            params![labels.model, json],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn read_labels(&mut self, model: &str) -> Result<Data> {
        let json: Option<String> = self
            .conn
            .query_row("SELECT labels_json FROM public_shot_label_vectors WHERE model = ?1", params![model], |r| r.get(0))
            .optional()
            .map_err(err)?;
        json.map(|json| serde_json::from_str(&json).map_err(err)).transpose().map(Data::Labels)
    }

    fn save(&mut self, content: &ClipContent) -> Result<Data> {
        let width = content.parts.first().map_or(0, |part| part.vector.len());
        let valid = !content.clip_id.is_empty()
            && content.clip_id.len() <= 200
            && !content.clip_id.chars().any(char::is_control)
            && !content.model.is_empty()
            && content.parts.iter().all(|part| {
                part.from < part.to
                    && !part.frames.is_empty()
                    && part.frames.iter().all(|frame| (part.from..=part.to).contains(frame))
                    && part.vector.len() == width
                    && (1..=MOST_DIMENSIONS).contains(&part.vector.len())
                    && part.vector.iter().all(|value| value.is_finite())
            })
            && (content.error.is_empty() || content.parts.is_empty());
        if !valid {
            return Err(qnc_ui_text::text("db.shot_content_invalid").into());
        }
        let parts = serde_json::to_string(&content.parts).map_err(err)?;
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        tx.execute(
            "INSERT INTO clip_content (clip_id, model, version, parts_json, error, took_ms, made_at_utc)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
             ON CONFLICT(clip_id) DO UPDATE SET model=excluded.model, version=excluded.version, parts_json=excluded.parts_json,
               error=excluded.error, took_ms=excluded.took_ms, made_at_utc=excluded.made_at_utc",
            params![content.clip_id, content.model, CONTENT_VERSION, parts, content.error, content.took_ms as i64],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn read_clip(&mut self, clip_id: &str) -> Result<Data> {
        let row = self
            .conn
            .query_row(
                "SELECT clip_id, model, parts_json, error, took_ms FROM public_clip_content WHERE clip_id = ?1 AND version = ?2",
                params![clip_id, CONTENT_VERSION],
                row,
            )
            .optional()
            .map_err(err)?;
        row.map(content).transpose().map(Data::Clip)
    }

    fn read_all(&mut self, model: &str) -> Result<Data> {
        let mut statement = self
            .conn
            .prepare("SELECT clip_id, model, parts_json, error, took_ms FROM public_clip_content WHERE model = ?1 AND version = ?2 ORDER BY clip_id")
            .map_err(err)?;
        let rows = statement.query_map(params![model, CONTENT_VERSION], row).map_err(err)?.collect::<std::result::Result<Vec<_>, _>>().map_err(err)?;
        rows.into_iter().map(content).collect::<Result<Vec<_>>>().map(Data::All)
    }

    fn connection(&self) -> &Connection {
        &self.conn
    }
}

type Row = (String, String, String, String, i64);

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Row> {
    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
}

fn content((clip_id, model, parts, error, took_ms): Row) -> Result<ClipContent> {
    Ok(ClipContent { clip_id, model, parts: serde_json::from_str(&parts).map_err(err)?, error, took_ms: took_ms.max(0) as u64 })
}

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

impl qnc_db_broker::OwnedTables for Store {
    const ID: &'static str = MODULE_ID;
    type Operation = Operation;
    type Data = Data;
    fn is_write(operation: &Operation) -> bool {
        operation.is_write()
    }
    fn attach(connection: Connection, access: Access) -> Result<Self> {
        Store::attach(connection, access)
    }
    fn execute(&mut self, operation: &Operation) -> Result<Data> {
        Store::execute(self, operation)
    }
    fn connection(&self) -> &Connection {
        Store::connection(self)
    }
}
