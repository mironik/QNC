use super::*;

pub(crate) fn ensure_virtual_shots_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS virtual_shot_numbers (
            clip_id TEXT NOT NULL,
            class TEXT NOT NULL,
            last INTEGER NOT NULL,
            PRIMARY KEY (clip_id, class)
        );
        CREATE TABLE IF NOT EXISTS virtual_shots (
            shot_id TEXT PRIMARY KEY,
            clip_id TEXT NOT NULL,
            class TEXT NOT NULL CHECK (class IN ('source', 'short', 'b_roll')),
            in_frame INTEGER NOT NULL,
            out_frame INTEGER NOT NULL,
            source_shot_id TEXT,
            name TEXT NOT NULL,
            created_at_utc INTEGER NOT NULL,
            in_still_uri TEXT,
            out_still_uri TEXT,
            still_status TEXT NOT NULL DEFAULT 'pending'
                CHECK (still_status IN ('pending', 'ready', 'failed')),
            still_error TEXT
        );",
    )
    .map_err(err)?;
    add_virtual_column_if_missing(conn, "in_still_uri", "TEXT")?;
    add_virtual_column_if_missing(conn, "out_still_uri", "TEXT")?;
    add_virtual_column_if_missing(
        conn,
        "still_status",
        "TEXT NOT NULL DEFAULT 'pending' CHECK (still_status IN ('pending', 'ready', 'failed'))",
    )?;
    add_virtual_column_if_missing(conn, "still_error", "TEXT")?;
    // A B-roll shot is kept once its cover is written (third audit 2026-10-17).
    add_virtual_column_if_missing(conn, "cover_pending", "INTEGER NOT NULL DEFAULT 0")?;
    conn.execute_batch(
        "DROP VIEW IF EXISTS public_short_clips;
        CREATE VIEW public_short_clips AS
        SELECT shot_id, clip_id, in_frame, out_frame, source_shot_id, name, created_at_utc,
               in_still_uri, out_still_uri, still_status, still_error
        FROM virtual_shots
        WHERE class = 'short';
        DROP VIEW IF EXISTS public_b_roll_clips;
        CREATE VIEW public_b_roll_clips AS
        SELECT shot_id, clip_id, in_frame, out_frame, source_shot_id, name, created_at_utc,
               in_still_uri, out_still_uri, still_status, still_error
        FROM virtual_shots
        WHERE class = 'b_roll' AND cover_pending = 0;",
    )
    .map_err(err)
}

pub(crate) fn add_virtual_column_if_missing(conn: &Connection, column: &str, definition: &str) -> Result<()> {
    let mut statement = conn
        .prepare("PRAGMA table_info(virtual_shots)")
        .map_err(err)?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    if columns.iter().any(|existing| existing == column) {
        return Ok(());
    }
    conn.execute_batch(&format!(
        "ALTER TABLE virtual_shots ADD COLUMN {column} {definition}"
    ))
    .map_err(err)
}
