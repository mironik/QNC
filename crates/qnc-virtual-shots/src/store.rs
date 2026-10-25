//! The virtual shot table of a project database and its requests (moved unchanged
//! from the former project content store; v5 `virtual_shots`).

use crate::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::time::{SystemTime, UNIX_EPOCH};

mod rows;
mod schema;
pub(crate) use rows::*;
use schema::*;

/// The tables this module owns; it writes no other.
const OWNED: [&str; 2] = ["virtual_shots", "virtual_shot_numbers"];

pub(crate) struct Store {
    conn: Connection,
    schema_ready: bool,
}

impl Store {
    /// Creates the virtual shot table if missing (read-write) and serves it; a
    /// read-only connection of a project without one reads no shots.
    pub(crate) fn attach(mut conn: Connection, access: Access) -> Result<Self> {
        if access == Access::ReadWrite {
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(err)?;
            ensure_virtual_shots_schema(&tx)?;
            tx.commit().map_err(err)?;
        }
        let schema_ready = object_exists(&conn, "table", "virtual_shots")?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        qnc_db_broker::write_only_own_tables(&conn, access, &OWNED);
        Ok(Self { conn, schema_ready })
    }

    pub(crate) fn execute(&mut self, operation: &Operation) -> Result<Data> {
        if !self.schema_ready {
            return match operation {
                Operation::ListShorts | Operation::ListBroll => Ok(Data::ShortClips(Vec::new())),
                _ => Err(qnc_ui_text::text("stills.project_subclips_initialised").into()),
            };
        }
        match operation {
            Operation::SaveShort {
                project_id,
                clip_id,
                clip_name,
                in_frame,
                out_frame,
            } => self.save_short(project_id, clip_id, clip_name, *in_frame, *out_frame),
            Operation::CreateCoverShot {
                project_id,
                clip_id,
                clip_name,
                in_frame,
                out_frame,
            } => self.create_cover_shot(project_id, clip_id, clip_name, (*in_frame, *out_frame)),
            Operation::ForgetCoverShot { shot_id } => self.forget_cover_shot(shot_id),
            Operation::ConfirmCoverShot { shot_id } => self.confirm_cover_shot(shot_id),
            Operation::ForgetUnconfirmedCoverShots { older_than_seconds } => {
                self.forget_unconfirmed_cover_shots(*older_than_seconds)
            }
            Operation::ListShorts => self.list_shorts(),
            Operation::ListBroll => self.list_b_roll(),
            Operation::MarkShortStills {
                shot_id,
                status,
                in_uri,
                out_uri,
                error,
            } => self.mark_short_stills(
                shot_id,
                status,
                in_uri.as_deref(),
                out_uri.as_deref(),
                error.as_deref(),
            ),
        }
    }

    fn save_short(
        &mut self,
        project_id: &str,
        clip_id: &str,
        clip_name: &str,
        in_frame: u64,
        out_frame: u64,
    ) -> Result<Data> {
        if project_id.trim().is_empty() {
            return Err(qnc_ui_text::text("story.msg.no_active_project").into());
        }
        qnc_media_records::valid_id(clip_id).map_err(err)?;
        if out_frame <= in_frame {
            return Err(qnc_ui_text::text("story.out_after_in").into());
        }
        let project_matches: bool = self
            .conn
            .query_row(
                "SELECT count(*)=1 AND min(project_id)=?1 FROM public_project_settings",
                [project_id],
                |row| row.get(0),
            )
            .map_err(err)?;
        if !project_matches {
            return Err(qnc_ui_text::text("db.project_database_belongs_another_project").into());
        }
        require_imported_clip(&self.conn, clip_id)?;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let source_shot_id = source_row(&tx, clip_id)?;
        let index = next_index(&tx, clip_id, "short", "_shot_")?;
        let shot_id = format!("{clip_id}_shot_{index:03}");
        let name = format!("{} {index:03}", clip_name.trim());
        let created = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0);
        tx.execute(
            "INSERT INTO virtual_shots (
                shot_id, clip_id, class, in_frame, out_frame, source_shot_id, name,
                created_at_utc, still_status
             ) VALUES (?1, ?2, 'short', ?3, ?4, ?5, ?6, ?7, 'pending')",
            params![
                shot_id,
                clip_id,
                in_frame as i64,
                out_frame as i64,
                source_shot_id,
                name,
                created
            ],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::SavedShort(SavedShort {
            shot_id,
            in_frame,
            out_frame,
        }))
    }

    fn list_shorts(&self) -> Result<Data> {
        if !object_exists(&self.conn, "view", "public_short_clips")? {
            return Ok(Data::ShortClips(Vec::new()));
        }
        let current_sql = "SELECT shot_id, clip_id, in_frame, out_frame, name,
                in_still_uri, out_still_uri, still_status
            FROM public_short_clips
            ORDER BY created_at_utc, shot_id";
        if let Ok(mut statement) = self.conn.prepare(current_sql) {
            let rows = statement
                .query_map([], short_clip_row)
                .map_err(err)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(err)?;
            return Ok(Data::ShortClips(rows));
        }
        let mut statement = self
            .conn
            .prepare(
                "SELECT shot_id, clip_id, in_frame, out_frame, name
                 FROM public_short_clips
                 ORDER BY created_at_utc, shot_id",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                Ok(ShortClip {
                    shot_id: row.get(0)?,
                    clip_id: row.get(1)?,
                    in_frame: row.get::<_, i64>(2)?.max(0) as u64,
                    out_frame: row.get::<_, i64>(3)?.max(0) as u64,
                    name: row.get(4)?,
                    in_still_uri: None,
                    out_still_uri: None,
                    still_status: "pending".into(),
                    b_roll: false,
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok(Data::ShortClips(rows))
    }

    /// v5 B-roll tab: the virtual shots of the covers.
    fn list_b_roll(&self) -> Result<Data> {
        if !object_exists(&self.conn, "view", "public_b_roll_clips")? {
            return Ok(Data::ShortClips(Vec::new()));
        }
        let mut statement = self
            .conn
            .prepare(
                "SELECT shot_id, clip_id, in_frame, out_frame, name,
                    in_still_uri, out_still_uri, still_status
                 FROM public_b_roll_clips
                 ORDER BY created_at_utc, shot_id",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                short_clip_row(row).map(|shot| ShortClip {
                    b_roll: true,
                    ..shot
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok(Data::ShortClips(rows))
    }

    fn mark_short_stills(
        &mut self,
        shot_id: &str,
        status: &str,
        in_uri: Option<&str>,
        out_uri: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<Data> {
        if shot_id.trim().is_empty() {
            return Err(qnc_ui_text::text("story.msg.subclip_not_found").into());
        }
        if !matches!(status, "ready" | "failed" | "pending") {
            return Err("Neispravan status slicica virtualnog kadra.".into());
        }
        if let Some(uri) = in_uri {
            qnc_contracts::parse_qnc_uri(uri).map_err(err)?;
        }
        if let Some(uri) = out_uri {
            qnc_contracts::parse_qnc_uri(uri).map_err(err)?;
        }
        if error_message.is_some_and(|value| value.len() > 4096) {
            return Err("Prevelika poruka greske.".into());
        }
        let changed = self
            .conn
            .execute(
                "UPDATE virtual_shots
                 SET still_status = ?2,
                     in_still_uri = COALESCE(?3, in_still_uri),
                     out_still_uri = COALESCE(?4, out_still_uri),
                     still_error = ?5
                 WHERE shot_id = ?1 AND class IN ('short', 'b_roll')",
                params![shot_id, status, in_uri, out_uri, error_message],
            )
            .map_err(err)?;
        if changed == 0 {
            Err(qnc_ui_text::text("story.msg.subclip_not_found").into())
        } else {
            Ok(Data::Changed)
        }
    }
    fn confirm_cover_shot(&mut self, shot_id: &str) -> Result<Data> {
        self.conn
            .execute("UPDATE virtual_shots SET cover_pending = 0 WHERE shot_id = ?1 AND class = 'b_roll'", [shot_id])
            .map_err(err)?;
        Ok(Data::Changed)
    }

    /// The unconfirmed B-roll shots older than `older_than` seconds: confirmed when a
    /// cover of the story uses them, otherwise removed. One transaction.
    fn forget_unconfirmed_cover_shots(&mut self, older_than: u64) -> Result<Data> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let before = now.saturating_sub(older_than) as i64;
        let covers = object_exists(&self.conn, "table", "story_covers")?;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        if covers {
            tx.execute(
                "UPDATE virtual_shots SET cover_pending = 0
                 WHERE class = 'b_roll' AND cover_pending = 1 AND created_at_utc < ?1
                   AND shot_id IN (SELECT virtual_shot_id FROM story_covers)",
                [before],
            )
            .map_err(err)?;
        }
        tx.execute(
            "DELETE FROM virtual_shots WHERE class = 'b_roll' AND cover_pending = 1 AND created_at_utc < ?1",
            [before],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// A B-roll shot no cover uses goes (its cover was refused); one in use stays.
    fn forget_cover_shot(&mut self, shot_id: &str) -> Result<Data> {
        let used = object_exists(&self.conn, "table", "story_covers")?
            && self
                .conn
                .query_row("SELECT EXISTS(SELECT 1 FROM story_covers WHERE virtual_shot_id = ?1)", [shot_id], |row| row.get(0))
                .map_err(err)?;
        if !used {
            self.conn
                .execute("DELETE FROM virtual_shots WHERE shot_id = ?1 AND class = 'b_roll'", [shot_id])
                .map_err(err)?;
        }
        Ok(Data::Changed)
    }

    /// v5 `add_virtual_shot_from_frames` for a cover (category `cover`): the source
    /// IN/OUT becomes a B-roll virtual shot; the Story module then puts it in a slot.
    fn create_cover_shot(
        &mut self,
        project_id: &str,
        clip_id: &str,
        clip_name: &str,
        (in_frame, out_frame): (u64, u64),
    ) -> Result<Data> {
        qnc_media_records::valid_id(clip_id).map_err(err)?;
        if out_frame <= in_frame {
            return Err(qnc_ui_text::text("story.out_after_in").into());
        }
        let project_matches: bool = self
            .conn
            .query_row(
                "SELECT count(*)=1 AND min(project_id)=?1 FROM public_project_settings",
                [project_id],
                |row| row.get(0),
            )
            .map_err(err)?;
        if !project_matches {
            return Err(qnc_ui_text::text("db.project_database_belongs_another_project").into());
        }
        require_imported_clip(&self.conn, clip_id)?;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let source_shot_id = source_row(&tx, clip_id)?;
        let index = next_index(&tx, clip_id, "b_roll", "_broll_")?;
        let shot_id = format!("{clip_id}_broll_{index:03}");
        let name = format!("{} B{index:03}", clip_name.trim());
        let created = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        tx.execute(
            "INSERT INTO virtual_shots (
                shot_id, clip_id, class, in_frame, out_frame, source_shot_id, name,
                created_at_utc, still_status, cover_pending
             ) VALUES (?1, ?2, 'b_roll', ?3, ?4, ?5, ?6, ?7, 'pending', 1)",
            params![
                shot_id,
                clip_id,
                in_frame as i64,
                out_frame as i64,
                source_shot_id,
                name,
                created as i64
            ],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Created(shot_id))
    }
}

impl Store {
    /// The connection the intermediary opens a leased write's transaction on.
    pub(crate) fn connection(&self) -> &Connection {
        &self.conn
    }
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
