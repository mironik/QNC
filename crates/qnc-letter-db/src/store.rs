//! The letter table of a project database and its requests.

use crate::*;
use rusqlite::{params, Connection};

/// The table this module owns; it writes no other.
const OWNED: [&str; 1] = ["word_letters"];

pub(crate) struct Store {
    conn: Connection,
    schema_ready: bool,
}

impl Store {
    pub(crate) fn attach(mut conn: Connection, access: Access) -> Result<Self> {
        if access == Access::ReadWrite {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(err)?;
            ensure_schema(&tx)?;
            tx.commit().map_err(err)?;
        }
        let schema_ready = object_exists(&conn, "view", "public_word_letters")?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        qnc_db_broker::write_only_own_tables(&conn, access, &OWNED);
        Ok(Self { conn, schema_ready })
    }

    pub(crate) fn execute(&mut self, operation: &Operation) -> Result<Data> {
        match operation {
            Operation::ReadClip { .. } if !self.schema_ready => Ok(Data::Words(Vec::new())),
            Operation::ReadClip { clip_id } => self.read(clip_id),
            Operation::Save { clip_id, engine, words } => self.save(clip_id, engine, words),
        }
    }

    fn save(&mut self, clip_id: &str, engine: &str, words: &[WordLetters]) -> Result<Data> {
        valid_clip(clip_id)?;
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        for word in words {
            let fine = word.letters.iter().all(|letter| match (letter.from, letter.to) {
                (Some(from), Some(to)) => from.is_finite() && to.is_finite() && from >= 0.0 && to >= from,
                (None, None) => true,
                _ => false,
            });
            if word.text.is_empty() || !fine {
                return Err(qnc_ui_text::text("db.letters_invalid").into());
            }
            let letters = serde_json::to_string(&word.letters).map_err(err)?;
            tx.execute(
                "INSERT INTO word_letters (clip_id, word_from, text, letters_json, engine, made_at_utc)
                 VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
                 ON CONFLICT(clip_id, word_from, text) DO UPDATE SET letters_json=excluded.letters_json,
                   engine=excluded.engine, made_at_utc=excluded.made_at_utc",
                params![clip_id, word.word_from as i64, word.text, letters, engine],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn read(&mut self, clip_id: &str) -> Result<Data> {
        valid_clip(clip_id)?;
        let mut statement = self.conn.prepare("SELECT word_from, text, letters_json FROM public_word_letters WHERE clip_id = ?1 ORDER BY word_from").map_err(err)?;
        let rows = statement
            .query_map(params![clip_id], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)))
            .map_err(err)?;
        let mut words = Vec::new();
        for row in rows {
            let (word_from, text, letters) = row.map_err(err)?;
            let letters = serde_json::from_str(&letters).map_err(err)?;
            words.push(WordLetters { word_from: word_from.max(0) as u64, text, letters });
        }
        Ok(Data::Words(words))
    }

    pub(crate) fn connection(&self) -> &Connection {
        &self.conn
    }
}

fn valid_clip(clip_id: &str) -> Result<()> {
    if clip_id.is_empty() || clip_id.len() > 200 || clip_id.chars().any(char::is_control) {
        return Err(qnc_ui_text::text("db.letters_invalid").into());
    }
    Ok(())
}

fn ensure_schema(conn: &Connection) -> Result<()> {
    if !object_exists(conn, "table", "word_letters")? {
        conn.execute_batch(
            "CREATE TABLE word_letters (
                clip_id TEXT NOT NULL,
                word_from INTEGER NOT NULL CHECK (word_from >= 0),
                text TEXT NOT NULL,
                letters_json TEXT NOT NULL,
                engine TEXT NOT NULL,
                made_at_utc TEXT NOT NULL,
                PRIMARY KEY (clip_id, word_from, text)
            );",
        )
        .map_err(err)?;
    }
    if !object_exists(conn, "view", "public_word_letters")? {
        conn.execute_batch(
            "CREATE VIEW public_word_letters AS
             SELECT clip_id, word_from, text, letters_json, engine, made_at_utc FROM word_letters;",
        )
        .map_err(err)?;
    }
    Ok(())
}

fn object_exists(conn: &Connection, kind: &str, name: &str) -> Result<bool> {
    conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type=?1 AND name=?2)", params![kind, name], |r| r.get(0))
        .map_err(err)
}

pub(crate) fn err(error: impl std::fmt::Display) -> String {
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
