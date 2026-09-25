use super::*;
use qnc_media_metadata::{MediaRepresentation, Signal, StreamDetails};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde_json::Value;
use std::{
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub struct ContentStore {
    conn: Connection,
    uri: String,
    access: Access,
    schema_ready: bool,
    has_thumbnail_uri: bool,
}

impl ContentStore {
    /// Bind the owned Ingest schema, including inside an existing project DB.
    pub fn open_owner_binding(file: &Path, uri: &str, access: Access) -> Result<Self> {
        let id = project_id(uri)?;
        if file.exists() {
            let check =
                Connection::open_with_flags(file, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(err)?;
            check.busy_timeout(Duration::from_secs(5)).map_err(err)?;
            validate_container(&check, &id)?;
            drop(check);
            if access == Access::ReadWrite {
                enable_owner_write(file)?;
            }
        }
        let flags = if access == Access::ReadOnly {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
        };
        let mut conn = Connection::open_with_flags(file, flags).map_err(err)?;
        conn.busy_timeout(Duration::from_secs(5)).map_err(err)?;
        if access == Access::ReadWrite {
            // Set the connection's journal policy before any schema read/recovery.
            // The owner directory denies deletion; keep the rollback journal.
            let mode: String = conn
                .pragma_query_value(None, "journal_mode", |r| r.get(0))
                .map_err(err)?;
            if !mode.eq_ignore_ascii_case("wal") {
                conn.pragma_update(None, "journal_mode", "PERSIST")
                    .map_err(|e| format!("Ingest journal: {e:?}"))?;
            }
            conn.pragma_update(None, "synchronous", "FULL")
                .map_err(err)?;
        }
        let project_settings: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name IN ('project_settings','public_project_settings'))",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        validate_container(&conn, &id)?;
        conn.pragma_update(None, "foreign_keys", true)
            .map_err(err)?;
        let mut schema: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='ingest_content_schema')",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        if !schema && access == Access::ReadWrite {
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(err)?;
            let schema_exists: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='ingest_content_schema')",
                    [],
                    |r| r.get(0),
                )
                .map_err(err)?;
            if !schema_exists {
                let tables: u32 = tx.query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                    [], |row| row.get(0),
                ).map_err(err)?;
                if tables != 0 && !project_settings {
                    return Err(
                        "Datoteka vec sadrzi drugu shemu; nema migracije ni preuzimanja baze."
                            .into(),
                    );
                }
                tx.execute_batch(include_str!("schema.sql"))
                    .map_err(|e| format!("Ingest schema: {e:?}"))?;
                tx.execute(
                    "INSERT INTO ingest_content_schema VALUES (?1,?2)",
                    params![SCHEMA_VERSION, id],
                )
                .map_err(err)?;
            }
            tx.commit()
                .map_err(|e| format!("Ingest schema commit: {e:?}"))?;
            schema = true;
        }
        if schema {
            let (version, stored_project): (String, String) = conn
                .query_row(
                    "SELECT version,project_id FROM ingest_content_schema",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .map_err(err)?;
            if version != SCHEMA_VERSION {
                return Err("Nepodrzana Ingest shema; migracije nisu dopustene.".into());
            }
            if stored_project != id {
                return Err("Ingest baza pripada drugom projektu.".into());
            }
        }
        if access == Access::ReadWrite {
            ensure_summary_columns(&conn)?;
            ensure_lease_column(&conn)?;
            ensure_runtime_table(&conn)?;
            ensure_filmstrip_schema(&conn)?;
            ensure_wave_schema(&conn)?;
            ensure_virtual_shots_schema(&conn)?;
            ensure_story_schema(&conn)?;
        }
        let has_thumbnail_uri = schema && has_column(&conn, "clips", "thumbnail_uri")?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        conn.authorizer(Some(move |ctx: rusqlite::hooks::AuthContext<'_>| {
            use rusqlite::hooks::{AuthAction as A, Authorization as R};
            match ctx.action {
                A::Insert { table_name }
                | A::Delete { table_name }
                | A::Update { table_name, .. }
                    if access == Access::ReadWrite && owned_table(table_name) =>
                {
                    R::Allow
                }
                A::Read { .. }
                | A::Select
                | A::Transaction { .. }
                | A::Savepoint { .. }
                | A::Function { .. }
                | A::Recursive => R::Allow,
                _ => R::Deny,
            }
        }));
        Ok(Self {
            conn,
            uri: uri.into(),
            access,
            schema_ready: schema,
            has_thumbnail_uri,
        })
    }

    pub fn execute(&mut self, request: &Request) -> Result<Data> {
        if request.version != VERSION || request.db_uri != self.uri {
            return Err("Neispravan DB ugovor.".into());
        }
        if request.operation.is_write() && self.access == Access::ReadOnly {
            return Err("Pristup je read-only.".into());
        }
        if !self.schema_ready {
            return match &request.operation {
                Operation::List { .. } => Ok(Data::Clips(Vec::new())),
                Operation::ListSummary { .. } => Ok(Data::ClipSummaries(Vec::new())),
                Operation::Stats => Ok(Data::CatalogStats(CatalogStats::default())),
                Operation::GetRuntime { .. } => Ok(Data::Runtime(None)),
                Operation::Inventory { source_uri, .. } => {
                    let source = qnc_contracts::parse_qnc_uri(source_uri).map_err(err)?;
                    if source.resource_kind != "source" {
                        return Err("Neispravan izvor.".into());
                    }
                    Ok(Data::Inventory(Vec::new()))
                }
                Operation::Read { clip_id } => {
                    qnc_media_records::valid_id(clip_id).map_err(err)?;
                    Ok(Data::Clip(None))
                }
                Operation::ReadFilmstrip { clip_id } => {
                    qnc_media_records::valid_id(clip_id).map_err(err)?;
                    Ok(Data::Filmstrip(None))
                }
                Operation::ReadWave { clip_id } => {
                    qnc_media_records::valid_id(clip_id).map_err(err)?;
                    Ok(Data::Wave(None))
                }
                Operation::ListShorts => Ok(Data::ShortClips(Vec::new())),
                Operation::ListSegments => Ok(Data::Segments(Vec::new())),
                Operation::ListMarkers => Ok(Data::Markers(Vec::new())),
                Operation::ListSlots => Ok(Data::Slots(Vec::new())),
                Operation::ListCovers => Ok(Data::Covers(Vec::new())),
                Operation::ReadStorySelection => {
                    Ok(Data::StorySelection(StorySelection::default()))
                }
                _ => Err("Ingest shema nije inicijalizirana.".into()),
            };
        }
        match &request.operation {
            Operation::Publish(clip) => self.publish(clip),
            Operation::PublishBatch(clips) => {
                if clips.is_empty() || clips.len() > 16 {
                    return Err("Neispravna velicina upisnog paketa.".into());
                }
                let tx = self
                    .conn
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                    .map_err(err)?;
                for clip in clips {
                    Self::write_clip(&tx, clip)?;
                }
                tx.commit().map_err(err)?;
                Ok(Data::Changed)
            }
            Operation::Inventory { source_uri, after } => {
                let source = qnc_contracts::parse_qnc_uri(source_uri).map_err(err)?;
                if source.resource_kind != "source" {
                    return Err("Neispravan izvor.".into());
                }
                let mut stmt = self.conn.prepare(
                    "SELECT clip_id,source_uri,original_uri,revision,final FROM clips WHERE source_uri=?1 AND clip_id>?2 ORDER BY clip_id LIMIT ?3"
                ).map_err(err)?;
                let rows = stmt
                    .query_map(
                        params![source_uri, after.as_deref().unwrap_or(""), PAGE_SIZE],
                        |r| {
                            Ok(InventoryClip {
                                clip_id: r.get(0)?,
                                source_uri: r.get(1)?,
                                original_uri: r.get(2)?,
                                revision: r.get(3)?,
                                final_record: r.get(4)?,
                            })
                        },
                    )
                    .map_err(err)?;
                Ok(Data::Inventory(
                    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(err)?,
                ))
            }
            Operation::RemoveMissing { clips } => {
                if clips.len() > 4096 {
                    return Err("Previse klipova u naredbi.".into());
                }
                let tx = self
                    .conn
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                    .map_err(err)?;
                let mut removed = Vec::new();
                for clip in clips {
                    let matches: bool = tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM clips WHERE clip_id=?1 AND source_uri=?2 AND original_uri=?3 AND revision=?4 AND import_status IN ('detected','failed'))",
                        params![clip.clip_id,clip.source_uri,clip.original_uri,clip.revision], |r| r.get(0),
                    ).map_err(err)?;
                    if !matches {
                        continue;
                    }
                    for table in [
                        "filmstrip_frames",
                        "clip_sources",
                        "clip_proxy",
                        "probe_records",
                        "filmstrip_artifacts",
                        "wave_artifacts",
                    ] {
                        tx.execute(
                            &format!("DELETE FROM {table} WHERE clip_id=?1"),
                            [&clip.clip_id],
                        )
                        .map_err(err)?;
                    }
                    tx.execute("DELETE FROM clips WHERE clip_id=?1", [&clip.clip_id])
                        .map_err(err)?;
                    removed.push(clip.clip_id.clone());
                }
                tx.commit().map_err(err)?;
                Ok(Data::Removed(removed))
            }
            Operation::Read { clip_id } => {
                qnc_media_records::valid_id(clip_id).map_err(err)?;
                let clip = self.conn.query_row(
                    "SELECT catalog_json,selected,import_status,import_error,imported_media_uri,thumbnail_uri FROM clips WHERE clip_id=?1",
                    [clip_id], row,
                ).optional().map_err(err)?;
                Ok(Data::Clip(clip.map(Box::new)))
            }
            Operation::ReadFilmstrip { clip_id } => self.read_filmstrip(clip_id),
            Operation::PublishFilmstrip(artifact) => self.publish_filmstrip(artifact),
            Operation::ReadWave { clip_id } => self.read_wave(clip_id),
            Operation::PublishWave(artifact) => self.publish_wave(artifact),
            Operation::SaveShort {
                project_id,
                clip_id,
                clip_name,
                in_frame,
                out_frame,
            } => self.save_short(project_id, clip_id, clip_name, *in_frame, *out_frame),
            Operation::ListShorts => self.list_shorts(),
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
            Operation::CreateSegment {
                project_id,
                kind,
                clip_id,
                in_frame,
                out_frame,
                fps_num,
                fps_den,
            } => self.create_segment(
                project_id,
                kind,
                clip_id,
                (*in_frame, *out_frame),
                (*fps_num, *fps_den),
            ),
            Operation::DeleteSegment { segment_id } => self.delete_segment(segment_id),
            Operation::MoveSegment { segment_id, up } => self.move_segment(segment_id, *up),
            Operation::ListSegments => self.list_segments(),
            Operation::CreateMarker {
                part_id,
                local_frame,
            } => self.create_marker(part_id, *local_frame),
            Operation::MoveMarker {
                marker_id,
                program_frame,
            } => self.move_marker(marker_id, *program_frame),
            Operation::DeleteMarker { marker_id } => self.delete_marker(marker_id),
            Operation::ListMarkers => self.list_markers(),
            Operation::ListSlots => self.list_slots(),
            Operation::ListCovers => self.list_covers(),
            Operation::ReadStorySelection => self.read_story_selection(),
            Operation::SelectPart { part_id } => self.select_part(part_id),
            Operation::SelectSlot { slot_id } => self.select_slot(slot_id),
            Operation::List { after } => {
                let mut statement = self
                    .conn
                    .prepare(
                        "SELECT catalog_json,selected,import_status,import_error,imported_media_uri,thumbnail_uri
                     FROM clips WHERE clip_id > ?1 ORDER BY clip_id LIMIT ?2",
                    )
                    .map_err(err)?;
                let rows = statement
                    .query_map(params![after.as_deref().unwrap_or(""), PAGE_SIZE], row)
                    .map_err(err)?;
                Ok(Data::Clips(
                    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(err)?,
                ))
            }
            Operation::ListSummary { after } => {
                let thumbnail_expr = if self.has_thumbnail_uri {
                    "c.thumbnail_uri"
                } else {
                    "NULL"
                };
                let sql = format!(
                    "SELECT c.clip_id,c.name,c.source_uri,s.source_name,s.serial_number,
                        s.volume_name,{thumbnail_expr},c.duration_seconds,c.selected,
                        c.import_status,c.import_error,c.imported_media_uri,c.revision,c.final
                     FROM clips c
                     JOIN clip_sources s ON s.clip_id=c.clip_id
                     WHERE c.clip_id > ?1
                     ORDER BY c.clip_id
                     LIMIT ?2"
                );
                let mut statement = self.conn.prepare(&sql).map_err(err)?;
                let rows = statement
                    .query_map(
                        params![after.as_deref().unwrap_or(""), PAGE_SIZE],
                        summary_row,
                    )
                    .map_err(err)?;
                Ok(Data::ClipSummaries(
                    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(err)?,
                ))
            }
            Operation::Stats => self.catalog_stats().map(Data::CatalogStats),
            Operation::Select { clip_ids, selected } => {
                if clip_ids.len() > 4096 {
                    return Err("Previse klipova u naredbi.".into());
                }
                let tx = self.conn.transaction().map_err(err)?;
                for id in clip_ids {
                    if tx
                        .execute(
                            "UPDATE clips SET selected=?1 WHERE clip_id=?2",
                            params![selected, id],
                        )
                        .map_err(err)?
                        != 1
                    {
                        return Err("Clip nije pronadjen u projektnoj bazi.".into());
                    }
                }
                tx.commit().map_err(err)?;
                Ok(Data::Changed)
            }
            Operation::QueueSelected => {
                let tx = self
                    .conn
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                    .map_err(err)?;
                let mut stmt = tx.prepare("SELECT catalog_json,selected,import_status,import_error,imported_media_uri,thumbnail_uri FROM clips WHERE selected != 0").map_err(err)?;
                let clips = stmt
                    .query_map([], row)
                    .map_err(err)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(err)?;
                drop(stmt);
                if clips.is_empty() {
                    return Err("Nema odabranih klipova.".into());
                }
                if clips.iter().any(|c| !ready(c)) {
                    return Err(
                        "Odabrani klip nema zavrsene metapodatke u bazi. Novi probe nije dopusten."
                            .into(),
                    );
                }
                tx.execute("UPDATE clips SET import_status='queued',import_error=NULL WHERE selected != 0 AND import_status IN ('detected','failed')", []).map_err(err)?;
                tx.commit().map_err(err)?;
                Ok(Data::Changed)
            }
            Operation::ClaimNext => {
                let tx = self
                    .conn
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                    .map_err(err)?;
                // A clip whose importer stopped reporting is offered again.
                tx.execute(
                    "UPDATE clips SET import_status='queued',import_claimed_at=NULL WHERE import_status='processing' AND (import_claimed_at IS NULL OR import_claimed_at < CAST(strftime('%s','now') AS INTEGER) - ?1)",
                    [IMPORT_LEASE_SECONDS],
                )
                .map_err(err)?;
                let mut clip = tx.query_row(
                    "SELECT catalog_json,selected,import_status,import_error,imported_media_uri,thumbnail_uri FROM clips WHERE import_status='queued' ORDER BY clip_id LIMIT 1",
                    [], row).optional().map_err(err)?;
                if let Some(clip) = &mut clip {
                    tx.execute(
                        "UPDATE clips SET import_status='processing',import_claimed_at=CAST(strftime('%s','now') AS INTEGER) WHERE clip_id=?1",
                        [clip.clip.id()],
                    )
                    .map_err(err)?;
                    clip.import_status = ImportStatus::Processing;
                }
                tx.commit().map_err(err)?;
                Ok(Data::Claimed(clip.map(Box::new)))
            }
            Operation::SetRuntime { key, value } => {
                if key.is_empty()
                    || key.len() > 64
                    || !key.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
                    || value.len() > 4096
                {
                    return Err("Neispravan runtime zapis.".into());
                }
                self.conn
                    .execute(
                        "INSERT INTO ingest_runtime(key,value,updated_at) VALUES(?1,?2,CAST(strftime('%s','now') AS INTEGER)) ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at=excluded.updated_at",
                        params![key, value],
                    )
                    .map_err(err)?;
                Ok(Data::Changed)
            }
            Operation::GetRuntime { key } => {
                let has_table: bool = self
                    .conn
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='ingest_runtime')",
                        [],
                        |r| r.get(0),
                    )
                    .map_err(err)?;
                if !has_table {
                    return Ok(Data::Runtime(None));
                }
                let entry = self
                    .conn
                    .query_row(
                        "SELECT value, CAST(strftime('%s','now') AS INTEGER) - updated_at FROM ingest_runtime WHERE key=?1",
                        [key],
                        |r| {
                            Ok(RuntimeEntry {
                                value: r.get(0)?,
                                age_seconds: r.get(1)?,
                            })
                        },
                    )
                    .optional()
                    .map_err(err)?;
                Ok(Data::Runtime(entry))
            }
            Operation::Heartbeat { clip_id } => {
                let n = self
                    .conn
                    .execute(
                        "UPDATE clips SET import_claimed_at=CAST(strftime('%s','now') AS INTEGER) WHERE clip_id=?1 AND import_status='processing'",
                        [clip_id],
                    )
                    .map_err(err)?;
                if n != 1 {
                    return Err("Import posao nije preuzet ili je vec zavrsen.".into());
                }
                Ok(Data::Changed)
            }
            Operation::FinishImport {
                clip_id,
                media_uri,
                thumbnail_uri,
                error,
            } => {
                if media_uri.is_some() == error.is_some() {
                    return Err("Nedostaje ishod importa.".into());
                }
                if let Some(uri) = media_uri {
                    qnc_contracts::parse_qnc_uri(uri).map_err(err)?;
                }
                if error.as_ref().is_some_and(|s| s.len() > 4096) {
                    return Err("Prevelika poruka greske.".into());
                }
                let status = if error.is_some() {
                    "failed"
                } else {
                    "imported"
                };
                let n = self.conn.execute(
                    "UPDATE clips SET import_status=?1,imported_media_uri=?2,import_error=?3,thumbnail_uri=COALESCE(?5,thumbnail_uri) WHERE clip_id=?4 AND import_status='processing'",
                    params![status,media_uri,error,clip_id,thumbnail_uri]).map_err(err)?;
                if n != 1 {
                    return Err("Import posao nije preuzet ili je vec zavrsen.".into());
                }
                Ok(Data::Changed)
            }
        }
    }

    fn catalog_stats(&self) -> Result<CatalogStats> {
        let thumbnail_expr = if self.has_thumbnail_uri {
            "thumbnail_uri"
        } else {
            "NULL"
        };
        let sql = format!(
            "SELECT clip_id,revision,selected,import_status,import_error,
                imported_media_uri,{thumbnail_expr},source_uri,original_uri
             FROM clips
             ORDER BY clip_id"
        );
        let mut statement = self.conn.prepare(&sql).map_err(err)?;
        let mut rows = statement.query([]).map_err(err)?;
        let mut stats = CatalogStats {
            fingerprint: CATALOG_FINGERPRINT_OFFSET,
            ..Default::default()
        };
        while let Some(row) = rows.next().map_err(err)? {
            let clip_id: String = row.get(0).map_err(err)?;
            let revision_raw: i64 = row.get(1).map_err(err)?;
            let revision = u32::try_from(revision_raw)
                .map_err(|_| "Neispravna revizija klipa u Ingest bazi.")?;
            let selected: bool = row.get(2).map_err(err)?;
            let import_status: String = row.get(3).map_err(err)?;
            let import_error: Option<String> = row.get(4).map_err(err)?;
            let imported_media_uri: Option<String> = row.get(5).map_err(err)?;
            let thumbnail_uri: Option<String> = row.get(6).map_err(err)?;
            let source_uri: String = row.get(7).map_err(err)?;
            let original_uri: String = row.get(8).map_err(err)?;

            stats.clip_count += 1;
            if selected {
                stats.selected_count += 1;
            }
            stats.revision_sum = stats.revision_sum.wrapping_add(revision as u64);
            stats.max_revision = stats.max_revision.max(revision);
            fingerprint_str(&mut stats.fingerprint, &clip_id);
            fingerprint_u64(&mut stats.fingerprint, revision as u64);
            fingerprint_u64(&mut stats.fingerprint, if selected { 1 } else { 0 });
            fingerprint_str(&mut stats.fingerprint, &import_status);
            fingerprint_opt_str(&mut stats.fingerprint, import_error.as_deref());
            fingerprint_opt_str(&mut stats.fingerprint, imported_media_uri.as_deref());
            fingerprint_opt_str(&mut stats.fingerprint, thumbnail_uri.as_deref());
            fingerprint_str(&mut stats.fingerprint, &source_uri);
            fingerprint_str(&mut stats.fingerprint, &original_uri);
        }
        Ok(stats)
    }

    fn publish(&mut self, clip: &CatalogClip) -> Result<Data> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        Self::write_clip(&tx, clip)?;
        let saved = tx.query_row("SELECT catalog_json,selected,import_status,import_error,imported_media_uri,thumbnail_uri FROM clips WHERE clip_id=?1",[clip.id()],row).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Saved(Box::new(saved)))
    }

    fn publish_filmstrip(&mut self, artifact: &FilmstripArtifactRecord) -> Result<Data> {
        validate_filmstrip_artifact(artifact)?;
        let clip_exists: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM clips WHERE clip_id=?1)",
                [&artifact.clip_id],
                |r| r.get(0),
            )
            .map_err(err)?;
        if !clip_exists {
            return Err("Clip nije pronadjen u projektnoj bazi.".into());
        }
        let json = serde_json::to_string(artifact).map_err(err)?;
        if json.len() > MAX_BYTES / PAGE_SIZE {
            return Err("Prevelik filmstrip zapis.".into());
        }
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let now = utc_stamp();
        tx.execute(
            "INSERT INTO filmstrip_artifacts
                (clip_id, frame_count, artifact_uri, created_at_utc, frames_json)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(clip_id) DO UPDATE SET
                frame_count=excluded.frame_count,
                artifact_uri=excluded.artifact_uri,
                created_at_utc=excluded.created_at_utc,
                frames_json=excluded.frames_json",
            params![
                artifact.clip_id,
                artifact.frame_count as i64,
                artifact.artifact_uri,
                now,
                json,
            ],
        )
        .map_err(err)?;
        tx.execute(
            "DELETE FROM filmstrip_frames WHERE clip_id=?1",
            [&artifact.clip_id],
        )
        .map_err(err)?;
        for frame in &artifact.frames {
            tx.execute(
                "INSERT INTO filmstrip_frames
                    (clip_id, frame_index, seek_sec, artifact_uri, updated_at_utc)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    artifact.clip_id,
                    frame.index as i64,
                    parse_seconds(&frame.seek_sec)?,
                    frame.artifact_uri,
                    now,
                ],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn read_filmstrip(&self, clip_id: &str) -> Result<Data> {
        qnc_media_records::valid_id(clip_id).map_err(err)?;
        let Some(mut artifact) = self
            .conn
            .query_row(
                "SELECT frames_json FROM filmstrip_artifacts WHERE clip_id=?1",
                [clip_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(err)?
            .map(|json| serde_json::from_str::<FilmstripArtifactRecord>(&json).map_err(err))
            .transpose()?
        else {
            return Ok(Data::Filmstrip(None));
        };
        let mut statement = self
            .conn
            .prepare(
                "SELECT frame_index, seek_sec, artifact_uri
                 FROM filmstrip_frames
                 WHERE clip_id=?1
                 ORDER BY frame_index",
            )
            .map_err(err)?;
        let frames = statement
            .query_map([clip_id], |row| {
                Ok(FilmstripFrameRecord {
                    index: row.get::<_, i64>(0)? as usize,
                    seek_sec: format!("{:.2}", row.get::<_, f64>(1)?),
                    artifact_uri: row.get(2)?,
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        if !frames.is_empty() {
            artifact.frames = frames;
            artifact.frame_count = artifact.frames.len();
        }
        validate_filmstrip_artifact(&artifact)?;
        Ok(Data::Filmstrip(Some(Box::new(artifact))))
    }

    fn publish_wave(&mut self, artifact: &WaveArtifactRecord) -> Result<Data> {
        validate_wave_artifact(artifact)?;
        let stored_source: Option<(String, Option<String>)> = self
            .conn
            .query_row(
                "SELECT c.original_uri, p.proxy_uri
                 FROM clips c
                 LEFT JOIN clip_proxy p ON p.clip_id=c.clip_id
                 WHERE c.clip_id=?1",
                [&artifact.clip_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(err)?;
        let Some((original_uri, proxy_uri)) = stored_source else {
            return Err("Clip nije pronadjen u projektnoj bazi.".into());
        };
        if artifact.source_uri != original_uri
            && proxy_uri.as_deref() != Some(artifact.source_uri.as_str())
        {
            return Err("Wave zapis ne pripada spremljenom originalu ili proxyju klipa.".into());
        }
        let json = serde_json::to_string(artifact).map_err(err)?;
        if json.len() > MAX_BYTES / PAGE_SIZE {
            return Err("Prevelik wave zapis.".into());
        }
        let now = utc_stamp();
        self.conn
            .execute(
                "INSERT INTO wave_artifacts
                    (clip_id, artifact_uri, created_at_utc, peaks_json)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(clip_id) DO UPDATE SET
                    artifact_uri=excluded.artifact_uri,
                    created_at_utc=excluded.created_at_utc,
                    peaks_json=excluded.peaks_json",
                params![artifact.clip_id, artifact.artifact_uri, now, json],
            )
            .map_err(err)?;
        Ok(Data::Changed)
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
            return Err("Nema aktivnog projekta.".into());
        }
        qnc_media_records::valid_id(clip_id).map_err(err)?;
        if out_frame <= in_frame {
            return Err("OUT mora biti najmanje jedan frame nakon IN.".into());
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
            return Err("Projektna baza pripada drugom projektu.".into());
        }
        require_imported_clip(&self.conn, clip_id)?;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let source_shot_id = source_row(&tx, clip_id)?;
        let index = next_short_index(&tx, clip_id)?;
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
        Ok(Data::SavedShort(Box::new(SavedShort {
            shot_id,
            in_frame,
            out_frame,
        })))
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
            return Err("Virtualni kadar nije pronadjen.".into());
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
                 WHERE shot_id = ?1 AND class = 'short'",
                params![shot_id, status, in_uri, out_uri, error_message],
            )
            .map_err(err)?;
        if changed == 0 {
            Err("Virtualni kadar nije pronadjen.".into())
        } else {
            Ok(Data::Changed)
        }
    }

    /// Appends a Ton or Off segment (docs/93 R8-R10). The range is copied from the
    /// source IN/OUT; a story has one timebase, a different one is refused.
    fn create_segment(
        &mut self,
        project_id: &str,
        kind: &str,
        clip_id: &str,
        (in_frame, out_frame): (u64, u64),
        (fps_num, fps_den): (u32, u32),
    ) -> Result<Data> {
        let kind = story_kind(kind)?;
        qnc_media_records::valid_id(clip_id).map_err(err)?;
        if out_frame <= in_frame {
            return Err("OUT mora biti najmanje jedan frame nakon IN.".into());
        }
        if fps_num == 0 || fps_den == 0 {
            return Err("Segment nema valjan source fps.".into());
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
            return Err("Projektna baza pripada drugom projektu.".into());
        }
        require_imported_clip(&self.conn, clip_id)?;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let story_timebase: Option<(u32, u32)> = tx
            .query_row(
                "SELECT source_fps_num, source_fps_den FROM story_parts
                 WHERE active = 1 ORDER BY sort_index LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(err)?;
        if let Some((num, den)) = story_timebase {
            // Same rate even when written differently (50/1 and 100/2).
            if u64::from(num) * u64::from(fps_den) != u64::from(fps_num) * u64::from(den) {
                return Err(format!(
                    "Klip ima {fps_num}/{fps_den} fps, a prica {num}/{den}; mijesani fps nije dopusten."
                ));
            }
        }
        let next: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(sort_index) + 1, 0) FROM story_parts WHERE active = 1",
                [],
                |row| row.get(0),
            )
            .map_err(err)?;
        let created = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let segment_id = format!("part_{created:x}");
        let now = story_now();
        // v5 `segment_source_from_clip_frames`.
        let (in_frame, out_frame) = (in_frame as i64, out_frame as i64);
        let duration = out_frame - in_frame;
        let fps = f64::from(fps_num) / f64::from(fps_den);
        tx.execute(
            "INSERT INTO story_parts (
                part_id, kind, sort_index, title, text, clip_id, virtual_shot_id,
                in_tc, out_tc, in_seconds, out_seconds, fps, source_fps_num, source_fps_den,
                in_frame, out_frame, duration_frames, duration_label, duration_color_key,
                active, created_at, updated_at
             ) VALUES (?1, ?2, ?3, '', '', ?4, '', ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                       ?12, ?13, ?14, ?15, ?16, 1, ?17, ?17)",
            params![
                segment_id,
                kind,
                next,
                clip_id,
                frame_timecode(in_frame, fps),
                frame_timecode(out_frame, fps),
                in_frame as f64 / fps,
                out_frame as f64 / fps,
                fps,
                fps_num,
                fps_den,
                in_frame,
                out_frame,
                duration,
                frames_label(duration, fps),
                duration_color_key(duration, fps),
                now
            ],
        )
        .map_err(err)?;
        finalize_story(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Created(segment_id))
    }

    /// v5 `delete_part`: the segment becomes inactive (`active = 0`); markers inside
    /// its program window go and later ones move left by its length
    /// (`shift_markers_after_part_removal_frames`); the selection moves to the
    /// nearest segment; the others close the gap.
    fn delete_segment(&mut self, segment_id: &str) -> Result<Data> {
        let segment_id = segment_id.trim();
        if segment_id.is_empty() {
            return Err("part_id required".into());
        }
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let deleted_sort: i64 = tx
            .query_row(
                "SELECT sort_index FROM story_parts WHERE part_id = ?1 AND active = 1",
                [segment_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(err)?
            .ok_or_else(|| format!("part not found: {segment_id}"))?;
        let fps = require_story_fps(&tx).ok();
        let window = segment_window(&tx, segment_id)?;
        tx.execute(
            "UPDATE story_parts SET active = 0, updated_at = ?2 WHERE part_id = ?1",
            params![segment_id, story_now()],
        )
        .map_err(err)?;
        if let (Some(fps), Some((start, end))) = (fps, window) {
            let (start, end) = (start as i64, end as i64);
            let inside = {
                let mut statement = tx
                    .prepare(
                        "SELECT marker_id FROM story_markers
                         WHERE timeline_frame > ?1 AND timeline_frame < ?2",
                    )
                    .map_err(err)?;
                let ids = statement
                    .query_map(params![start, end], |row| row.get::<_, String>(0))
                    .map_err(err)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(err)?;
                ids
            };
            delete_markers_with_slots(&tx, &inside)?;
            let shifted = {
                let mut statement = tx
                    .prepare("SELECT marker_id, timeline_frame FROM story_markers WHERE timeline_frame >= ?1")
                    .map_err(err)?;
                let rows = statement
                    .query_map([end], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                    })
                    .map_err(err)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(err)?;
                rows
            };
            let now = story_now();
            for (marker_id, frame) in shifted {
                let frame = (frame - (end - start)).max(0);
                tx.execute(
                    "UPDATE story_markers SET timeline_frame = ?1, timeline_sec = ?2, tc = ?3,
                        updated_at = ?4
                     WHERE marker_id = ?5",
                    params![
                        frame,
                        timeline_sec(frame, fps),
                        frame_timecode(frame, fps),
                        now,
                        marker_id
                    ],
                )
                .map_err(err)?;
            }
        }
        let selected: String = tx
            .query_row(
                "SELECT selected_part_id FROM story_state WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .map_err(err)?;
        if selected == segment_id {
            let neighbour: String = tx
                .query_row(
                    "SELECT part_id FROM story_parts WHERE part_id != ?1 AND active != 0
                     ORDER BY ABS(sort_index - ?2) ASC, sort_index ASC LIMIT 1",
                    params![segment_id, deleted_sort],
                    |row| row.get(0),
                )
                .optional()
                .map_err(err)?
                .unwrap_or_default();
            tx.execute(
                "UPDATE story_state SET selected_part_id = ?1 WHERE id = 1",
                [neighbour],
            )
            .map_err(err)?;
        }
        renumber_segments(&tx)?;
        finalize_story(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// Swaps with the neighbour; at the edge nothing changes (docs/93 R13).
    fn move_segment(&mut self, segment_id: &str, up: bool) -> Result<Data> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let mut order = segment_order(&tx)?;
        let index = order
            .iter()
            .position(|id| id == segment_id)
            .ok_or("Segment nije pronadjen.")?;
        let other = if up {
            index.checked_sub(1)
        } else {
            Some(index + 1).filter(|next| *next < order.len())
        };
        if let Some(other) = other {
            order.swap(index, other);
            for (sort_index, id) in order.iter().enumerate() {
                tx.execute(
                    "UPDATE story_parts SET sort_index = ?1, updated_at = ?3
                     WHERE part_id = ?2",
                    params![sort_index as i64, id, story_now()],
                )
                .map_err(err)?;
            }
            finalize_story(&tx)?;
        }
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// v5 `create_marker_from_part_frame`: M is placed on a Wrap segment at a frame
    /// inside it (`local_to_timeline_frame`, clamped to the segment); the program
    /// frame follows. A marker already on that frame is refreshed, never duplicated.
    fn create_marker(&mut self, part_id: &str, local_frame: u64) -> Result<Data> {
        let fps = require_story_fps(&self.conn)?;
        let part_id = part_id.trim();
        if part_id.is_empty() {
            return Err("part_id required".into());
        }
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let (start, end) =
            segment_window(&tx, part_id)?.ok_or_else(|| format!("part not found: {part_id}"))?;
        let local = (local_frame as i64).min((end - start) as i64).max(0);
        let frame = start as i64 + local;
        let tc = frame_timecode(frame, fps);
        let existing: Option<String> = tx
            .query_row(
                "SELECT marker_id FROM story_markers WHERE timeline_frame = ?1
                 ORDER BY marker_id LIMIT 1",
                [frame],
                |row| row.get(0),
            )
            .optional()
            .map_err(err)?;
        let marker_id = match existing {
            Some(marker_id) => {
                tx.execute(
                    "UPDATE story_markers
                     SET tc = ?1, origin_part_id = ?2, origin_local_frame = ?3,
                         origin_local_sec = ?4, updated_at = ?5
                     WHERE marker_id = ?6",
                    params![
                        tc,
                        part_id,
                        local,
                        local as f64 / fps,
                        story_now(),
                        marker_id
                    ],
                )
                .map_err(err)?;
                marker_id
            }
            None => {
                let marker_id = new_marker_id();
                let now = story_now();
                tx.execute(
                    "INSERT INTO story_markers
                        (marker_id, timeline_frame, timeline_sec, tc, label, sort_index,
                         origin_part_id, origin_local_frame, origin_local_sec, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?4, 0, ?5, ?6, ?7, ?8, ?8)",
                    params![
                        marker_id,
                        frame,
                        timeline_sec(frame, fps),
                        tc,
                        part_id,
                        local,
                        local as f64 / fps,
                        now
                    ],
                )
                .map_err(err)?;
                marker_id
            }
        };
        finalize_story(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Created(marker_id))
    }

    /// v5 `update_marker_frame`: inside the story length, on a free frame; the start
    /// and the end marker are locked. The label is kept.
    fn move_marker(&mut self, marker_id: &str, program_frame: u64) -> Result<Data> {
        let fps = require_story_fps(&self.conn)?;
        let marker_id = marker_id.trim();
        if marker_id.is_empty() {
            return Err("marker_id required".into());
        }
        let frame = program_frame as i64;
        let duration = program_length(&self.conn)?;
        if frame > duration {
            return Err(format!(
                "M marker mora biti unutar trajanja storyja ({:.3} s).",
                timeline_sec(duration, fps)
            ));
        }
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        locked_check(&tx, marker_id, duration)?;
        let taken: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM story_markers
                 WHERE marker_id != ?1 AND timeline_frame = ?2)",
                params![marker_id, frame],
                |row| row.get(0),
            )
            .map_err(err)?;
        if taken {
            return Err(format!("marker already exists at timeline_frame={frame}"));
        }
        tx.execute(
            "UPDATE story_markers SET timeline_frame = ?1, timeline_sec = ?2, tc = ?3,
                updated_at = ?4
             WHERE marker_id = ?5",
            params![
                frame,
                timeline_sec(frame, fps),
                frame_timecode(frame, fps),
                story_now(),
                marker_id
            ],
        )
        .map_err(err)?;
        finalize_story(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// v5 `delete_marker`: the start and the end marker are locked.
    fn delete_marker(&mut self, marker_id: &str) -> Result<Data> {
        require_story_fps(&self.conn)?;
        let marker_id = marker_id.trim();
        if marker_id.is_empty() {
            return Err("marker_id required".into());
        }
        let duration = program_length(&self.conn)?;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        locked_check(&tx, marker_id, duration)?;
        delete_markers_with_slots(&tx, &[marker_id.to_string()])?;
        finalize_story(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// v5 `list_markers`: every marker by program frame, the locked start and end
    /// included (`system_role`).
    fn list_markers(&self) -> Result<Data> {
        if !object_exists(&self.conn, "view", "public_story_markers")? {
            return Ok(Data::Markers(Vec::new()));
        }
        let mut statement = self
            .conn
            .prepare(
                "SELECT marker_id, program_frame, system_role
                 FROM public_story_markers
                 ORDER BY program_frame, marker_id",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                Ok(ProgramMarker {
                    marker_id: row.get(0)?,
                    program_frame: row.get::<_, i64>(1)?.max(0) as u64,
                    system_role: row.get(2)?,
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok(Data::Markers(rows))
    }

    /// v5 `list_parts`: every segment, deleted ones included (`active = false`),
    /// by `sort_index`; the program is the active ones.
    fn list_segments(&self) -> Result<Data> {
        if !object_exists(&self.conn, "view", "public_story_parts")? {
            return Ok(Data::Segments(Vec::new()));
        }
        let mut statement = self
            .conn
            .prepare(
                "SELECT segment_id, kind, sort_index, clip_id, in_frame, out_frame, fps_num,
                        fps_den, active, a1_source_channel
                 FROM public_story_parts ORDER BY sort_index, segment_id",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                Ok(ProgramSegment {
                    segment_id: row.get(0)?,
                    kind: row.get(1)?,
                    sort_index: row.get::<_, i64>(2)?.max(0) as u32,
                    clip_id: row.get(3)?,
                    in_frame: row.get::<_, i64>(4)?.max(0) as u64,
                    out_frame: row.get::<_, i64>(5)?.max(0) as u64,
                    fps_num: row.get(6)?,
                    fps_den: row.get(7)?,
                    active: row.get::<_, i64>(8)? != 0,
                    a1_source_channel: channel(row.get(9)?),
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok(Data::Segments(rows))
    }

    /// v5 `marker_slots_snapshot`: the stored slots in order, with `has_cover`.
    fn list_slots(&self) -> Result<Data> {
        if !object_exists(&self.conn, "view", "public_story_marker_slots")? {
            return Ok(Data::Slots(Vec::new()));
        }
        let mut statement = self
            .conn
            .prepare(
                "SELECT slot_id, start_frame, end_frame, start_marker_id, end_marker_id, has_cover
                 FROM public_story_marker_slots ORDER BY slot_index",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                Ok(ProgramSlot {
                    slot_id: row.get(0)?,
                    start_frame: row.get::<_, i64>(1)?.max(0) as u64,
                    end_frame: row.get::<_, i64>(2)?.max(0) as u64,
                    start_marker_id: row.get(3)?,
                    end_marker_id: row.get(4)?,
                    has_cover: row.get::<_, i64>(5)? != 0,
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok(Data::Slots(rows))
    }

    /// v5 `list_covers`: every cover with its slot, frames and A2 source channel.
    fn list_covers(&self) -> Result<Data> {
        if !object_exists(&self.conn, "view", "public_story_covers")? {
            return Ok(Data::Covers(Vec::new()));
        }
        let mut statement = self
            .conn
            .prepare(
                "SELECT cover_id, slot_id, clip_id, virtual_shot_id, timeline_start_frame,
                        timeline_end_frame, source_in_frame, source_out_frame, fps_num, fps_den,
                        a2_source_channel
                 FROM public_story_covers ORDER BY timeline_start_frame, cover_id",
            )
            .map_err(err)?;
        let frame = |value: i64| value.max(0) as u64;
        let rows = statement
            .query_map([], |row| {
                Ok(ProgramCover {
                    cover_id: row.get(0)?,
                    slot_id: row.get(1)?,
                    clip_id: row.get(2)?,
                    virtual_shot_id: row.get(3)?,
                    program_start_frame: frame(row.get(4)?),
                    program_end_frame: frame(row.get(5)?),
                    source_in_frame: frame(row.get(6)?),
                    source_out_frame: frame(row.get(7)?),
                    fps_num: row.get(8)?,
                    fps_den: row.get(9)?,
                    a2_source_channel: channel(row.get(10)?),
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok(Data::Covers(rows))
    }

    fn read_story_selection(&self) -> Result<Data> {
        if !object_exists(&self.conn, "view", "public_story_state")? {
            return Ok(Data::StorySelection(StorySelection::default()));
        }
        let selection = self
            .conn
            .query_row(
                "SELECT selected_part_id, selected_slot_id, selected_cover_id
                 FROM public_story_state LIMIT 1",
                [],
                |row| {
                    Ok(StorySelection {
                        selected_part_id: row.get(0)?,
                        selected_slot_id: row.get(1)?,
                        selected_cover_id: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(err)?
            .unwrap_or_default();
        Ok(Data::StorySelection(selection))
    }

    /// v5 `select_part`: an existing segment, or an empty id to clear.
    fn select_part(&mut self, part_id: &str) -> Result<Data> {
        let part_id = part_id.trim();
        if !part_id.is_empty() {
            let exists: bool = self
                .conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM story_parts WHERE part_id = ?1)",
                    [part_id],
                    |row| row.get(0),
                )
                .map_err(err)?;
            if !exists {
                return Err(format!("part not found: {part_id}"));
            }
        }
        self.conn
            .execute(
                "UPDATE story_state SET selected_part_id = ?1, draft_updated_at = ?2,
                    updated_at = ?2 WHERE id = 1",
                params![part_id, story_now()],
            )
            .map_err(err)?;
        Ok(Data::Changed)
    }

    /// v5 `select_marker_slot`: an existing slot, or an empty id to clear.
    fn select_slot(&mut self, slot_id: &str) -> Result<Data> {
        let slot_id = slot_id.trim();
        if !slot_id.is_empty() {
            let exists: bool = self
                .conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM story_marker_slots WHERE slot_id = ?1)",
                    [slot_id],
                    |row| row.get(0),
                )
                .map_err(err)?;
            if !exists {
                return Err(format!("slot not found: {slot_id}"));
            }
        }
        self.conn
            .execute(
                "UPDATE story_state SET selected_slot_id = ?1, draft_updated_at = ?2,
                    updated_at = ?2 WHERE id = 1",
                params![slot_id, story_now()],
            )
            .map_err(err)?;
        Ok(Data::Changed)
    }

    fn read_wave(&self, clip_id: &str) -> Result<Data> {
        qnc_media_records::valid_id(clip_id).map_err(err)?;
        let row = self
            .conn
            .query_row(
                "SELECT artifact_uri,peaks_json FROM wave_artifacts WHERE clip_id=?1",
                [clip_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(err)?;
        let Some((artifact_uri, json)) = row else {
            return Ok(Data::Wave(None));
        };
        let artifact = serde_json::from_str::<WaveArtifactRecord>(&json).map_err(err)?;
        if artifact.clip_id != clip_id || artifact.artifact_uri != artifact_uri {
            return Err("Wave zapis ne odgovara trazenom klipu.".into());
        }
        validate_wave_artifact(&artifact)?;
        Ok(Data::Wave(Some(Box::new(artifact))))
    }

    fn write_clip(tx: &rusqlite::Transaction<'_>, clip: &CatalogClip) -> Result<()> {
        clip.validate()?;
        let mut stored = clip.clone();
        let old: Option<(i64, bool, String)> = tx
            .query_row(
                "SELECT revision,final,catalog_json FROM clips WHERE clip_id=?1",
                [clip.id()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(err)?;
        if let Some((revision, final_record, old_json)) = old {
            let json = serde_json::to_string(&stored).map_err(err)?;
            if old_json == json {
                return Ok(());
            }
            let old: CatalogClip = serde_json::from_str(&old_json).map_err(err)?;
            if old.source_uri != stored.source_uri
                || old.snapshot.binding != stored.snapshot.binding
            {
                return Err("Postojeci clip_id ne smije preuzeti drugi izvor ili medij.".into());
            }
            if final_record {
                if stored.snapshot.phase != Phase::Final {
                    return Ok(());
                }
                if same_final_clip(&old, &stored) {
                    stored.media_records_uri = old.media_records_uri;
                    stored.snapshot = old.snapshot;
                } else {
                    return Err("Zavrseni probe zapis se ne smije zamijeniti.".into());
                }
            }
            if revision > stored.snapshot.revision as i64 {
                return Err("Zavrseni probe zapis se ne smije zamijeniti.".into());
            }
        }
        let json = serde_json::to_string(&stored).map_err(err)?;
        if json.len() > MAX_BYTES / PAGE_SIZE {
            return Err("Prevelik zapis klipa.".into());
        }
        let clip = &stored;
        let original = &clip.snapshot.metadata.original;
        let video = original.streams.iter().find_map(|s| match &s.details {
            StreamDetails::Video(v) => Some(v.as_ref()),
            _ => None,
        });
        let duration = original
            .duration_seconds
            .as_ref()
            .map(|d| d.value.numerator as f64 / d.value.denominator as f64);
        let fps = video.and_then(|v| v.frame_rate.as_ref()).map(|f| &f.value);
        tx.execute("INSERT INTO clips(clip_id,source_uri,original_uri,name,created_at_utc,duration_seconds,duration_frames,fps_num,fps_den,thumbnail_uri,catalog_json,revision,final)
            VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
            ON CONFLICT(clip_id) DO UPDATE SET name=excluded.name,catalog_json=excluded.catalog_json,revision=excluded.revision,final=excluded.final,
            duration_seconds=excluded.duration_seconds,duration_frames=excluded.duration_frames,fps_num=excluded.fps_num,fps_den=excluded.fps_den,
            created_at_utc=excluded.created_at_utc,
            thumbnail_uri=CASE WHEN clips.import_status='imported' THEN clips.thumbnail_uri ELSE excluded.thumbnail_uri END",
            params![clip.id(),clip.source_uri,original.media_uri,clip.name,original.tags.get("creation_time").map(|f| &f.value),duration,
                video.and_then(|v|v.exact_frame_count()),fps.map(|f|f.fps_num),fps.map(|f|f.fps_den),clip.thumbnail_uri.as_deref(),json,
                clip.snapshot.revision,clip.snapshot.phase == Phase::Final]).map_err(err)?;
        tx.execute("INSERT INTO clip_sources VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(clip_id) DO UPDATE SET original_container=excluded.original_container,original_codec=excluded.original_codec,serial_number=excluded.serial_number,volume_name=excluded.volume_name,source_name=excluded.source_name",
            params![clip.id(),clip.source_uri,original.media_uri,original.container.as_ref().map(|v| &v.value),codec(original),clip.serial_number,clip.volume_name,clip.source_name]).map_err(err)?;
        if let Some(proxy) = &clip.snapshot.metadata.proxy {
            tx.execute("INSERT INTO clip_proxy VALUES(?1,?2,?3,?4) ON CONFLICT(clip_id) DO UPDATE SET proxy_container=excluded.proxy_container,proxy_codec=excluded.proxy_codec",
                params![clip.id(),proxy.media_uri,proxy.container.as_ref().map(|v| &v.value),codec(proxy)]).map_err(err)?;
        }
        tx.execute("INSERT INTO probe_records VALUES(?1,?2,?3,?4,?5) ON CONFLICT(clip_id) DO UPDATE SET probe_json=excluded.probe_json,probed_at_utc=excluded.probed_at_utc,record_revision=excluded.record_revision",
            params![clip.id(),serde_json::to_string(&clip.snapshot.metadata).map_err(err)?,format!("unix_ms:{}",clip.snapshot.recorded_at_unix_ms),clip.media_records_uri,clip.snapshot.revision]).map_err(err)?;
        Ok(())
    }
}

fn same_final_clip(old: &CatalogClip, new: &CatalogClip) -> bool {
    old.source_uri == new.source_uri
        && old.media_records_uri == new.media_records_uri
        && old.snapshot.binding == new.snapshot.binding
        && old.snapshot.phase == new.snapshot.phase
        && old.snapshot.completeness == new.snapshot.completeness
        && same_metadata_values(&old.snapshot.metadata, &new.snapshot.metadata)
}

fn same_metadata_values(
    old: &qnc_media_metadata::ClipMetadata,
    new: &qnc_media_metadata::ClipMetadata,
) -> bool {
    let (Ok(mut old), Ok(mut new)) = (serde_json::to_value(old), serde_json::to_value(new)) else {
        return false;
    };
    strip_metadata_provenance(&mut old);
    strip_metadata_provenance(&mut new);
    old == new
}

fn strip_metadata_provenance(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("evidence");
            map.remove("evidence_id");
            map.remove("locator");
            for child in map.values_mut() {
                strip_metadata_provenance(child);
            }
        }
        Value::Array(items) => {
            for child in items {
                strip_metadata_provenance(child);
            }
        }
        _ => {}
    }
}

fn owned_table(name: &str) -> bool {
    matches!(
        name,
        "clips"
            | "clip_sources"
            | "clip_proxy"
            | "probe_records"
            | "filmstrip_artifacts"
            | "filmstrip_frames"
            | "wave_artifacts"
            | "virtual_shots"
            | "story_parts"
            | "story_markers"
            | "story_marker_slots"
            | "story_covers"
            | "story_state"
            | "story_object_history"
            | "ingest_runtime"
    )
}

fn validate_container(conn: &Connection, id: &str) -> Result<()> {
    // All schema/identity checks must observe one snapshot during concurrent bootstrap.
    let snapshot = conn.unchecked_transaction().map_err(err)?;
    validate_container_snapshot(&snapshot, id)
}

fn validate_container_snapshot(conn: &Connection, id: &str) -> Result<()> {
    let has_project: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='public_project_settings')",
            [],
            |r| r.get(0),
        )
        .map_err(err)?;
    if has_project {
        let matches: bool = conn
            .query_row(
                "SELECT count(*)=1 AND min(project_id)=?1 FROM public_project_settings",
                [id],
                |r| r.get(0),
            )
            .map_err(err)?;
        if !matches {
            return Err("Projektna baza pripada drugom projektu.".into());
        }
    }
    let has_content: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='ingest_content_schema')",
            [],
            |r| r.get(0),
        )
        .map_err(err)?;
    if has_content {
        let matches: bool = conn.query_row(
            "SELECT count(*)=1 AND min(project_id)=?1 AND min(version)=?2 FROM ingest_content_schema",
            params![id, SCHEMA_VERSION], |r| r.get(0),
        ).map_err(err)?;
        if !matches {
            return Err("Pogresan projekt ili verzija Ingest baze; nema migracije.".into());
        }
    } else if !has_project {
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        if count != 0 {
            return Err("Nepoznata postojeca baza nije Ingest odrediste.".into());
        }
    }
    Ok(())
}

// Only the validated output DB and its journal directory need owner write access.
// Never recurse or remove delete ACLs; card/source paths are not accepted here.
fn enable_owner_write(file: &Path) -> Result<()> {
    // SQLite companions belong to this validated binding, not separate databases.
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut name = file.as_os_str().to_os_string();
        name.push(suffix);
        let path = std::path::PathBuf::from(name);
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_file() => metadata,
            Ok(_) => return Err("DB write target must be a regular file.".into()),
            Err(error) if !suffix.is_empty() && error.kind() == std::io::ErrorKind::NotFound => {
                continue
            }
            Err(error) => return Err(err(error)),
        };
        let mut permissions = metadata.permissions();
        #[cfg(windows)]
        if permissions.readonly() {
            permissions.set_readonly(false);
            std::fs::set_permissions(&path, permissions).map_err(err)?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = permissions.mode();
            if mode & 0o200 == 0 {
                permissions.set_mode(mode | 0o200);
                std::fs::set_permissions(&path, permissions).map_err(err)?;
            }
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for (path, bits) in [(file.parent().ok_or("DB parent missing")?, 0o300)] {
            let permissions = std::fs::metadata(path).map_err(err)?.permissions();
            let mode = permissions.mode();
            if mode & bits != bits {
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode | bits))
                    .map_err(err)?;
            }
        }
    }
    Ok(())
}

fn codec(media: &MediaRepresentation) -> Option<&str> {
    media
        .streams
        .iter()
        .find_map(|stream| match stream.codec.as_ref().map(|f| &f.value) {
            Some(Signal::Known(codec)) => Some(codec.as_str()),
            _ => None,
        })
}

/// Seconds an import may go without a heartbeat before another importer may take
/// its clip over (an importer that died or lost its connection).
const IMPORT_LEASE_SECONDS: i64 = 120;

fn ensure_lease_column(conn: &Connection) -> Result<()> {
    if !has_column(conn, "clips", "import_claimed_at")? {
        conn.execute("ALTER TABLE clips ADD COLUMN import_claimed_at INTEGER", [])
            .map_err(err)?;
    }
    Ok(())
}
fn ensure_summary_columns(conn: &Connection) -> Result<()> {
    let added_thumbnail = if !has_column(conn, "clips", "thumbnail_uri")? {
        conn.execute("ALTER TABLE clips ADD COLUMN thumbnail_uri TEXT", [])
            .map_err(err)?;
        true
    } else {
        false
    };
    if added_thumbnail || !has_column(conn, "public_clips", "thumbnail_uri")? {
        conn.execute_batch(
            "DROP VIEW IF EXISTS public_clips;
            CREATE VIEW public_clips AS SELECT clip_id,source_uri,original_uri,name,created_at_utc,
                duration_seconds,duration_frames,fps_num,fps_den,selected,import_status,
                imported_media_uri,import_error,thumbnail_uri FROM clips;",
        )
        .map_err(err)?;
    }
    Ok(())
}

fn ensure_filmstrip_schema(conn: &Connection) -> Result<()> {
    if !object_exists(conn, "table", "filmstrip_frames")? {
        conn.execute_batch(
            "CREATE TABLE filmstrip_frames (
            clip_id TEXT NOT NULL REFERENCES clips(clip_id),
            frame_index INTEGER NOT NULL,
            seek_sec REAL NOT NULL,
            artifact_uri TEXT NOT NULL,
            updated_at_utc TEXT NOT NULL,
            PRIMARY KEY (clip_id, frame_index)
        );",
        )
        .map_err(err)?;
    }
    if !object_exists(conn, "view", "public_filmstrip_frames")? {
        conn.execute_batch(
            "CREATE VIEW public_filmstrip_frames AS
            SELECT * FROM filmstrip_frames;",
        )
        .map_err(err)?;
    }
    Ok(())
}

fn ensure_wave_schema(conn: &Connection) -> Result<()> {
    if !object_exists(conn, "table", "wave_artifacts")? {
        conn.execute_batch(
            "CREATE TABLE wave_artifacts (
            clip_id TEXT PRIMARY KEY REFERENCES clips(clip_id),
            artifact_uri TEXT NOT NULL,
            created_at_utc TEXT NOT NULL,
            peaks_json TEXT NOT NULL
        );",
        )
        .map_err(err)?;
    }
    if !object_exists(conn, "view", "public_wave_artifacts")? {
        conn.execute_batch(
            "CREATE VIEW public_wave_artifacts AS
            SELECT * FROM wave_artifacts;",
        )
        .map_err(err)?;
    }
    Ok(())
}

fn ensure_virtual_shots_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS virtual_shots (
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
    conn.execute_batch(
        "DROP VIEW IF EXISTS public_short_clips;
        CREATE VIEW public_short_clips AS
        SELECT shot_id, clip_id, in_frame, out_frame, source_shot_id, name, created_at_utc,
               in_still_uri, out_still_uri, still_status, still_error
        FROM virtual_shots
        WHERE class = 'short';",
    )
    .map_err(err)
}

fn ensure_story_schema(conn: &Connection) -> Result<()> {
    // Development records are removed, not converted (AGENTS section 3):
    // program_segments / program_markers, story_covers from before the
    // marker-pair slot binding or the A2 channel, and a story from before the
    // A1 channel (its markers, slots and covers go with it).
    let part_columns = table_columns(conn, "story_parts")?;
    if !part_columns.is_empty() && !part_columns.iter().any(|c| c == "a1_source_channel") {
        conn.execute_batch(
            "DROP VIEW IF EXISTS public_story_parts;
             DROP VIEW IF EXISTS public_story_markers;
             DROP VIEW IF EXISTS public_story_marker_slots;
             DROP VIEW IF EXISTS public_story_covers;
             DROP TABLE IF EXISTS story_covers;
             DROP TABLE IF EXISTS story_marker_slots;
             DROP TABLE IF EXISTS story_markers;
             DROP TABLE story_parts;",
        )
        .map_err(err)?;
    }
    let cover_columns = table_columns(conn, "story_covers")?;
    if !cover_columns.is_empty()
        && !["slot_id", "a2_source_channel"]
            .iter()
            .all(|needed| cover_columns.iter().any(|column| column == needed))
    {
        conn.execute_batch("DROP VIEW IF EXISTS public_story_covers; DROP TABLE story_covers;")
            .map_err(err)?;
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
            updated_at TEXT
         );
         CREATE TABLE IF NOT EXISTS story_parts (
            part_id TEXT PRIMARY KEY,
            kind TEXT NOT NULL CHECK (kind IN ('tonovi', 'offovi')),
            sort_index INTEGER NOT NULL,
            title TEXT NOT NULL DEFAULT '',
            text TEXT NOT NULL DEFAULT '',
            clip_id TEXT NOT NULL DEFAULT '',
            virtual_shot_id TEXT NOT NULL DEFAULT '',
            in_tc TEXT NOT NULL DEFAULT '',
            out_tc TEXT NOT NULL DEFAULT '',
            in_seconds REAL,
            out_seconds REAL,
            fps REAL NOT NULL DEFAULT 0,
            source_fps_num INTEGER NOT NULL DEFAULT 0,
            source_fps_den INTEGER NOT NULL DEFAULT 1,
            in_frame INTEGER NOT NULL DEFAULT 0,
            out_frame INTEGER NOT NULL DEFAULT 0,
            duration_frames INTEGER NOT NULL DEFAULT 0,
            duration_label TEXT NOT NULL DEFAULT '',
            duration_color_key TEXT NOT NULL DEFAULT '',
            active INTEGER NOT NULL DEFAULT 1,
            a1_source_channel INTEGER NOT NULL DEFAULT 0 CHECK (a1_source_channel >= 0),
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_story_parts_sort ON story_parts(sort_index);
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
         CREATE TABLE IF NOT EXISTS story_covers (
            cover_id TEXT PRIMARY KEY,
            slot_id TEXT NOT NULL DEFAULT '',
            timeline_start_frame INTEGER NOT NULL DEFAULT 0,
            timeline_end_frame INTEGER NOT NULL DEFAULT 0,
            timeline_start_sec REAL NOT NULL DEFAULT 0,
            timeline_end_sec REAL NOT NULL DEFAULT 0,
            slot_signature TEXT NOT NULL DEFAULT '',
            slot_index INTEGER NOT NULL DEFAULT 0,
            clip_id TEXT NOT NULL DEFAULT '',
            virtual_shot_id TEXT NOT NULL DEFAULT '',
            title TEXT NOT NULL DEFAULT '',
            note TEXT NOT NULL DEFAULT '',
            in_tc TEXT NOT NULL DEFAULT '',
            out_tc TEXT NOT NULL DEFAULT '',
            in_seconds REAL,
            out_seconds REAL,
            source_in_frame INTEGER NOT NULL DEFAULT 0,
            source_out_frame INTEGER NOT NULL DEFAULT 0,
            source_fps REAL NOT NULL DEFAULT 0,
            source_fps_num INTEGER NOT NULL DEFAULT 0,
            source_fps_den INTEGER NOT NULL DEFAULT 1,
            sort_index INTEGER NOT NULL DEFAULT 0,
            a2_source_channel INTEGER NOT NULL DEFAULT 0 CHECK (a2_source_channel >= 0),
            created_at TEXT NOT NULL,
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
         DROP VIEW IF EXISTS public_story_parts;
         CREATE VIEW public_story_parts AS
         SELECT part_id AS segment_id, kind, sort_index, clip_id, in_frame, out_frame,
                source_fps_num AS fps_num, source_fps_den AS fps_den, active,
                duration_frames, duration_label, duration_color_key, a1_source_channel
         FROM story_parts;
         DROP VIEW IF EXISTS public_story_markers;
         CREATE VIEW public_story_markers AS
         SELECT marker_id, timeline_frame AS program_frame, system_role
         FROM story_markers;
         DROP VIEW IF EXISTS public_story_state;
         CREATE VIEW public_story_state AS
         SELECT selected_part_id, selected_shot_id, selected_slot_id, selected_cover_id,
                draft_updated_at, committed_at, updated_at
         FROM story_state;
         DROP VIEW IF EXISTS public_story_marker_slots;
         CREATE VIEW public_story_marker_slots AS
         SELECT s.slot_id, s.slot_index, s.start_frame, s.end_frame, s.duration_frames,
                s.start_marker_id, s.end_marker_id, s.slot_signature, s.updated_at,
                EXISTS(SELECT 1 FROM story_covers c WHERE c.slot_id = s.slot_id) AS has_cover
         FROM story_marker_slots s;
         DROP VIEW IF EXISTS public_story_covers;
         CREATE VIEW public_story_covers AS
         SELECT cover_id, slot_id, slot_signature, slot_index, timeline_start_frame,
                timeline_end_frame, clip_id, virtual_shot_id, source_in_frame, source_out_frame,
                source_fps_num AS fps_num, source_fps_den AS fps_den, a2_source_channel
         FROM story_covers;",
    )
    .map_err(err)?;
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

/// v5 `now_str`.
fn story_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("epoch_{secs}")
}

/// v5 `validate_kind`: a story segment is `tonovi` or `offovi`.
fn story_kind(kind: &str) -> Result<&'static str> {
    match kind.trim() {
        "tonovi" => Ok("tonovi"),
        "offovi" => Ok("offovi"),
        other => Err(format!("Nepoznata vrsta segmenta: {other}.")),
    }
}

fn new_marker_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("marker_{nanos:x}")
}

/// v5 `require_current_story_program_source_fps`: the source rate of the first
/// active segment; markers and slots need it.
fn require_story_fps(conn: &Connection) -> Result<f64> {
    let rate: Option<(i64, i64)> = conn
        .query_row(
            "SELECT source_fps_num, source_fps_den FROM story_parts
             WHERE active = 1 AND source_fps_num > 0 AND source_fps_den > 0
             ORDER BY sort_index LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(err)?;
    rate.map(|(num, den)| num as f64 / den as f64)
        .ok_or_else(|| "timeline_fps_invalid: story program nema valjan source FPS".into())
}

/// v5 `round3(frame_to_seconds(frame, fps))`.
fn timeline_sec(frame: i64, fps: f64) -> f64 {
    if fps <= 0.0 {
        return 0.0;
    }
    ((frame.max(0) as f64 / fps) * 1000.0).round() / 1000.0
}

/// v5 `frame_to_timecode`: `hh:mm:ss:ff` with the rate rounded to whole frames.
fn frame_timecode(frame: i64, fps: f64) -> String {
    let fps = fps.round().max(1.0) as i64;
    let total = frame.max(0);
    let total_sec = total / fps;
    format!(
        "{:02}:{:02}:{:02}:{:02}",
        total_sec / 3600,
        (total_sec / 60) % 60,
        total_sec % 60,
        total % fps
    )
}

/// v5 `seconds_frames_label_from_frames`: `seconds:frames`.
fn frames_label(frames: i64, fps: f64) -> String {
    let fps = fps.round().max(1.0) as i64;
    let frames = frames.max(0);
    format!("{}:{:02}", frames / fps, frames % fps)
}

/// v5 `duration_color_key_from_frames` with the default 3, 5, 7 second marks.
fn duration_color_key(frames: i64, fps: f64) -> &'static str {
    if fps <= 0.0 {
        return "over_7";
    }
    match frames.max(0) as f64 / fps {
        seconds if seconds < 3.0 => "under_3",
        seconds if seconds < 5.0 => "under_5",
        seconds if seconds < 7.0 => "under_7",
        _ => "over_7",
    }
}

/// v5 lock of the start and the end marker on delete and move.
fn locked_check(conn: &Connection, marker_id: &str, duration: i64) -> Result<()> {
    let marker: Option<(i64, String)> = conn
        .query_row(
            "SELECT timeline_frame, system_role FROM story_markers WHERE marker_id = ?1",
            [marker_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(err)?;
    let (frame, role) = marker.ok_or_else(|| format!("marker not found: {marker_id}"))?;
    if frame == 0 || role == "program_start" {
        return Err("Početni M marker je zaključan.".into());
    }
    if role == "program_end" || frame == duration {
        return Err("Završni M marker je zaključan.".into());
    }
    Ok(())
}

struct StoryPartSpan {
    part_id: String,
    frames: i64,
}

/// Active segments in order with v5 `part_span_frames`.
fn active_part_spans(conn: &Connection) -> Result<Vec<StoryPartSpan>> {
    let mut statement = conn
        .prepare(
            "SELECT part_id, duration_frames, in_frame, out_frame FROM story_parts
             WHERE active = 1 ORDER BY sort_index",
        )
        .map_err(err)?;
    let rows = statement
        .query_map([], |row| {
            let duration: i64 = row.get(1)?;
            let (in_frame, out_frame): (i64, i64) = (row.get(2)?, row.get(3)?);
            let frames = if duration > 0 {
                duration
            } else {
                (out_frame - in_frame).max(0)
            };
            Ok(StoryPartSpan {
                part_id: row.get(0)?,
                frames,
            })
        })
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    Ok(rows)
}

fn first_marker(conn: &Connection, sql: &str, value: i64) -> Result<Option<String>> {
    conn.query_row(sql, [value], |row| row.get(0))
        .optional()
        .map_err(err)
}

/// v5 `ensure_start_marker` + `ensure_end_marker`: exactly one locked start
/// marker at frame 0 on the first segment and one locked end marker at the
/// program length on the last segment; no segments, no markers.
fn ensure_boundary_markers(conn: &Connection) -> Result<()> {
    let parts = active_part_spans(conn)?;
    let duration: i64 = parts.iter().map(|part| part.frames).sum();
    if duration <= 0 {
        conn.execute_batch(
            "DELETE FROM story_covers;
             DELETE FROM story_marker_slots;
             DELETE FROM story_markers;
             UPDATE story_state SET selected_slot_id = '' WHERE id = 1;",
        )
        .map_err(err)?;
        return Ok(());
    }
    let fps = require_story_fps(conn)?;
    let now = story_now();
    let first = parts
        .first()
        .map(|part| part.part_id.as_str())
        .unwrap_or("");
    let start_tc = frame_timecode(0, fps);
    let start = match first_marker(
        conn,
        "SELECT marker_id FROM story_markers WHERE system_role = 'program_start'
         AND ?1 = ?1 ORDER BY timeline_frame, marker_id LIMIT 1",
        0,
    )? {
        Some(id) => Some(id),
        None => first_marker(
            conn,
            "SELECT marker_id FROM story_markers WHERE timeline_frame = ?1
             ORDER BY marker_id LIMIT 1",
            0,
        )?,
    };
    match start {
        Some(id) => {
            conn.execute(
                "UPDATE story_markers
                 SET timeline_frame = 0, origin_part_id = ?1, origin_local_frame = 0,
                     origin_local_sec = 0, tc = ?2, label = ?2, system_role = 'program_start'
                 WHERE marker_id = ?3",
                params![first, start_tc, id],
            )
            .map_err(err)?;
            conn.execute(
                "UPDATE story_markers SET system_role = ''
                 WHERE system_role = 'program_start' AND marker_id != ?1",
                [&id],
            )
            .map_err(err)?;
        }
        None => {
            conn.execute(
                "INSERT INTO story_markers
                    (marker_id, timeline_frame, timeline_sec, tc, label, sort_index, system_role,
                     origin_part_id, origin_local_frame, origin_local_sec, created_at, updated_at)
                 VALUES (?1, 0, 0, ?2, ?2, 0, 'program_start', ?3, 0, 0, ?4, ?4)",
                params![new_marker_id(), start_tc, first, now],
            )
            .map_err(err)?;
        }
    }
    let at_duration = first_marker(
        conn,
        "SELECT marker_id FROM story_markers
         WHERE timeline_frame = ?1 AND system_role != 'program_start'
         ORDER BY marker_id LIMIT 1",
        duration,
    )?;
    let system_end = first_marker(
        conn,
        "SELECT marker_id FROM story_markers WHERE system_role = 'program_end'
         AND ?1 = ?1 ORDER BY timeline_frame, marker_id LIMIT 1",
        0,
    )?;
    if let (Some(system_end), Some(at_duration)) = (&system_end, &at_duration) {
        if system_end != at_duration {
            conn.execute(
                "DELETE FROM story_markers WHERE marker_id = ?1",
                [system_end],
            )
            .map_err(err)?;
        }
    }
    let last = parts.last().expect("duration > 0 has a segment");
    let end_sec = timeline_sec(duration, fps);
    let end_tc = frame_timecode(duration, fps);
    let local_sec = last.frames as f64 / fps;
    match at_duration.or(system_end) {
        Some(id) => {
            conn.execute(
                "UPDATE story_markers
                 SET timeline_frame = ?1, timeline_sec = ?2, tc = ?3, label = ?3,
                     sort_index = 0, system_role = 'program_end', origin_part_id = ?4,
                     origin_local_frame = ?5, origin_local_sec = ?6, updated_at = ?7
                 WHERE marker_id = ?8",
                params![
                    duration,
                    end_sec,
                    end_tc,
                    last.part_id,
                    last.frames,
                    local_sec,
                    now,
                    id
                ],
            )
            .map_err(err)?;
            conn.execute(
                "UPDATE story_markers SET system_role = ''
                 WHERE system_role = 'program_end' AND marker_id != ?1",
                [&id],
            )
            .map_err(err)?;
        }
        None => {
            conn.execute(
                "INSERT INTO story_markers
                    (marker_id, timeline_frame, timeline_sec, tc, label, sort_index, system_role,
                     origin_part_id, origin_local_frame, origin_local_sec, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4, 0, 'program_end', ?5, ?6, ?7, ?8, ?8)",
                params![
                    new_marker_id(),
                    duration,
                    end_sec,
                    end_tc,
                    last.part_id,
                    last.frames,
                    local_sec,
                    now
                ],
            )
            .map_err(err)?;
        }
    }
    Ok(())
}

/// v5 `finalize_story_mutation` under the v5 rule: boundary markers, slots from
/// adjacent markers, covers bound to their slot again, selection kept valid.
fn finalize_story(conn: &Connection) -> Result<()> {
    ensure_boundary_markers(conn)?;
    recompute_marker_slots(conn)?;
    rebind_covers(conn)?;
    normalize_story_selection(conn)
}

/// v5 `recompute_marker_slots`: one slot between every two adjacent markers,
/// empty spans skipped. Identity is the marker pair (v5 rule), not seconds.
fn recompute_marker_slots(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM story_marker_slots", [])
        .map_err(err)?;
    let fps = match require_story_fps(conn) {
        Ok(fps) => fps,
        Err(_) => return Ok(()),
    };
    let markers = {
        let mut statement = conn
            .prepare("SELECT marker_id, timeline_frame FROM story_markers ORDER BY timeline_frame, marker_id")
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        rows
    };
    let now = story_now();
    let mut slot_index = 0i64;
    for pair in markers.windows(2) {
        let (start_id, start) = (&pair[0].0, pair[0].1.max(0));
        let (end_id, end) = (&pair[1].0, pair[1].1.max(start));
        if end <= start {
            continue;
        }
        let (start_sec, end_sec) = (timeline_sec(start, fps), timeline_sec(end, fps));
        conn.execute(
            "INSERT INTO story_marker_slots
                (slot_id, slot_index, start_frame, end_frame, duration_frames,
                 start_sec, end_sec, duration_sec, start_marker_id, end_marker_id,
                 slot_signature, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                slot_id(start_id, end_id),
                slot_index,
                start,
                end,
                end - start,
                start_sec,
                end_sec,
                timeline_sec(end - start, fps),
                start_id,
                end_id,
                format!("start:{start_sec:.3}|end:{end_sec:.3}"),
                now
            ],
        )
        .map_err(err)?;
        slot_index += 1;
    }
    Ok(())
}

/// Deletes markers with their slots: the covers of every slot that starts or
/// ends on one of them go too (user decision 2026-09-24).
fn delete_markers_with_slots(conn: &Connection, marker_ids: &[String]) -> Result<()> {
    if marker_ids.is_empty() {
        return Ok(());
    }
    let covers = {
        let mut statement = conn
            .prepare("SELECT cover_id, slot_id FROM story_covers")
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        rows
    };
    for (cover_id, slot) in covers {
        let (start, end) = slot.split_once('|').unwrap_or(("", ""));
        if marker_ids.iter().any(|id| id == start || id == end) {
            conn.execute("DELETE FROM story_covers WHERE cover_id = ?1", [cover_id])
                .map_err(err)?;
        }
    }
    for marker_id in marker_ids {
        conn.execute(
            "DELETE FROM story_markers WHERE marker_id = ?1",
            [marker_id],
        )
        .map_err(err)?;
    }
    Ok(())
}

/// Slot identity: the ids of its start and end marker.
fn slot_id(start_marker_id: &str, end_marker_id: &str) -> String {
    format!("{start_marker_id}|{end_marker_id}")
}

struct SlotRow {
    slot_id: String,
    slot_index: i64,
    start_frame: i64,
    end_frame: i64,
    start_marker_id: String,
    signature: String,
}

/// v5 rule: a cover is removed only by the user. Deleting a marker deletes its
/// slots and their covers (`delete_markers_with_slots`). Any other change keeps
/// the cover on its logical slot: the same marker pair (moved markers give it the
/// new frames), else the slot that keeps its start marker (a new marker split the
/// slot: the cover is trimmed). A cover without a slot keeps no frames.
fn rebind_covers(conn: &Connection) -> Result<()> {
    let slots = {
        let mut statement = conn
            .prepare(
                "SELECT slot_id, slot_index, start_frame, end_frame, start_marker_id,
                        slot_signature
                 FROM story_marker_slots ORDER BY slot_index",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                Ok(SlotRow {
                    slot_id: row.get(0)?,
                    slot_index: row.get(1)?,
                    start_frame: row.get(2)?,
                    end_frame: row.get(3)?,
                    start_marker_id: row.get(4)?,
                    signature: row.get(5)?,
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        rows
    };
    let covers = {
        let mut statement = conn
            .prepare("SELECT cover_id, slot_id FROM story_covers ORDER BY created_at, cover_id")
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        rows
    };
    let fps = require_story_fps(conn).unwrap_or(0.0);
    let mut taken: Vec<&str> = Vec::new();
    // Exact pairs first, so a cover that still has its slot keeps it.
    let mut order: Vec<usize> = (0..covers.len()).collect();
    order.sort_by_key(|&index| !slots.iter().any(|slot| slot.slot_id == covers[index].1));
    for index in order {
        let (cover_id, old_slot) = &covers[index];
        let start_marker = old_slot.split_once('|').map_or("", |(start, _)| start);
        let free = |slot: &&SlotRow| !taken.contains(&slot.slot_id.as_str());
        let target = slots
            .iter()
            .filter(free)
            .find(|slot| slot.slot_id == *old_slot)
            .or_else(|| {
                slots
                    .iter()
                    .filter(free)
                    .find(|slot| !start_marker.is_empty() && slot.start_marker_id == start_marker)
            });
        match target {
            Some(slot) => {
                taken.push(slot.slot_id.as_str());
                conn.execute(
                    "UPDATE story_covers
                     SET slot_id = ?1, slot_signature = ?2, slot_index = ?3,
                         timeline_start_frame = ?4, timeline_end_frame = ?5,
                         timeline_start_sec = ?6, timeline_end_sec = ?7
                     WHERE cover_id = ?8",
                    params![
                        slot.slot_id,
                        slot.signature,
                        slot.slot_index,
                        slot.start_frame,
                        slot.end_frame,
                        timeline_sec(slot.start_frame, fps),
                        timeline_sec(slot.end_frame, fps),
                        cover_id
                    ],
                )
                .map_err(err)?;
            }
            None => {
                conn.execute(
                    "UPDATE story_covers
                     SET timeline_end_frame = timeline_start_frame, timeline_end_sec = timeline_start_sec
                     WHERE cover_id = ?1",
                    [cover_id],
                )
                .map_err(err)?;
            }
        }
    }
    Ok(())
}

/// v5 `normalize_selected_slot_id` / `normalize_selected_cover_id`.
fn normalize_story_selection(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "UPDATE story_state SET selected_slot_id = ''
         WHERE id = 1 AND selected_slot_id != ''
           AND selected_slot_id NOT IN (SELECT slot_id FROM story_marker_slots);
         UPDATE story_state SET selected_cover_id = ''
         WHERE id = 1 AND selected_cover_id != ''
           AND selected_cover_id NOT IN (SELECT cover_id FROM story_covers);",
    )
    .map_err(err)
}

/// Program length in frames: the segments one after another.
fn program_length(conn: &Connection) -> Result<i64> {
    conn.query_row(
        "SELECT COALESCE(SUM(out_frame - in_frame), 0) FROM story_parts WHERE active = 1",
        [],
        |row| row.get(0),
    )
    .map_err(err)
}

/// Program frames `[start, end)` of a segment in the stored order.
fn segment_window(conn: &Connection, segment_id: &str) -> Result<Option<(u64, u64)>> {
    let mut statement = conn
        .prepare(
            "SELECT part_id, out_frame - in_frame FROM story_parts
             WHERE active = 1 ORDER BY sort_index",
        )
        .map_err(err)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    let mut start = 0u64;
    for (id, frames) in rows {
        let end = start + frames.max(0) as u64;
        if id == segment_id {
            return Ok(Some((start, end)));
        }
        start = end;
    }
    Ok(None)
}

fn segment_order(conn: &Connection) -> Result<Vec<String>> {
    let mut statement = conn
        .prepare("SELECT part_id FROM story_parts WHERE active = 1 ORDER BY sort_index")
        .map_err(err)?;
    let ids = statement
        .query_map([], |row| row.get(0))
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<String>>>()
        .map_err(err)?;
    Ok(ids)
}

fn renumber_segments(conn: &Connection) -> Result<()> {
    for (sort_index, id) in segment_order(conn)?.iter().enumerate() {
        conn.execute(
            "UPDATE story_parts SET sort_index = ?1 WHERE part_id = ?2",
            params![sort_index as i64, id],
        )
        .map_err(err)?;
    }
    Ok(())
}

fn add_virtual_column_if_missing(conn: &Connection, column: &str, definition: &str) -> Result<()> {
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

fn require_imported_clip(conn: &Connection, clip_id: &str) -> Result<()> {
    let status: Option<String> = conn
        .query_row(
            "SELECT import_status FROM public_clips WHERE clip_id = ?1",
            [clip_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(err)?;
    match status.as_deref() {
        Some("imported") | Some("done") => Ok(()),
        Some(_) => Err(format!("Klip '{clip_id}' nije uvezen.")),
        None => Err("Klip nije pronadjen.".into()),
    }
}

fn source_row(conn: &Connection, clip_id: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT shot_id FROM virtual_shots WHERE clip_id = ?1 AND class = 'source' LIMIT 1",
        [clip_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(err)
}

fn next_short_index(conn: &Connection, clip_id: &str) -> Result<u32> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM virtual_shots WHERE clip_id = ?1 AND class = 'short'",
            [clip_id],
            |row| row.get(0),
        )
        .map_err(err)?;
    Ok(u32::try_from(count).unwrap_or(0).saturating_add(1))
}

fn validate_filmstrip_artifact(artifact: &FilmstripArtifactRecord) -> Result<()> {
    qnc_media_records::valid_id(&artifact.clip_id).map_err(err)?;
    qnc_media_records::validate_resource_uri(&artifact.artifact_uri).map_err(err)?;
    if !matches!(
        artifact.status.as_str(),
        "missing" | "building" | "ready" | "error"
    ) {
        return Err("Neispravan filmstrip status.".into());
    }
    parse_seconds(&artifact.duration_sec)?;
    if artifact.frame_count == 0
        || artifact.frame_count > 64
        || artifact.frames.len() != artifact.frame_count
    {
        return Err("Neispravan broj filmstrip slicica.".into());
    }
    for (expected, frame) in artifact.frames.iter().enumerate() {
        if frame.index != expected {
            return Err("Filmstrip slicice nisu u pravilnom redoslijedu.".into());
        }
        parse_seconds(&frame.seek_sec)?;
        qnc_media_records::validate_resource_uri(&frame.artifact_uri).map_err(err)?;
        if !frame
            .artifact_uri
            .starts_with(&format!("{}/", artifact.artifact_uri.trim_end_matches('/')))
        {
            return Err("Filmstrip slicica ne pripada artifact rootu.".into());
        }
    }
    Ok(())
}

fn validate_wave_artifact(artifact: &WaveArtifactRecord) -> Result<()> {
    qnc_wave::validate_artifact(artifact)
}

fn parse_seconds(raw: &str) -> Result<f64> {
    let value = raw
        .parse::<f64>()
        .map_err(|_| "Neispravan filmstrip seek.")?;
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err("Neispravan filmstrip seek.".into())
    }
}

fn utc_stamp() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("unix_ms:{millis}")
}

fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut statement = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(err)?;
    let rows = statement
        .query_map([], |r| r.get::<_, String>(1))
        .map_err(err)?;
    for row in rows {
        if row.map_err(err)? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn object_exists(conn: &Connection, kind: &str, name: &str) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type=?1 AND name=?2)",
        params![kind, name],
        |r| r.get(0),
    )
    .map_err(err)
}

fn import_status(value: String) -> rusqlite::Result<ImportStatus> {
    match value.as_str() {
        "detected" => Ok(ImportStatus::Detected),
        "queued" => Ok(ImportStatus::Queued),
        "processing" => Ok(ImportStatus::Processing),
        "imported" => Ok(ImportStatus::Imported),
        "failed" => Ok(ImportStatus::Failed),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredClip> {
    let json: String = row.get(0)?;
    let mut clip: CatalogClip = serde_json::from_str(&json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let thumbnail_uri: Option<String> = row.get(5)?;
    if thumbnail_uri.is_some() {
        clip.thumbnail_uri = thumbnail_uri;
    }
    let import_status = import_status(row.get(2)?)?;
    Ok(StoredClip {
        clip,
        selected: row.get(1)?,
        import_status,
        import_error: row.get(3)?,
        imported_media_uri: row.get(4)?,
    })
}
fn summary_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredClipSummary> {
    Ok(StoredClipSummary {
        clip_id: row.get(0)?,
        name: row.get(1)?,
        source_uri: row.get(2)?,
        source_name: row.get(3)?,
        serial_number: row.get(4)?,
        volume_name: row.get(5)?,
        thumbnail_uri: row.get(6)?,
        duration_seconds: row.get::<_, Option<f64>>(7)?.unwrap_or(0.0),
        selected: row.get(8)?,
        import_status: import_status(row.get(9)?)?,
        import_error: row.get(10)?,
        imported_media_uri: row.get(11)?,
        revision: row.get(12)?,
        final_record: row.get(13)?,
    })
}

fn short_clip_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ShortClip> {
    Ok(ShortClip {
        shot_id: row.get(0)?,
        clip_id: row.get(1)?,
        in_frame: row.get::<_, i64>(2)?.max(0) as u64,
        out_frame: row.get::<_, i64>(3)?.max(0) as u64,
        name: row.get(4)?,
        in_still_uri: row.get(5)?,
        out_still_uri: row.get(6)?,
        still_status: row.get(7)?,
    })
}

const CATALOG_FINGERPRINT_OFFSET: u64 = 14_695_981_039_346_656_037;
const CATALOG_FINGERPRINT_PRIME: u64 = 1_099_511_628_211;

fn fingerprint_str(hash: &mut u64, value: &str) {
    fingerprint_u64(hash, value.len() as u64);
    for byte in value.as_bytes() {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(CATALOG_FINGERPRINT_PRIME);
    }
}

fn fingerprint_opt_str(hash: &mut u64, value: Option<&str>) {
    match value {
        Some(value) => {
            fingerprint_u64(hash, 1);
            fingerprint_str(hash, value);
        }
        None => fingerprint_u64(hash, 0),
    }
}

fn fingerprint_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(CATALOG_FINGERPRINT_PRIME);
    }
}
fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn ensure_runtime_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ingest_runtime(
            key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at INTEGER NOT NULL);
         CREATE VIEW IF NOT EXISTS public_ingest_runtime AS SELECT key,value,updated_at FROM ingest_runtime;",
    )
    .map_err(err)
}

fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    conn.prepare(&format!("PRAGMA table_info({table})"))
        .and_then(|mut statement| {
            statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(err)
}

/// A stored source channel index; the column never holds a negative one.
fn channel(value: i64) -> u16 {
    u16::try_from(value.max(0)).unwrap_or(u16::MAX)
}
