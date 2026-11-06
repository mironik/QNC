use super::*;

/// The columns a story of the current schema has (user 2026-10-30: segments and covers are
/// rows of one table, `story_items`, and `role` says which a row is).
pub(crate) const SCHEMA_COLUMNS: &[(&str, &[&str])] = &[
    ("story_state", &["story_fps_num", "story_fps_den"]),
    (
        "story_items",
        &[
            "role", "kind", "lead", "slot_id", "source_in_frame", "source_out_frame", "v_lead_frames", "v_tail_frames",
            "a1_source_channel", "a1_lead_frames", "a1_tail_frames", "a2_source_channel", "a2_lead_frames", "a2_tail_frames",
        ],
    ),
];

/// Columns of `story_items` added to a story made before them, its rows kept (a story
/// without one is brought up by its owner, never removed).
pub(crate) const ADDED_ITEM_COLUMNS: &[(&str, &str)] = &[("level_keys", "TEXT NOT NULL DEFAULT ''")];

/// The tables of the story before segments and covers shared one table; a project that still
/// has them is an older story.
const OLD_TABLES: [&str; 2] = ["story_parts", "story_covers"];

/// A story table lacks a column of the current schema, the story still has the tables of
/// before `story_items`, or it has no revision yet.
pub(crate) fn schema_outdated(conn: &Connection) -> Result<bool> {
    if !table_columns(conn, "story_state")?.is_empty() && table_columns(conn, "story_revision")?.is_empty() {
        return Ok(true);
    }
    for table in OLD_TABLES {
        if !table_columns(conn, table)?.is_empty() {
            return Ok(true);
        }
    }
    for (table, needed) in SCHEMA_COLUMNS {
        let columns = table_columns(conn, table)?;
        if !columns.is_empty() && needed.iter().any(|column| !columns.iter().any(|have| have == column)) {
            return Ok(true);
        }
    }
    let items = table_columns(conn, "story_items")?;
    Ok(!items.is_empty() && ADDED_ITEM_COLUMNS.iter().any(|(column, _)| !items.iter().any(|have| have == column)))
}

/// The revision of the story: kept apart from the story tables, so Undo never takes it
/// back (a revision is never given to two different stories).
pub(crate) fn bump_revision(conn: &Connection) -> Result<()> {
    conn.execute(
        "INSERT INTO story_revision (id, revision) VALUES (1, 1)
         ON CONFLICT(id) DO UPDATE SET revision = revision + 1",
        [],
    )
    .map_err(err)?;
    Ok(())
}

pub(crate) fn read_revision(conn: &Connection) -> Result<u64> {
    let revision: Option<i64> = conn
        .query_row("SELECT revision FROM story_revision WHERE id = 1", [], |row| row.get(0))
        .optional()
        .map_err(err)?;
    Ok(revision.unwrap_or(0).max(0) as u64)
}

pub(crate) fn ensure_story_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS story_revision (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            revision INTEGER NOT NULL
         );",
    )
    .map_err(err)?;
    // Development records are removed, not converted (AGENTS section 3, user 2026-10-30 "nema
    // migracije"): a story of separate segment and cover tables, or of an older shared table,
    // goes whole: its markers, slots, history and undo steps go with it.
    let items = table_columns(conn, "story_items")?;
    let older_items = !items.is_empty()
        && SCHEMA_COLUMNS
            .iter()
            .filter(|(table, _)| *table == "story_items")
            .any(|(_, needed)| needed.iter().any(|column| !items.iter().any(|have| have == column)));
    let older_tables = OLD_TABLES.iter().any(|table| table_columns(conn, table).map(|c| !c.is_empty()).unwrap_or(false));
    if older_items || older_tables {
        conn.execute_batch(
            "DROP VIEW IF EXISTS public_story_parts;
             DROP VIEW IF EXISTS public_story_covers;
             DROP VIEW IF EXISTS public_story_items;
             DROP VIEW IF EXISTS public_story_markers;
             DROP VIEW IF EXISTS public_story_marker_slots;
             DROP VIEW IF EXISTS public_story_state;
             DROP TABLE IF EXISTS story_covers;
             DROP TABLE IF EXISTS story_parts;
             DROP TABLE IF EXISTS story_items;
             DROP TABLE IF EXISTS story_marker_slots;
             DROP TABLE IF EXISTS story_markers;
             DROP TABLE IF EXISTS story_object_history;
             DROP TABLE IF EXISTS story_state;",
        )
        .map_err(err)?;
        if !table_columns(conn, "story_undo")?.is_empty() {
            conn.execute("DELETE FROM story_undo", []).map_err(err)?;
        }
    }
    // The J and L of a sound may be negative, a hole in the sound (user 2026-11-05: "nowhere a
    // hole in the picture, only a hole in the sound"): a table made with them never below 0 is
    // built again with the same rows (no conversion: every row is valid as it is).
    let checked = conn
        .query_row("SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'story_items'", [], |row| row.get::<_, String>(0))
        .optional()
        .map_err(err)?
        .is_some_and(|sql| sql.contains("CHECK (a1_lead_frames >= 0)"));
    if checked {
        conn.execute_batch(
            "DROP VIEW IF EXISTS public_story_parts;
             DROP VIEW IF EXISTS public_story_covers;
             DROP VIEW IF EXISTS public_story_items;
             DROP VIEW IF EXISTS public_story_markers;
             DROP VIEW IF EXISTS public_story_marker_slots;
             DROP VIEW IF EXISTS public_story_state;
             DROP INDEX IF EXISTS idx_story_items_role_sort;
             DROP INDEX IF EXISTS idx_story_items_slot;
             ALTER TABLE story_items RENAME TO story_items_checked;",
        )
        .map_err(err)?;
    }
    // Columns added to a story made before them; its rows stay (the views below read them).
    let have = table_columns(conn, "story_items")?;
    for (column, definition) in ADDED_ITEM_COLUMNS {
        if !have.is_empty() && !have.iter().any(|name| name == column) {
            conn.execute_batch(&format!("ALTER TABLE story_items ADD COLUMN {column} {definition};")).map_err(err)?;
        }
    }
    conn.execute_batch(
        "DROP VIEW IF EXISTS public_program_segments;
         DROP VIEW IF EXISTS public_program_markers;
         DROP TABLE IF EXISTS program_markers;
         DROP TABLE IF EXISTS program_segments;
         CREATE TABLE IF NOT EXISTS story_state (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            selected_part_id TEXT NOT NULL DEFAULT '',
            selected_shot_id TEXT NOT NULL DEFAULT '',
            selected_slot_id TEXT NOT NULL DEFAULT '',
            selected_cover_id TEXT NOT NULL DEFAULT '',
            draft_updated_at TEXT,
            committed_at TEXT,
            updated_at TEXT,
            story_fps_num INTEGER NOT NULL DEFAULT 0,
            story_fps_den INTEGER NOT NULL DEFAULT 1
         );
         -- One row is one piece of the story (user 2026-10-30): `role` says what it is. A
         -- segment ('tonovi' / 'offovi') lies in order (`sort_index`) and carries A1; a cover
         -- lies in the slot between two M markers (`slot_id`, `timeline_*`) and overwrites the
         -- picture and A2 there. Both have a source, a picture J/L and a sound per lane.
         CREATE TABLE IF NOT EXISTS story_items (
            item_id TEXT PRIMARY KEY,
            role TEXT NOT NULL CHECK (role IN ('segment', 'cover')),
            kind TEXT NOT NULL DEFAULT ''
                CHECK ((role = 'segment' AND kind IN ('tonovi', 'offovi')) OR (role = 'cover' AND kind = '')),
            sort_index INTEGER NOT NULL DEFAULT 0,
            lead TEXT NOT NULL DEFAULT '' CHECK (lead IN ('', 'in', 'out')),
            active INTEGER NOT NULL DEFAULT 1,
            slot_id TEXT NOT NULL DEFAULT '',
            timeline_start_frame INTEGER NOT NULL DEFAULT 0,
            timeline_end_frame INTEGER NOT NULL DEFAULT 0,
            timeline_start_sec REAL NOT NULL DEFAULT 0,
            timeline_end_sec REAL NOT NULL DEFAULT 0,
            slot_signature TEXT NOT NULL DEFAULT '',
            slot_index INTEGER NOT NULL DEFAULT 0,
            title TEXT NOT NULL DEFAULT '',
            text TEXT NOT NULL DEFAULT '',
            note TEXT NOT NULL DEFAULT '',
            clip_id TEXT NOT NULL DEFAULT '',
            virtual_shot_id TEXT NOT NULL DEFAULT '',
            in_tc TEXT NOT NULL DEFAULT '',
            out_tc TEXT NOT NULL DEFAULT '',
            in_seconds REAL,
            out_seconds REAL,
            source_in_frame INTEGER NOT NULL DEFAULT 0,
            source_out_frame INTEGER NOT NULL DEFAULT 0,
            source_fps REAL NOT NULL DEFAULT 0,
            source_fps_num INTEGER NOT NULL DEFAULT 0,
            source_fps_den INTEGER NOT NULL DEFAULT 1,
            duration_frames INTEGER NOT NULL DEFAULT 0,
            duration_label TEXT NOT NULL DEFAULT '',
            duration_color_key TEXT NOT NULL DEFAULT '',
            v_lead_frames INTEGER NOT NULL DEFAULT 0 CHECK (v_lead_frames >= 0),
            v_tail_frames INTEGER NOT NULL DEFAULT 0 CHECK (v_tail_frames >= 0),
            a1_source_channel INTEGER NOT NULL DEFAULT 0 CHECK (a1_source_channel >= 0),
            a1_level_db INTEGER NOT NULL DEFAULT 0,
            a1_off INTEGER NOT NULL DEFAULT 0,
            a1_lead_frames INTEGER NOT NULL DEFAULT 0,
            a1_tail_frames INTEGER NOT NULL DEFAULT 0,
            a2_source_channel INTEGER NOT NULL DEFAULT 0 CHECK (a2_source_channel >= 0),
            a2_level_db INTEGER NOT NULL DEFAULT 0,
            a2_off INTEGER NOT NULL DEFAULT 0,
            a2_lead_frames INTEGER NOT NULL DEFAULT 0,
            a2_tail_frames INTEGER NOT NULL DEFAULT 0,
            level_keys TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_story_items_role_sort ON story_items(role, sort_index);
         -- `origin` is added below to a story made before it (it keeps its rows).
         CREATE INDEX IF NOT EXISTS idx_story_items_slot ON story_items(slot_id);
         CREATE TABLE IF NOT EXISTS story_markers (
            marker_id TEXT PRIMARY KEY,
            timeline_frame INTEGER NOT NULL DEFAULT 0,
            timeline_sec REAL NOT NULL DEFAULT 0,
            tc TEXT NOT NULL DEFAULT '',
            label TEXT NOT NULL DEFAULT '',
            sort_index INTEGER NOT NULL DEFAULT 0,
            system_role TEXT NOT NULL DEFAULT '',
            origin_part_id TEXT NOT NULL DEFAULT '',
            origin_local_frame INTEGER,
            origin_local_sec REAL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_story_markers_frame ON story_markers(timeline_frame);
         CREATE TABLE IF NOT EXISTS story_marker_slots (
            slot_id TEXT PRIMARY KEY,
            slot_index INTEGER NOT NULL,
            start_frame INTEGER NOT NULL DEFAULT 0,
            end_frame INTEGER NOT NULL DEFAULT 0,
            duration_frames INTEGER NOT NULL DEFAULT 0,
            start_sec REAL NOT NULL,
            end_sec REAL NOT NULL,
            duration_sec REAL NOT NULL DEFAULT 0,
            start_marker_id TEXT NOT NULL DEFAULT '',
            end_marker_id TEXT NOT NULL DEFAULT '',
            slot_signature TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS story_object_history (
            object_type TEXT NOT NULL,
            object_id TEXT NOT NULL,
            state TEXT NOT NULL DEFAULT '',
            snapshot_json TEXT NOT NULL DEFAULT '{}',
            updated_at TEXT NOT NULL,
            PRIMARY KEY (object_type, object_id)
         );
         DROP VIEW IF EXISTS public_story_items;
         CREATE VIEW public_story_items AS
         SELECT item_id, role, kind, sort_index, lead, active, slot_id, timeline_start_frame,
                timeline_end_frame, clip_id, virtual_shot_id, source_in_frame, source_out_frame,
                source_fps_num AS fps_num, source_fps_den AS fps_den, duration_frames,
                v_lead_frames, v_tail_frames, a1_source_channel, a1_level_db, a1_off,
                a1_lead_frames, a1_tail_frames, a2_source_channel, a2_level_db, a2_off,
                a2_lead_frames, a2_tail_frames
         FROM story_items;
         DROP VIEW IF EXISTS public_story_parts;
         CREATE VIEW public_story_parts AS
         SELECT item_id AS segment_id, kind, sort_index, clip_id, source_in_frame AS in_frame,
                source_out_frame AS out_frame, source_fps_num AS fps_num, source_fps_den AS fps_den,
                active, duration_frames, duration_label, duration_color_key, a1_source_channel,
                a1_level_db, a1_off, v_lead_frames, lead, a1_lead_frames, a1_tail_frames, v_tail_frames,
                a2_source_channel, a2_level_db, a2_off, a2_lead_frames, a2_tail_frames, level_keys
         FROM story_items WHERE role = 'segment';
         DROP VIEW IF EXISTS public_story_markers;
         CREATE VIEW public_story_markers AS
         SELECT marker_id, timeline_frame AS program_frame, system_role
         FROM story_markers;
         DROP VIEW IF EXISTS public_story_state;
         CREATE VIEW public_story_state AS
         SELECT selected_part_id, selected_shot_id, selected_slot_id, selected_cover_id,
                draft_updated_at, committed_at, updated_at, story_fps_num, story_fps_den
         FROM story_state;
         DROP VIEW IF EXISTS public_story_marker_slots;
         CREATE VIEW public_story_marker_slots AS
         SELECT s.slot_id, s.slot_index, s.start_frame, s.end_frame, s.duration_frames,
                s.start_marker_id, s.end_marker_id, s.slot_signature, s.updated_at,
                EXISTS(SELECT 1 FROM story_items c WHERE c.role = 'cover' AND c.slot_id = s.slot_id) AS has_cover
         FROM story_marker_slots s;
         DROP VIEW IF EXISTS public_story_covers;
         CREATE VIEW public_story_covers AS
         SELECT item_id AS cover_id, slot_id, slot_signature, slot_index, timeline_start_frame,
                timeline_end_frame, clip_id, virtual_shot_id, source_in_frame, source_out_frame,
                source_fps_num AS fps_num, source_fps_den AS fps_den, a2_source_channel,
                a2_level_db, a2_off, a2_lead_frames, a2_tail_frames, level_keys
         FROM story_items WHERE role = 'cover';",
    )
    .map_err(err)?;
    // Where a segment came from (user 2026-10-31: a part of the skeleton sent again replaces
    // its own segments). Added to a story made before it; its rows stay.
    if !table_columns(conn, "story_items")?.iter().any(|column| column == "origin") {
        conn.execute_batch("ALTER TABLE story_items ADD COLUMN origin TEXT NOT NULL DEFAULT '';").map_err(err)?;
    }
    if checked {
        let now = items_now(conn);
        let columns = table_columns(conn, "story_items_checked")?.into_iter().filter(|column| now.contains(column)).collect::<Vec<_>>().join(", ");
        conn.execute_batch(&format!("INSERT INTO story_items ({columns}) SELECT {columns} FROM story_items_checked; DROP TABLE story_items_checked;")).map_err(err)?;
    }
    // Cover identity is the marker pair. The seconds signature stays a column
    // from v5; this owner does not use it as the slot id.
    conn.execute(
        "INSERT INTO story_state (id, updated_at) VALUES (1, ?1)
         ON CONFLICT(id) DO NOTHING",
        [story_now()],
    )
    .map_err(err)?;
    Ok(())
}

/// The columns of `story_items` as it is now.
fn items_now(conn: &Connection) -> Vec<String> {
    table_columns(conn, "story_items").unwrap_or_default()
}
